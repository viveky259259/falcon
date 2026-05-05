//! Multi-language analysis for platform channel code (Kotlin/Swift).
//! Uses text-based heuristics since tree-sitter grammars aren't bundled.

use crate::config::Severity;
use crate::reporters::Issue;
use std::path::Path;

/// Analyze platform channel code in Kotlin and Swift files.
pub fn analyze_platform_channels(root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    let android_dir = root.join("android");
    let ios_dir = root.join("ios");

    if android_dir.exists() {
        issues.extend(analyze_kotlin_files(&android_dir, root));
    }
    if ios_dir.exists() {
        issues.extend(analyze_swift_files(&ios_dir, root));
    }

    issues
}

fn analyze_kotlin_files(dir: &Path, _root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "kt" || ext == "java")
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        check_kotlin_channel_issues(entry.path(), &source, &mut issues);
    }

    issues
}

fn analyze_swift_files(dir: &Path, _root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "swift"))
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        check_swift_channel_issues(entry.path(), &source, &mut issues);
    }

    issues
}

fn check_kotlin_channel_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("MethodChannel") && trimmed.contains("\"") {
            if let Some(channel_name) = extract_string_literal(trimmed) {
                if !channel_name.contains('/') && !channel_name.contains('.') {
                    issues.push(Issue {
                        rule: "platform-channel-naming".to_string(),
                        message: format!(
                            "Platform channel '{}' should use reverse-domain naming (e.g. 'com.example/channel')",
                            channel_name
                        ),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    });
                }
            }
        }

        if trimmed.contains("catch")
            && trimmed.contains("Exception")
            && !trimmed.contains("FlutterError")
        {
            let next_lines: String = source.lines().skip(i).take(3).collect::<Vec<_>>().join(" ");
            if next_lines.contains("{}") || next_lines.contains("{ }") {
                issues.push(Issue {
                    rule: "platform-empty-catch".to_string(),
                    message: "Empty catch block in platform channel code — errors should be forwarded to Flutter via result.error()".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }

        if trimmed.contains("runOnUiThread")
            && source
                .lines()
                .skip(i)
                .take(5)
                .any(|l| l.contains("result.success") || l.contains("result.error"))
        {
            issues.push(Issue {
                rule: "platform-thread-safety".to_string(),
                message: "Ensure MethodChannel result is called on the main thread".to_string(),
                severity: Severity::Info,
                file: file.to_path_buf(),
                line: i + 1,
                column: 1,
            });
        }
    }
}

fn check_swift_channel_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("FlutterMethodChannel") && trimmed.contains("\"") {
            if let Some(channel_name) = extract_string_literal(trimmed) {
                if !channel_name.contains('/') && !channel_name.contains('.') {
                    issues.push(Issue {
                        rule: "platform-channel-naming".to_string(),
                        message: format!(
                            "Platform channel '{}' should use reverse-domain naming (e.g. 'com.example/channel')",
                            channel_name
                        ),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    });
                }
            }
        }

        if trimmed.contains("catch") && !trimmed.contains("FlutterError") {
            let next_lines: String = source.lines().skip(i).take(3).collect::<Vec<_>>().join(" ");
            if next_lines.contains("{}")
                || next_lines.contains("{ }")
                || next_lines.contains("catch { }")
            {
                issues.push(Issue {
                    rule: "platform-empty-catch".to_string(),
                    message: "Empty catch block in platform channel code — errors should be forwarded to Flutter via result()".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }

        if trimmed.contains("DispatchQueue.global")
            && source
                .lines()
                .skip(i)
                .take(5)
                .any(|l| l.contains("result(") || l.contains("FlutterResult"))
        {
            issues.push(Issue {
                rule: "platform-thread-safety".to_string(),
                message: "Ensure FlutterResult is called on the main thread (DispatchQueue.main)"
                    .to_string(),
                severity: Severity::Info,
                file: file.to_path_buf(),
                line: i + 1,
                column: 1,
            });
        }
    }
}

fn extract_string_literal(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Summary of platform channel analysis.
pub fn print_platform_summary(issues: &[Issue]) {
    use colored::Colorize;

    let kt_count = issues
        .iter()
        .filter(|i| {
            i.file
                .extension()
                .map_or(false, |e| e == "kt" || e == "java")
        })
        .count();
    let swift_count = issues
        .iter()
        .filter(|i| i.file.extension().map_or(false, |e| e == "swift"))
        .count();

    println!();
    println!(
        "  {} Platform Channel Analysis",
        "falcon".bright_cyan().bold()
    );
    println!();
    println!("  Kotlin/Java issues: {}", kt_count);
    println!("  Swift issues:       {}", swift_count);
    println!("  Total:              {}", issues.len());

    if issues.is_empty() {
        println!();
        println!("  {} No platform channel issues found.", "✓".green().bold());
    }
    println!();
}
