//! Localization Coverage Module — Analyzes Flutter project localization files (ARB format).
//!
//! This module provides comprehensive localization (l10n) auditing for Flutter projects,
//! detecting and reporting on:
//! - **Missing translations** — Keys present in the template ARB but missing in other locale ARBs
//! - **Extra/orphaned keys** — Keys in locale ARBs that don't exist in the template
//! - **Unused l10n keys** — Keys defined in ARB files but never referenced in Dart code
//! - **Placeholder mismatches** — Template has `{name}` placeholder but translation is missing it
//! - **Empty translations** — Keys with empty string values
//! - **Locale coverage score** — Per-locale and overall coverage percentages
//!
//! # Example
//!
//! ```ignore
//! let report = analyze_l10n_coverage(Path::new("."))?;
//! print_l10n_report(&report);
//! write_l10n_html_report(&report, Path::new("l10n_coverage.html"))?;
//! ```

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Severity level for localization issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum L10nSeverity {
    Info,
    Warning,
    Error,
}

impl L10nSeverity {
    fn score_deduction(&self) -> u32 {
        match self {
            L10nSeverity::Info => 2,
            L10nSeverity::Warning => 5,
            L10nSeverity::Error => 10,
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            L10nSeverity::Info => "ℹ",
            L10nSeverity::Warning => "⚠",
            L10nSeverity::Error => "✕",
        }
    }

    fn color_name(&self) -> &'static str {
        match self {
            L10nSeverity::Info => "cyan",
            L10nSeverity::Warning => "yellow",
            L10nSeverity::Error => "red",
        }
    }
}

/// Detailed issue with localization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L10nIssue {
    pub severity: L10nSeverity,
    pub category: String, // "Missing", "Extra", "Unused", "Placeholder", "Empty"
    pub locale: String,
    pub key: String,
    pub detail: String,
    pub suggestion: String,
}

/// Per-locale localization report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleReport {
    pub locale: String,
    pub file: PathBuf,
    pub total_keys: usize,
    pub missing_keys: Vec<String>,
    pub extra_keys: Vec<String>,
    pub empty_values: Vec<String>,
    pub placeholder_mismatches: Vec<String>,
    pub coverage_pct: f64,
}

/// Complete localization coverage report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L10nCoverageReport {
    pub template_locale: String,
    pub template_key_count: usize,
    pub locales: Vec<LocaleReport>,
    pub unused_keys: Vec<String>,
    pub overall_coverage_pct: f64,
    pub score: u32, // 0-100
    pub issues: Vec<L10nIssue>,
}

/// Represents a parsed ARB file
#[derive(Debug, Clone)]
struct ArbFile {
    locale: String,
    path: PathBuf,
    keys: HashMap<String, ArbValue>,
}

/// Represents a value in an ARB file
#[derive(Debug, Clone)]
struct ArbValue {
    text: String,
    placeholders: Vec<String>,
}

/// Find all ARB files in a Flutter project
fn find_arb_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut arb_files = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Some(ext) = entry.path().extension() {
            if ext == "arb" {
                arb_files.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(arb_files)
}

/// Parse an ARB file and extract locale, keys, and placeholders
fn parse_arb_file(path: &Path) -> Result<ArbFile> {
    let content = fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&content)?;

    // Extract locale from filename (e.g., "app_en.arb" -> "en")
    let filename = path.file_stem().unwrap_or_default().to_string_lossy();
    let locale = if filename.contains('_') {
        filename.split('_').last().unwrap_or("unknown").to_string()
    } else {
        "base".to_string()
    };

    let mut keys = HashMap::new();

    if let Some(obj) = json.as_object() {
        for (key, value) in obj.iter() {
            // Skip metadata keys (starting with @)
            if key.starts_with('@') {
                continue;
            }

            let text = match value.as_str() {
                Some(s) => s.to_string(),
                None => continue,
            };

            let placeholders = extract_placeholders(&text);
            keys.insert(key.clone(), ArbValue { text, placeholders });
        }
    }

    Ok(ArbFile {
        locale,
        path: path.to_path_buf(),
        keys,
    })
}

/// Extract placeholders from a string using regex-like pattern {name}
fn extract_placeholders(text: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut chars = text.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '{' {
            chars.next(); // consume first '{'
                          // Handle double-brace syntax: {{name}} is treated as placeholder "name"
            let double_brace = chars.peek() == Some(&'{');
            if double_brace {
                chars.next(); // consume second '{'
            }
            let mut placeholder = String::new();
            while let Some(&c) = chars.peek() {
                if c == '}' {
                    chars.next(); // consume first '}'
                    if double_brace && chars.peek() == Some(&'}') {
                        chars.next(); // consume second '}'
                    }
                    if !placeholder.is_empty() {
                        placeholders.push(placeholder);
                    }
                    break;
                }
                placeholder.push(c);
                chars.next();
            }
        } else {
            chars.next();
        }
    }

    placeholders
}

/// Find all Dart files in a project
fn find_dart_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut dart_files = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Some(ext) = entry.path().extension() {
            if ext == "dart" {
                dart_files.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(dart_files)
}

/// Search Dart source files for l10n key usage
fn find_used_keys(dart_files: &[PathBuf]) -> Result<HashSet<String>> {
    let mut used_keys = HashSet::new();

    // Common patterns for accessing l10n keys:
    // - AppLocalizations.of(context)?.keyName
    // - context.l10n.keyName
    // - S.of(context).keyName
    let patterns = [
        r"\.of\(.*?\)\??\s*\.\s*([a-zA-Z_][a-zA-Z0-9_]*)",
        r"\.l10n\s*\.\s*([a-zA-Z_][a-zA-Z0-9_]*)",
        r"S\.of\(.*?\)\s*\.\s*([a-zA-Z_][a-zA-Z0-9_]*)",
    ];

    for dart_file in dart_files {
        let content = match fs::read_to_string(dart_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Simple pattern matching without regex for basic extraction
        for _pattern in &patterns {
            find_keys_in_content(&content, &mut used_keys);
        }
    }

    Ok(used_keys)
}

/// Extract keys from Dart content using simple pattern matching
fn find_keys_in_content(content: &str, used_keys: &mut HashSet<String>) {
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();

    for i in 0..len {
        // Look for patterns like ".keyName" or ".l10n.keyName"
        if (i == 0 || chars[i - 1] == '.' || chars[i - 1] == '?' || chars[i - 1] == ')')
            && chars[i] == '.'
        {
            if i + 1 < len && (chars[i + 1].is_alphabetic() || chars[i + 1] == '_') {
                let mut key = String::new();
                let mut j = i + 1;
                while j < len && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    key.push(chars[j]);
                    j += 1;
                }
                if !key.is_empty() && key != "l10n" && key != "of" {
                    used_keys.insert(key);
                }
            }
        }
    }
}

/// Determine the template (base) ARB file
fn identify_template_file(arb_files: &[ArbFile]) -> Option<&ArbFile> {
    // Look for app_en.arb or similar patterns
    for arb in arb_files {
        if arb.locale == "en" || arb.locale == "base" {
            return Some(arb);
        }
    }

    // Fallback to first file
    arb_files.first()
}

/// Analyze localization coverage for a Flutter project
pub fn analyze_l10n_coverage(path: &Path) -> Result<L10nCoverageReport> {
    let arb_paths = find_arb_files(path)?;

    if arb_paths.is_empty() {
        anyhow::bail!("No ARB files found in the project");
    }

    // Parse all ARB files
    let mut arb_files = Vec::new();
    for arb_path in arb_paths {
        match parse_arb_file(&arb_path) {
            Ok(arb) => arb_files.push(arb),
            Err(e) => {
                log::warn!("Failed to parse {}: {}", arb_path.display(), e);
            }
        }
    }

    if arb_files.is_empty() {
        anyhow::bail!("Failed to parse any ARB files");
    }

    // Identify template file
    let template = identify_template_file(&arb_files)
        .ok_or_else(|| anyhow::anyhow!("Could not identify template ARB file"))?;

    let template_locale = template.locale.clone();
    let template_key_count = template.keys.len();

    // Find used keys
    let dart_files = find_dart_files(path).unwrap_or_default();
    let used_keys = find_used_keys(&dart_files).unwrap_or_default();

    // Identify unused keys
    let unused_keys: Vec<String> = template
        .keys
        .keys()
        .filter(|k| !used_keys.contains(*k))
        .cloned()
        .collect();

    // Generate per-locale reports
    let mut locale_reports = Vec::new();
    let mut all_issues = Vec::new();

    for arb in &arb_files {
        if arb.locale == template_locale {
            continue; // Skip template itself
        }

        let missing_keys: Vec<String> = template
            .keys
            .keys()
            .filter(|k| !arb.keys.contains_key(*k))
            .cloned()
            .collect();

        let extra_keys: Vec<String> = arb
            .keys
            .keys()
            .filter(|k| !template.keys.contains_key(*k))
            .cloned()
            .collect();

        let empty_values: Vec<String> = arb
            .keys
            .iter()
            .filter(|(_, v)| v.text.trim().is_empty())
            .map(|(k, _)| k.clone())
            .collect();

        let mut placeholder_mismatches = Vec::new();
        for (key, template_value) in &template.keys {
            if let Some(locale_value) = arb.keys.get(key) {
                let template_placeholders = &template_value.placeholders;
                let locale_placeholders = &locale_value.placeholders;

                // Check if placeholders don't match
                if template_placeholders.len() != locale_placeholders.len()
                    || !template_placeholders
                        .iter()
                        .all(|p| locale_placeholders.contains(p))
                {
                    placeholder_mismatches.push(key.clone());
                }
            }
        }

        // Calculate coverage
        let translated_keys = arb.keys.len() - extra_keys.len();
        let coverage_pct = if template_key_count > 0 {
            (translated_keys as f64 / template_key_count as f64) * 100.0
        } else {
            0.0
        };

        // Generate issues for this locale
        for key in &missing_keys {
            all_issues.push(L10nIssue {
                severity: L10nSeverity::Error,
                category: "Missing".to_string(),
                locale: arb.locale.clone(),
                key: key.clone(),
                detail: format!(
                    "Key '{}' is defined in template but missing in {}",
                    key, arb.locale
                ),
                suggestion: format!("Add translation for '{}' in {}", key, arb.locale),
            });
        }

        for key in &extra_keys {
            all_issues.push(L10nIssue {
                severity: L10nSeverity::Warning,
                category: "Extra".to_string(),
                locale: arb.locale.clone(),
                key: key.clone(),
                detail: format!("Key '{}' exists in {} but not in template", key, arb.locale),
                suggestion: format!("Remove '{}' from {} or add to template", key, arb.locale),
            });
        }

        for key in &empty_values {
            all_issues.push(L10nIssue {
                severity: L10nSeverity::Warning,
                category: "Empty".to_string(),
                locale: arb.locale.clone(),
                key: key.clone(),
                detail: format!("Key '{}' has an empty translation in {}", key, arb.locale),
                suggestion: format!("Provide a translation for '{}' in {}", key, arb.locale),
            });
        }

        for key in &placeholder_mismatches {
            all_issues.push(L10nIssue {
                severity: L10nSeverity::Error,
                category: "Placeholder".to_string(),
                locale: arb.locale.clone(),
                key: key.clone(),
                detail: format!(
                    "Placeholders mismatch for '{}' between template and {}",
                    key, arb.locale
                ),
                suggestion: format!(
                    "Ensure placeholders in '{}' match the template version",
                    key
                ),
            });
        }

        locale_reports.push(LocaleReport {
            locale: arb.locale.clone(),
            file: arb.path.clone(),
            total_keys: arb.keys.len(),
            missing_keys,
            extra_keys,
            empty_values,
            placeholder_mismatches,
            coverage_pct,
        });
    }

    // Add unused keys issues
    for key in &unused_keys {
        all_issues.push(L10nIssue {
            severity: L10nSeverity::Info,
            category: "Unused".to_string(),
            locale: template_locale.clone(),
            key: key.clone(),
            detail: format!("Key '{}' is defined but never used in code", key),
            suggestion: format!(
                "Consider removing unused key '{}' from localization files",
                key
            ),
        });
    }

    // Calculate overall coverage
    let total_translated = locale_reports.iter().map(|l| l.total_keys).sum::<usize>();
    let total_expected = locale_reports.len() * template_key_count;
    let overall_coverage_pct = if total_expected > 0 {
        (total_translated as f64 / total_expected as f64) * 100.0
    } else {
        0.0
    };

    // Calculate score (0-100)
    let mut score = 100u32;
    for issue in &all_issues {
        score = score.saturating_sub(issue.severity.score_deduction());
    }

    Ok(L10nCoverageReport {
        template_locale,
        template_key_count,
        locales: locale_reports,
        unused_keys,
        overall_coverage_pct,
        score,
        issues: all_issues,
    })
}

/// Print a beautiful localization coverage report to stdout
pub fn print_l10n_report(report: &L10nCoverageReport) {
    println!();
    println!("{}", "📋 LOCALIZATION COVERAGE REPORT".bright_cyan().bold());
    println!("{}", "=".repeat(60).cyan());

    // Header information
    println!(
        "{}",
        format!(
            "Template Locale: {} | Keys: {}",
            report.template_locale.cyan(),
            report.template_key_count.to_string().yellow()
        )
    );
    println!(
        "{}",
        format!(
            "Overall Coverage: {:.1}% | Score: {}/100",
            report.overall_coverage_pct, report.score
        )
        .bright_white()
    );
    println!();

    // Locale coverage matrix
    println!("{}", "LOCALE COVERAGE MATRIX".bright_cyan().bold());
    println!("{}", "-".repeat(60).cyan());

    // Header row
    print!("{:<12}", "Locale");
    print!("{:>10}", "Keys");
    print!("{:>10}", "Missing");
    print!("{:>10}", "Extra");
    print!("{:>10}", "Coverage");
    println!();
    println!("{}", "-".repeat(60).cyan());

    for locale in &report.locales {
        let coverage_str = format!("{:.1}%", locale.coverage_pct);
        let coverage_color = if locale.coverage_pct >= 95.0 {
            coverage_str.bright_green()
        } else if locale.coverage_pct >= 80.0 {
            coverage_str.bright_yellow()
        } else {
            coverage_str.bright_red()
        };

        print!("{:<12}", locale.locale.cyan());
        print!("{:>10}", locale.total_keys.to_string().white());
        print!("{:>10}", locale.missing_keys.len().to_string().bright_red());
        print!(
            "{:>10}",
            locale.extra_keys.len().to_string().bright_yellow()
        );
        print!("{:>10}", coverage_color);
        println!();
    }

    println!();

    // Issues summary
    if !report.issues.is_empty() {
        println!("{}", "ISSUES SUMMARY".bright_cyan().bold());
        println!("{}", "-".repeat(60).cyan());

        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;

        for issue in &report.issues {
            match issue.severity {
                L10nSeverity::Error => errors += 1,
                L10nSeverity::Warning => warnings += 1,
                L10nSeverity::Info => infos += 1,
            }
        }

        if errors > 0 {
            println!(
                "  {} {}",
                L10nSeverity::Error.symbol().bright_red(),
                format!("{} errors", errors).bright_red()
            );
        }
        if warnings > 0 {
            println!(
                "  {} {}",
                L10nSeverity::Warning.symbol().bright_yellow(),
                format!("{} warnings", warnings).bright_yellow()
            );
        }
        if infos > 0 {
            println!(
                "  {} {}",
                L10nSeverity::Info.symbol().bright_cyan(),
                format!("{} info", infos).bright_cyan()
            );
        }
        println!();
    }

    // Top issues
    if !report.issues.is_empty() {
        println!("{}", "TOP ISSUES".bright_cyan().bold());
        println!("{}", "-".repeat(60).cyan());

        // Group by category
        let mut by_category: HashMap<&str, Vec<_>> = HashMap::new();
        for issue in &report.issues {
            by_category
                .entry(issue.category.as_str())
                .or_insert_with(Vec::new)
                .push(issue);
        }

        for (category, issues) in by_category {
            let count = issues.len();
            println!(
                "{} ({} {})",
                category.bright_magenta(),
                count,
                if count == 1 { "issue" } else { "issues" }
            );

            for issue in issues.iter().take(3) {
                println!(
                    "  {} [{}] {}: {}",
                    issue.severity.symbol(),
                    issue.locale,
                    issue.key.yellow(),
                    issue.detail
                );
                println!("    {} {}", "→".cyan(), issue.suggestion);
            }

            if count > 3 {
                println!("  {} ... and {} more", "...".dimmed(), count - 3);
            }
            println!();
        }
    }

    // Unused keys
    if !report.unused_keys.is_empty() {
        println!("{}", "UNUSED KEYS".bright_cyan().bold());
        println!("{}", "-".repeat(60).cyan());
        for key in report.unused_keys.iter().take(5) {
            println!("  {} {}", "◦".cyan(), key.yellow());
        }
        if report.unused_keys.len() > 5 {
            println!(
                "  {} ... and {} more",
                "...".dimmed(),
                report.unused_keys.len() - 5
            );
        }
        println!();
    }

    println!("{}", "=".repeat(60).cyan());
}

/// Write an HTML report for localization coverage
pub fn write_l10n_html_report(report: &L10nCoverageReport, path: &Path) -> Result<()> {
    let html_content = generate_html_report(report);
    fs::write(path, html_content)?;
    Ok(())
}

/// Generate HTML content for localization report
fn generate_html_report(report: &L10nCoverageReport) -> String {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Localization Coverage Report</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            padding: 40px 20px;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 12px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            overflow: hidden;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 40px;
        }
        .header h1 {
            font-size: 32px;
            margin-bottom: 10px;
        }
        .header p {
            opacity: 0.9;
            font-size: 14px;
        }
        .stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            padding: 40px;
            background: #f8f9fa;
            border-bottom: 1px solid #e9ecef;
        }
        .stat {
            text-align: center;
        }
        .stat-value {
            font-size: 32px;
            font-weight: bold;
            color: #667eea;
            margin-bottom: 5px;
        }
        .stat-label {
            font-size: 14px;
            color: #6c757d;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }
        .content {
            padding: 40px;
        }
        .section {
            margin-bottom: 40px;
        }
        .section h2 {
            font-size: 20px;
            color: #333;
            margin-bottom: 20px;
            padding-bottom: 10px;
            border-bottom: 2px solid #667eea;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin-bottom: 20px;
        }
        thead {
            background: #f8f9fa;
            font-weight: 600;
            color: #333;
        }
        th {
            padding: 12px;
            text-align: left;
            font-size: 13px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            color: #6c757d;
        }
        td {
            padding: 12px;
            border-bottom: 1px solid #e9ecef;
        }
        tr:hover {
            background: #f8f9fa;
        }
        .coverage-bar {
            width: 100%;
            height: 20px;
            background: #e9ecef;
            border-radius: 10px;
            overflow: hidden;
            margin: 5px 0;
        }
        .coverage-fill {
            height: 100%;
            background: linear-gradient(90deg, #28a745 0%, #ffc107 50%, #dc3545 100%);
            transition: width 0.3s ease;
        }
        .badge {
            display: inline-block;
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 12px;
            font-weight: 600;
            text-transform: uppercase;
        }
        .badge-error { background: #f8d7da; color: #721c24; }
        .badge-warning { background: #fff3cd; color: #856404; }
        .badge-info { background: #d1ecf1; color: #0c5460; }
        .issue-list {
            background: #f8f9fa;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 20px;
        }
        .issue-item {
            padding: 12px;
            border-left: 4px solid #667eea;
            margin-bottom: 12px;
            background: white;
            border-radius: 4px;
        }
        .issue-item:last-child {
            margin-bottom: 0;
        }
        .issue-key {
            font-weight: 600;
            color: #333;
            margin-bottom: 5px;
        }
        .issue-detail {
            font-size: 13px;
            color: #6c757d;
            margin-bottom: 5px;
        }
        .issue-suggestion {
            font-size: 13px;
            color: #667eea;
            font-style: italic;
        }
        .footer {
            background: #f8f9fa;
            padding: 20px 40px;
            text-align: center;
            color: #6c757d;
            font-size: 12px;
        }
        .progress-excellent { color: #28a745; }
        .progress-good { color: #ffc107; }
        .progress-poor { color: #dc3545; }
    </style>
</head>
<body>
    <div class="container">
"#,
    );

    // Header
    html.push_str(&format!(
        r#"        <div class="header">
            <h1>📋 Localization Coverage Report</h1>
            <p>Flutter/Dart Project Localization Analysis</p>
        </div>
"#
    ));

    // Stats
    html.push_str(&format!(
        r#"        <div class="stats">
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Template Keys</div>
            </div>
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Locales</div>
            </div>
            <div class="stat">
                <div class="stat-value" class="progress-{}">{:.1}%</div>
                <div class="stat-label">Overall Coverage</div>
            </div>
            <div class="stat">
                <div class="stat-value">{}/100</div>
                <div class="stat-label">Quality Score</div>
            </div>
        </div>
"#,
        report.template_key_count,
        report.locales.len(),
        if report.overall_coverage_pct >= 95.0 {
            "excellent"
        } else if report.overall_coverage_pct >= 80.0 {
            "good"
        } else {
            "poor"
        },
        report.overall_coverage_pct,
        report.score
    ));

    // Content
    html.push_str(
        r#"        <div class="content">
"#,
    );

    // Locale Coverage Table
    html.push_str(
        r#"            <div class="section">
                <h2>Locale Coverage Matrix</h2>
                <table>
                    <thead>
                        <tr>
                            <th>Locale</th>
                            <th>Keys</th>
                            <th>Missing</th>
                            <th>Extra</th>
                            <th>Empty</th>
                            <th>Coverage</th>
                        </tr>
                    </thead>
                    <tbody>
"#,
    );

    for locale in &report.locales {
        let coverage_class = if locale.coverage_pct >= 95.0 {
            "progress-excellent"
        } else if locale.coverage_pct >= 80.0 {
            "progress-good"
        } else {
            "progress-poor"
        };

        html.push_str(&format!(
            r#"                        <tr>
                            <td><strong>{}</strong><br><small>{}</small></td>
                            <td>{}</td>
                            <td><span class="badge badge-error">{}</span></td>
                            <td><span class="badge badge-warning">{}</span></td>
                            <td><span class="badge badge-warning">{}</span></td>
                            <td>
                                <div class="coverage-bar">
                                    <div class="coverage-fill" style="width: {:.1}%"></div>
                                </div>
                                <span class="{}">{:.1}%</span>
                            </td>
                        </tr>
"#,
            locale.locale,
            locale.file.display(),
            locale.total_keys,
            locale.missing_keys.len(),
            locale.extra_keys.len(),
            locale.empty_values.len(),
            locale.coverage_pct,
            coverage_class,
            locale.coverage_pct
        ));
    }

    html.push_str(
        r#"                    </tbody>
                </table>
            </div>
"#,
    );

    // Issues
    if !report.issues.is_empty() {
        html.push_str(
            r#"            <div class="section">
                <h2>Issues</h2>
"#,
        );

        let mut by_category: HashMap<&str, Vec<_>> = HashMap::new();
        for issue in &report.issues {
            by_category
                .entry(issue.category.as_str())
                .or_insert_with(Vec::new)
                .push(issue);
        }

        for (category, issues) in by_category {
            let badge_class = if category == "Missing" || category == "Placeholder" {
                "badge-error"
            } else if category == "Extra" || category == "Empty" {
                "badge-warning"
            } else {
                "badge-info"
            };

            html.push_str(&format!(
                r#"                <div style="margin-bottom: 30px;">
                    <h3>{} <span class="badge {}">{} {}</span></h3>
                    <div class="issue-list">
"#,
                category,
                badge_class,
                issues.len(),
                if issues.len() == 1 { "issue" } else { "issues" }
            ));

            for issue in issues.iter().take(10) {
                html.push_str(&format!(
                    r#"                        <div class="issue-item">
                            <div class="issue-key">{} [{}]</div>
                            <div class="issue-detail">{}</div>
                            <div class="issue-suggestion">💡 {}</div>
                        </div>
"#,
                    issue.key, issue.locale, issue.detail, issue.suggestion
                ));
            }

            if issues.len() > 10 {
                html.push_str(&format!(
                    r#"                        <div style="text-align: center; padding: 10px; color: #999;">
                            ... and {} more issues
                        </div>
"#,
                    issues.len() - 10
                ));
            }

            html.push_str(
                r#"                    </div>
                </div>
"#,
            );
        }

        html.push_str(
            r#"            </div>
"#,
        );
    }

    // Unused keys
    if !report.unused_keys.is_empty() {
        html.push_str(&format!(
            r#"            <div class="section">
                <h2>Unused Keys ({} total)</h2>
                <div class="issue-list">
"#,
            report.unused_keys.len()
        ));

        for key in report.unused_keys.iter().take(20) {
            html.push_str(&format!(
                r#"                    <div class="issue-item" style="border-left-color: #999;">
                        <div class="issue-key">{}</div>
                        <div class="issue-detail">Defined but never referenced in code</div>
                    </div>
"#,
                key
            ));
        }

        if report.unused_keys.len() > 20 {
            html.push_str(&format!(
                r#"                    <div style="text-align: center; padding: 10px; color: #999;">
                        ... and {} more unused keys
                    </div>
"#,
                report.unused_keys.len() - 20
            ));
        }

        html.push_str(
            r#"                </div>
            </div>
"#,
        );
    }

    html.push_str(
        r#"        </div>
        <div class="footer">
            <p>Generated by Falcon - Flutter/Dart Static Analysis CLI</p>
        </div>
    </div>
</body>
</html>
"#,
    );

    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_placeholders() {
        let text = "Hello {name}, you have {count} messages";
        let placeholders = extract_placeholders(text);
        assert_eq!(placeholders.len(), 2);
        assert!(placeholders.contains(&"name".to_string()));
        assert!(placeholders.contains(&"count".to_string()));
    }

    #[test]
    fn test_extract_placeholders_nested() {
        let text = "Value: {{nested}}";
        let placeholders = extract_placeholders(text);
        assert_eq!(placeholders.len(), 1);
        assert!(placeholders.contains(&"nested".to_string()));
    }

    #[test]
    fn test_extract_placeholders_none() {
        let text = "Plain text without placeholders";
        let placeholders = extract_placeholders(text);
        assert_eq!(placeholders.len(), 0);
    }

    #[test]
    fn test_parse_arb_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let arb_path = temp_dir.path().join("app_en.arb");

        let arb_content = r#"{
            "greeting": "Hello {name}",
            "farewell": "Goodbye",
            "@greeting": {
                "description": "Greeting message"
            }
        }"#;

        fs::write(&arb_path, arb_content)?;
        let arb = parse_arb_file(&arb_path)?;

        assert_eq!(arb.locale, "en");
        assert_eq!(arb.keys.len(), 2);
        assert!(arb.keys.contains_key("greeting"));
        assert!(arb.keys.contains_key("farewell"));

        let greeting = &arb.keys["greeting"];
        assert_eq!(greeting.placeholders.len(), 1);
        assert!(greeting.placeholders.contains(&"name".to_string()));

        Ok(())
    }

    #[test]
    fn test_find_keys_in_content() {
        let content = r#"
            var localizations = AppLocalizations.of(context)?.greeting;
            String farewell = context.l10n.farewell;
            String title = S.of(context).title;
        "#;

        let mut used_keys = HashSet::new();
        find_keys_in_content(content, &mut used_keys);

        assert!(used_keys.len() > 0);
    }

    #[test]
    fn test_l10n_severity_score() {
        assert_eq!(L10nSeverity::Info.score_deduction(), 2);
        assert_eq!(L10nSeverity::Warning.score_deduction(), 5);
        assert_eq!(L10nSeverity::Error.score_deduction(), 10);
    }

    #[test]
    fn test_score_calculation() {
        let mut report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 10,
            locales: Vec::new(),
            unused_keys: Vec::new(),
            overall_coverage_pct: 100.0,
            score: 100,
            issues: Vec::new(),
        };

        // Add some errors
        for _ in 0..5 {
            report.issues.push(L10nIssue {
                severity: L10nSeverity::Error,
                category: "Missing".to_string(),
                locale: "fr".to_string(),
                key: "test".to_string(),
                detail: "test".to_string(),
                suggestion: "test".to_string(),
            });
        }

        let mut score = 100u32;
        for issue in &report.issues {
            score = score.saturating_sub(issue.severity.score_deduction());
        }

        assert_eq!(score, 50); // 100 - (5 * 10)
    }

    #[test]
    fn test_html_report_generation() {
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 10,
            locales: vec![LocaleReport {
                locale: "fr".to_string(),
                file: PathBuf::from("app_fr.arb"),
                total_keys: 8,
                missing_keys: vec!["key1".to_string()],
                extra_keys: vec![],
                empty_values: vec![],
                placeholder_mismatches: vec![],
                coverage_pct: 80.0,
            }],
            unused_keys: vec!["unused1".to_string()],
            overall_coverage_pct: 80.0,
            score: 85,
            issues: vec![],
        };

        let html = generate_html_report(&report);
        assert!(html.contains("Localization Coverage Report"));
        assert!(html.contains("80.0%"));
        assert!(html.contains("85/100"));
        assert!(html.contains("app_fr.arb"));
    }
}
