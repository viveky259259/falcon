//! Flutter Upgrade Compatibility Checker — detect deprecated APIs
//! and patterns that may break in future Flutter versions.

use crate::reporters::Issue;
use crate::config::Severity;
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
    ("FlatButton(", "Flutter 1.22", Some("Flutter 2.0"), "Replace with TextButton", "FlatButton is removed — use TextButton with TextButton.styleFrom()"),
    ("RaisedButton(", "Flutter 1.22", Some("Flutter 2.0"), "Replace with ElevatedButton", "RaisedButton is removed — use ElevatedButton"),
    ("OutlineButton(", "Flutter 1.22", Some("Flutter 2.0"), "Replace with OutlinedButton", "OutlineButton is removed — use OutlinedButton"),
    ("accentColor", "Flutter 2.0", Some("Flutter 3.0"), "Use colorScheme.secondary", "ThemeData.accentColor is removed — use Theme.of(context).colorScheme.secondary"),
    ("bodyText1", "Flutter 3.0", Some("Flutter 4.0"), "Use bodyLarge", "TextTheme.bodyText1 → bodyLarge"),
    ("bodyText2", "Flutter 3.0", Some("Flutter 4.0"), "Use bodyMedium", "TextTheme.bodyText2 → bodyMedium"),
    ("headline1", "Flutter 3.0", Some("Flutter 4.0"), "Use displayLarge", "TextTheme.headline1 → displayLarge"),
    ("headline2", "Flutter 3.0", Some("Flutter 4.0"), "Use displayMedium", "TextTheme.headline2 → displayMedium"),
    ("headline3", "Flutter 3.0", Some("Flutter 4.0"), "Use displaySmall", "TextTheme.headline3 → displaySmall"),
    ("headline4", "Flutter 3.0", Some("Flutter 4.0"), "Use headlineMedium", "TextTheme.headline4 → headlineMedium"),
    ("headline5", "Flutter 3.0", Some("Flutter 4.0"), "Use headlineSmall", "TextTheme.headline5 → headlineSmall"),
    ("headline6", "Flutter 3.0", Some("Flutter 4.0"), "Use titleLarge", "TextTheme.headline6 → titleLarge"),
    ("subtitle1", "Flutter 3.0", Some("Flutter 4.0"), "Use titleMedium", "TextTheme.subtitle1 → titleMedium"),
    ("subtitle2", "Flutter 3.0", Some("Flutter 4.0"), "Use titleSmall", "TextTheme.subtitle2 → titleSmall"),
    ("caption", "Flutter 3.0", Some("Flutter 4.0"), "Use bodySmall", "TextTheme.caption → bodySmall"),
    ("overline", "Flutter 3.0", Some("Flutter 4.0"), "Use labelSmall", "TextTheme.overline → labelSmall"),
    ("button", "Flutter 3.0", Some("Flutter 4.0"), "Use labelLarge", "TextTheme.button → labelLarge"),
    (".brightnessOf(", "Flutter 3.0", None, "Use MediaQuery.platformBrightnessOf", "ThemeData.brightnessOf is deprecated"),
    ("WillPopScope(", "Flutter 3.12", None, "Use PopScope", "WillPopScope is deprecated — use PopScope with canPop and onPopInvokedWithResult"),
    ("MaterialApp.router(", "Never", None, "Valid API — no migration needed", ""),
    ("showSnackBar(", "Never", None, "", ""),
];

/// Check a project for Flutter upgrade compatibility issues.
pub fn check_upgrade_compatibility(root: &Path) -> Vec<CompatFinding> {
    let mut findings = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
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
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("///") {
            continue;
        }

        for &(pattern, deprecated_in, removed_in, migration, message) in DEPRECATED_APIS {
            if message.is_empty() { continue; }
            if trimmed.contains(pattern) {
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
    let deprecated: Vec<&CompatFinding> = findings.iter().filter(|f| f.removed_in.is_none()).collect();

    if !removed.is_empty() {
        println!(
            "  {} Removed APIs ({} usages) — will cause compile errors:",
            "●".bright_red().bold(),
            removed.len()
        );
        for f in removed.iter().take(15) {
            let rel = f.issue.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
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
            let rel = f.issue.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            println!(
                "    {} {}:{} {}",
                "⚠".yellow(),
                rel,
                f.issue.line,
                f.issue.message
            );
            println!(
                "      {} {}",
                "→".green(),
                f.migration.bright_white()
            );
        }
        if deprecated.len() > 10 {
            println!("    ... and {} more", deprecated.len() - 10);
        }
        println!();
    }

    println!("  Total: {} issues ({} removed, {} deprecated)",
        findings.len(),
        removed.len(),
        deprecated.len()
    );
    println!();
}
