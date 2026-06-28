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
        (100.0 - issues_per_file * 5.0).clamp(0.0, 100.0) as u32
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
