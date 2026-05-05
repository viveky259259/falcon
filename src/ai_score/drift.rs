use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::convention::{detect_conventions, ConventionReport};

/// A single drift finding where new code deviates from established patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftFinding {
    pub file: String,
    pub line: usize,
    pub category: DriftCategory,
    pub expected: String,
    pub actual: String,
    pub message: String,
}

/// Categories of convention drift.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DriftCategory {
    Naming,
    Architecture,
    ErrorHandling,
    StateManagement,
    Pattern,
}

impl std::fmt::Display for DriftCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftCategory::Naming => write!(f, "naming"),
            DriftCategory::Architecture => write!(f, "architecture"),
            DriftCategory::ErrorHandling => write!(f, "error-handling"),
            DriftCategory::StateManagement => write!(f, "state-management"),
            DriftCategory::Pattern => write!(f, "pattern"),
        }
    }
}

/// Drift detection report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub conventions: ConventionReport,
    pub findings: Vec<DriftFinding>,
    pub drift_score: f64,
    pub files_analyzed: usize,
}

/// Detect convention drift in changed files (or all files if no git ref given).
pub fn detect_drift(root: &Path, since: Option<&str>) -> anyhow::Result<DriftReport> {
    let conventions = detect_conventions(root)?;

    let files = if let Some(git_ref) = since {
        get_changed_files(root, git_ref)?
    } else {
        collect_dart_files(root)
    };

    let files_analyzed = files.len();
    let mut findings = Vec::new();

    for file_path in &files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let rel = file_path
            .strip_prefix(root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        check_naming_drift(&conventions, &rel, &source, &mut findings);
        check_architecture_drift(&conventions, &rel, &mut findings);
        check_error_handling_drift(&conventions, &rel, &source, &mut findings);
        check_state_management_drift(&conventions, &rel, &source, &mut findings);
    }

    let drift_score = if files_analyzed > 0 {
        let per_file = findings.len() as f64 / files_analyzed as f64;
        (100.0 - per_file * 20.0).max(0.0).min(100.0)
    } else {
        100.0
    };

    Ok(DriftReport {
        conventions,
        findings,
        drift_score,
        files_analyzed,
    })
}

fn check_naming_drift(
    conventions: &ConventionReport,
    file_name: &str,
    source: &str,
    findings: &mut Vec<DriftFinding>,
) {
    if conventions.naming.file_naming == "snake_case" {
        let base = std::path::Path::new(file_name)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        if base != base.to_lowercase()
            && !base.ends_with(".g.dart")
            && !base.ends_with(".freezed.dart")
        {
            findings.push(DriftFinding {
                file: file_name.to_string(),
                line: 1,
                category: DriftCategory::Naming,
                expected: "snake_case".to_string(),
                actual: base.to_string(),
                message: format!(
                    "File '{}' doesn't follow the project's snake_case naming convention",
                    base
                ),
            });
        }
    }

    if conventions.naming.class_naming == "PascalCase" {
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ") || trimmed.starts_with("abstract class ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let class_idx = parts.iter().position(|p| *p == "class").unwrap_or(0) + 1;
                if let Some(class_name) = parts.get(class_idx) {
                    let name = class_name.trim_end_matches('{').trim_end_matches('<');
                    if !name.is_empty() && !name.chars().next().unwrap_or('a').is_uppercase() {
                        findings.push(DriftFinding {
                            file: file_name.to_string(),
                            line: i + 1,
                            category: DriftCategory::Naming,
                            expected: "PascalCase".to_string(),
                            actual: name.to_string(),
                            message: format!(
                                "Class '{}' doesn't follow PascalCase convention",
                                name
                            ),
                        });
                    }
                }
            }
        }
    }
}

fn check_architecture_drift(
    conventions: &ConventionReport,
    file_path: &str,
    findings: &mut Vec<DriftFinding>,
) {
    let pattern = &conventions.architecture.pattern;

    if pattern == "Clean Architecture" {
        let parts: Vec<&str> = file_path.split('/').collect();
        if parts.len() >= 2 {
            let top_dir = parts
                .iter()
                .find(|p| matches!(**p, "domain" | "data" | "presentation" | "core" | "shared"));
            if top_dir.is_none() && !file_path.contains("test") && !file_path.contains("main.dart")
            {
                findings.push(DriftFinding {
                    file: file_path.to_string(),
                    line: 1,
                    category: DriftCategory::Architecture,
                    expected: "Clean Architecture (domain/data/presentation)".to_string(),
                    actual: "file outside layer structure".to_string(),
                    message: format!(
                        "File '{}' is outside the Clean Architecture layer structure",
                        file_path
                    ),
                });
            }
        }
    } else if pattern == "Feature-First" {
        let parts: Vec<&str> = file_path.split('/').collect();
        if parts.len() >= 2 {
            let has_feature = parts
                .iter()
                .any(|p| *p == "features" || *p == "core" || *p == "shared");
            if !has_feature && !file_path.contains("test") && !file_path.contains("main.dart") {
                findings.push(DriftFinding {
                    file: file_path.to_string(),
                    line: 1,
                    category: DriftCategory::Architecture,
                    expected: "Feature-First (features/)".to_string(),
                    actual: "file outside feature structure".to_string(),
                    message: format!(
                        "File '{}' is outside the Feature-First structure",
                        file_path
                    ),
                });
            }
        }
    }
}

fn check_error_handling_drift(
    conventions: &ConventionReport,
    file_name: &str,
    source: &str,
    findings: &mut Vec<DriftFinding>,
) {
    if conventions.error_handling.uses_result_type || conventions.error_handling.uses_either {
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("try {") && !file_name.contains("test") {
                let has_catch_rethrow = source.lines().skip(i).take(10).any(|l| {
                    l.contains("rethrow") || l.contains("Result.failure") || l.contains("Left(")
                });
                if !has_catch_rethrow {
                    let has_empty_catch = source
                        .lines()
                        .skip(i)
                        .take(10)
                        .any(|l| l.trim() == "} catch (e) {}" || l.trim() == "} catch (e) {");
                    if has_empty_catch {
                        findings.push(DriftFinding {
                            file: file_name.to_string(),
                            line: i + 1,
                            category: DriftCategory::ErrorHandling,
                            expected: "Result/Either pattern".to_string(),
                            actual: "empty catch block".to_string(),
                            message: "Project uses Result/Either pattern but this code has an empty catch block".to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn check_state_management_drift(
    conventions: &ConventionReport,
    file_name: &str,
    source: &str,
    findings: &mut Vec<DriftFinding>,
) {
    let dominant = match &conventions.state_management {
        Some(sm) => sm.clone(),
        None => return,
    };

    if file_name.contains("test") {
        return;
    }

    let mixed_signals: Vec<(&str, &str)> = vec![
        ("BLoC", "GetxController"),
        ("BLoC", "ChangeNotifier"),
        ("Riverpod", "GetxController"),
        ("Riverpod", "BlocProvider"),
        ("Provider", "GetxController"),
    ];

    for (primary, conflicting_marker) in &mixed_signals {
        if dominant == *primary && source.contains(conflicting_marker) {
            findings.push(DriftFinding {
                file: file_name.to_string(),
                line: 1,
                category: DriftCategory::StateManagement,
                expected: dominant.clone(),
                actual: format!("uses {}", conflicting_marker),
                message: format!(
                    "Project uses {} but this file contains {}",
                    dominant, conflicting_marker
                ),
            });
        }
    }
}

fn get_changed_files(root: &Path, git_ref: &str) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", git_ref])
        .current_dir(root)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let files: Vec<std::path::PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(".dart"))
        .map(|line| root.join(line))
        .filter(|p| p.exists())
        .collect();

    Ok(files)
}

fn collect_dart_files(root: &Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// Print drift report to the console.
pub fn print_drift_report(report: &DriftReport) {
    println!();
    println!(
        "  {} Convention Drift Detector",
        "falcon".bright_cyan().bold()
    );
    println!();

    let drift_color = if report.drift_score >= 90.0 {
        format!("{:.0}%", report.drift_score).bright_green()
    } else if report.drift_score >= 70.0 {
        format!("{:.0}%", report.drift_score).yellow()
    } else {
        format!("{:.0}%", report.drift_score).red()
    };

    println!("  Convention Adherence: {}", drift_color.bold());
    println!(
        "  Architecture:        {}",
        report.conventions.architecture.pattern.bright_white()
    );
    if let Some(ref sm) = report.conventions.state_management {
        println!("  State Management:    {}", sm.bright_white());
    }
    println!("  Files analyzed:      {}", report.files_analyzed);
    println!("  Drift findings:      {}", report.findings.len());

    if !report.findings.is_empty() {
        println!();

        let mut by_category: std::collections::HashMap<&DriftCategory, Vec<&DriftFinding>> =
            std::collections::HashMap::new();
        for f in &report.findings {
            by_category.entry(&f.category).or_default().push(f);
        }

        for (cat, cat_findings) in &by_category {
            println!(
                "  {} {} ({} findings)",
                "▸".bright_cyan(),
                cat.to_string().bright_white().bold(),
                cat_findings.len()
            );
            for f in cat_findings.iter().take(5) {
                println!(
                    "    {} {}:{}  {}",
                    "·".dimmed(),
                    f.file.dimmed(),
                    f.line,
                    f.message
                );
            }
            if cat_findings.len() > 5 {
                println!(
                    "    {} ... and {} more",
                    "·".dimmed(),
                    cat_findings.len() - 5
                );
            }
            println!();
        }
    } else {
        println!();
        println!(
            "  {} No convention drift detected — code is consistent!",
            "✓".green().bold()
        );
        println!();
    }
}
