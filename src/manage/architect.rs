//! Architecture Governor — enforce architectural decisions, detect violations,
//! and suggest structural improvements.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
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
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
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
    module_map.sort_by_key(|module| Reverse(module.file_count));
    complexity_hotspots.sort_by_key(|hotspot| Reverse(hotspot.lines));

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
        "Feature-First" if file.contains("/features/") => {
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
