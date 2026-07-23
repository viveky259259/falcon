use crate::config::FalconConfig;
use crate::parser::DartParser;
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ReviewObservation {
    pub category: ObservationCategory,
    pub message: String,
    pub file: PathBuf,
    pub line: usize,
    pub suggestion: Option<String>,
    pub severity: ObservationSeverity,
}

#[derive(Debug, Clone)]
pub enum ObservationCategory {
    PatternConsistency,
    NamingConvention,
    ErrorHandling,
    MissingTest,
    CodeStyle,
    Performance,
}

impl ObservationCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PatternConsistency => "Pattern",
            Self::NamingConvention => "Naming",
            Self::ErrorHandling => "Error Handling",
            Self::MissingTest => "Testing",
            Self::CodeStyle => "Style",
            Self::Performance => "Performance",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObservationSeverity {
    Critical,
    Suggestion,
    Nitpick,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ReviewStrictness {
    Quick,
    Standard,
    Thorough,
}

pub struct ReviewReport {
    pub observations: Vec<ReviewObservation>,
    pub files_reviewed: usize,
    pub lines_changed: usize,
}

/// Review changed files since a git ref.
pub fn review_diff(
    root: &Path,
    git_ref: &str,
    _config: &FalconConfig,
    strictness: ReviewStrictness,
) -> anyhow::Result<ReviewReport> {
    let dart_files = changed_dart_files(root, git_ref)?;

    if dart_files.is_empty() {
        return Ok(ReviewReport {
            observations: Vec::new(),
            files_reviewed: 0,
            lines_changed: 0,
        });
    }

    let mut observations = Vec::new();
    let mut total_lines = 0;
    let dart_files_relative: Vec<PathBuf> = dart_files
        .iter()
        .map(|file| relative_to_root(root, file))
        .collect();

    let project_patterns = collect_project_patterns(root, &dart_files_relative);

    for (file, relative_file) in dart_files.iter().zip(dart_files_relative.iter()) {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        total_lines += source.lines().count();

        check_error_handling(file, &source, &mut observations);
        check_naming_consistency(file, &source, &project_patterns, &mut observations);

        match strictness {
            ReviewStrictness::Standard | ReviewStrictness::Thorough => {
                check_pattern_consistency(file, &source, &project_patterns, &mut observations);
            }
            ReviewStrictness::Quick => {}
        }

        if matches!(strictness, ReviewStrictness::Thorough) {
            check_missing_tests(root, relative_file, &source, &mut observations);
        }
    }

    Ok(ReviewReport {
        observations,
        files_reviewed: dart_files.len(),
        lines_changed: total_lines,
    })
}

/// Return existing Dart files changed relative to `base_ref...HEAD`.
pub fn changed_dart_files(root: &Path, base_ref: &str) -> anyhow::Result<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args([
            "diff",
            "--name-only",
            "--diff-filter=ACMR",
            &format!("{}...HEAD", base_ref),
        ])
        .current_dir(root)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(".dart"))
        .map(|line| root.join(line))
        .filter(|path| path.is_file())
        .collect())
}

/// Return Dart file renames relative to `base_ref...HEAD` as current path -> old path.
pub fn renamed_dart_files(
    root: &Path,
    base_ref: &str,
) -> anyhow::Result<HashMap<PathBuf, PathBuf>> {
    let output = std::process::Command::new("git")
        .args([
            "diff",
            "--name-status",
            "--diff-filter=R",
            "-M",
            &format!("{}...HEAD", base_ref),
        ])
        .current_dir(root)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let mut renames = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.split('\t');
        let status = parts.next().unwrap_or_default();
        let old = parts.next().unwrap_or_default();
        let new = parts.next().unwrap_or_default();
        if status.starts_with('R') && old.ends_with(".dart") && new.ends_with(".dart") {
            renames.insert(root.join(new), root.join(old));
        }
    }

    Ok(renames)
}

fn relative_to_root(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root).unwrap_or(file).to_path_buf()
}

#[derive(Default)]
struct ProjectPatterns {
    error_handling: ErrorHandlingPattern,
    _naming: NamingPatterns,
}

#[derive(Default)]
struct ErrorHandlingPattern {
    uses_try_catch: usize,
    uses_result_type: usize,
    uses_either: usize,
    total_error_sites: usize,
}

#[derive(Default)]
struct NamingPatterns {
    _private_prefix_underscore: usize,
    _camel_case_methods: usize,
    _screaming_case_constants: usize,
}

fn collect_project_patterns(root: &Path, _changed_files: &[PathBuf]) -> ProjectPatterns {
    let mut patterns = ProjectPatterns::default();

    let lib_path = root.join("lib");
    if !lib_path.exists() {
        return patterns;
    }

    for entry in walkdir::WalkDir::new(&lib_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .take(50)
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            patterns.error_handling.total_error_sites += source.matches("catch").count();
            patterns.error_handling.uses_try_catch += source.matches("try {").count();
            patterns.error_handling.uses_result_type += source.matches("Result<").count();
            patterns.error_handling.uses_either += source.matches("Either<").count();
        }
    }

    patterns
}

fn check_error_handling(file: &Path, source: &str, observations: &mut Vec<ReviewObservation>) {
    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("catch (e)") && !trimmed.contains("on ") {
            let next_lines: Vec<&str> = source.lines().skip(line_num + 1).take(3).collect();
            let catch_body = next_lines.join(" ");

            if catch_body.contains("print(") || catch_body.trim().starts_with('}') {
                observations.push(ReviewObservation {
                    category: ObservationCategory::ErrorHandling,
                    message: "Generic catch with no meaningful handling — errors will be silently swallowed.".to_string(),
                    file: file.to_path_buf(),
                    line: line_num + 1,
                    suggestion: Some("Use specific exception types (on FormatException catch (e)) and handle or rethrow.".to_string()),
                    severity: ObservationSeverity::Critical,
                });
            }
        }

        if trimmed.contains("catch (e) {}") || trimmed.contains("catch (_) {}") {
            observations.push(ReviewObservation {
                category: ObservationCategory::ErrorHandling,
                message: "Empty catch block — all exceptions are silently ignored.".to_string(),
                file: file.to_path_buf(),
                line: line_num + 1,
                suggestion: Some(
                    "At minimum, log the error. Better: handle it properly or let it propagate."
                        .to_string(),
                ),
                severity: ObservationSeverity::Critical,
            });
        }
    }
}

fn check_naming_consistency(
    file: &Path,
    source: &str,
    _patterns: &ProjectPatterns,
    observations: &mut Vec<ReviewObservation>,
) {
    let mut parser = match DartParser::new() {
        Ok(p) => p,
        Err(_) => return,
    };

    let tree = match parser.parse(source) {
        Some(t) => t,
        None => return,
    };

    let root = tree.root_node();
    check_naming_node(root, source, file, observations);
}

fn check_naming_node(
    node: tree_sitter::Node,
    source: &str,
    file: &Path,
    observations: &mut Vec<ReviewObservation>,
) {
    if node.kind() == "class_declaration" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        if let Some(name_start) = text.find("class ") {
            let rest = &text[name_start + 6..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_lowercase()) {
                observations.push(ReviewObservation {
                    category: ObservationCategory::NamingConvention,
                    message: format!("Class '{}' should use UpperCamelCase.", name),
                    file: file.to_path_buf(),
                    line: node.start_position().row + 1,
                    suggestion: Some(format!(
                        "Rename to '{}{}'.",
                        name[..1].to_uppercase(),
                        &name[1..]
                    )),
                    severity: ObservationSeverity::Suggestion,
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_naming_node(child, source, file, observations);
    }
}

fn check_pattern_consistency(
    file: &Path,
    source: &str,
    patterns: &ProjectPatterns,
    observations: &mut Vec<ReviewObservation>,
) {
    let dominant_error_pattern =
        if patterns.error_handling.uses_result_type > patterns.error_handling.uses_try_catch {
            "Result type"
        } else if patterns.error_handling.uses_either > patterns.error_handling.uses_try_catch {
            "Either type"
        } else {
            "try-catch"
        };

    let file_uses_try_catch = source.matches("try {").count();
    let file_uses_result = source.matches("Result<").count();
    let file_uses_either = source.matches("Either<").count();

    if dominant_error_pattern == "Result type" && file_uses_try_catch > 0 && file_uses_result == 0 {
        observations.push(ReviewObservation {
            category: ObservationCategory::PatternConsistency,
            message: format!(
                "This file uses try-catch, but the project predominantly uses {}. Consider aligning for consistency.",
                dominant_error_pattern
            ),
            file: file.to_path_buf(),
            line: 1,
            suggestion: Some("Wrap error-prone code in Result<T, E> to match project conventions.".to_string()),
            severity: ObservationSeverity::Suggestion,
        });
    }

    if dominant_error_pattern == "Either type" && file_uses_try_catch > 0 && file_uses_either == 0 {
        observations.push(ReviewObservation {
            category: ObservationCategory::PatternConsistency,
            message: format!(
                "This file uses try-catch, but the project predominantly uses {}.",
                dominant_error_pattern
            ),
            file: file.to_path_buf(),
            line: 1,
            suggestion: None,
            severity: ObservationSeverity::Suggestion,
        });
    }
}

fn check_missing_tests(
    root: &Path,
    file: &PathBuf,
    source: &str,
    observations: &mut Vec<ReviewObservation>,
) {
    let file_str = file.to_string_lossy();
    if file_str.contains("test") || file_str.contains("generated") {
        return;
    }

    let test_dir = root.join("test");
    if !test_dir.exists() {
        return;
    }

    let file_name = file.file_stem().and_then(|f| f.to_str()).unwrap_or("");

    let test_file = test_dir.join(format!("{}_test.dart", file_name));

    if !test_file.exists() {
        let public_functions: Vec<&str> = source
            .lines()
            .filter(|l| {
                let t = l.trim();
                (t.starts_with("void ")
                    || t.starts_with("Future")
                    || t.starts_with("String ")
                    || t.starts_with("int ")
                    || t.starts_with("bool "))
                    && !t.starts_with("void _")
                    && t.contains('(')
                    && !t.starts_with("//")
            })
            .collect();

        if public_functions.len() >= 2 {
            observations.push(ReviewObservation {
                category: ObservationCategory::MissingTest,
                message: format!(
                    "File has {} public functions but no corresponding test file ({}_test.dart).",
                    public_functions.len(),
                    file_name
                ),
                file: root.join(file),
                line: 1,
                suggestion: Some(format!(
                    "Create test/{}_test.dart with tests for the public API.",
                    file_name
                )),
                severity: ObservationSeverity::Suggestion,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn empty_patterns() -> ProjectPatterns {
        ProjectPatterns::default()
    }

    fn patterns_with(
        uses_try_catch: usize,
        uses_result_type: usize,
        uses_either: usize,
    ) -> ProjectPatterns {
        ProjectPatterns {
            error_handling: ErrorHandlingPattern {
                uses_try_catch,
                uses_result_type,
                uses_either,
                total_error_sites: uses_try_catch + uses_result_type + uses_either,
            },
            _naming: NamingPatterns::default(),
        }
    }

    // --- ObservationCategory::label ---

    #[test]
    fn observation_category_pattern_consistency_label() {
        assert_eq!(ObservationCategory::PatternConsistency.label(), "Pattern");
    }

    #[test]
    fn observation_category_naming_convention_label() {
        assert_eq!(ObservationCategory::NamingConvention.label(), "Naming");
    }

    #[test]
    fn observation_category_error_handling_label() {
        assert_eq!(ObservationCategory::ErrorHandling.label(), "Error Handling");
    }

    #[test]
    fn observation_category_missing_test_label() {
        assert_eq!(ObservationCategory::MissingTest.label(), "Testing");
    }

    #[test]
    fn observation_category_code_style_label() {
        assert_eq!(ObservationCategory::CodeStyle.label(), "Style");
    }

    #[test]
    fn observation_category_performance_label() {
        assert_eq!(ObservationCategory::Performance.label(), "Performance");
    }

    // --- check_error_handling ---

    #[test]
    fn check_error_handling_flags_generic_catch_with_print() {
        let file = PathBuf::from("test.dart");
        let source = "try {\n  x();\n} catch (e) {\n  print(e);\n}";
        let mut observations = Vec::new();
        check_error_handling(&file, source, &mut observations);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].severity, ObservationSeverity::Critical);
        assert!(matches!(
            observations[0].category,
            ObservationCategory::ErrorHandling
        ));
        assert!(
            observations[0].message.contains("Generic catch")
                || observations[0].message.contains("silently swallowed")
        );
    }

    #[test]
    fn check_error_handling_flags_empty_catch_e() {
        let file = PathBuf::from("test.dart");
        let source = "try { x(); } catch (e) {}";
        let mut observations = Vec::new();
        check_error_handling(&file, source, &mut observations);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].severity, ObservationSeverity::Critical);
        assert!(observations[0].message.contains("Empty catch block"));
    }

    #[test]
    fn check_error_handling_flags_empty_catch_underscore() {
        let file = PathBuf::from("test.dart");
        let source = "try { x(); } catch (_) {}";
        let mut observations = Vec::new();
        check_error_handling(&file, source, &mut observations);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].severity, ObservationSeverity::Critical);
    }

    #[test]
    fn check_error_handling_clean_code_no_observations() {
        let file = PathBuf::from("test.dart");
        let source = "try { x(); } on FormatException catch (e) { log(e); }";
        let mut observations = Vec::new();
        check_error_handling(&file, source, &mut observations);
        assert!(observations.is_empty());
    }

    // --- check_pattern_consistency ---

    #[test]
    fn check_pattern_consistency_flags_try_catch_when_result_dominates() {
        let file = PathBuf::from("feature.dart");
        let source = "try { doWork(); } catch (e) { handle(e); }";
        let patterns = patterns_with(1, 10, 0);
        let mut observations = Vec::new();
        check_pattern_consistency(&file, source, &patterns, &mut observations);
        assert_eq!(observations.len(), 1);
        assert!(observations[0].message.contains("Result type"));
    }

    #[test]
    fn check_pattern_consistency_flags_try_catch_when_either_dominates() {
        let file = PathBuf::from("feature.dart");
        let source = "try { doWork(); } catch (e) { handle(e); }";
        let patterns = patterns_with(1, 0, 10);
        let mut observations = Vec::new();
        check_pattern_consistency(&file, source, &patterns, &mut observations);
        assert_eq!(observations.len(), 1);
        assert!(observations[0].message.contains("Either type"));
    }

    #[test]
    fn check_pattern_consistency_no_observation_when_try_catch_dominates() {
        let file = PathBuf::from("feature.dart");
        let source = "try { doWork(); } catch (e) { handle(e); }";
        let patterns = patterns_with(10, 0, 0);
        let mut observations = Vec::new();
        check_pattern_consistency(&file, source, &patterns, &mut observations);
        assert!(observations.is_empty());
    }

    // --- check_naming_consistency / check_naming_node ---

    #[test]
    fn check_naming_lowercase_class_flagged() {
        let file = PathBuf::from("foo.dart");
        let source = "class foo {}";
        let patterns = empty_patterns();
        let mut observations = Vec::new();
        check_naming_consistency(&file, source, &patterns, &mut observations);
        assert_eq!(observations.len(), 1);
        assert!(matches!(
            observations[0].category,
            ObservationCategory::NamingConvention
        ));
        assert_eq!(observations[0].severity, ObservationSeverity::Suggestion);
        assert!(observations[0].message.contains("UpperCamelCase"));
        let suggestion = observations[0].suggestion.as_deref().unwrap_or("");
        assert!(suggestion.contains("Foo"));
    }

    #[test]
    fn check_naming_proper_case_no_observation() {
        let file = PathBuf::from("foo.dart");
        let source = "class Foo {}";
        let patterns = empty_patterns();
        let mut observations = Vec::new();
        check_naming_consistency(&file, source, &patterns, &mut observations);
        assert!(observations.is_empty());
    }

    #[test]
    fn check_naming_invalid_dart_returns_early() {
        let file = PathBuf::from("bad.dart");
        let source = "@@@ not valid dart";
        let patterns = empty_patterns();
        let mut observations = Vec::new();
        check_naming_consistency(&file, source, &patterns, &mut observations);
        // Parser may return a tree with errors but no class_declaration nodes,
        // so no NamingConvention observations should be produced.
        let naming_obs: Vec<_> = observations
            .iter()
            .filter(|o| matches!(o.category, ObservationCategory::NamingConvention))
            .collect();
        assert!(naming_obs.is_empty());
    }

    // --- check_missing_tests ---

    #[test]
    fn check_missing_tests_no_test_dir_returns_early() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file = PathBuf::from("lib/my_feature.dart");
        let source = "void doA() {}\nvoid doB() {}\nvoid doC() {}";
        let mut observations = Vec::new();
        check_missing_tests(root, &file, source, &mut observations);
        assert!(observations.is_empty());
    }

    #[test]
    fn check_missing_tests_flags_file_without_test() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("test")).unwrap();
        let file = PathBuf::from("lib/my_feature.dart");
        let source = "void doA() {}\nvoid doB() {}\nvoid doC() {}";
        let mut observations = Vec::new();
        check_missing_tests(root, &file, source, &mut observations);
        assert_eq!(observations.len(), 1);
        assert!(matches!(
            observations[0].category,
            ObservationCategory::MissingTest
        ));
        assert!(observations[0].message.contains("my_feature_test.dart"));
    }

    #[test]
    fn check_missing_tests_skips_when_test_file_exists() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("test")).unwrap();
        std::fs::write(root.join("test").join("my_feature_test.dart"), "").unwrap();
        let file = PathBuf::from("lib/my_feature.dart");
        let source = "void doA() {}\nvoid doB() {}\nvoid doC() {}";
        let mut observations = Vec::new();
        check_missing_tests(root, &file, source, &mut observations);
        assert!(observations.is_empty());
    }

    // --- collect_project_patterns ---

    #[test]
    fn collect_project_patterns_no_lib_dir_returns_default() {
        let dir = TempDir::new().unwrap();
        let patterns = collect_project_patterns(dir.path(), &[]);
        assert_eq!(patterns.error_handling.uses_try_catch, 0);
        assert_eq!(patterns.error_handling.uses_result_type, 0);
        assert_eq!(patterns.error_handling.uses_either, 0);
        assert_eq!(patterns.error_handling.total_error_sites, 0);
    }

    #[test]
    fn collect_project_patterns_counts_try_catch_and_result_in_lib() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let lib_dir = root.join("lib");
        std::fs::create_dir(&lib_dir).unwrap();
        let content =
            "void foo() {\n  try { } catch (e) {}\n  Result<int, String> r = doSomething();\n}";
        std::fs::write(lib_dir.join("a.dart"), content).unwrap();
        let patterns = collect_project_patterns(root, &[]);
        assert!(patterns.error_handling.uses_try_catch > 0);
        assert!(patterns.error_handling.uses_result_type > 0);
        assert!(patterns.error_handling.total_error_sites > 0);
    }
}

pub fn print_review(report: &ReviewReport) {
    println!();
    println!(
        "  {} PR Review ({} file(s), ~{} lines)",
        "falcon review".bright_cyan().bold(),
        report.files_reviewed,
        report.lines_changed
    );

    if report.observations.is_empty() {
        println!();
        println!("  {} No issues found — LGTM!", "✓".green().bold());
        println!();
        return;
    }

    let critical: Vec<_> = report
        .observations
        .iter()
        .filter(|o| o.severity == ObservationSeverity::Critical)
        .collect();
    let suggestions: Vec<_> = report
        .observations
        .iter()
        .filter(|o| o.severity == ObservationSeverity::Suggestion)
        .collect();
    let nitpicks: Vec<_> = report
        .observations
        .iter()
        .filter(|o| o.severity == ObservationSeverity::Nitpick)
        .collect();

    println!(
        "  {} critical  {} suggestions  {} nitpicks",
        critical.len().to_string().red().bold(),
        suggestions.len().to_string().yellow(),
        nitpicks.len().to_string().dimmed()
    );
    println!();

    for obs in &report.observations {
        let icon = match obs.severity {
            ObservationSeverity::Critical => "🔴",
            ObservationSeverity::Suggestion => "🟡",
            ObservationSeverity::Nitpick => "⚪",
        };

        let rel = obs.file.file_name().and_then(|f| f.to_str()).unwrap_or("?");

        println!(
            "  {} [{}] {}:{} {}",
            icon,
            obs.category.label(),
            rel,
            obs.line,
            obs.message
        );

        if let Some(ref suggestion) = obs.suggestion {
            println!("     {} {}", "→".bright_green(), suggestion);
        }
    }
    println!();
}
