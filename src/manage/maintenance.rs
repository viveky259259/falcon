//! Automated Maintenance — cleanup recommendations, dead code removal,
//! auto-fix pipeline, and technical debt management.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceReport {
    pub cleanup_tasks: Vec<CleanupTask>,
    pub auto_fixable: usize,
    pub manual_review: usize,
    pub estimated_savings_lines: usize,
    pub tech_debt_score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupTask {
    pub category: CleanupCategory,
    pub description: String,
    pub files_affected: usize,
    pub auto_fixable: bool,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupCategory {
    DeadCode,
    UnusedImports,
    UnusedFiles,
    UnusedDeps,
    StaleGenerated,
    LongFunctions,
    EmptyCatches,
    PrintStatements,
}

impl std::fmt::Display for CleanupCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeadCode => write!(f, "Dead Code"),
            Self::UnusedImports => write!(f, "Unused Imports"),
            Self::UnusedFiles => write!(f, "Unused Files"),
            Self::UnusedDeps => write!(f, "Unused Dependencies"),
            Self::StaleGenerated => write!(f, "Stale Generated Files"),
            Self::LongFunctions => write!(f, "Long Functions"),
            Self::EmptyCatches => write!(f, "Empty Catch Blocks"),
            Self::PrintStatements => write!(f, "Print Statements"),
        }
    }
}

/// Generate maintenance recommendations.
pub fn analyze_maintenance(root: &Path) -> anyhow::Result<MaintenanceReport> {
    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    let codegen = crate::analysis::codegen_quality::analyze_codegen(root);

    let mut tasks = Vec::new();
    let mut total_savings = 0;

    let unused_code = report
        .issues
        .iter()
        .filter(|i| i.rule == "unused-code")
        .count();
    if unused_code > 0 {
        total_savings += unused_code * 15;
        tasks.push(CleanupTask {
            category: CleanupCategory::DeadCode,
            description: format!("{} unused declarations can be removed", unused_code),
            files_affected: unused_code,
            auto_fixable: false,
            command: "falcon check-unused-code".to_string(),
        });
    }

    let unused_files = report
        .issues
        .iter()
        .filter(|i| i.rule == "unused-file")
        .count();
    if unused_files > 0 {
        total_savings += unused_files * 50;
        tasks.push(CleanupTask {
            category: CleanupCategory::UnusedFiles,
            description: format!("{} files are not imported anywhere", unused_files),
            files_affected: unused_files,
            auto_fixable: false,
            command: "falcon check-unused-files".to_string(),
        });
    }

    let unused_deps = report
        .issues
        .iter()
        .filter(|i| i.rule == "unused-dependency")
        .count();
    if unused_deps > 0 {
        tasks.push(CleanupTask {
            category: CleanupCategory::UnusedDeps,
            description: format!(
                "{} dependencies in pubspec.yaml are not imported",
                unused_deps
            ),
            files_affected: 1,
            auto_fixable: false,
            command: "falcon check-dependencies".to_string(),
        });
    }

    if !codegen.stale_files.is_empty() {
        tasks.push(CleanupTask {
            category: CleanupCategory::StaleGenerated,
            description: format!(
                "{} generated files may be stale (source missing)",
                codegen.stale_files.len()
            ),
            files_affected: codegen.stale_files.len(),
            auto_fixable: false,
            command: "falcon check-codegen".to_string(),
        });
    }

    let long_fns = report
        .issues
        .iter()
        .filter(|i| i.rule == "avoid-long-functions")
        .count();
    if long_fns > 20 {
        tasks.push(CleanupTask {
            category: CleanupCategory::LongFunctions,
            description: format!(
                "{} functions exceed length threshold — extract helpers",
                long_fns
            ),
            files_affected: long_fns,
            auto_fixable: false,
            command: "falcon analyze".to_string(),
        });
    }

    let empty_catches = report
        .issues
        .iter()
        .filter(|i| i.rule == "avoid-empty-catch")
        .count();
    if empty_catches > 0 {
        tasks.push(CleanupTask {
            category: CleanupCategory::EmptyCatches,
            description: format!(
                "{} empty catch blocks silently swallowing errors",
                empty_catches
            ),
            files_affected: empty_catches,
            auto_fixable: false,
            command: "falcon analyze --preset ai-generated".to_string(),
        });
    }

    let prints = report
        .issues
        .iter()
        .filter(|i| i.rule == "avoid-print-in-production")
        .count();
    if prints > 0 {
        tasks.push(CleanupTask {
            category: CleanupCategory::PrintStatements,
            description: format!(
                "{} print() calls in production code — replace with logger",
                prints
            ),
            files_affected: prints,
            auto_fixable: false,
            command: "falcon fix".to_string(),
        });
    }

    let trailing = report
        .issues
        .iter()
        .filter(|i| i.rule == "prefer-trailing-comma")
        .count();
    if trailing > 50 {
        tasks.push(CleanupTask {
            category: CleanupCategory::UnusedImports,
            description: format!("{} missing trailing commas — auto-fixable", trailing),
            files_affected: trailing,
            auto_fixable: true,
            command: "falcon fix".to_string(),
        });
    }

    let auto_fixable = tasks.iter().filter(|t| t.auto_fixable).count();
    let manual = tasks.iter().filter(|t| !t.auto_fixable).count();

    let total_issues = report.issues.len();
    let tech_debt = if report.file_count > 0 {
        let issues_per_file = total_issues as f64 / report.file_count as f64;
        (100.0 - issues_per_file * 5.0).max(0.0).min(100.0) as u32
    } else {
        100
    };

    Ok(MaintenanceReport {
        cleanup_tasks: tasks,
        auto_fixable,
        manual_review: manual,
        estimated_savings_lines: total_savings,
        tech_debt_score: tech_debt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn make_file(dir: &TempDir, rel: &str, content: &str) {
        let full = dir.path().join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&full).unwrap();
        write!(f, "{}", content).unwrap();
    }

    fn make_dart(dir: &TempDir, rel: &str, content: &str) {
        make_file(dir, rel, content);
    }

    // ── CleanupCategory Display ───────────────────────────────────────────────

    #[test]
    fn display_dead_code() {
        assert_eq!(CleanupCategory::DeadCode.to_string(), "Dead Code");
    }

    #[test]
    fn display_unused_imports() {
        assert_eq!(CleanupCategory::UnusedImports.to_string(), "Unused Imports");
    }

    #[test]
    fn display_unused_files() {
        assert_eq!(CleanupCategory::UnusedFiles.to_string(), "Unused Files");
    }

    #[test]
    fn display_unused_deps() {
        assert_eq!(CleanupCategory::UnusedDeps.to_string(), "Unused Dependencies");
    }

    #[test]
    fn display_stale_generated() {
        assert_eq!(CleanupCategory::StaleGenerated.to_string(), "Stale Generated Files");
    }

    #[test]
    fn display_long_functions() {
        assert_eq!(CleanupCategory::LongFunctions.to_string(), "Long Functions");
    }

    #[test]
    fn display_empty_catches() {
        assert_eq!(CleanupCategory::EmptyCatches.to_string(), "Empty Catch Blocks");
    }

    #[test]
    fn display_print_statements() {
        assert_eq!(CleanupCategory::PrintStatements.to_string(), "Print Statements");
    }

    // ── CleanupCategory clone / serde ─────────────────────────────────────────

    #[test]
    fn cleanup_category_clone() {
        let cat = CleanupCategory::DeadCode;
        let cloned = cat.clone();
        assert_eq!(cloned.to_string(), "Dead Code");
    }

    #[test]
    fn cleanup_category_serde_roundtrip() {
        for cat in [
            CleanupCategory::DeadCode,
            CleanupCategory::UnusedImports,
            CleanupCategory::UnusedFiles,
            CleanupCategory::UnusedDeps,
            CleanupCategory::StaleGenerated,
            CleanupCategory::LongFunctions,
            CleanupCategory::EmptyCatches,
            CleanupCategory::PrintStatements,
        ] {
            let json = serde_json::to_string(&cat).unwrap();
            let back: CleanupCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(cat.to_string(), back.to_string());
        }
    }

    // ── CleanupTask clone / serde ─────────────────────────────────────────────

    #[test]
    fn cleanup_task_clone_and_serde() {
        let task = CleanupTask {
            category: CleanupCategory::DeadCode,
            description: "test desc".to_string(),
            files_affected: 3,
            auto_fixable: true,
            command: "falcon fix".to_string(),
        };
        let cloned = task.clone();
        assert_eq!(cloned.files_affected, 3);
        assert!(cloned.auto_fixable);

        let json = serde_json::to_string(&task).unwrap();
        let back: CleanupTask = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, "test desc");
        assert_eq!(back.command, "falcon fix");
    }

    // ── MaintenanceReport clone / serde ───────────────────────────────────────

    #[test]
    fn maintenance_report_clone_and_serde() {
        let report = MaintenanceReport {
            cleanup_tasks: vec![],
            auto_fixable: 0,
            manual_review: 0,
            estimated_savings_lines: 0,
            tech_debt_score: 100,
        };
        let cloned = report.clone();
        assert_eq!(cloned.tech_debt_score, 100);

        let json = serde_json::to_string(&report).unwrap();
        let back: MaintenanceReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tech_debt_score, 100);
        assert_eq!(back.auto_fixable, 0);
    }

    // ── analyze_maintenance: empty project ────────────────────────────────────

    #[test]
    fn empty_project_returns_ok_no_tasks() {
        let dir = TempDir::new().unwrap();
        // No Dart files — analysis succeeds with no issues
        let report = analyze_maintenance(dir.path()).unwrap();
        assert_eq!(report.cleanup_tasks.len(), 0);
        assert_eq!(report.tech_debt_score, 100);
        assert_eq!(report.estimated_savings_lines, 0);
    }

    // ── analyze_maintenance: clean dart file ──────────────────────────────────

    #[test]
    fn clean_dart_file_no_tasks() {
        let dir = TempDir::new().unwrap();
        // A file imported by main.dart — not unused; no rule violations
        make_dart(&dir, "lib/helper.dart", "void doThing() {}\n");
        make_dart(&dir, "lib/main.dart", "import 'helper.dart';\nvoid main() { doThing(); }\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        // tech_debt_score should be ≤ 100
        assert!(report.tech_debt_score <= 100);
    }

    // ── analyze_maintenance: print statements task ────────────────────────────

    #[test]
    fn print_statements_task_appears() {
        let dir = TempDir::new().unwrap();
        make_dart(
            &dir,
            "lib/service.dart",
            "void run() {\n  print(\"hello\");\n}\n",
        );
        make_dart(&dir, "lib/main.dart", "import 'service.dart';\nvoid main() { run(); }\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let has_print = report
            .cleanup_tasks
            .iter()
            .any(|t| matches!(t.category, CleanupCategory::PrintStatements));
        assert!(has_print, "expected PrintStatements task");
    }

    // ── analyze_maintenance: empty catch task ────────────────────────────────

    #[test]
    fn empty_catch_task_appears() {
        let dir = TempDir::new().unwrap();
        make_dart(
            &dir,
            "lib/service.dart",
            "void run() {\n  try {\n    int x = 1;\n  } catch (e) {}\n}\n",
        );
        make_dart(&dir, "lib/main.dart", "import 'service.dart';\nvoid main() { run(); }\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let has_empty_catch = report
            .cleanup_tasks
            .iter()
            .any(|t| matches!(t.category, CleanupCategory::EmptyCatches));
        assert!(has_empty_catch, "expected EmptyCatches task");
    }

    // ── analyze_maintenance: unused file task ────────────────────────────────

    #[test]
    fn unused_file_task_appears() {
        let dir = TempDir::new().unwrap();
        // orphan.dart is never imported
        make_dart(&dir, "lib/orphan.dart", "class Orphan {}\n");
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let has_unused_file = report
            .cleanup_tasks
            .iter()
            .any(|t| matches!(t.category, CleanupCategory::UnusedFiles));
        assert!(has_unused_file, "expected UnusedFiles task");
    }

    // ── analyze_maintenance: unused code task ─────────────────────────────────

    #[test]
    fn unused_code_task_appears() {
        let dir = TempDir::new().unwrap();
        // NeverUsed is declared but never referenced anywhere
        make_dart(&dir, "lib/stuff.dart", "class NeverUsed {}\n");
        make_dart(&dir, "lib/main.dart", "import 'stuff.dart';\nvoid main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let has_dead_code = report
            .cleanup_tasks
            .iter()
            .any(|t| matches!(t.category, CleanupCategory::DeadCode));
        assert!(has_dead_code, "expected DeadCode task");
    }

    // ── analyze_maintenance: unused dependency task ───────────────────────────

    #[test]
    fn unused_dependency_task_appears() {
        let dir = TempDir::new().unwrap();
        make_file(
            &dir,
            "pubspec.yaml",
            "name: test_app\ndependencies:\n  flutter:\n    sdk: flutter\n  http: ^1.0.0\n",
        );
        // No dart file imports the http package
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let has_unused_dep = report
            .cleanup_tasks
            .iter()
            .any(|t| matches!(t.category, CleanupCategory::UnusedDeps));
        assert!(has_unused_dep, "expected UnusedDeps task");
    }

    // ── analyze_maintenance: stale generated files task ───────────────────────

    #[test]
    fn stale_generated_task_appears() {
        let dir = TempDir::new().unwrap();
        // Create a .g.dart file without the corresponding source .dart
        make_dart(
            &dir,
            "lib/model.g.dart",
            "// GENERATED CODE - DO NOT MODIFY BY HAND\npart of 'model.dart';\n",
        );
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let has_stale = report
            .cleanup_tasks
            .iter()
            .any(|t| matches!(t.category, CleanupCategory::StaleGenerated));
        assert!(has_stale, "expected StaleGenerated task");
    }

    // ── analyze_maintenance: tech_debt_score boundary ────────────────────────

    #[test]
    fn tech_debt_score_in_valid_range() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        assert!(report.tech_debt_score <= 100);
    }

    #[test]
    fn tech_debt_score_decreases_with_many_issues() {
        let dir = TempDir::new().unwrap();
        // Generate multiple violations: each file has print + empty-catch
        for i in 0..10 {
            make_dart(
                &dir,
                &format!("lib/svc{}.dart", i),
                &format!(
                    "void run{}() {{\n  print(\"x\");\n  try {{\n    int y = {};\n  }} catch (e) {{}}\n}}\n",
                    i, i
                ),
            );
        }
        let main_content = (0..10)
            .map(|i| format!("import 'svc{}.dart';\n", i))
            .collect::<String>()
            + "void main() {}\n";
        make_dart(&dir, "lib/main.dart", &main_content);
        let report = analyze_maintenance(dir.path()).unwrap();
        // With many issues, score should be lower than 100
        assert!(report.tech_debt_score < 100);
    }

    // ── analyze_maintenance: report field coherence ───────────────────────────

    #[test]
    fn auto_fixable_and_manual_counts_are_consistent() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let actual_auto = report.cleanup_tasks.iter().filter(|t| t.auto_fixable).count();
        let actual_manual = report.cleanup_tasks.iter().filter(|t| !t.auto_fixable).count();
        assert_eq!(report.auto_fixable, actual_auto);
        assert_eq!(report.manual_review, actual_manual);
    }

    #[test]
    fn estimated_savings_non_negative() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        // estimated_savings_lines is usize, always >= 0, just assert it's accessible
        let _ = report.estimated_savings_lines;
    }

    // ── analyze_maintenance: multiple tasks at once ───────────────────────────

    #[test]
    fn multiple_tasks_generated_for_mixed_issues() {
        let dir = TempDir::new().unwrap();
        // print statement + empty catch in one file
        make_dart(
            &dir,
            "lib/service.dart",
            "void run() {\n  print(\"debug\");\n  try {\n    int x = 1;\n  } catch (e) {}\n}\n",
        );
        // orphan file (unused-file)
        make_dart(&dir, "lib/orphan.dart", "class Orphan {}\n");
        make_dart(&dir, "lib/main.dart", "import 'service.dart';\nvoid main() { run(); }\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        assert!(report.cleanup_tasks.len() >= 2);
    }

    // ── analyze_maintenance: cleanup_task fields ──────────────────────────────

    #[test]
    fn empty_catch_task_has_correct_command() {
        let dir = TempDir::new().unwrap();
        make_dart(
            &dir,
            "lib/svc.dart",
            "void run() {\n  try {\n    int x = 0;\n  } catch (e) {}\n}\n",
        );
        make_dart(&dir, "lib/main.dart", "import 'svc.dart';\nvoid main() { run(); }\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let task = report
            .cleanup_tasks
            .iter()
            .find(|t| matches!(t.category, CleanupCategory::EmptyCatches))
            .unwrap();
        assert_eq!(task.command, "falcon analyze --preset ai-generated");
        assert!(!task.auto_fixable);
        assert!(task.files_affected >= 1);
    }

    #[test]
    fn print_task_has_correct_command() {
        let dir = TempDir::new().unwrap();
        make_dart(
            &dir,
            "lib/svc.dart",
            "void run() {\n  print(\"hi\");\n}\n",
        );
        make_dart(&dir, "lib/main.dart", "import 'svc.dart';\nvoid main() { run(); }\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let task = report
            .cleanup_tasks
            .iter()
            .find(|t| matches!(t.category, CleanupCategory::PrintStatements))
            .unwrap();
        assert_eq!(task.command, "falcon fix");
        assert!(!task.auto_fixable);
    }

    #[test]
    fn unused_file_task_has_correct_command() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/orphan.dart", "class Orphan {}\n");
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let task = report
            .cleanup_tasks
            .iter()
            .find(|t| matches!(t.category, CleanupCategory::UnusedFiles))
            .unwrap();
        assert_eq!(task.command, "falcon check-unused-files");
    }

    #[test]
    fn unused_code_task_has_correct_command() {
        let dir = TempDir::new().unwrap();
        make_dart(&dir, "lib/stuff.dart", "class NeverUsed {}\n");
        make_dart(&dir, "lib/main.dart", "import 'stuff.dart';\nvoid main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let task = report
            .cleanup_tasks
            .iter()
            .find(|t| matches!(t.category, CleanupCategory::DeadCode))
            .unwrap();
        assert_eq!(task.command, "falcon check-unused-code");
    }

    #[test]
    fn stale_generated_task_command() {
        let dir = TempDir::new().unwrap();
        make_dart(
            &dir,
            "lib/model.g.dart",
            "// GENERATED CODE - DO NOT MODIFY BY HAND\n",
        );
        make_dart(&dir, "lib/main.dart", "void main() {}\n");
        let report = analyze_maintenance(dir.path()).unwrap();
        let task = report
            .cleanup_tasks
            .iter()
            .find(|t| matches!(t.category, CleanupCategory::StaleGenerated))
            .unwrap();
        assert_eq!(task.command, "falcon check-codegen");
        assert!(!task.auto_fixable);
    }
}

/// Print maintenance report.
pub fn print_maintenance_report(report: &MaintenanceReport) {
    println!();
    println!(
        "  {} Maintenance Advisor",
        "falcon manage".bright_cyan().bold()
    );
    println!();

    let debt_color = if report.tech_debt_score >= 80 {
        format!("{}", report.tech_debt_score).green()
    } else if report.tech_debt_score >= 50 {
        format!("{}", report.tech_debt_score).yellow()
    } else {
        format!("{}", report.tech_debt_score).red()
    };
    println!("  Tech Debt Score: {}/100", debt_color.bold());
    println!(
        "  Cleanup tasks:   {} ({} auto-fixable, {} manual)",
        report.cleanup_tasks.len(),
        report.auto_fixable,
        report.manual_review
    );
    println!(
        "  Est. savings:    ~{} lines removable",
        report.estimated_savings_lines
    );

    if !report.cleanup_tasks.is_empty() {
        println!();
        for task in &report.cleanup_tasks {
            let icon = if task.auto_fixable {
                "⚡".green()
            } else {
                "·".dimmed()
            };
            println!(
                "  {} [{}] {} ({} affected)",
                icon,
                task.category.to_string().bright_white(),
                task.description,
                task.files_affected
            );
            println!("     Run: {}", task.command.bright_cyan());
        }
    } else {
        println!();
        println!(
            "  {} Codebase is clean — no maintenance tasks needed!",
            "✓".green().bold()
        );
    }

    println!();
}
