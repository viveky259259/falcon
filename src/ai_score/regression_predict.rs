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
    let mut rule_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for issue in &report.issues {
        *rule_counts.entry(issue.rule.clone()).or_default() += 1;
    }

    let dispose_issues = rule_counts.get("ensure-dispose-lifecycle").copied().unwrap_or(0);
    let stream_issues = rule_counts.get("ensure-stream-subscription-cancel").copied().unwrap_or(0);
    if dispose_issues + stream_issues > 5 {
        let prob = ((dispose_issues + stream_issues) as f64 / report.file_count as f64).min(0.95);
        predictions.push(RiskPrediction {
            category: RiskCategory::MemoryLeak,
            probability: prob,
            impact: "App slowdown and eventual crash on long user sessions".to_string(),
            evidence: vec![
                format!("{} undisposed controllers", dispose_issues),
                format!("{} uncancelled stream subscriptions", stream_issues),
            ],
            recommendation: "Add dispose() calls for all controllers and cancel stream subscriptions".to_string(),
            timeframe: "Within 2-4 weeks of production use".to_string(),
        });
    }

    let empty_catch = rule_counts.get("avoid-empty-catch").copied().unwrap_or(0);
    let unawaited = rule_counts.get("avoid-unawaited-futures").copied().unwrap_or(0);
    if empty_catch + unawaited > 10 {
        let prob = ((empty_catch + unawaited) as f64 / (report.file_count as f64 * 2.0)).min(0.9);
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
        let prob = (dynamic_count as f64 / (report.file_count as f64 * 5.0)).min(0.7);
        predictions.push(RiskPrediction {
            category: RiskCategory::StateCorruption,
            probability: prob,
            impact: "Type confusion bugs surface as corrupted state or unexpected nulls".to_string(),
            evidence: vec![
                format!("{} uses of 'dynamic' type bypass compile-time safety", dynamic_count),
            ],
            recommendation: "Replace dynamic with explicit types or generics".to_string(),
            timeframe: "Within 1-3 months as codebase grows".to_string(),
        });
    }

    let creds = rule_counts.get("avoid-hardcoded-credentials").copied().unwrap_or(0);
    let print_prod = rule_counts.get("avoid-print-in-production").copied().unwrap_or(0);
    if creds > 0 {
        predictions.push(RiskPrediction {
            category: RiskCategory::SecurityBreach,
            probability: 0.8,
            impact: "Hardcoded credentials extracted from app binary by attackers".to_string(),
            evidence: vec![
                format!("{} hardcoded credentials in source code", creds),
                format!("{} print statements that may leak sensitive data", print_prod),
            ],
            recommendation: "Move all secrets to environment variables or secure storage".to_string(),
            timeframe: "Immediately upon app store publication".to_string(),
        });
    }

    let long_fn = rule_counts.get("avoid-long-functions").copied().unwrap_or(0);
    let widget_nesting = rule_counts.get("avoid-excessive-widget-nesting").copied().unwrap_or(0);
    if long_fn > 30 || widget_nesting > 5 {
        predictions.push(RiskPrediction {
            category: RiskCategory::PerformanceDegradation,
            probability: 0.6,
            impact: "Janky UI, slow screen transitions, dropped frames".to_string(),
            evidence: vec![
                format!("{} overly long build methods", long_fn),
                format!("{} deeply nested widget trees", widget_nesting),
            ],
            recommendation: "Extract widgets into smaller components, use const constructors".to_string(),
            timeframe: "Noticeable on mid-range devices within 1 month".to_string(),
        });
    }

    predictions.sort_by(|a, b| b.probability.partial_cmp(&a.probability).unwrap_or(std::cmp::Ordering::Equal));
    Ok(predictions)
}

/// Print risk predictions.
pub fn print_risk_predictions(predictions: &[RiskPrediction]) {
    println!();
    println!(
        "  {} Regression Prediction",
        "falcon".bright_cyan().bold()
    );
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
