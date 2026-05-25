//! Architecture Governor — enforce architectural decisions, detect violations,
//! and suggest structural improvements.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchReport {
    pub detected_pattern: String,
    pub layer_compliance: f64,
    pub violations: Vec<ArchViolation>,
    pub suggestions: Vec<String>,
    pub module_map: Vec<ModuleInfo>,
    pub complexity_hotspots: Vec<Hotspot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchViolation {
    pub file: String,
    pub violation_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub path: String,
    pub file_count: usize,
    pub line_count: usize,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    pub file: String,
    pub lines: usize,
    pub complexity_score: usize,
    pub reason: String,
}

/// Analyze project architecture and enforce governance.
pub fn analyze_architecture(root: &Path) -> anyhow::Result<ArchReport> {
    let conventions = crate::ai_score::convention::detect_conventions(root)?;
    let detected_pattern = conventions.architecture.pattern.clone();

    let mut violations = Vec::new();
    let mut module_map = Vec::new();
    let mut complexity_hotspots = Vec::new();
    let mut dir_stats: HashMap<String, (usize, usize)> = HashMap::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| !e.path().to_string_lossy().contains("/test/"))
        .filter(|e| !e.path().to_string_lossy().contains(".g.dart"))
    {
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        let source = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let lines = source.lines().count();

        let parts: Vec<&str> = rel.split('/').collect();
        let dir = if parts.len() > 1 {
            parts[..parts.len() - 1].join("/")
        } else {
            ".".to_string()
        };
        let entry_stats = dir_stats.entry(dir.clone()).or_default();
        entry_stats.0 += 1;
        entry_stats.1 += lines;

        if lines > 500 {
            complexity_hotspots.push(Hotspot {
                file: rel.clone(),
                lines,
                complexity_score: lines / 50,
                reason: format!("{} lines — consider splitting into smaller files", lines),
            });
        }

        check_arch_violations(&detected_pattern, &rel, &source, &mut violations);
    }

    for (dir, (files, lines)) in &dir_stats {
        let layer = classify_layer(&detected_pattern, dir);
        module_map.push(ModuleInfo {
            path: dir.clone(),
            file_count: *files,
            line_count: *lines,
            layer,
        });
    }
    module_map.sort_by(|a, b| b.file_count.cmp(&a.file_count));
    complexity_hotspots.sort_by(|a, b| b.lines.cmp(&a.lines));

    let total_files: usize = dir_stats.values().map(|(f, _)| f).sum();
    let violation_rate = if total_files > 0 {
        1.0 - (violations.len() as f64 / total_files as f64)
    } else {
        1.0
    };
    let layer_compliance = (violation_rate * 100.0).max(0.0);

    let mut suggestions = Vec::new();
    if detected_pattern == "Flat/Custom" {
        suggestions.push(
            "Consider adopting Clean Architecture or Feature-First structure for scalability"
                .to_string(),
        );
        suggestions.push("Run: falcon refactor-sim --scenario clean-architecture".to_string());
    }
    if complexity_hotspots.len() > 5 {
        suggestions.push(format!(
            "{} files exceed 500 lines — break them into focused modules",
            complexity_hotspots.len()
        ));
    }
    if violations.len() > 10 {
        suggestions.push(
            "Many architecture violations — enforce with falcon check-layers in CI".to_string(),
        );
    }

    Ok(ArchReport {
        detected_pattern,
        layer_compliance,
        violations,
        suggestions,
        module_map,
        complexity_hotspots,
    })
}

fn check_arch_violations(
    pattern: &str,
    file: &str,
    source: &str,
    violations: &mut Vec<ArchViolation>,
) {
    match pattern {
        "Clean Architecture" => {
            if file.contains("/domain/") && source.contains("import") {
                for line in source.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("import")
                        && (trimmed.contains("/data/") || trimmed.contains("/presentation/"))
                    {
                        violations.push(ArchViolation {
                            file: file.to_string(),
                            violation_type: "layer-violation".to_string(),
                            message: "Domain layer imports from data/presentation — domain should be independent".to_string(),
                        });
                        break;
                    }
                }
            }
            if file.contains("/data/")
                && source
                    .lines()
                    .any(|l| l.trim().starts_with("import") && l.contains("/presentation/"))
            {
                violations.push(ArchViolation {
                    file: file.to_string(),
                    violation_type: "layer-violation".to_string(),
                    message: "Data layer imports from presentation — data should not know about UI"
                        .to_string(),
                });
            }
        }
        "Feature-First" => {
            if file.contains("/features/") {
                let parts: Vec<&str> = file.split("/features/").collect();
                if parts.len() > 1 {
                    let feature = parts[1].split('/').next().unwrap_or("");
                    for line in source.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("import")
                            && trimmed.contains("/features/")
                            && !trimmed.contains(feature)
                        {
                            violations.push(ArchViolation {
                                file: file.to_string(),
                                violation_type: "cross-feature".to_string(),
                                message: format!("Feature '{}' imports from another feature — use shared/core instead", feature),
                            });
                            break;
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn classify_layer(pattern: &str, dir: &str) -> String {
    match pattern {
        "Clean Architecture" => {
            if dir.contains("domain") {
                "Domain"
            } else if dir.contains("data") {
                "Data"
            } else if dir.contains("presentation") {
                "Presentation"
            } else if dir.contains("core") {
                "Core"
            } else {
                "Other"
            }
        }
        "Feature-First" => {
            if dir.contains("features") {
                "Feature"
            } else if dir.contains("core") {
                "Core"
            } else if dir.contains("shared") {
                "Shared"
            } else {
                "Other"
            }
        }
        _ => "Unclassified",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Create a Dart file inside `dir` at the given relative path.
    fn make_dart(dir: &TempDir, rel: &str, content: &str) {
        let full = dir.path().join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&full).unwrap();
        write!(f, "{}", content).unwrap();
    }

    // ── classify_layer ────────────────────────────────────────────────────────

    #[test]
    fn classify_layer_clean_domain() {
        assert_eq!(classify_layer("Clean Architecture", "lib/domain/entities"), "Domain");
    }

    #[test]
    fn classify_layer_clean_data() {
        assert_eq!(classify_layer("Clean Architecture", "lib/data/repositories"), "Data");
    }

    #[test]
    fn classify_layer_clean_presentation() {
        assert_eq!(classify_layer("Clean Architecture", "lib/presentation/screens"), "Presentation");
    }

    #[test]
    fn classify_layer_clean_core() {
        assert_eq!(classify_layer("Clean Architecture", "lib/core/utils"), "Core");
    }

    #[test]
    fn classify_layer_clean_other() {
        assert_eq!(classify_layer("Clean Architecture", "lib/helpers"), "Other");
    }

    #[test]
    fn classify_layer_feature_first_features() {
        assert_eq!(classify_layer("Feature-First", "lib/features/auth"), "Feature");
    }

    #[test]
    fn classify_layer_feature_first_core() {
        assert_eq!(classify_layer("Feature-First", "lib/core"), "Core");
    }

    #[test]
    fn classify_layer_feature_first_shared() {
        assert_eq!(classify_layer("Feature-First", "lib/shared/widgets"), "Shared");
    }

    #[test]
    fn classify_layer_feature_first_other() {
        assert_eq!(classify_layer("Feature-First", "lib/utils"), "Other");
    }

    #[test]
    fn classify_layer_unknown_pattern() {
        assert_eq!(classify_layer("MVC/MVVM", "lib/domain"), "Unclassified");
        assert_eq!(classify_layer("Flat/Custom", "lib/features"), "Unclassified");
    }

    // ── check_arch_violations ─────────────────────────────────────────────────

    #[test]
    fn no_violations_clean_arch_clean_domain() {
        let mut violations = Vec::new();
        let source = "class UserEntity {}";
        check_arch_violations("Clean Architecture", "lib/domain/entities/user.dart", source, &mut violations);
        assert!(violations.is_empty());
    }

    #[test]
    fn clean_arch_domain_imports_data_triggers_violation() {
        let mut violations = Vec::new();
        let source = "import 'package:app/data/repositories/user_repo.dart';\nclass UserUseCase {}";
        check_arch_violations("Clean Architecture", "lib/domain/usecases/user_use_case.dart", source, &mut violations);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, "layer-violation");
        assert!(violations[0].message.contains("Domain layer imports from data/presentation"));
    }

    #[test]
    fn clean_arch_domain_imports_presentation_triggers_violation() {
        let mut violations = Vec::new();
        let source = "import 'package:app/presentation/screens/home.dart';\nclass Domain {}";
        check_arch_violations("Clean Architecture", "lib/domain/usecases/something.dart", source, &mut violations);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, "layer-violation");
    }

    #[test]
    fn clean_arch_data_imports_presentation_triggers_violation() {
        let mut violations = Vec::new();
        let source = "import 'package:app/presentation/widgets/spinner.dart';\nclass DataRepo {}";
        check_arch_violations("Clean Architecture", "lib/data/repos/user.dart", source, &mut violations);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, "layer-violation");
        assert!(violations[0].message.contains("Data layer imports from presentation"));
    }

    #[test]
    fn clean_arch_data_no_presentation_import_no_violation() {
        let mut violations = Vec::new();
        let source = "import 'package:app/domain/entities/user.dart';\nclass UserRepo {}";
        check_arch_violations("Clean Architecture", "lib/data/repos/user.dart", source, &mut violations);
        assert!(violations.is_empty());
    }

    #[test]
    fn clean_arch_domain_no_banned_import_no_violation() {
        let mut violations = Vec::new();
        // import present but doesn't reference /data/ or /presentation/
        let source = "import 'package:app/core/utils.dart';\nclass Entity {}";
        check_arch_violations("Clean Architecture", "lib/domain/entities/entity.dart", source, &mut violations);
        assert!(violations.is_empty());
    }

    #[test]
    fn feature_first_cross_feature_import_triggers_violation() {
        let mut violations = Vec::new();
        let source = "import 'package:app/features/auth/login.dart';\nclass ProfilePage {}";
        check_arch_violations("Feature-First", "lib/features/profile/profile_page.dart", source, &mut violations);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, "cross-feature");
        assert!(violations[0].message.contains("profile"));
    }

    #[test]
    fn feature_first_same_feature_import_no_violation() {
        let mut violations = Vec::new();
        let source = "import 'package:app/features/auth/widgets/button.dart';\nclass LoginPage {}";
        check_arch_violations("Feature-First", "lib/features/auth/pages/login_page.dart", source, &mut violations);
        assert!(violations.is_empty());
    }

    #[test]
    fn feature_first_non_feature_file_no_violation() {
        let mut violations = Vec::new();
        let source = "import 'package:app/features/auth/widgets/button.dart';\nclass SharedWidget {}";
        check_arch_violations("Feature-First", "lib/shared/widgets/common.dart", source, &mut violations);
        assert!(violations.is_empty());
    }

    #[test]
    fn unknown_pattern_no_violations() {
        let mut violations = Vec::new();
        let source = "import 'package:app/data/repos/user.dart';\nclass Anything {}";
        check_arch_violations("Flat/Custom", "lib/domain/file.dart", source, &mut violations);
        assert!(violations.is_empty());
    }

    // ── analyze_architecture (orchestrator) ───────────────────────────────────

    #[test]
    fn analyze_architecture_empty_dir_returns_flat_custom() {
        let dir = TempDir::new().unwrap();
        let report = analyze_architecture(dir.path()).unwrap();
        assert_eq!(report.detected_pattern, "Flat/Custom");
        assert!((report.layer_compliance - 100.0).abs() < f64::EPSILON,
            "No files → compliance should be 100.0");
        assert!(report.violations.is_empty());
        assert!(report.module_map.is_empty());
        assert!(report.complexity_hotspots.is_empty());
    }

    #[test]
    fn analyze_architecture_flat_custom_suggestion_present() {
        let dir = TempDir::new().unwrap();
        // Place a single Dart file with no domain/data/features dirs
        make_dart(&dir, "lib/main.dart", "void main() {}");
        let report = analyze_architecture(dir.path()).unwrap();
        assert_eq!(report.detected_pattern, "Flat/Custom");
        assert!(report.suggestions.iter().any(|s| s.contains("Clean Architecture")));
    }

    #[test]
    fn analyze_architecture_clean_arch_detected() {
        let dir = TempDir::new().unwrap();
        // detect_conventions needs both "domain" and "data" directories
        make_dart(&dir, "lib/domain/entities/user.dart", "class User {}");
        make_dart(&dir, "lib/data/repos/user_repo.dart", "class UserRepo {}");
        let report = analyze_architecture(dir.path()).unwrap();
        assert_eq!(report.detected_pattern, "Clean Architecture");
    }

    #[test]
    fn analyze_architecture_feature_first_detected() {
        let dir = TempDir::new().unwrap();
        // detect_conventions needs a "features" directory
        make_dart(&dir, "lib/features/auth/login.dart", "class LoginPage {}");
        let report = analyze_architecture(dir.path()).unwrap();
        assert_eq!(report.detected_pattern, "Feature-First");
    }

    #[test]
    fn analyze_architecture_hotspot_for_large_file() {
        let dir = TempDir::new().unwrap();
        // 501 lines triggers a hotspot; create 6 to also trigger the suggestion
        let big_content = "// line\n".repeat(501);
        for i in 0..6 {
            make_dart(&dir, &format!("lib/big{}.dart", i), &big_content);
        }
        let report = analyze_architecture(dir.path()).unwrap();
        assert!(report.complexity_hotspots.len() >= 6);
        assert!(report.complexity_hotspots[0].lines > 500);
        // With >5 hotspots the suggestion should mention 500 lines
        assert!(report.suggestions.iter().any(|s| s.contains("500 lines")));
    }

    #[test]
    fn analyze_architecture_single_hotspot_no_suggestion() {
        let dir = TempDir::new().unwrap();
        // Only 1 hotspot → no "500 lines" suggestion (threshold is >5)
        let big_content = "// line\n".repeat(501);
        make_dart(&dir, "lib/only_one_big.dart", &big_content);
        let report = analyze_architecture(dir.path()).unwrap();
        assert_eq!(report.complexity_hotspots.len(), 1);
        assert!(report.complexity_hotspots[0].reason.contains("lines"));
        // suggestion only emitted if >5 hotspots; here it should NOT appear
        assert!(!report.suggestions.iter().any(|s| s.contains("500 lines")));
    }

    #[test]
    fn analyze_architecture_small_file_no_hotspot() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/small.dart", "class Small {}\n");
        let report = analyze_architecture(dir.path()).unwrap();
        assert!(report.complexity_hotspots.is_empty());
    }

    #[test]
    fn analyze_architecture_test_files_ignored() {
        let dir = TempDir::new().unwrap();
        // Files inside /test/ must be skipped
        let big_content = "// line\n".repeat(501);
        make_dart(&dir, "test/widget_test.dart", &big_content);
        let report = analyze_architecture(dir.path()).unwrap();
        assert!(report.complexity_hotspots.is_empty());
        assert!(report.module_map.is_empty());
    }

    #[test]
    fn analyze_architecture_generated_files_ignored() {
        let dir = TempDir::new().unwrap();
        // .g.dart files must be skipped
        make_dart(&dir, "lib/models/user.g.dart", "class UserG {}");
        let report = analyze_architecture(dir.path()).unwrap();
        assert!(report.module_map.is_empty());
    }

    #[test]
    fn analyze_architecture_module_map_populated() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/domain/entities/user.dart", "class User {}");
        make_dart(&dir, "lib/data/repos/user_repo.dart", "class UserRepo {}");
        let report = analyze_architecture(dir.path()).unwrap();
        assert!(!report.module_map.is_empty());
        // each module_map entry has a known layer
        for m in &report.module_map {
            assert!(!m.layer.is_empty());
        }
    }

    #[test]
    fn analyze_architecture_many_violations_triggers_ci_suggestion() {
        let dir = TempDir::new().unwrap();
        // Create 11+ domain files that each import from data → 11 violations
        for i in 0..12 {
            let content = format!(
                "import 'package:app/data/repos/repo{}.dart';\nclass Entity{} {{}}\n",
                i, i
            );
            make_dart(&dir, &format!("lib/domain/entities/entity{}.dart", i), &content);
        }
        // Also need a data dir for Clean Architecture detection
        make_dart(&dir, "lib/data/source.dart", "class Source {}");
        let report = analyze_architecture(dir.path()).unwrap();
        assert_eq!(report.detected_pattern, "Clean Architecture");
        assert!(report.violations.len() > 10);
        assert!(report.suggestions.iter().any(|s| s.contains("enforce with falcon check-layers")));
    }

    #[test]
    fn analyze_architecture_compliance_decreases_with_violations() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/domain/use.dart", "import 'package:app/data/r.dart';\nclass U {}");
        make_dart(&dir, "lib/data/repo.dart", "class Repo {}");
        let report = analyze_architecture(dir.path()).unwrap();
        assert_eq!(report.detected_pattern, "Clean Architecture");
        assert!(report.layer_compliance < 100.0, "Compliance should drop with violations");
    }
}

/// Print architecture report.
pub fn print_arch_report(report: &ArchReport) {
    println!();
    println!(
        "  {} Architecture Governor",
        "falcon manage".bright_cyan().bold()
    );
    println!();

    println!(
        "  Pattern:         {}",
        report.detected_pattern.bright_white().bold()
    );
    println!("  Compliance:      {:.0}%", report.layer_compliance);
    println!("  Violations:      {}", report.violations.len());
    println!("  Modules:         {}", report.module_map.len());

    if !report.violations.is_empty() {
        println!();
        println!("  {} Violations:", "▸".red());
        for v in report.violations.iter().take(10) {
            println!(
                "    {} {} — {}",
                "✗".red(),
                v.file.bright_white(),
                v.message.dimmed()
            );
        }
        if report.violations.len() > 10 {
            println!("    ... and {} more", report.violations.len() - 10);
        }
    }

    if !report.complexity_hotspots.is_empty() {
        println!();
        println!("  {} Complexity Hotspots:", "▸".yellow());
        for h in report.complexity_hotspots.iter().take(5) {
            println!(
                "    {} {} — {}",
                "⚠".yellow(),
                h.file.bright_white(),
                h.reason.dimmed()
            );
        }
    }

    if !report.suggestions.is_empty() {
        println!();
        println!("  {} Suggestions:", "▸".green());
        for s in &report.suggestions {
            println!("    {} {}", "→".green(), s);
        }
    }

    println!();
}
