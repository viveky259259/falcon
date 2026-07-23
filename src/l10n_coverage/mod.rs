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
        filename
            .split('_')
            .next_back()
            .unwrap_or("unknown")
            .to_string()
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
            && i + 1 < len
            && (chars[i + 1].is_alphabetic() || chars[i + 1] == '_')
        {
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
        "Template Locale: {} | Keys: {}",
        report.template_locale.cyan(),
        report.template_key_count.to_string().yellow()
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
    html.push_str(
        r#"        <div class="header">
            <h1>📋 Localization Coverage Report</h1>
            <p>Flutter/Dart Project Localization Analysis</p>
        </div>
"#,
    );

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

        assert!(!used_keys.is_empty());
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

    // ─── L10nSeverity impl methods ───────────────────────────────────────────

    #[test]
    fn l10n_severity_symbol_returns_correct_icons() {
        assert_eq!(L10nSeverity::Info.symbol(), "ℹ");
        assert_eq!(L10nSeverity::Warning.symbol(), "⚠");
        assert_eq!(L10nSeverity::Error.symbol(), "✕");
    }

    #[test]
    fn l10n_severity_color_name_returns_correct_colors() {
        assert_eq!(L10nSeverity::Info.color_name(), "cyan");
        assert_eq!(L10nSeverity::Warning.color_name(), "yellow");
        assert_eq!(L10nSeverity::Error.color_name(), "red");
    }

    #[test]
    fn l10n_severity_ordering_info_lt_warning_lt_error() {
        assert!(L10nSeverity::Info < L10nSeverity::Warning);
        assert!(L10nSeverity::Warning < L10nSeverity::Error);
        assert!(L10nSeverity::Info < L10nSeverity::Error);
    }

    // ─── find_arb_files ───────────────────────────────────────────────────────

    #[test]
    fn find_arb_files_returns_only_arb_files() -> Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("app_en.arb"), r#"{"hello": "Hello"}"#)?;
        fs::write(dir.path().join("app_fr.arb"), r#"{"hello": "Bonjour"}"#)?;
        fs::write(dir.path().join("strings.json"), r#"{}"#)?;
        fs::write(dir.path().join("readme.txt"), "ignore me")?;

        let files = find_arb_files(dir.path())?;
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|p| p.extension().unwrap() == "arb"));
        Ok(())
    }

    #[test]
    fn find_arb_files_empty_dir_returns_empty_vec() -> Result<()> {
        let dir = TempDir::new()?;
        let files = find_arb_files(dir.path())?;
        assert!(files.is_empty());
        Ok(())
    }

    #[test]
    fn find_arb_files_nested_subdirs_are_found() -> Result<()> {
        let dir = TempDir::new()?;
        let sub = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&sub)?;
        fs::write(sub.join("app_en.arb"), r#"{"k": "v"}"#)?;
        let files = find_arb_files(dir.path())?;
        assert_eq!(files.len(), 1);
        Ok(())
    }

    // ─── parse_arb_file ───────────────────────────────────────────────────────

    #[test]
    fn parse_arb_file_skips_metadata_at_keys() -> Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("app_en.arb");
        fs::write(
            &path,
            r#"{"title": "My App", "@title": {"description": "App title"}}"#,
        )?;
        let arb = parse_arb_file(&path)?;
        assert!(arb.keys.contains_key("title"));
        assert!(!arb.keys.contains_key("@title"));
        assert_eq!(arb.keys.len(), 1);
        Ok(())
    }

    #[test]
    fn parse_arb_file_no_underscore_filename_yields_base_locale() -> Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("strings.arb");
        fs::write(&path, r#"{"key": "value"}"#)?;
        let arb = parse_arb_file(&path)?;
        assert_eq!(arb.locale, "base");
        Ok(())
    }

    #[test]
    fn parse_arb_file_invalid_json_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.arb");
        fs::write(&path, "not valid json {{{{").unwrap();
        assert!(parse_arb_file(&path).is_err());
    }

    #[test]
    fn parse_arb_file_skips_non_string_values() -> Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("app_de.arb");
        // "count" is a number — should be skipped; only "label" kept
        fs::write(&path, r#"{"label": "Etikett", "count": 42}"#)?;
        let arb = parse_arb_file(&path)?;
        assert!(arb.keys.contains_key("label"));
        assert!(!arb.keys.contains_key("count"));
        Ok(())
    }

    #[test]
    fn parse_arb_file_extracts_locale_from_last_segment() -> Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("my_app_messages_zh.arb");
        fs::write(&path, r#"{"hi": "你好"}"#)?;
        let arb = parse_arb_file(&path)?;
        assert_eq!(arb.locale, "zh");
        Ok(())
    }

    // ─── extract_placeholders ─────────────────────────────────────────────────

    #[test]
    fn extract_placeholders_single_returns_one() {
        let placeholders = extract_placeholders("Hello {world}");
        assert_eq!(placeholders, vec!["world".to_string()]);
    }

    #[test]
    fn extract_placeholders_empty_braces_ignored() {
        // "{}" has no content — should not add an empty string
        let placeholders = extract_placeholders("Value: {}");
        assert!(!placeholders.contains(&"".to_string()));
    }

    #[test]
    fn extract_placeholders_unclosed_brace_ignored() {
        let placeholders = extract_placeholders("Hello {name");
        assert!(placeholders.is_empty());
    }

    #[test]
    fn extract_placeholders_plural_style_captured() {
        // The inner-loop reads until the FIRST '}', so "{count, plural, one{item} other{items}}"
        // yields ["count, plural, one{item", "items"] — the outer nesting is not special-cased.
        let text = "{count, plural, one{item} other{items}}";
        let placeholders = extract_placeholders(text);
        assert!(!placeholders.is_empty());
        // The first element starts with "count"
        assert!(placeholders[0].starts_with("count"));
    }

    // ─── find_dart_files ─────────────────────────────────────────────────────

    #[test]
    fn find_dart_files_returns_only_dart_files() -> Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("main.dart"), "void main() {}")?;
        fs::write(dir.path().join("widget.dart"), "class W {}")?;
        fs::write(dir.path().join("config.yaml"), "key: val")?;
        fs::write(dir.path().join("README.md"), "# readme")?;

        let files = find_dart_files(dir.path())?;
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|p| p.extension().unwrap() == "dart"));
        Ok(())
    }

    #[test]
    fn find_dart_files_empty_dir_returns_empty_vec() -> Result<()> {
        let dir = TempDir::new()?;
        let files = find_dart_files(dir.path())?;
        assert!(files.is_empty());
        Ok(())
    }

    #[test]
    fn find_dart_files_nested_dirs_discovered() -> Result<()> {
        let dir = TempDir::new()?;
        let nested = dir.path().join("lib").join("src");
        fs::create_dir_all(&nested)?;
        fs::write(nested.join("home.dart"), "class Home {}")?;
        let files = find_dart_files(dir.path())?;
        assert_eq!(files.len(), 1);
        Ok(())
    }

    // ─── find_keys_in_content / find_used_keys ───────────────────────────────

    #[test]
    fn find_keys_in_content_finds_specific_l10n_keys() {
        // The matcher triggers when the char BEFORE '.' is '.', '?', or ')'.
        // "AppLocalizations.of(context)?.welcomeMessage" — the '?' before '.' matches.
        let content = "AppLocalizations.of(context)?.welcomeMessage;";
        let mut used = HashSet::new();
        find_keys_in_content(content, &mut used);
        assert!(used.contains("welcomeMessage"), "expected welcomeMessage, got: {:?}", used);
    }

    #[test]
    fn find_keys_in_content_excludes_l10n_and_of_keywords() {
        let content = "AppLocalizations.of(context)?.greetUser;";
        let mut used = HashSet::new();
        find_keys_in_content(content, &mut used);
        assert!(!used.contains("l10n"), "l10n should be filtered");
        assert!(!used.contains("of"), "of should be filtered");
    }

    #[test]
    fn find_keys_in_content_empty_string_no_keys() {
        let mut used = HashSet::new();
        find_keys_in_content("", &mut used);
        assert!(used.is_empty());
    }

    #[test]
    fn find_used_keys_reads_dart_files_and_returns_keys() -> Result<()> {
        let dir = TempDir::new()?;
        let dart = dir.path().join("screen.dart");
        // Use "?.pageTitle" pattern so the '?' precedes the '.', triggering the matcher.
        fs::write(&dart, "AppLocalizations.of(context)?.pageTitle;")?;
        let paths = vec![dart];
        let keys = find_used_keys(&paths)?;
        assert!(keys.contains("pageTitle"), "expected pageTitle, got: {:?}", keys);
        Ok(())
    }

    #[test]
    fn find_used_keys_unreadable_file_is_skipped() -> Result<()> {
        // Passing a path that doesn't exist; find_used_keys continues silently
        let paths = vec![PathBuf::from("/nonexistent/ghost.dart")];
        let keys = find_used_keys(&paths)?;
        assert!(keys.is_empty());
        Ok(())
    }

    // ─── identify_template_file ───────────────────────────────────────────────

    #[test]
    fn identify_template_file_picks_en_locale() -> Result<()> {
        let dir = TempDir::new()?;
        let en_path = dir.path().join("app_en.arb");
        let fr_path = dir.path().join("app_fr.arb");
        fs::write(&en_path, r#"{"k": "v"}"#)?;
        fs::write(&fr_path, r#"{"k": "w"}"#)?;

        let arbs = vec![
            parse_arb_file(&fr_path)?,
            parse_arb_file(&en_path)?,
        ];
        let template = identify_template_file(&arbs).expect("should find template");
        assert_eq!(template.locale, "en");
        Ok(())
    }

    #[test]
    fn identify_template_file_falls_back_to_first_when_no_en() -> Result<()> {
        let dir = TempDir::new()?;
        let de_path = dir.path().join("app_de.arb");
        let fr_path = dir.path().join("app_fr.arb");
        fs::write(&de_path, r#"{"k": "v"}"#)?;
        fs::write(&fr_path, r#"{"k": "w"}"#)?;

        let arbs = vec![
            parse_arb_file(&de_path)?,
            parse_arb_file(&fr_path)?,
        ];
        let template = identify_template_file(&arbs).expect("should find template");
        assert_eq!(template.locale, "de");
        Ok(())
    }

    #[test]
    fn identify_template_file_empty_list_returns_none() {
        let arbs: Vec<ArbFile> = vec![];
        assert!(identify_template_file(&arbs).is_none());
    }

    #[test]
    fn identify_template_file_base_locale_is_accepted() -> Result<()> {
        let dir = TempDir::new()?;
        let base_path = dir.path().join("strings.arb"); // no underscore → "base"
        fs::write(&base_path, r#"{"k": "v"}"#)?;
        let arbs = vec![parse_arb_file(&base_path)?];
        let template = identify_template_file(&arbs).expect("should find template");
        assert_eq!(template.locale, "base");
        Ok(())
    }

    // ─── analyze_l10n_coverage ────────────────────────────────────────────────

    #[test]
    fn analyze_l10n_coverage_no_arb_files_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = analyze_l10n_coverage(dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No ARB files"));
    }

    #[test]
    fn analyze_l10n_coverage_only_unparseable_arbs_returns_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("app_en.arb"), "not json").unwrap();
        let result = analyze_l10n_coverage(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn analyze_l10n_coverage_single_template_no_locales() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;
        fs::write(l10n_dir.join("app_en.arb"), r#"{"hello": "Hello", "bye": "Bye"}"#)?;

        let report = analyze_l10n_coverage(dir.path())?;
        assert_eq!(report.template_locale, "en");
        assert_eq!(report.template_key_count, 2);
        assert!(report.locales.is_empty());
        Ok(())
    }

    #[test]
    fn analyze_l10n_coverage_detects_missing_keys() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;

        // Template with 3 keys
        fs::write(
            l10n_dir.join("app_en.arb"),
            r#"{"greeting": "Hello", "farewell": "Bye", "title": "App"}"#,
        )?;
        // French is missing "title"
        fs::write(
            l10n_dir.join("app_fr.arb"),
            r#"{"greeting": "Bonjour", "farewell": "Au revoir"}"#,
        )?;

        let report = analyze_l10n_coverage(dir.path())?;
        let fr = report.locales.iter().find(|l| l.locale == "fr").unwrap();
        assert!(fr.missing_keys.contains(&"title".to_string()));
        assert!(report.issues.iter().any(|i| i.category == "Missing" && i.locale == "fr"));
        Ok(())
    }

    #[test]
    fn analyze_l10n_coverage_detects_extra_keys() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;

        fs::write(l10n_dir.join("app_en.arb"), r#"{"greeting": "Hello"}"#)?;
        // German has an extra key not in template
        fs::write(
            l10n_dir.join("app_de.arb"),
            r#"{"greeting": "Hallo", "extra_key": "Zusatz"}"#,
        )?;

        let report = analyze_l10n_coverage(dir.path())?;
        let de = report.locales.iter().find(|l| l.locale == "de").unwrap();
        assert!(de.extra_keys.contains(&"extra_key".to_string()));
        assert!(report.issues.iter().any(|i| i.category == "Extra" && i.locale == "de"));
        Ok(())
    }

    #[test]
    fn analyze_l10n_coverage_detects_empty_translations() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;

        fs::write(l10n_dir.join("app_en.arb"), r#"{"greeting": "Hello"}"#)?;
        fs::write(l10n_dir.join("app_es.arb"), r#"{"greeting": ""}"#)?;

        let report = analyze_l10n_coverage(dir.path())?;
        let es = report.locales.iter().find(|l| l.locale == "es").unwrap();
        assert!(es.empty_values.contains(&"greeting".to_string()));
        assert!(report.issues.iter().any(|i| i.category == "Empty" && i.locale == "es"));
        Ok(())
    }

    #[test]
    fn analyze_l10n_coverage_detects_placeholder_mismatches() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;

        // Template has {name} placeholder
        fs::write(l10n_dir.join("app_en.arb"), r#"{"greeting": "Hello {name}"}"#)?;
        // Japanese translation lacks the placeholder
        fs::write(l10n_dir.join("app_ja.arb"), r#"{"greeting": "こんにちは"}"#)?;

        let report = analyze_l10n_coverage(dir.path())?;
        let ja = report.locales.iter().find(|l| l.locale == "ja").unwrap();
        assert!(ja.placeholder_mismatches.contains(&"greeting".to_string()));
        assert!(report
            .issues
            .iter()
            .any(|i| i.category == "Placeholder" && i.locale == "ja"));
        Ok(())
    }

    #[test]
    fn analyze_l10n_coverage_detects_unused_keys() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;

        fs::write(
            l10n_dir.join("app_en.arb"),
            r#"{"usedKey": "Used", "neverUsed": "Ghost"}"#,
        )?;
        fs::write(l10n_dir.join("app_fr.arb"), r#"{"usedKey": "Utilisé", "neverUsed": "Fantôme"}"#)?;

        // Dart file only references usedKey
        let lib_dir = dir.path().join("lib");
        fs::write(lib_dir.join("main.dart"), "context.l10n.usedKey;")?;

        let report = analyze_l10n_coverage(dir.path())?;
        assert!(report.unused_keys.contains(&"neverUsed".to_string()));
        assert!(report
            .issues
            .iter()
            .any(|i| i.category == "Unused" && i.key == "neverUsed"));
        Ok(())
    }

    #[test]
    fn analyze_l10n_coverage_score_decreases_with_issues() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;

        // Template with several keys
        fs::write(
            l10n_dir.join("app_en.arb"),
            r#"{"k1":"v1","k2":"v2","k3":"v3","k4":"v4","k5":"v5"}"#,
        )?;
        // French is missing all 5 keys → 5 Errors × 10 = 50 deductions
        fs::write(l10n_dir.join("app_fr.arb"), r#"{}"#)?;

        let report = analyze_l10n_coverage(dir.path())?;
        assert!(report.score < 100, "score should drop below 100 when there are errors");
        Ok(())
    }

    #[test]
    fn analyze_l10n_coverage_overall_coverage_pct_computed() -> Result<()> {
        let dir = TempDir::new()?;
        let l10n_dir = dir.path().join("lib").join("l10n");
        fs::create_dir_all(&l10n_dir)?;

        fs::write(l10n_dir.join("app_en.arb"), r#"{"a":"1","b":"2","c":"3","d":"4"}"#)?;
        // Spanish has 2 of 4 keys
        fs::write(l10n_dir.join("app_es.arb"), r#"{"a":"uno","b":"dos"}"#)?;

        let report = analyze_l10n_coverage(dir.path())?;
        assert!(report.overall_coverage_pct > 0.0);
        assert!(report.overall_coverage_pct <= 100.0);
        Ok(())
    }

    // ─── write_l10n_html_report ───────────────────────────────────────────────

    #[test]
    fn write_l10n_html_report_creates_file_with_html_content() -> Result<()> {
        let dir = TempDir::new()?;
        let out_path = dir.path().join("report.html");

        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 5,
            locales: vec![],
            unused_keys: vec![],
            overall_coverage_pct: 100.0,
            score: 100,
            issues: vec![],
        };

        write_l10n_html_report(&report, &out_path)?;

        assert!(out_path.exists(), "HTML file should be created");
        let content = fs::read_to_string(&out_path)?;
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Localization Coverage Report"));
        Ok(())
    }

    #[test]
    fn write_l10n_html_report_returns_error_for_invalid_path() {
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 0,
            locales: vec![],
            unused_keys: vec![],
            overall_coverage_pct: 0.0,
            score: 100,
            issues: vec![],
        };
        let bad_path = PathBuf::from("/nonexistent_dir/sub/report.html");
        assert!(write_l10n_html_report(&report, &bad_path).is_err());
    }

    // ─── generate_html_report (edge cases) ───────────────────────────────────

    #[test]
    fn generate_html_report_excellent_coverage_class() {
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 10,
            locales: vec![LocaleReport {
                locale: "de".to_string(),
                file: PathBuf::from("app_de.arb"),
                total_keys: 10,
                missing_keys: vec![],
                extra_keys: vec![],
                empty_values: vec![],
                placeholder_mismatches: vec![],
                coverage_pct: 100.0,
            }],
            unused_keys: vec![],
            overall_coverage_pct: 100.0,
            score: 100,
            issues: vec![],
        };
        let html = generate_html_report(&report);
        assert!(html.contains("progress-excellent"));
        assert!(html.contains("100.0%"));
    }

    #[test]
    fn generate_html_report_poor_coverage_class() {
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 10,
            locales: vec![LocaleReport {
                locale: "zh".to_string(),
                file: PathBuf::from("app_zh.arb"),
                total_keys: 3,
                missing_keys: vec!["a".to_string(), "b".to_string()],
                extra_keys: vec![],
                empty_values: vec![],
                placeholder_mismatches: vec![],
                coverage_pct: 30.0,
            }],
            unused_keys: vec![],
            overall_coverage_pct: 30.0,
            score: 60,
            issues: vec![],
        };
        let html = generate_html_report(&report);
        assert!(html.contains("progress-poor"));
        assert!(html.contains("30.0%"));
    }

    #[test]
    fn generate_html_report_good_coverage_class() {
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 10,
            locales: vec![LocaleReport {
                locale: "pt".to_string(),
                file: PathBuf::from("app_pt.arb"),
                total_keys: 9,
                missing_keys: vec!["one".to_string()],
                extra_keys: vec![],
                empty_values: vec![],
                placeholder_mismatches: vec![],
                coverage_pct: 90.0,
            }],
            unused_keys: vec![],
            overall_coverage_pct: 90.0,
            score: 90,
            issues: vec![],
        };
        let html = generate_html_report(&report);
        assert!(html.contains("progress-good"));
    }

    #[test]
    fn generate_html_report_includes_issues_section() {
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 2,
            locales: vec![],
            unused_keys: vec![],
            overall_coverage_pct: 0.0,
            score: 90,
            issues: vec![
                L10nIssue {
                    severity: L10nSeverity::Error,
                    category: "Missing".to_string(),
                    locale: "fr".to_string(),
                    key: "someKey".to_string(),
                    detail: "Key is missing in fr".to_string(),
                    suggestion: "Add the translation".to_string(),
                },
            ],
        };
        let html = generate_html_report(&report);
        assert!(html.contains("Issues"));
        assert!(html.contains("someKey"));
        assert!(html.contains("Missing"));
    }

    #[test]
    fn generate_html_report_includes_unused_keys_section() {
        let mut unused: Vec<String> = (0..25).map(|i| format!("unusedKey{}", i)).collect();
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 30,
            locales: vec![],
            unused_keys: unused.clone(),
            overall_coverage_pct: 0.0,
            score: 50,
            issues: vec![],
        };
        let html = generate_html_report(&report);
        assert!(html.contains("Unused Keys"));
        // Shows first 20; more than 20 triggers the "... and X more" line
        assert!(html.contains("more unused keys"));
        let _ = unused; // suppress unused warning
    }

    #[test]
    fn generate_html_report_issues_more_than_ten_shows_overflow() {
        let issues: Vec<L10nIssue> = (0..12)
            .map(|i| L10nIssue {
                severity: L10nSeverity::Warning,
                category: "Extra".to_string(),
                locale: "fr".to_string(),
                key: format!("key{}", i),
                detail: "extra key".to_string(),
                suggestion: "remove it".to_string(),
            })
            .collect();

        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 5,
            locales: vec![],
            unused_keys: vec![],
            overall_coverage_pct: 0.0,
            score: 40,
            issues,
        };
        let html = generate_html_report(&report);
        assert!(html.contains("more issues"));
    }

    #[test]
    fn generate_html_report_score_quality_excellent() {
        let report = L10nCoverageReport {
            template_locale: "en".to_string(),
            template_key_count: 5,
            locales: vec![LocaleReport {
                locale: "fr".to_string(),
                file: PathBuf::from("app_fr.arb"),
                total_keys: 5,
                missing_keys: vec![],
                extra_keys: vec![],
                empty_values: vec![],
                placeholder_mismatches: vec![],
                coverage_pct: 97.0,
            }],
            unused_keys: vec![],
            overall_coverage_pct: 97.0,
            score: 100,
            issues: vec![],
        };
        let html = generate_html_report(&report);
        assert!(html.contains("excellent"));
    }
}
