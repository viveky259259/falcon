use crate::config::FalconConfig;
use crate::metrics;
use crate::parser::DartParser;
use colored::Colorize;
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

    gods.sort_by_key(|e| std::cmp::Reverse(e.lines));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_dart_file(dir: &TempDir, name: &str, content: &str) {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", content).unwrap();
    }

    fn make_dart_file_in(dir: &std::path::Path, name: &str, content: &str) {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", content).unwrap();
    }

    fn simple_stats(
        path: &str,
        lines: usize,
        classes: usize,
        functions: usize,
        max_cc: u32,
        avg_cc: f64,
        mi: f64,
    ) -> FileStats {
        FileStats {
            path: PathBuf::from(path),
            lines,
            classes,
            functions,
            max_complexity: max_cc,
            avg_complexity: avg_cc,
            maintainability: mi,
        }
    }

    fn dummy_path() -> &'static Path {
        Path::new("/tmp")
    }

    // ── count_kind (via analyze_codebase with real Dart) ─────────────────────

    #[test]
    fn count_kind_returns_zero_for_no_matches() {
        let mut parser = crate::parser::DartParser::new().unwrap();
        let source = "void main() {}";
        let tree = parser.parse(source).unwrap();
        let root = tree.root_node();
        assert_eq!(count_kind(root, "class_declaration"), 0);
    }

    #[test]
    fn count_kind_counts_class_declarations() {
        let mut parser = crate::parser::DartParser::new().unwrap();
        let source = "class A {} class B {}";
        let tree = parser.parse(source).unwrap();
        let root = tree.root_node();
        assert_eq!(count_kind(root, "class_declaration"), 2);
    }

    #[test]
    fn count_kind_single_class() {
        let mut parser = crate::parser::DartParser::new().unwrap();
        let source = "class MyClass { void foo() {} }";
        let tree = parser.parse(source).unwrap();
        let root = tree.root_node();
        assert_eq!(count_kind(root, "class_declaration"), 1);
    }

    #[test]
    fn count_kind_nested_nodes() {
        let mut parser = crate::parser::DartParser::new().unwrap();
        let source = "class A { class B {} }";
        let tree = parser.parse(source).unwrap();
        let root = tree.root_node();
        // nested classes should also be counted
        let cnt = count_kind(root, "class_declaration");
        assert!(cnt >= 1);
    }

    // ── find_god_files ────────────────────────────────────────────────────────

    #[test]
    fn find_god_files_empty_stats_returns_empty() {
        let result = find_god_files(&[], dummy_path());
        assert!(result.is_empty());
    }

    #[test]
    fn find_god_files_normal_file_excluded() {
        let stats = vec![simple_stats("a.dart", 100, 1, 5, 5, 2.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert!(result.is_empty());
    }

    #[test]
    fn find_god_files_triggers_on_lines_over_500() {
        let stats = vec![simple_stats("big.dart", 600, 1, 5, 5, 2.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lines, 600);
    }

    #[test]
    fn find_god_files_triggers_on_classes_over_5() {
        let stats = vec![simple_stats("many_classes.dart", 100, 6, 5, 5, 2.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn find_god_files_triggers_on_functions_over_30() {
        let stats = vec![simple_stats("many_fns.dart", 100, 1, 31, 5, 2.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn find_god_files_sorted_by_lines_descending() {
        let stats = vec![
            simple_stats("small.dart", 510, 1, 5, 5, 2.0, 80.0),
            simple_stats("big.dart", 1000, 1, 5, 5, 2.0, 80.0),
            simple_stats("medium.dart", 700, 1, 5, 5, 2.0, 80.0),
        ];
        let result = find_god_files(&stats, dummy_path());
        assert_eq!(result.len(), 3);
        assert!(result[0].lines >= result[1].lines);
        assert!(result[1].lines >= result[2].lines);
    }

    #[test]
    fn find_god_files_truncated_to_10() {
        let stats: Vec<FileStats> = (0..15)
            .map(|i| simple_stats(&format!("f{}.dart", i), 600, 1, 5, 5, 2.0, 80.0))
            .collect();
        let result = find_god_files(&stats, dummy_path());
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn find_god_files_suggestions_classes_over_3() {
        // needs > 5 classes to enter god files, then > 3 classes to get the suggestion
        let stats = vec![simple_stats("a.dart", 100, 6, 5, 5, 2.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert_eq!(result.len(), 1);
        assert!(result[0]
            .decomposition_suggestions
            .iter()
            .any(|s| s.contains("classes")));
    }

    #[test]
    fn find_god_files_suggestions_functions_over_20() {
        let stats = vec![simple_stats("a.dart", 100, 1, 31, 5, 2.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert!(result[0]
            .decomposition_suggestions
            .iter()
            .any(|s| s.contains("functions")));
    }

    #[test]
    fn find_god_files_suggestions_lines_over_500() {
        let stats = vec![simple_stats("a.dart", 600, 1, 5, 5, 2.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert!(result[0]
            .decomposition_suggestions
            .iter()
            .any(|s| s.contains("files")));
    }

    #[test]
    fn find_god_files_suggestions_high_complexity() {
        let stats = vec![simple_stats("a.dart", 600, 1, 5, 16, 5.0, 80.0)];
        let result = find_god_files(&stats, dummy_path());
        assert!(result[0]
            .decomposition_suggestions
            .iter()
            .any(|s| s.contains("Refactor")));
    }

    // ── find_hotspots ─────────────────────────────────────────────────────────

    #[test]
    fn find_hotspots_empty_stats_returns_empty() {
        let result = find_hotspots(&[], dummy_path());
        assert!(result.is_empty());
    }

    #[test]
    fn find_hotspots_normal_file_not_flagged() {
        let stats = vec![simple_stats("a.dart", 100, 1, 5, 5, 2.0, 80.0)];
        let result = find_hotspots(&stats, dummy_path());
        assert!(result.is_empty());
    }

    #[test]
    fn find_hotspots_high_complexity_flagged() {
        // complexity > 20 + lines > 300 gives score > 10
        let stats = vec![simple_stats("complex.dart", 400, 1, 5, 25, 10.0, 80.0)];
        let result = find_hotspots(&stats, dummy_path());
        assert_eq!(result.len(), 1);
        assert!(result[0].reason.contains("cyclomatic complexity"));
    }

    #[test]
    fn find_hotspots_low_maintainability_flagged() {
        // maintainability < 40 and > 0, plus lines > 300 to push score > 10
        let stats = vec![simple_stats("low_mi.dart", 400, 1, 5, 5, 2.0, 20.0)];
        let result = find_hotspots(&stats, dummy_path());
        assert_eq!(result.len(), 1);
        assert!(result[0].reason.contains("maintainability index"));
    }

    #[test]
    fn find_hotspots_zero_maintainability_not_flagged_for_mi() {
        // maintainability == 0.0 skips the MI check; needs other triggers to score > 10
        let stats = vec![simple_stats("zero_mi.dart", 100, 1, 5, 5, 2.0, 0.0)];
        let result = find_hotspots(&stats, dummy_path());
        // score would be 0, so not added
        assert!(result.is_empty());
    }

    #[test]
    fn find_hotspots_sorted_by_score_descending() {
        let stats = vec![
            simple_stats("a.dart", 400, 1, 5, 22, 10.0, 20.0),
            simple_stats("b.dart", 500, 1, 5, 30, 15.0, 20.0),
        ];
        let result = find_hotspots(&stats, dummy_path());
        assert!(result.len() >= 2);
        assert!(result[0].score >= result[1].score);
    }

    #[test]
    fn find_hotspots_truncated_to_15() {
        let stats: Vec<FileStats> = (0..20)
            .map(|i| simple_stats(&format!("f{}.dart", i), 400, 1, 5, 25, 10.0, 20.0))
            .collect();
        let result = find_hotspots(&stats, dummy_path());
        assert!(result.len() <= 15);
    }

    // ── calculate_tech_debt ───────────────────────────────────────────────────

    #[test]
    fn tech_debt_empty_stats_score_100() {
        let debt = calculate_tech_debt(&[]);
        assert_eq!(debt.score, 100.0);
        assert_eq!(debt.effort_hours, 0.0);
        assert!(debt.categories.is_empty());
    }

    #[test]
    fn tech_debt_clean_file_has_low_hours() {
        let stats = vec![simple_stats("a.dart", 100, 1, 5, 5, 2.0, 80.0)];
        let debt = calculate_tech_debt(&stats);
        assert_eq!(debt.effort_hours, 0.0);
        assert!(debt.categories.is_empty());
    }

    #[test]
    fn tech_debt_high_complexity_adds_hours() {
        let stats = vec![simple_stats("a.dart", 100, 1, 5, 20, 5.0, 80.0)];
        let debt = calculate_tech_debt(&stats);
        assert!(debt.effort_hours > 0.0);
        assert!(debt.categories.contains_key("Complexity"));
    }

    #[test]
    fn tech_debt_large_file_adds_hours() {
        let stats = vec![simple_stats("a.dart", 500, 1, 5, 5, 2.0, 80.0)];
        let debt = calculate_tech_debt(&stats);
        assert!(debt.effort_hours > 0.0);
        assert!(debt.categories.contains_key("File size"));
    }

    #[test]
    fn tech_debt_low_maintainability_adds_hours() {
        let stats = vec![simple_stats("a.dart", 100, 1, 5, 5, 2.0, 30.0)];
        let debt = calculate_tech_debt(&stats);
        assert!(debt.effort_hours > 0.0);
        assert!(debt.categories.contains_key("Maintainability"));
    }

    #[test]
    fn tech_debt_score_clamped_0_to_100() {
        // Very many terrible files; score should not go below 0
        let stats: Vec<FileStats> = (0..100)
            .map(|i| simple_stats(&format!("f{}.dart", i), 5000, 1, 5, 100, 50.0, 1.0))
            .collect();
        let debt = calculate_tech_debt(&stats);
        assert!(debt.score >= 0.0);
        assert!(debt.score <= 100.0);
    }

    // ── calculate_health_score ────────────────────────────────────────────────

    #[test]
    fn health_score_empty_returns_100() {
        assert_eq!(calculate_health_score(&[]), 100.0);
    }

    #[test]
    fn health_score_perfect_file_near_100() {
        let stats = vec![simple_stats("a.dart", 100, 1, 5, 1, 1.0, 100.0)];
        let score = calculate_health_score(&stats);
        // maintainability 100 → 40pts, complexity 1 avg → ~28.5pts, no god file → 30pts
        assert!(score > 90.0, "Expected score > 90 but got {}", score);
    }

    #[test]
    fn health_score_clamped_0_to_100() {
        let stats: Vec<FileStats> = (0..10)
            .map(|i| simple_stats(&format!("f{}.dart", i), 600, 1, 5, 1, 50.0, 0.0))
            .collect();
        let score = calculate_health_score(&stats);
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    #[test]
    fn health_score_all_god_files_lowers_structure() {
        let all_god: Vec<FileStats> = (0..5)
            .map(|i| simple_stats(&format!("f{}.dart", i), 600, 1, 5, 1, 1.0, 100.0))
            .collect();
        let no_god: Vec<FileStats> = (0..5)
            .map(|i| simple_stats(&format!("f{}.dart", i), 100, 1, 5, 1, 1.0, 100.0))
            .collect();
        assert!(calculate_health_score(&all_god) < calculate_health_score(&no_god));
    }

    // ── generate_narrative ────────────────────────────────────────────────────

    fn empty_debt() -> TechDebt {
        TechDebt {
            score: 100.0,
            effort_hours: 0.0,
            categories: HashMap::new(),
        }
    }

    #[test]
    fn narrative_contains_file_and_line_counts() {
        let s = generate_narrative(42, 1234, &[], &[], 85.0, &empty_debt());
        assert!(s.contains("42"));
        assert!(s.contains("1234"));
    }

    #[test]
    fn narrative_health_label_healthy() {
        let s = generate_narrative(1, 100, &[], &[], 80.0, &empty_debt());
        assert!(s.contains("healthy"));
    }

    #[test]
    fn narrative_health_label_moderate() {
        let s = generate_narrative(1, 100, &[], &[], 65.0, &empty_debt());
        assert!(s.contains("moderate"));
    }

    #[test]
    fn narrative_health_label_needs_attention() {
        let s = generate_narrative(1, 100, &[], &[], 50.0, &empty_debt());
        assert!(s.contains("needs attention"));
    }

    #[test]
    fn narrative_health_label_at_risk() {
        let s = generate_narrative(1, 100, &[], &[], 30.0, &empty_debt());
        assert!(s.contains("at risk"));
    }

    #[test]
    fn narrative_with_god_files_mentions_god_file() {
        let god = GodFile {
            path: PathBuf::from("big.dart"),
            lines: 800,
            classes: 3,
            functions: 10,
            complexity: 5,
            decomposition_suggestions: vec![],
        };
        let s = generate_narrative(5, 1000, &[god], &[], 70.0, &empty_debt());
        assert!(s.contains("god file"));
        assert!(s.contains("800"));
    }

    #[test]
    fn narrative_with_hotspots_mentions_hotspot() {
        let hs = Hotspot {
            path: PathBuf::from("hot.dart"),
            reason: "cyclomatic complexity 25".to_string(),
            score: 20.0,
        };
        let s = generate_narrative(5, 1000, &[], &[hs], 70.0, &empty_debt());
        assert!(s.contains("hotspot"));
    }

    #[test]
    fn narrative_no_god_files_no_god_section() {
        let s = generate_narrative(5, 1000, &[], &[], 70.0, &empty_debt());
        assert!(!s.contains("god file"));
    }

    #[test]
    fn narrative_no_hotspots_no_hotspot_section() {
        let s = generate_narrative(5, 1000, &[], &[], 70.0, &empty_debt());
        assert!(!s.contains("hotspot"));
    }

    #[test]
    fn narrative_always_contains_tech_debt() {
        let s = generate_narrative(5, 1000, &[], &[], 70.0, &empty_debt());
        assert!(s.contains("Tech debt"));
    }

    // ── analyze_codebase (orchestrator) ──────────────────────────────────────

    #[test]
    fn analyze_codebase_empty_dir_returns_zero_files() {
        let dir = TempDir::new().unwrap();
        let config = crate::config::FalconConfig::default();
        let report = analyze_codebase(dir.path(), &config).unwrap();
        assert_eq!(report.total_files, 0);
        assert_eq!(report.total_lines, 0);
        assert!(report.god_files.is_empty());
        assert!(report.hotspots.is_empty());
    }

    #[test]
    fn analyze_codebase_non_dart_files_ignored() {
        let dir = TempDir::new().unwrap();
        make_dart_file(&dir, "readme.txt", "hello world");
        make_dart_file(&dir, "style.css", ".foo { color: red; }");
        let config = crate::config::FalconConfig::default();
        let report = analyze_codebase(dir.path(), &config).unwrap();
        assert_eq!(report.total_files, 0);
    }

    #[test]
    fn analyze_codebase_single_dart_file_counted() {
        let dir = TempDir::new().unwrap();
        make_dart_file(&dir, "main.dart", "void main() {}\n");
        let config = crate::config::FalconConfig::default();
        let report = analyze_codebase(dir.path(), &config).unwrap();
        assert_eq!(report.total_files, 1);
        assert!(report.total_lines >= 1);
    }

    #[test]
    fn analyze_codebase_uses_lib_subdir_when_present() {
        let dir = TempDir::new().unwrap();
        let lib_dir = dir.path().join("lib");
        std::fs::create_dir_all(&lib_dir).unwrap();
        make_dart_file_in(&lib_dir, "app.dart", "void main() {}\n");
        // file outside lib
        make_dart_file(&dir, "outside.dart", "void helper() {}\n");
        let config = crate::config::FalconConfig::default();
        let report = analyze_codebase(dir.path(), &config).unwrap();
        // only lib/ is walked, so only 1 file
        assert_eq!(report.total_files, 1);
    }

    #[test]
    fn analyze_codebase_returns_valid_narrative() {
        let dir = TempDir::new().unwrap();
        make_dart_file(&dir, "main.dart", "class Foo {} void main() {}\n");
        let config = crate::config::FalconConfig::default();
        let report = analyze_codebase(dir.path(), &config).unwrap();
        assert!(!report.narrative.is_empty());
        assert!(report.narrative.contains("Dart files"));
    }

    #[test]
    fn analyze_codebase_health_score_in_range() {
        let dir = TempDir::new().unwrap();
        make_dart_file(&dir, "a.dart", "class A {} void main() {}\n");
        let config = crate::config::FalconConfig::default();
        let report = analyze_codebase(dir.path(), &config).unwrap();
        assert!(report.health_score >= 0.0 && report.health_score <= 100.0);
    }

    #[test]
    fn analyze_codebase_multiple_dart_files() {
        let dir = TempDir::new().unwrap();
        for i in 0..5 {
            make_dart_file(&dir, &format!("file{}.dart", i), "void main() {}\n");
        }
        let config = crate::config::FalconConfig::default();
        let report = analyze_codebase(dir.path(), &config).unwrap();
        assert_eq!(report.total_files, 5);
    }
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
