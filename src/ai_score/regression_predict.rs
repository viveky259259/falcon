//! Regression Prediction — predict production risks based on code patterns,
//! historical data, and known anti-pattern correlations.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A predicted risk for the codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPrediction {
    pub category: RiskCategory,
    pub probability: f64,
    pub impact: String,
    pub evidence: Vec<String>,
    pub recommendation: String,
    pub timeframe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskCategory {
    MemoryLeak,
    CrashAtScale,
    StateCorruption,
    SecurityBreach,
    PerformanceDegradation,
    DataLoss,
}

impl std::fmt::Display for RiskCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskCategory::MemoryLeak => write!(f, "Memory Leak"),
            RiskCategory::CrashAtScale => write!(f, "Crash at Scale"),
            RiskCategory::StateCorruption => write!(f, "State Corruption"),
            RiskCategory::SecurityBreach => write!(f, "Security Breach"),
            RiskCategory::PerformanceDegradation => write!(f, "Performance Degradation"),
            RiskCategory::DataLoss => write!(f, "Data Loss"),
        }
    }
}

/// Predict production risks for a project.
pub fn predict_risks(root: &Path) -> anyhow::Result<Vec<RiskPrediction>> {
    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    let mut predictions = Vec::new();
    let mut rule_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for issue in &report.issues {
        *rule_counts.entry(issue.rule.clone()).or_default() += 1;
    }

    let file_count = report.file_count.max(1) as f64;

    let dispose_issues = rule_counts
        .get("ensure-dispose-lifecycle")
        .copied()
        .unwrap_or(0);
    let stream_issues = rule_counts
        .get("ensure-stream-subscription-cancel")
        .copied()
        .unwrap_or(0);
    if dispose_issues + stream_issues > 5 {
        let prob = ((dispose_issues + stream_issues) as f64 / file_count).min(0.95);
        predictions.push(RiskPrediction {
            category: RiskCategory::MemoryLeak,
            probability: prob,
            impact: "App slowdown and eventual crash on long user sessions".to_string(),
            evidence: vec![
                format!("{} undisposed controllers", dispose_issues),
                format!("{} uncancelled stream subscriptions", stream_issues),
            ],
            recommendation:
                "Add dispose() calls for all controllers and cancel stream subscriptions"
                    .to_string(),
            timeframe: "Within 2-4 weeks of production use".to_string(),
        });
    }

    let empty_catch = rule_counts.get("avoid-empty-catch").copied().unwrap_or(0);
    let unawaited = rule_counts
        .get("avoid-unawaited-futures")
        .copied()
        .unwrap_or(0);
    if empty_catch + unawaited > 10 {
        let prob = ((empty_catch + unawaited) as f64 / (file_count * 2.0)).min(0.9);
        predictions.push(RiskPrediction {
            category: RiskCategory::CrashAtScale,
            probability: prob,
            impact: "Unhandled exceptions cause crashes under real-world conditions".to_string(),
            evidence: vec![
                format!("{} empty catch blocks silently swallowing errors", empty_catch),
                format!("{} unawaited futures that can throw unhandled", unawaited),
            ],
            recommendation: "Replace empty catches with proper error handling and await all futures".to_string(),
            timeframe: "First week in production with real user traffic".to_string(),
        });
    }

    let dynamic_count = rule_counts.get("avoid-dynamic").copied().unwrap_or(0);
    if dynamic_count > 20 {
        let prob = (dynamic_count as f64 / (file_count * 5.0)).min(0.7);
        predictions.push(RiskPrediction {
            category: RiskCategory::StateCorruption,
            probability: prob,
            impact: "Type confusion bugs surface as corrupted state or unexpected nulls"
                .to_string(),
            evidence: vec![format!(
                "{} uses of 'dynamic' type bypass compile-time safety",
                dynamic_count
            )],
            recommendation: "Replace dynamic with explicit types or generics".to_string(),
            timeframe: "Within 1-3 months as codebase grows".to_string(),
        });
    }

    let creds = rule_counts
        .get("avoid-hardcoded-credentials")
        .copied()
        .unwrap_or(0);
    let print_prod = rule_counts
        .get("avoid-print-in-production")
        .copied()
        .unwrap_or(0);
    if creds > 0 {
        predictions.push(RiskPrediction {
            category: RiskCategory::SecurityBreach,
            probability: 0.8,
            impact: "Hardcoded credentials extracted from app binary by attackers".to_string(),
            evidence: vec![
                format!("{} hardcoded credentials in source code", creds),
                format!(
                    "{} print statements that may leak sensitive data",
                    print_prod
                ),
            ],
            recommendation: "Move all secrets to environment variables or secure storage"
                .to_string(),
            timeframe: "Immediately upon app store publication".to_string(),
        });
    }

    let long_fn = rule_counts
        .get("avoid-long-functions")
        .copied()
        .unwrap_or(0);
    let widget_nesting = rule_counts
        .get("avoid-excessive-widget-nesting")
        .copied()
        .unwrap_or(0);
    if long_fn > 30 || widget_nesting > 5 {
        predictions.push(RiskPrediction {
            category: RiskCategory::PerformanceDegradation,
            probability: 0.6,
            impact: "Janky UI, slow screen transitions, dropped frames".to_string(),
            evidence: vec![
                format!("{} overly long build methods", long_fn),
                format!("{} deeply nested widget trees", widget_nesting),
            ],
            recommendation: "Extract widgets into smaller components, use const constructors"
                .to_string(),
            timeframe: "Noticeable on mid-range devices within 1 month".to_string(),
        });
    }

    predictions.sort_by(|a, b| {
        b.probability
            .partial_cmp(&a.probability)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(predictions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── helpers ────────────────────────────────────────────────────────────────

    fn write_file(dir: &std::path::Path, rel: &str, content: &str) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full, content).unwrap();
    }

    // ── RiskCategory Display ───────────────────────────────────────────────────

    #[test]
    fn test_risk_category_display_memory_leak() {
        assert_eq!(RiskCategory::MemoryLeak.to_string(), "Memory Leak");
    }

    #[test]
    fn test_risk_category_display_crash_at_scale() {
        assert_eq!(RiskCategory::CrashAtScale.to_string(), "Crash at Scale");
    }

    #[test]
    fn test_risk_category_display_state_corruption() {
        assert_eq!(RiskCategory::StateCorruption.to_string(), "State Corruption");
    }

    #[test]
    fn test_risk_category_display_security_breach() {
        assert_eq!(RiskCategory::SecurityBreach.to_string(), "Security Breach");
    }

    #[test]
    fn test_risk_category_display_performance_degradation() {
        assert_eq!(
            RiskCategory::PerformanceDegradation.to_string(),
            "Performance Degradation"
        );
    }

    #[test]
    fn test_risk_category_display_data_loss() {
        assert_eq!(RiskCategory::DataLoss.to_string(), "Data Loss");
    }

    // ── RiskCategory equality / clone ─────────────────────────────────────────

    #[test]
    fn test_risk_category_eq() {
        assert_eq!(RiskCategory::MemoryLeak, RiskCategory::MemoryLeak);
        assert_ne!(RiskCategory::MemoryLeak, RiskCategory::DataLoss);
    }

    #[test]
    fn test_risk_category_clone() {
        let original = RiskCategory::SecurityBreach;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // ── RiskPrediction serde / clone ──────────────────────────────────────────

    #[test]
    fn test_risk_prediction_serde_roundtrip() {
        let pred = RiskPrediction {
            category: RiskCategory::MemoryLeak,
            probability: 0.75,
            impact: "App crash".to_string(),
            evidence: vec!["3 undisposed controllers".to_string()],
            recommendation: "Call dispose()".to_string(),
            timeframe: "2 weeks".to_string(),
        };
        let json = serde_json::to_string(&pred).unwrap();
        let decoded: RiskPrediction = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.category, RiskCategory::MemoryLeak);
        assert!((decoded.probability - 0.75).abs() < 1e-9);
        assert_eq!(decoded.impact, "App crash");
        assert_eq!(decoded.evidence, vec!["3 undisposed controllers".to_string()]);
        assert_eq!(decoded.recommendation, "Call dispose()");
        assert_eq!(decoded.timeframe, "2 weeks");
    }

    #[test]
    fn test_risk_prediction_clone() {
        let pred = RiskPrediction {
            category: RiskCategory::SecurityBreach,
            probability: 0.8,
            impact: "Data stolen".to_string(),
            evidence: vec!["1 hardcoded credential".to_string()],
            recommendation: "Use env vars".to_string(),
            timeframe: "Immediately".to_string(),
        };
        let cloned = pred.clone();
        assert_eq!(cloned.category, RiskCategory::SecurityBreach);
        assert!((cloned.probability - 0.8).abs() < 1e-9);
        assert_eq!(cloned.impact, pred.impact);
    }

    #[test]
    fn test_risk_prediction_serde_all_categories() {
        for cat in &[
            RiskCategory::MemoryLeak,
            RiskCategory::CrashAtScale,
            RiskCategory::StateCorruption,
            RiskCategory::SecurityBreach,
            RiskCategory::PerformanceDegradation,
            RiskCategory::DataLoss,
        ] {
            let pred = RiskPrediction {
                category: cat.clone(),
                probability: 0.5,
                impact: "some impact".to_string(),
                evidence: vec![],
                recommendation: "fix it".to_string(),
                timeframe: "soon".to_string(),
            };
            let json = serde_json::to_string(&pred).unwrap();
            let decoded: RiskPrediction = serde_json::from_str(&json).unwrap();
            assert_eq!(&decoded.category, cat);
        }
    }

    // ── predict_risks: empty project → no predictions ─────────────────────────

    #[test]
    fn test_predict_risks_empty_dir_no_predictions() {
        let tmp = TempDir::new().unwrap();
        let result = predict_risks(tmp.path()).unwrap();
        assert!(
            result.is_empty(),
            "empty project should produce no risk predictions"
        );
    }

    #[test]
    fn test_predict_risks_clean_dart_no_predictions() {
        let tmp = TempDir::new().unwrap();
        // A simple clean Dart file with no rule violations
        write_file(
            tmp.path(),
            "lib/main.dart",
            "void main() { print('hello'); }",
        );
        let result = predict_risks(tmp.path()).unwrap();
        // No rule thresholds exceeded — no predictions expected
        assert!(
            result.is_empty(),
            "clean Dart code should not trigger any risk predictions"
        );
    }

    // ── predict_risks: SecurityBreach (creds > 0) ─────────────────────────────

    #[test]
    fn test_predict_risks_security_breach_hardcoded_credentials() {
        let tmp = TempDir::new().unwrap();
        // 'apiKey = "abc123secret"' triggers avoid-hardcoded-credentials
        write_file(
            tmp.path(),
            "lib/config.dart",
            r#"
class AppConfig {
  static const String apiKey = 'supersecretapikey123';
  static const String token = 'myauthtoken456';
}
"#,
        );
        let result = predict_risks(tmp.path()).unwrap();
        let security = result
            .iter()
            .find(|p| p.category == RiskCategory::SecurityBreach);
        assert!(
            security.is_some(),
            "hardcoded credentials should trigger SecurityBreach prediction"
        );
        let s = security.unwrap();
        assert!((s.probability - 0.8).abs() < 1e-9, "SecurityBreach probability should be 0.8");
    }

    #[test]
    fn test_predict_risks_security_breach_evidence_fields() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "lib/secrets.dart",
            "const password = 'hunter2secret';",
        );
        let result = predict_risks(tmp.path()).unwrap();
        let security = result
            .iter()
            .find(|p| p.category == RiskCategory::SecurityBreach)
            .unwrap();
        assert!(!security.impact.is_empty());
        assert!(!security.recommendation.is_empty());
        assert!(!security.timeframe.is_empty());
        assert_eq!(security.evidence.len(), 2, "SecurityBreach should have 2 evidence items");
    }

    // ── predict_risks: MemoryLeak (dispose+stream > 5) ────────────────────────

    #[test]
    fn test_predict_risks_memory_leak_triggers() {
        let tmp = TempDir::new().unwrap();
        // Each State class without dispose() that has TextEditingController generates
        // one ensure-dispose-lifecycle issue. Create 6 separate files to exceed the
        // threshold of > 5.
        for i in 0..6 {
            let content = format!(
                r#"
import 'package:flutter/material.dart';

class Page{}State extends State<Page{}> {{
  late TextEditingController _ctrl = TextEditingController();

  @override
  Widget build(BuildContext context) {{
    return Container();
  }}
}}
"#,
                i, i
            );
            write_file(tmp.path(), &format!("lib/page{}.dart", i), &content);
        }
        let result = predict_risks(tmp.path()).unwrap();
        let memory = result
            .iter()
            .find(|p| p.category == RiskCategory::MemoryLeak);
        assert!(
            memory.is_some(),
            "6 undisposed controllers should trigger MemoryLeak prediction"
        );
    }

    // ── predict_risks: CrashAtScale (empty_catch + unawaited > 10) ────────────

    #[test]
    fn test_predict_risks_crash_at_scale_triggers() {
        let tmp = TempDir::new().unwrap();
        // Build a file with 12 empty catch blocks to exceed threshold > 10
        let catches: String = (0..12)
            .map(|i| {
                format!(
                    r#"
  Future<void> doWork{}() async {{
    try {{
      final x = await Future.value({});
    }} catch (e) {{}}
  }}
"#,
                    i, i
                )
            })
            .collect();
        let content = format!("class Service {{\n{}\n}}", catches);
        write_file(tmp.path(), "lib/service.dart", &content);

        let result = predict_risks(tmp.path()).unwrap();
        let crash = result
            .iter()
            .find(|p| p.category == RiskCategory::CrashAtScale);
        assert!(
            crash.is_some(),
            "12 empty catch blocks should trigger CrashAtScale prediction"
        );
    }

    // ── predict_risks: StateCorruption (dynamic > 20) ─────────────────────────

    #[test]
    fn test_predict_risks_state_corruption_triggers() {
        let tmp = TempDir::new().unwrap();
        // Generate 22 functions using `dynamic` parameter type to exceed threshold > 20
        let fns: String = (0..22)
            .map(|i| format!("  dynamic getValue{}(dynamic input{}) => input{};\n", i, i, i))
            .collect();
        let content = format!("class DynService {{\n{}}}\n", fns);
        write_file(tmp.path(), "lib/dyn_service.dart", &content);

        let result = predict_risks(tmp.path()).unwrap();
        let state = result
            .iter()
            .find(|p| p.category == RiskCategory::StateCorruption);
        assert!(
            state.is_some(),
            "22 dynamic usages should trigger StateCorruption prediction"
        );
        let s = state.unwrap();
        assert!(s.probability > 0.0 && s.probability <= 0.7);
    }

    // ── predict_risks: results sorted by probability descending ───────────────

    #[test]
    fn test_predict_risks_sorted_by_probability_descending() {
        let tmp = TempDir::new().unwrap();
        // Trigger SecurityBreach (fixed 0.8) and PerformanceDegradation (fixed 0.6)
        // by creating credential + widget-nesting violations.
        // SecurityBreach (0.8) should appear before PerformanceDegradation (0.6).

        // Credential file → SecurityBreach
        write_file(
            tmp.path(),
            "lib/creds.dart",
            "const apiKey = 'realsecretkey99';",
        );

        // Widget nesting > 5: create 6 files each with one deeply-nested build method
        for i in 0..6 {
            let nesting = "Container(child: ".repeat(12);
            let closing = ")".repeat(12);
            let content = format!(
                r#"
import 'package:flutter/material.dart';

class MyWidget{} extends StatelessWidget {{
  @override
  Widget build(BuildContext context) {{
    return {}Container(){};
  }}
}}
"#,
                i, nesting, closing
            );
            write_file(tmp.path(), &format!("lib/widget{}.dart", i), &content);
        }

        let result = predict_risks(tmp.path()).unwrap();
        assert!(
            result.len() >= 2,
            "expected at least 2 predictions; got {}",
            result.len()
        );
        for w in result.windows(2) {
            assert!(
                w[0].probability >= w[1].probability,
                "predictions should be sorted descending by probability"
            );
        }
    }

    // ── predict_risks: result type and structure ───────────────────────────────

    #[test]
    fn test_predict_risks_returns_ok_for_valid_dir() {
        let tmp = TempDir::new().unwrap();
        let result = predict_risks(tmp.path());
        assert!(result.is_ok(), "predict_risks should return Ok for a valid dir");
    }

    #[test]
    fn test_predict_risks_probability_clamped() {
        let tmp = TempDir::new().unwrap();
        // Generate many dispose-lifecycle issues in a single file to test probability cap
        // With 1 file and many issues, prob = issues/1.0, capped at 0.95
        let classes: String = (0..20)
            .map(|i| {
                format!(
                    r#"
class Widget{}State extends State<Widget{}> {{
  late TextEditingController _c{} = TextEditingController();
  late ScrollController _s{} = ScrollController();
  @override
  Widget build(BuildContext context) {{ return Container(); }}
}}
"#,
                    i, i, i, i
                )
            })
            .collect();
        write_file(tmp.path(), "lib/widgets.dart", &classes);

        let result = predict_risks(tmp.path()).unwrap();
        for pred in &result {
            assert!(
                pred.probability <= 1.0,
                "probability should never exceed 1.0, got {}",
                pred.probability
            );
            assert!(
                pred.probability >= 0.0,
                "probability should be non-negative, got {}",
                pred.probability
            );
        }
    }
}

/// Print risk predictions.
pub fn print_risk_predictions(predictions: &[RiskPrediction]) {
    println!();
    println!("  {} Regression Prediction", "falcon".bright_cyan().bold());
    println!();

    if predictions.is_empty() {
        println!(
            "  {} No significant production risks detected.",
            "✓".green().bold()
        );
        println!();
        return;
    }

    println!("  {} risk(s) predicted:\n", predictions.len());

    for pred in predictions {
        let prob_pct = format!("{:.0}%", pred.probability * 100.0);
        let prob_color = if pred.probability > 0.7 {
            prob_pct.bright_red().bold()
        } else if pred.probability > 0.4 {
            prob_pct.yellow().bold()
        } else {
            prob_pct.dimmed()
        };

        println!(
            "  {} {} (probability: {})",
            "▸".bright_red(),
            pred.category.to_string().bright_white().bold(),
            prob_color
        );
        println!("    Impact:    {}", pred.impact);
        println!("    Timeframe: {}", pred.timeframe.bright_yellow());

        println!("    Evidence:");
        for e in &pred.evidence {
            println!("      {} {}", "·".dimmed(), e);
        }

        println!("    {} {}", "→".green(), pred.recommendation);
        println!();
    }
}
