//! Unified Project Health Dashboard — combines all Falcon analyses into
//! a single health report with actionable priorities.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Unified project health report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub project_name: String,
    pub overall_health: u32,
    pub grade: String,
    pub dimensions: HealthDimensions,
    pub top_priorities: Vec<Priority>,
    pub stats: ProjectStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDimensions {
    pub code_quality: u32,
    pub security: u32,
    pub performance: u32,
    pub maintainability: u32,
    pub test_coverage: u32,
    pub dependency_health: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Priority {
    pub rank: usize,
    pub category: String,
    pub description: String,
    pub impact: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub dart_files: usize,
    pub total_lines: usize,
    pub total_issues: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub unused_files: usize,
    pub generated_files: usize,
}

/// Generate a unified health report for a Flutter project.
pub fn generate_health_report(root: &Path) -> anyhow::Result<HealthReport> {
    let project_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    let ai_score = crate::ai_score::score::score_from_report(&report)?;

    let vuln_findings = crate::analysis::vuln_radar::scan_vulnerabilities(root);
    let perf_report = crate::analysis::devtools_bridge::analyze_performance(root);
    let codegen = crate::analysis::codegen_quality::analyze_codegen(root);

    let code_quality = ai_score.overall;

    let security = if vuln_findings.is_empty() {
        100
    } else {
        100u32.saturating_sub(vuln_findings.len() as u32 * 15)
    };

    let perf_issues = perf_report.all_issues().len();
    let performance = 100u32.saturating_sub(perf_issues as u32 * 5);

    let long_fn = report
        .issues
        .iter()
        .filter(|i| i.rule == "avoid-long-functions")
        .count();
    let nested = report
        .issues
        .iter()
        .filter(|i| i.rule == "avoid-nested-conditionals")
        .count();
    let unused = report
        .issues
        .iter()
        .filter(|i| i.rule.starts_with("unused"))
        .count();
    let maintainability = 100u32.saturating_sub((long_fn + nested + unused) as u32);

    let test_dir = root.join("test");
    let test_files = if test_dir.exists() {
        walkdir::WalkDir::new(&test_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
            .count()
    } else {
        0
    };
    let test_coverage = if report.file_count > 0 {
        ((test_files as f64 / report.file_count as f64) * 100.0).min(100.0) as u32
    } else {
        0
    };

    let dep_issues = report
        .issues
        .iter()
        .filter(|i| i.rule == "unused-dependency")
        .count();
    let dependency_health = 100u32.saturating_sub(dep_issues as u32 * 10);

    let overall = (code_quality as f64 * 0.30
        + security as f64 * 0.20
        + performance as f64 * 0.15
        + maintainability as f64 * 0.15
        + test_coverage as f64 * 0.10
        + dependency_health as f64 * 0.10) as u32;

    let grade = match overall {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    }
    .to_string();

    let mut priorities = Vec::new();
    let mut rank = 1;

    if security < 80 {
        priorities.push(Priority {
            rank,
            category: "Security".to_string(),
            description: format!("{} vulnerability findings", vuln_findings.len()),
            impact: "Critical — fix before deployment".to_string(),
            command: "falcon vuln-scan".to_string(),
        });
        rank += 1;
    }
    if ai_score.error_handling.score < 50 {
        priorities.push(Priority {
            rank,
            category: "Error Handling".to_string(),
            description: format!(
                "Score {}/100 — empty catches, unawaited futures",
                ai_score.error_handling.score
            ),
            impact: "High — crashes in production".to_string(),
            command: "falcon analyze --preset ai-generated".to_string(),
        });
        rank += 1;
    }
    if ai_score.resource_safety.score < 60 {
        priorities.push(Priority {
            rank,
            category: "Resource Safety".to_string(),
            description: format!(
                "Score {}/100 — undisposed controllers",
                ai_score.resource_safety.score
            ),
            impact: "High — memory leaks".to_string(),
            command: "falcon check-widgets".to_string(),
        });
        rank += 1;
    }
    if perf_issues > 10 {
        priorities.push(Priority {
            rank,
            category: "Performance".to_string(),
            description: format!("{} performance anti-patterns", perf_issues),
            impact: "Medium — janky UI".to_string(),
            command: "falcon x check-perf".to_string(),
        });
        rank += 1;
    }
    if test_coverage < 30 {
        priorities.push(Priority {
            rank,
            category: "Test Coverage".to_string(),
            description: format!(
                "{} test files for {} source files",
                test_files, report.file_count
            ),
            impact: "Medium — regression risk".to_string(),
            command: "falcon test-gen --write".to_string(),
        });
    }

    let mut total_lines = 0;
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
    {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            total_lines += content.lines().count();
        }
    }

    Ok(HealthReport {
        project_name,
        overall_health: overall,
        grade,
        dimensions: HealthDimensions {
            code_quality,
            security,
            performance,
            maintainability,
            test_coverage,
            dependency_health,
        },
        top_priorities: priorities,
        stats: ProjectStats {
            dart_files: report.file_count,
            total_lines,
            total_issues: report.issues.len(),
            error_count: report.error_count(),
            warning_count: report.warning_count(),
            unused_files: report
                .issues
                .iter()
                .filter(|i| i.rule == "unused-file")
                .count(),
            generated_files: codegen.total_generated_files,
        },
    })
}

/// Print health report.
pub fn print_health_report(report: &HealthReport) {
    println!();
    println!(
        "  {} Project Health — {}",
        "falcon manage".bright_cyan().bold(),
        report.project_name.bright_white().bold()
    );
    println!("  {}", "═".repeat(55));

    let health_color = match report.overall_health {
        85..=100 => report.overall_health.to_string().bright_green().bold(),
        70..=84 => report.overall_health.to_string().yellow().bold(),
        _ => report.overall_health.to_string().red().bold(),
    };
    println!();
    println!(
        "  Overall Health: {}/100 (Grade: {})",
        health_color, report.grade
    );
    println!();

    print_bar("Code Quality", report.dimensions.code_quality, 30);
    print_bar("Security", report.dimensions.security, 20);
    print_bar("Performance", report.dimensions.performance, 15);
    print_bar("Maintainability", report.dimensions.maintainability, 15);
    print_bar("Test Coverage", report.dimensions.test_coverage, 10);
    print_bar("Dependencies", report.dimensions.dependency_health, 10);

    println!();
    println!("  {} Stats", "▸".bright_cyan());
    println!(
        "    Files: {} ({} generated) | Lines: {} | Issues: {} ({} errors, {} warnings)",
        report.stats.dart_files,
        report.stats.generated_files,
        report.stats.total_lines,
        report.stats.total_issues,
        report.stats.error_count,
        report.stats.warning_count
    );

    if !report.top_priorities.is_empty() {
        println!();
        println!("  {} Action Items (by priority)", "▸".bright_yellow());
        for p in &report.top_priorities {
            println!(
                "    {}. [{}] {} — {}",
                p.rank,
                p.category.bright_white(),
                p.description,
                p.impact.dimmed()
            );
            println!("       Run: {}", p.command.bright_cyan());
        }
    }

    println!();
}

fn print_bar(name: &str, score: u32, weight: u32) {
    let bar_len = (score as usize) / 5;
    let bar = "█".repeat(bar_len);
    let empty = "░".repeat(20 - bar_len);
    let color = if score >= 80 {
        format!("{:>3}", score).green()
    } else if score >= 60 {
        format!("{:>3}", score).yellow()
    } else {
        format!("{:>3}", score).red()
    };
    println!(
        "    {:<18} {}/100 {}{} ({}%)",
        name,
        color,
        bar.bright_cyan(),
        empty.dimmed(),
        weight
    );
}
