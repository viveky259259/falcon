use crate::config::FalconConfig;
use crate::metrics;
use crate::parser::DartParser;
use colored::Colorize;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct CodebaseReport {
    pub total_files: usize,
    pub total_lines: usize,
    pub god_files: Vec<GodFile>,
    pub health_score: f64,
    pub tech_debt: TechDebt,
    pub hotspots: Vec<Hotspot>,
    pub narrative: String,
}

#[derive(Debug)]
pub struct GodFile {
    pub path: PathBuf,
    pub lines: usize,
    pub classes: usize,
    pub functions: usize,
    pub complexity: u32,
    pub decomposition_suggestions: Vec<String>,
}

#[derive(Debug)]
pub struct TechDebt {
    pub score: f64,
    pub effort_hours: f64,
    pub categories: HashMap<String, f64>,
}

#[derive(Debug)]
pub struct Hotspot {
    pub path: PathBuf,
    pub reason: String,
    pub score: f64,
}

pub fn analyze_codebase(root: &Path, config: &FalconConfig) -> anyhow::Result<CodebaseReport> {
    let exclude_patterns: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let lib_path = root.join("lib");
    let base = if lib_path.exists() { &lib_path } else { root };

    let dart_files: Vec<PathBuf> = WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !exclude_patterns.iter().any(|p| p.matches_path(rel))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total_files = dart_files.len();
    let mut total_lines = 0;
    let mut file_stats: Vec<FileStats> = Vec::new();

    for file in &dart_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let lines = source.lines().count();
        total_lines += lines;

        let mut parser = match DartParser::new() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let tree = match parser.parse(&source) {
            Some(t) => t,
            None => continue,
        };

        let root_node = tree.root_node();
        let m = metrics::calculate_file_metrics(root_node, &source, &config.metrics);

        let classes_count = count_kind(root_node, "class_declaration");
        let functions_count =
            count_kind(root_node, "function_signature") + count_kind(root_node, "method_signature");

        let max_cc = m
            .functions
            .iter()
            .map(|f| f.cyclomatic_complexity)
            .max()
            .unwrap_or(0);
        let avg_cc = if !m.functions.is_empty() {
            m.functions
                .iter()
                .map(|f| f.cyclomatic_complexity as f64)
                .sum::<f64>()
                / m.functions.len() as f64
        } else {
            0.0
        };
        let avg_mi = if !m.functions.is_empty() {
            m.functions
                .iter()
                .map(|f| f.maintainability_index)
                .sum::<f64>()
                / m.functions.len() as f64
        } else {
            100.0
        };

        file_stats.push(FileStats {
            path: file.clone(),
            lines,
            classes: classes_count,
            functions: functions_count,
            max_complexity: max_cc,
            avg_complexity: avg_cc,
            maintainability: avg_mi,
        });
    }

    let god_files = find_god_files(&file_stats, root);
    let hotspots = find_hotspots(&file_stats, root);
    let tech_debt = calculate_tech_debt(&file_stats);
    let health_score = calculate_health_score(&file_stats);
    let narrative = generate_narrative(
        total_files,
        total_lines,
        &god_files,
        &hotspots,
        health_score,
        &tech_debt,
    );

    Ok(CodebaseReport {
        total_files,
        total_lines,
        god_files,
        health_score,
        tech_debt,
        hotspots,
        narrative,
    })
}

#[derive(Debug)]
struct FileStats {
    path: PathBuf,
    lines: usize,
    classes: usize,
    functions: usize,
    max_complexity: u32,
    avg_complexity: f64,
    maintainability: f64,
}

fn count_kind(node: tree_sitter::Node, kind: &str) -> usize {
    let mut count = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            count += 1;
        }
        count += count_kind(child, kind);
    }
    count
}

fn find_god_files(stats: &[FileStats], _root: &Path) -> Vec<GodFile> {
    let mut gods: Vec<GodFile> = stats
        .iter()
        .filter(|s| s.lines > 500 || s.classes > 5 || s.functions > 30)
        .map(|s| {
            let mut suggestions = Vec::new();

            if s.classes > 3 {
                suggestions.push(format!(
                    "Extract {} classes into separate files (one class per file).",
                    s.classes
                ));
            }

            if s.functions > 20 {
                suggestions.push(format!(
                    "Group {} functions into focused utility classes or mixins.",
                    s.functions
                ));
            }

            if s.lines > 500 {
                let chunks = (s.lines / 200) + 1;
                suggestions.push(format!(
                    "Split into ~{} files of ~200 lines each, organized by responsibility.",
                    chunks
                ));
            }

            if s.max_complexity > 15 {
                suggestions.push(
                    "Refactor high-complexity functions using early returns or strategy pattern."
                        .to_string(),
                );
            }

            GodFile {
                path: s.path.clone(),
                lines: s.lines,
                classes: s.classes,
                functions: s.functions,
                complexity: s.max_complexity,
                decomposition_suggestions: suggestions,
            }
        })
        .collect();

    gods.sort_by_key(|god| Reverse(god.lines));
    gods.truncate(10);
    gods
}

fn find_hotspots(stats: &[FileStats], _root: &Path) -> Vec<Hotspot> {
    let mut hotspots: Vec<Hotspot> = Vec::new();

    for s in stats {
        let mut score = 0.0;
        let mut reasons = Vec::new();

        if s.max_complexity > 20 {
            score += (s.max_complexity as f64 - 20.0) * 2.0;
            reasons.push(format!("cyclomatic complexity {}", s.max_complexity));
        }

        if s.maintainability < 40.0 && s.maintainability > 0.0 {
            score += (40.0 - s.maintainability) * 1.5;
            reasons.push(format!("maintainability index {:.0}", s.maintainability));
        }

        if s.lines > 300 {
            score += (s.lines as f64 - 300.0) * 0.1;
            reasons.push(format!("{} lines", s.lines));
        }

        if score > 10.0 && !reasons.is_empty() {
            hotspots.push(Hotspot {
                path: s.path.clone(),
                reason: reasons.join(", "),
                score,
            });
        }
    }

    hotspots.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hotspots.truncate(15);
    hotspots
}

fn calculate_tech_debt(stats: &[FileStats]) -> TechDebt {
    let mut total_hours = 0.0;
    let mut categories: HashMap<String, f64> = HashMap::new();

    for s in stats {
        if s.max_complexity > 15 {
            let hours = (s.max_complexity as f64 - 15.0) * 0.5;
            total_hours += hours;
            *categories.entry("Complexity".to_string()).or_default() += hours;
        }

        if s.lines > 400 {
            let hours = (s.lines as f64 - 400.0) * 0.02;
            total_hours += hours;
            *categories.entry("File size".to_string()).or_default() += hours;
        }

        if s.maintainability < 50.0 && s.maintainability > 0.0 {
            let hours = (50.0 - s.maintainability) * 0.2;
            total_hours += hours;
            *categories.entry("Maintainability".to_string()).or_default() += hours;
        }
    }

    let max_debt = stats.len() as f64 * 5.0;
    let score = if max_debt > 0.0 {
        ((1.0 - (total_hours / max_debt)) * 100.0).clamp(0.0, 100.0)
    } else {
        100.0
    };

    TechDebt {
        score,
        effort_hours: total_hours,
        categories,
    }
}

fn calculate_health_score(stats: &[FileStats]) -> f64 {
    if stats.is_empty() {
        return 100.0;
    }

    let avg_maintainability: f64 =
        stats.iter().map(|s| s.maintainability).sum::<f64>() / stats.len() as f64;
    let avg_complexity: f64 =
        stats.iter().map(|s| s.avg_complexity).sum::<f64>() / stats.len() as f64;
    let god_file_ratio = stats.iter().filter(|s| s.lines > 500).count() as f64 / stats.len() as f64;

    let maintainability_score = (avg_maintainability / 100.0 * 40.0).min(40.0);
    let complexity_score = ((20.0 - avg_complexity) / 20.0 * 30.0).clamp(0.0, 30.0);
    let structure_score = ((1.0 - god_file_ratio) * 30.0).clamp(0.0, 30.0);

    (maintainability_score + complexity_score + structure_score).clamp(0.0, 100.0)
}

fn generate_narrative(
    total_files: usize,
    total_lines: usize,
    god_files: &[GodFile],
    hotspots: &[Hotspot],
    health_score: f64,
    tech_debt: &TechDebt,
) -> String {
    let mut narrative = String::new();

    let health_label = if health_score >= 80.0 {
        "healthy"
    } else if health_score >= 60.0 {
        "moderate"
    } else if health_score >= 40.0 {
        "needs attention"
    } else {
        "at risk"
    };

    narrative.push_str(&format!(
        "Codebase overview: {} Dart files, ~{} lines of code. Health score: {:.0}/100 ({}).\n\n",
        total_files, total_lines, health_score, health_label
    ));

    if !god_files.is_empty() {
        narrative.push_str(&format!(
            "Structure: {} god file(s) detected (>500 lines or >5 classes). ",
            god_files.len()
        ));
        if let Some(biggest) = god_files.first() {
            narrative.push_str(&format!(
                "Largest is {} lines with {} classes. ",
                biggest.lines, biggest.classes
            ));
        }
        narrative.push_str("These should be top priority for decomposition.\n\n");
    }

    if !hotspots.is_empty() {
        narrative.push_str(&format!(
            "Complexity: {} hotspot(s) flagged for high complexity or low maintainability. ",
            hotspots.len()
        ));
        narrative.push_str("Focus refactoring effort here for maximum impact.\n\n");
    }

    narrative.push_str(&format!(
        "Tech debt: {:.0}/100. Estimated {:.1} hours of remediation effort across {} categories.",
        tech_debt.score,
        tech_debt.effort_hours,
        tech_debt.categories.len()
    ));

    narrative
}

pub fn print_codebase_report(report: &CodebaseReport, root: &Path) {
    println!();
    println!("  {} Codebase Intelligence", "falcon".bright_cyan().bold());
    println!();

    let health_color = if report.health_score >= 80.0 {
        colored::Color::Green
    } else if report.health_score >= 60.0 {
        colored::Color::Yellow
    } else {
        colored::Color::Red
    };

    println!(
        "  Health Score: {}",
        format!("{:.0}/100", report.health_score)
            .color(health_color)
            .bold()
    );
    println!(
        "  Files: {}  Lines: {}",
        report.total_files, report.total_lines
    );
    println!();

    println!("  {}", "Narrative".bright_green().bold());
    for line in report.narrative.lines() {
        if !line.is_empty() {
            println!("    {}", line);
        }
    }
    println!();

    if !report.god_files.is_empty() {
        println!(
            "  {} ({} files)",
            "God Files".bright_red().bold(),
            report.god_files.len()
        );
        for gf in &report.god_files {
            let rel = gf.path.strip_prefix(root).unwrap_or(&gf.path);
            println!(
                "    {} — {} lines, {} classes, {} functions, complexity {}",
                rel.display().to_string().bright_white(),
                gf.lines,
                gf.classes,
                gf.functions,
                gf.complexity
            );
            for suggestion in &gf.decomposition_suggestions {
                println!("      {} {}", "→".bright_green(), suggestion);
            }
        }
        println!();
    }

    if !report.hotspots.is_empty() {
        println!(
            "  {} ({} files)",
            "Complexity Hotspots".bright_yellow().bold(),
            report.hotspots.len()
        );
        for hs in &report.hotspots {
            let rel = hs.path.strip_prefix(root).unwrap_or(&hs.path);
            println!(
                "    {:.0}  {} — {}",
                hs.score,
                rel.display().to_string().bright_white(),
                hs.reason
            );
        }
        println!();
    }

    println!(
        "  {} {:.0}/100 ({:.1}h estimated effort)",
        "Tech Debt Score:".bright_blue().bold(),
        report.tech_debt.score,
        report.tech_debt.effort_hours
    );
    for (cat, hours) in &report.tech_debt.categories {
        println!("    {} — {:.1}h", cat, hours);
    }
    println!();
}
