use crate::config::FalconConfig;
use crate::parser::DartParser;
use colored::Colorize;
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
    let changed_files = get_changed_files(root, git_ref)?;
    let dart_files: Vec<_> = changed_files
        .iter()
        .filter(|f| f.to_string_lossy().ends_with(".dart"))
        .cloned()
        .collect();

    if dart_files.is_empty() {
        return Ok(ReviewReport {
            observations: Vec::new(),
            files_reviewed: 0,
            lines_changed: 0,
        });
    }

    let mut observations = Vec::new();
    let mut total_lines = 0;

    let project_patterns = collect_project_patterns(root, &dart_files);

    for file in &dart_files {
        let abs = root.join(file);
        let source = match std::fs::read_to_string(&abs) {
            Ok(s) => s,
            Err(_) => continue,
        };

        total_lines += source.lines().count();

        check_error_handling(&abs, &source, &mut observations);
        check_naming_consistency(&abs, &source, &project_patterns, &mut observations);

        match strictness {
            ReviewStrictness::Standard | ReviewStrictness::Thorough => {
                check_pattern_consistency(&abs, &source, &project_patterns, &mut observations);
            }
            ReviewStrictness::Quick => {}
        }

        if matches!(strictness, ReviewStrictness::Thorough) {
            check_missing_tests(root, file, &source, &mut observations);
        }
    }

    Ok(ReviewReport {
        observations,
        files_reviewed: dart_files.len(),
        lines_changed: total_lines,
    })
}

fn get_changed_files(root: &Path, git_ref: &str) -> anyhow::Result<Vec<PathBuf>> {
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

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect())
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
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
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

fn check_error_handling(
    file: &Path,
    source: &str,
    observations: &mut Vec<ReviewObservation>,
) {
    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("catch (e)") && !trimmed.contains("on ") {
            let next_lines: Vec<&str> = source
                .lines()
                .skip(line_num + 1)
                .take(3)
                .collect();
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
                suggestion: Some("At minimum, log the error. Better: handle it properly or let it propagate.".to_string()),
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
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() && name.chars().next().map_or(false, |c| c.is_lowercase()) {
                observations.push(ReviewObservation {
                    category: ObservationCategory::NamingConvention,
                    message: format!("Class '{}' should use UpperCamelCase.", name),
                    file: file.to_path_buf(),
                    line: node.start_position().row + 1,
                    suggestion: Some(format!("Rename to '{}{}'.", name[..1].to_uppercase(), &name[1..])),
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
    let dominant_error_pattern = if patterns.error_handling.uses_result_type > patterns.error_handling.uses_try_catch {
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

    let file_name = file
        .file_stem()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    let test_file = test_dir.join(format!("{}_test.dart", file_name));

    if !test_file.exists() {
        let public_functions: Vec<&str> = source
            .lines()
            .filter(|l| {
                let t = l.trim();
                (t.starts_with("void ") || t.starts_with("Future") || t.starts_with("String ") || t.starts_with("int ") || t.starts_with("bool "))
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
                suggestion: Some(format!("Create test/{}_test.dart with tests for the public API.", file_name)),
                severity: ObservationSeverity::Suggestion,
            });
        }
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
        println!(
            "  {} No issues found — LGTM!",
            "✓".green().bold()
        );
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

        let rel = obs
            .file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("?");

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
