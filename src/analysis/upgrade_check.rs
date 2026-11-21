//! Flutter Upgrade Compatibility Checker — detect deprecated APIs
//! and patterns that may break in future Flutter versions.

use crate::config::Severity;
use crate::reporters::Issue;
use colored::Colorize;
use std::path::Path;

/// A compatibility finding with migration guidance.
#[derive(Debug, Clone)]
pub struct CompatFinding {
    pub issue: Issue,
    pub deprecated_in: String,
    pub removed_in: Option<String>,
    pub migration: String,
}

/// Known deprecated Flutter/Dart APIs with version info.
const DEPRECATED_APIS: &[(&str, &str, Option<&str>, &str, &str)] = &[
    (
        "FlatButton(",
        "Flutter 1.22",
        Some("Flutter 2.0"),
        "Replace with TextButton",
        "FlatButton is removed — use TextButton with TextButton.styleFrom()",
    ),
    (
        "RaisedButton(",
        "Flutter 1.22",
        Some("Flutter 2.0"),
        "Replace with ElevatedButton",
        "RaisedButton is removed — use ElevatedButton",
    ),
    (
        "OutlineButton(",
        "Flutter 1.22",
        Some("Flutter 2.0"),
        "Replace with OutlinedButton",
        "OutlineButton is removed — use OutlinedButton",
    ),
    (
        "accentColor",
        "Flutter 2.0",
        Some("Flutter 3.0"),
        "Use colorScheme.secondary",
        "ThemeData.accentColor is removed — use Theme.of(context).colorScheme.secondary",
    ),
    (
        "bodyText1",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use bodyLarge",
        "TextTheme.bodyText1 → bodyLarge",
    ),
    (
        "bodyText2",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use bodyMedium",
        "TextTheme.bodyText2 → bodyMedium",
    ),
    (
        "headline1",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use displayLarge",
        "TextTheme.headline1 → displayLarge",
    ),
    (
        "headline2",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use displayMedium",
        "TextTheme.headline2 → displayMedium",
    ),
    (
        "headline3",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use displaySmall",
        "TextTheme.headline3 → displaySmall",
    ),
    (
        "headline4",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use headlineMedium",
        "TextTheme.headline4 → headlineMedium",
    ),
    (
        "headline5",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use headlineSmall",
        "TextTheme.headline5 → headlineSmall",
    ),
    (
        "headline6",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use titleLarge",
        "TextTheme.headline6 → titleLarge",
    ),
    (
        "subtitle1",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use titleMedium",
        "TextTheme.subtitle1 → titleMedium",
    ),
    (
        "subtitle2",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use titleSmall",
        "TextTheme.subtitle2 → titleSmall",
    ),
    (
        "caption",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use bodySmall",
        "TextTheme.caption → bodySmall",
    ),
    (
        "overline",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use labelSmall",
        "TextTheme.overline → labelSmall",
    ),
    (
        "button",
        "Flutter 3.0",
        Some("Flutter 4.0"),
        "Use labelLarge",
        "TextTheme.button → labelLarge",
    ),
    (
        ".brightnessOf(",
        "Flutter 3.0",
        None,
        "Use MediaQuery.platformBrightnessOf",
        "ThemeData.brightnessOf is deprecated",
    ),
    (
        "WillPopScope(",
        "Flutter 3.12",
        None,
        "Use PopScope",
        "WillPopScope is deprecated — use PopScope with canPop and onPopInvokedWithResult",
    ),
    (
        "MaterialApp.router(",
        "Never",
        None,
        "Valid API — no migration needed",
        "",
    ),
    ("showSnackBar(", "Never", None, "", ""),
];

/// Check a project for Flutter upgrade compatibility issues.
pub fn check_upgrade_compatibility(root: &Path) -> Vec<CompatFinding> {
    let mut findings = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let p = e.path().to_string_lossy();
            !p.contains(".g.dart") && !p.contains(".freezed.dart") && !p.contains("/test/")
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        check_file_compat(entry.path(), &source, &mut findings);
    }

    findings
}

fn check_file_compat(file: &Path, source: &str, findings: &mut Vec<CompatFinding>) {
    for (i, line) in source.lines().enumerate() {
        let code = code_only(line);
        if code.trim().is_empty() {
            continue;
        }

        for &(pattern, deprecated_in, removed_in, migration, message) in DEPRECATED_APIS {
            if message.is_empty() {
                continue;
            }
            if uses_api(&code, pattern, message.starts_with("TextTheme.")) {
                let severity = if removed_in.is_some() {
                    Severity::Error
                } else {
                    Severity::Warning
                };

                findings.push(CompatFinding {
                    issue: Issue {
                        rule: "flutter-upgrade-compat".to_string(),
                        message: message.to_string(),
                        severity,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    },
                    deprecated_in: deprecated_in.to_string(),
                    removed_in: removed_in.map(|s| s.to_string()),
                    migration: migration.to_string(),
                });
            }
        }
    }
}

/// `line` with string-literal contents blanked out and any trailing `//`
/// comment removed, so text like `'pushed the button'` can't match an API.
fn code_only(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    chars.next();
                    out.push_str("  ");
                    continue;
                }
                if c == q {
                    quote = None;
                    out.push(c);
                } else {
                    out.push(' ');
                }
            }
            None => {
                if c == '/' && chars.peek() == Some(&'/') {
                    break;
                }
                if c == '\'' || c == '"' {
                    quote = Some(c);
                }
                out.push(c);
            }
        }
    }
    out
}

/// Whether `code` uses `pattern`. Patterns with punctuation (`FlatButton(`)
/// match as written; bare names match whole identifiers only, and TextTheme
/// style names (`button`, `caption`, ...) only as a member access (`.button`).
fn uses_api(code: &str, pattern: &str, member_only: bool) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    if !pattern.chars().all(is_ident) {
        return code.contains(pattern);
    }
    code.match_indices(pattern).any(|(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + pattern.len()..].chars().next();
        let whole = !before.is_some_and(is_ident) && !after.is_some_and(is_ident);
        whole && (!member_only || before == Some('.'))
    })
}

/// Print upgrade compatibility report.
pub fn print_compat_report(findings: &[CompatFinding]) {
    println!();
    println!(
        "  {} Flutter Upgrade Compatibility Check",
        "falcon".bright_cyan().bold()
    );
    println!();

    if findings.is_empty() {
        println!(
            "  {} No deprecated API usage detected — ready for upgrade!",
            "✓".green().bold()
        );
        println!();
        return;
    }

    let removed: Vec<&CompatFinding> = findings.iter().filter(|f| f.removed_in.is_some()).collect();
    let deprecated: Vec<&CompatFinding> =
        findings.iter().filter(|f| f.removed_in.is_none()).collect();

    if !removed.is_empty() {
        println!(
            "  {} Removed APIs ({} usages) — will cause compile errors:",
            "●".bright_red().bold(),
            removed.len()
        );
        for f in removed.iter().take(15) {
            let rel = f
                .issue
                .file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            println!(
                "    {} {}:{} {}",
                "✗".red(),
                rel,
                f.issue.line,
                f.issue.message
            );
            println!(
                "      {} {} → {}",
                "→".green(),
                f.deprecated_in.dimmed(),
                f.migration.bright_white()
            );
        }
        if removed.len() > 15 {
            println!("    ... and {} more", removed.len() - 15);
        }
        println!();
    }

    if !deprecated.is_empty() {
        println!(
            "  {} Deprecated APIs ({} usages) — will warn but still compile:",
            "●".yellow(),
            deprecated.len()
        );
        for f in deprecated.iter().take(10) {
            let rel = f
                .issue
                .file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            println!(
                "    {} {}:{} {}",
                "⚠".yellow(),
                rel,
                f.issue.line,
                f.issue.message
            );
            println!("      {} {}", "→".green(), f.migration.bright_white());
        }
        if deprecated.len() > 10 {
            println!("    ... and {} more", deprecated.len() - 10);
        }
        println!();
    }

    println!(
        "  Total: {} issues ({} removed, {} deprecated)",
        findings.len(),
        removed.len(),
        deprecated.len()
    );
    println!();
}

#[cfg(test)]
mod code_match_tests {
    use super::*;
    use std::path::Path;

    fn findings(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        check_file_compat(Path::new("lib/a.dart"), src, &mut out);
        out.into_iter().map(|f| f.issue.message).collect()
    }

    #[test]
    fn words_inside_strings_and_comments_are_ignored() {
        let src = "Text('You have pushed the button this many times:'), // caption here\n";
        assert!(findings(src).is_empty(), "{:?}", findings(src));
    }

    #[test]
    fn deprecated_text_theme_members_are_found() {
        let found = findings("final s = Theme.of(context).textTheme.button;\n");
        assert_eq!(found, vec!["TextTheme.button → labelLarge".to_string()]);
    }

    #[test]
    fn identifiers_that_only_contain_a_name_are_ignored() {
        assert!(findings("final submitButton = button2; myButton(caption: x);\n").is_empty());
        assert_eq!(findings("ThemeData(accentColor: c)\n").len(), 1);
        assert_eq!(findings("child: FlatButton(onPressed: f)\n").len(), 1);
    }
}
