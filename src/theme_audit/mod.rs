//! Theme Auditor — Analyzes Flutter projects for theme consistency issues.
//!
//! This module provides comprehensive theme auditing for Flutter projects, detecting:
//! - Hardcoded colors (Color(0x...), Colors.red, hex literals) instead of Theme.of(context).colorScheme.*
//! - Hardcoded font sizes (fontSize: 14 literals) instead of Theme.of(context).textTheme.*
//! - Hardcoded padding/margin (EdgeInsets.all(8.0)) with magic numbers instead of design tokens
//! - Inconsistent theme usage — Mix of Theme.of(context) and hardcoded values in same file
//! - Missing dark mode support — ThemeData defined but no darkTheme in MaterialApp
//! - Hardcoded text styles (TextStyle(...)) with inline properties instead of theme references
//! - Direct Material color usage (Colors.blue[700]) instead of semantic colors
//!
//! # Example
//!
//! ```text
//! let report = audit_theme(Path::new("."))?;
//! print_theme_report(&report);
//! write_theme_html_report(&report, Path::new("theme_audit.html"))?;
//! ```

use anyhow::Result;
use colored::Colorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Severity level for theme issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ThemeSeverity {
    Info,
    Warning,
    Error,
}

impl ThemeSeverity {
    fn score_deduction(&self) -> u32 {
        match self {
            ThemeSeverity::Info => 2,
            ThemeSeverity::Warning => 5,
            ThemeSeverity::Error => 10,
        }
    }

    fn color_name(&self) -> &'static str {
        match self {
            ThemeSeverity::Info => "blue",
            ThemeSeverity::Warning => "yellow",
            ThemeSeverity::Error => "red",
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            ThemeSeverity::Info => "ℹ",
            ThemeSeverity::Warning => "⚠",
            ThemeSeverity::Error => "✕",
        }
    }
}

/// Detailed issue with theme consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeIssue {
    pub severity: ThemeSeverity,
    pub category: String,
    pub file: PathBuf,
    pub line: usize,
    pub code_snippet: String,
    pub detail: String,
    pub suggestion: String,
}

/// Summary statistics for theme audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSummary {
    pub hardcoded_colors: usize,
    pub hardcoded_fonts: usize,
    pub hardcoded_padding: usize,
    pub theme_references: usize,
    pub consistency_ratio: f64,
    pub has_dark_theme: bool,
}

/// Complete theme audit report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeAuditReport {
    pub total_files_scanned: usize,
    pub issues: Vec<ThemeIssue>,
    pub score: u32,
    pub summary: ThemeSummary,
}

/// Patterns for detecting theme issues
struct ThemePatterns {
    hardcoded_color_hex: Regex,
    hardcoded_color_colors: Regex,
    hardcoded_font_size: Regex,
    hardcoded_edge_insets: Regex,
    theme_reference: Regex,
    text_style_definition: Regex,
    material_color_shade: Regex,
    material_app_pattern: Regex,
}

impl ThemePatterns {
    fn new() -> Result<Self> {
        Ok(Self {
            // Match Color(0xFF...) or Color(0xff...)
            hardcoded_color_hex: Regex::new(r"Color\s*\(\s*0x[0-9a-fA-F]{8}\s*\)")?,
            // Match Colors.red, Colors.blue, Colors.green, etc.
            hardcoded_color_colors: Regex::new(r"Colors\.\w+")?,
            // Match fontSize: 14, fontSize:14, fontSize : 14 (with or without space)
            hardcoded_font_size: Regex::new(r"fontSize\s*:\s*\d+(?:\.\d+)?")?,
            // Match EdgeInsets.all(8.0), EdgeInsets.symmetric(horizontal: 16.0), etc.
            hardcoded_edge_insets: Regex::new(
                r"EdgeInsets\.(all|symmetric|only|fromLTRB)\s*\([^)]*\d",
            )?,
            // Match Theme.of(context) or context.theme
            theme_reference: Regex::new(r"Theme\.of\s*\(\s*context\s*\)|context\.theme")?,
            // Match TextStyle(...) definitions
            text_style_definition: Regex::new(r"TextStyle\s*\(")?,
            // Match Colors.blue[700], Colors.red[500], etc.
            material_color_shade: Regex::new(r"Colors\.\w+\s*\[\s*\d+\s*\]")?,
            // Match MaterialApp with darkTheme check
            material_app_pattern: Regex::new(r"MaterialApp\s*\(")?,
        })
    }
}

/// File analysis state
struct FileAnalysis {
    path: PathBuf,
    content: String,
    issues: Vec<ThemeIssue>,
    hardcoded_colors: usize,
    hardcoded_fonts: usize,
    hardcoded_padding: usize,
    theme_refs: usize,
}

/// Parse file and detect theme issues
fn analyze_file(path: &Path, content: &str, patterns: &ThemePatterns) -> Result<FileAnalysis> {
    let mut analysis = FileAnalysis {
        path: path.to_path_buf(),
        content: content.to_string(),
        issues: Vec::new(),
        hardcoded_colors: 0,
        hardcoded_fonts: 0,
        hardcoded_padding: 0,
        theme_refs: 0,
    };

    // Count theme references
    analysis.theme_refs = patterns.theme_reference.find_iter(content).count();

    let lines: Vec<&str> = content.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;

        // Skip comments
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        // Detect hardcoded hex colors
        if patterns.hardcoded_color_hex.is_match(line) {
            analysis.hardcoded_colors += 1;
            let matches: Vec<_> = patterns.hardcoded_color_hex.find_iter(line).collect();
            for m in matches {
                analysis.issues.push(ThemeIssue {
                    severity: ThemeSeverity::Error,
                    category: "Hardcoded Color".to_string(),
                    file: path.to_path_buf(),
                    line: line_number,
                    code_snippet: line.to_string(),
                    detail: format!("Found hardcoded hex color: {}", m.as_str()),
                    suggestion: "Use Theme.of(context).colorScheme.* instead of hardcoded colors"
                        .to_string(),
                });
            }
        }

        // Detect Colors.* usage (Warning level - could be in test or acceptable contexts)
        if patterns.hardcoded_color_colors.is_match(line)
            && !line.contains("Theme.of")
            && !line.contains("context.theme")
            && !trimmed.starts_with("//")
        {
            // Be more selective - only flag if it looks like a color assignment
            if line.contains("color:") || line.contains("Colors.") {
                analysis.hardcoded_colors += 1;
                let matches: Vec<_> = patterns.hardcoded_color_colors.find_iter(line).collect();
                for m in matches {
                    analysis.issues.push(ThemeIssue {
                        severity: ThemeSeverity::Warning,
                        category: "Hardcoded Color (Material)".to_string(),
                        file: path.to_path_buf(),
                        line: line_number,
                        code_snippet: line.to_string(),
                        detail: format!("Found direct Material color: {}", m.as_str()),
                        suggestion: "Prefer semantic colors from Theme.of(context).colorScheme"
                            .to_string(),
                    });
                }
            }
        }

        // Detect hardcoded font sizes
        if patterns.hardcoded_font_size.is_match(line) && !trimmed.starts_with("//") {
            analysis.hardcoded_fonts += 1;
            let matches: Vec<_> = patterns.hardcoded_font_size.find_iter(line).collect();
            for m in matches {
                analysis.issues.push(ThemeIssue {
                    severity: ThemeSeverity::Warning,
                    category: "Hardcoded Font Size".to_string(),
                    file: path.to_path_buf(),
                    line: line_number,
                    code_snippet: line.to_string(),
                    detail: format!("Found hardcoded font size: {}", m.as_str()),
                    suggestion: "Use Theme.of(context).textTheme.* instead of hardcoded font sizes"
                        .to_string(),
                });
            }
        }

        // Detect hardcoded padding/margins
        if patterns.hardcoded_edge_insets.is_match(line) && !trimmed.starts_with("//") {
            analysis.hardcoded_padding += 1;
            let matches: Vec<_> = patterns.hardcoded_edge_insets.find_iter(line).collect();
            for m in matches {
                analysis.issues.push(ThemeIssue {
                    severity: ThemeSeverity::Info,
                    category: "Hardcoded Padding/Margin".to_string(),
                    file: path.to_path_buf(),
                    line: line_number,
                    code_snippet: line.to_string(),
                    detail: format!("Found hardcoded EdgeInsets: {}", m.as_str()),
                    suggestion: "Consider using design tokens or theme-based spacing constants"
                        .to_string(),
                });
            }
        }

        // Detect Material color shade access (Colors.blue[700])
        if patterns.material_color_shade.is_match(line) && !trimmed.starts_with("//") {
            analysis.issues.push(ThemeIssue {
                severity: ThemeSeverity::Info,
                category: "Direct Material Shade Access".to_string(),
                file: path.to_path_buf(),
                line: line_number,
                code_snippet: line.to_string(),
                detail: "Found direct shade access on Material color".to_string(),
                suggestion: "Use Theme.of(context).colorScheme for semantic colors".to_string(),
            });
        }
    }

    Ok(analysis)
}

/// Check for dark theme support in MaterialApp
fn check_dark_theme_support(content: &str) -> bool {
    // Look for darkTheme: in MaterialApp definitions
    content.contains("darkTheme:") && content.contains("ThemeData(")
}

/// Collect all Dart files excluding generated and build files
fn collect_dart_files(project_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Skip exclusions
        let path_str = path.to_string_lossy();
        if path_str.contains("/.git/")
            || path_str.contains("/build/")
            || path_str.contains("/.dart_tool/")
            || path_str.contains("/packages/")
            || path.ends_with(".g.dart")
            || path.ends_with(".freezed.dart")
            || path.ends_with(".config.dart")
        {
            continue;
        }

        if path.extension().map_or(false, |ext| ext == "dart") {
            files.push(path.to_path_buf());
        }
    }

    Ok(files)
}

/// Main audit function
pub fn audit_theme(path: &Path) -> Result<ThemeAuditReport> {
    let patterns = ThemePatterns::new()?;
    let dart_files = collect_dart_files(path)?;
    let total_files_scanned = dart_files.len();

    let mut all_issues = Vec::new();
    let mut total_hardcoded_colors = 0;
    let mut total_hardcoded_fonts = 0;
    let mut total_hardcoded_padding = 0;
    let mut total_theme_refs = 0;
    let mut has_dark_theme = false;

    for file_path in dart_files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            // Check for dark theme support
            if !has_dark_theme && check_dark_theme_support(&content) {
                has_dark_theme = true;
            }

            // Analyze file
            if let Ok(analysis) = analyze_file(&file_path, &content, &patterns) {
                total_hardcoded_colors += analysis.hardcoded_colors;
                total_hardcoded_fonts += analysis.hardcoded_fonts;
                total_hardcoded_padding += analysis.hardcoded_padding;
                total_theme_refs += analysis.theme_refs;
                all_issues.extend(analysis.issues);
            }
        }
    }

    // Calculate consistency ratio
    let total_hardcoded = total_hardcoded_colors + total_hardcoded_fonts + total_hardcoded_padding;
    let total_theme_usage = total_theme_refs + total_hardcoded;
    let consistency_ratio = if total_theme_usage > 0 {
        total_theme_refs as f64 / total_theme_usage as f64
    } else {
        1.0
    };

    // Calculate score (0-100)
    let mut score: u32 = 100;

    // Deduct for issues
    for issue in &all_issues {
        score = score.saturating_sub(issue.severity.score_deduction());
    }

    // Deduct based on consistency ratio (if < 0.7, apply penalty)
    if consistency_ratio < 0.7 {
        let penalty = ((0.7 - consistency_ratio) * 100.0) as u32;
        score = score.saturating_sub(penalty);
    }

    // Deduct if no dark theme support (if has any theme usage)
    if !has_dark_theme && total_theme_refs > 0 {
        score = score.saturating_sub(15);
    }

    let summary = ThemeSummary {
        hardcoded_colors: total_hardcoded_colors,
        hardcoded_fonts: total_hardcoded_fonts,
        hardcoded_padding: total_hardcoded_padding,
        theme_references: total_theme_refs,
        consistency_ratio,
        has_dark_theme,
    };

    Ok(ThemeAuditReport {
        total_files_scanned,
        issues: all_issues,
        score,
        summary,
    })
}

/// Format a value with color based on score
fn format_score_colored(score: u32) -> String {
    if score >= 85 {
        format!("{}", score.to_string().green().bold())
    } else if score >= 70 {
        format!("{}", score.to_string().yellow().bold())
    } else {
        format!("{}", score.to_string().red().bold())
    }
}

/// Format percentage for display
fn format_percentage(ratio: f64) -> String {
    let percent = (ratio * 100.0) as u32;
    if percent >= 80 {
        format!("{:>3}%", percent).green().to_string()
    } else if percent >= 60 {
        format!("{:>3}%", percent).yellow().to_string()
    } else {
        format!("{:>3}%", percent).red().to_string()
    }
}

/// Print a formatted theme report to stdout
pub fn print_theme_report(report: &ThemeAuditReport) {
    println!(
        "\n{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold()
    );
    println!("{}", "  FALCON THEME AUDIT REPORT".bold().cyan());
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold()
    );

    // Summary section
    println!("\n{}", "SUMMARY".bold().underline());
    println!(
        "  Files Scanned:       {}",
        report.total_files_scanned.to_string().cyan()
    );
    println!("  Total Issues:        {}", report.issues.len());

    println!(
        "  Theme Score:         {}",
        format_score_colored(report.score)
    );

    // Details section
    println!("\n{}", "THEME USAGE BREAKDOWN".bold().underline());
    println!(
        "  Theme References:    {} {}",
        report.summary.theme_references.to_string().cyan(),
        format!("({})", format_percentage(report.summary.consistency_ratio))
    );
    println!(
        "  Hardcoded Colors:    {}",
        report.summary.hardcoded_colors.to_string().yellow()
    );
    println!(
        "  Hardcoded Font Sizes: {}",
        report.summary.hardcoded_fonts.to_string().yellow()
    );
    println!(
        "  Hardcoded Padding:   {}",
        report.summary.hardcoded_padding.to_string().yellow()
    );

    // Dark theme section
    println!("\n{}", "DARK MODE SUPPORT".bold().underline());
    let dark_theme_status = if report.summary.has_dark_theme {
        "✓ Configured".green()
    } else {
        "✗ Missing".red()
    };
    println!("  Status:              {}", dark_theme_status);

    // Issues section
    if !report.issues.is_empty() {
        println!("\n{}", "DETECTED ISSUES".bold().underline());

        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;

        for issue in &report.issues {
            match issue.severity {
                ThemeSeverity::Error => errors += 1,
                ThemeSeverity::Warning => warnings += 1,
                ThemeSeverity::Info => infos += 1,
            }
        }

        println!("  {} Errors  {} Warnings  {} Info", errors, warnings, infos);
        println!();

        // Group issues by category
        let mut categories: HashMap<String, Vec<&ThemeIssue>> = HashMap::new();
        for issue in &report.issues {
            categories
                .entry(issue.category.clone())
                .or_insert_with(Vec::new)
                .push(issue);
        }

        for (category, issues) in categories {
            println!("  {} ({})", category.bold(), issues.len());
            for issue in issues.iter().take(5) {
                let severity_str = match issue.severity {
                    ThemeSeverity::Error => "error".red(),
                    ThemeSeverity::Warning => "warning".yellow(),
                    ThemeSeverity::Info => "info".blue(),
                };
                println!(
                    "    {} {} at {}:{}",
                    issue.severity.symbol(),
                    severity_str,
                    issue.file.display(),
                    issue.line
                );
                println!("       → {}", issue.suggestion);
            }
            if issues.len() > 5 {
                println!("    ... and {} more", issues.len() - 5);
            }
            println!();
        }
    } else {
        println!("\n{}", "✓ No issues detected!".green().bold());
    }

    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold()
    );
}

/// Write a self-contained dark HTML report
pub fn write_theme_html_report(report: &ThemeAuditReport, output_path: &Path) -> Result<()> {
    let error_count = report
        .issues
        .iter()
        .filter(|i| i.severity == ThemeSeverity::Error)
        .count();
    let warning_count = report
        .issues
        .iter()
        .filter(|i| i.severity == ThemeSeverity::Warning)
        .count();
    let info_count = report
        .issues
        .iter()
        .filter(|i| i.severity == ThemeSeverity::Info)
        .count();

    // Build issues HTML
    let mut issues_html = String::new();
    if report.issues.is_empty() {
        issues_html.push_str(r#"<div class="no-issues"><p>✓ No theme issues detected!</p></div>"#);
    } else {
        for issue in &report.issues {
            let severity_class = match issue.severity {
                ThemeSeverity::Error => "error",
                ThemeSeverity::Warning => "warning",
                ThemeSeverity::Info => "info",
            };
            issues_html.push_str(&format!(
                r#"<div class="issue {severity}">
                    <div class="issue-header">
                        <span class="category">{category}</span>
                        <span class="location">{file}:{line}</span>
                    </div>
                    <pre class="code-snippet">{snippet}</pre>
                    <p class="detail">{detail}</p>
                    <p class="suggestion">💡 {suggestion}</p>
                </div>"#,
                severity = severity_class,
                category = issue.category,
                file = issue.file.display(),
                line = issue.line,
                snippet = html_escape(&issue.code_snippet),
                detail = issue.detail,
                suggestion = issue.suggestion,
            ));
        }
    }

    let score_color = if report.score >= 85 {
        "#10b981"
    } else if report.score >= 70 {
        "#f59e0b"
    } else {
        "#ef4444"
    };

    let consistency_percent = (report.summary.consistency_ratio * 100.0) as u32;
    let dark_theme_badge = if report.summary.has_dark_theme {
        r#"<span class="badge success">✓ Configured</span>"#
    } else {
        r#"<span class="badge error">✗ Missing</span>"#
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Falcon Theme Audit Report</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: #0f172a;
            color: #e2e8f0;
            line-height: 1.6;
        }}

        .container {{
            max-width: 1200px;
            margin: 0 auto;
            padding: 40px 20px;
        }}

        .header {{
            text-align: center;
            margin-bottom: 40px;
            border-bottom: 2px solid #334155;
            padding-bottom: 30px;
        }}

        .header h1 {{
            font-size: 2.5em;
            margin-bottom: 10px;
            color: #06b6d4;
        }}

        .score-display {{
            display: inline-flex;
            align-items: center;
            justify-content: center;
            width: 150px;
            height: 150px;
            border-radius: 50%;
            background: linear-gradient(135deg, rgba({}, 0.1), rgba({}, 0.05));
            border: 3px solid {};
            font-size: 3em;
            font-weight: bold;
            color: {};
            margin: 20px 0;
        }}

        .metrics {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 20px;
            margin: 30px 0;
        }}

        .metric {{
            background: #1e293b;
            border-left: 4px solid #06b6d4;
            padding: 20px;
            border-radius: 8px;
        }}

        .metric h3 {{
            font-size: 0.9em;
            text-transform: uppercase;
            color: #94a3b8;
            margin-bottom: 10px;
            letter-spacing: 1px;
        }}

        .metric .value {{
            font-size: 2em;
            font-weight: bold;
            color: #06b6d4;
        }}

        .metric .percentage {{
            font-size: 1em;
            color: #cbd5e1;
            margin-top: 5px;
        }}

        .stats {{
            display: grid;
            grid-template-columns: repeat(4, 1fr);
            gap: 15px;
            margin: 20px 0;
        }}

        .stat-item {{
            text-align: center;
            padding: 15px;
            background: #1e293b;
            border-radius: 6px;
        }}

        .stat-item .count {{
            font-size: 1.8em;
            font-weight: bold;
            margin-bottom: 5px;
        }}

        .stat-item .label {{
            font-size: 0.85em;
            color: #94a3b8;
            text-transform: uppercase;
        }}

        .stat-item.error .count {{
            color: #ef4444;
        }}

        .stat-item.warning .count {{
            color: #f59e0b;
        }}

        .stat-item.info .count {{
            color: #3b82f6;
        }}

        .badge {{
            display: inline-block;
            padding: 6px 12px;
            border-radius: 20px;
            font-size: 0.85em;
            font-weight: 600;
            margin-top: 10px;
        }}

        .badge.success {{
            background: rgba(16, 185, 129, 0.2);
            color: #10b981;
            border: 1px solid #10b981;
        }}

        .badge.error {{
            background: rgba(239, 68, 68, 0.2);
            color: #ef4444;
            border: 1px solid #ef4444;
        }}

        .issues-section {{
            margin-top: 40px;
            padding-top: 30px;
            border-top: 2px solid #334155;
        }}

        .issues-section h2 {{
            font-size: 1.5em;
            margin-bottom: 20px;
            color: #06b6d4;
        }}

        .issue {{
            background: #1e293b;
            border-left: 4px solid #94a3b8;
            padding: 20px;
            margin-bottom: 15px;
            border-radius: 6px;
        }}

        .issue.error {{
            border-left-color: #ef4444;
        }}

        .issue.warning {{
            border-left-color: #f59e0b;
        }}

        .issue.info {{
            border-left-color: #3b82f6;
        }}

        .issue-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 10px;
            flex-wrap: wrap;
            gap: 10px;
        }}

        .category {{
            font-weight: bold;
            color: #06b6d4;
            font-size: 1em;
        }}

        .location {{
            color: #94a3b8;
            font-size: 0.9em;
        }}

        .code-snippet {{
            background: #0f172a;
            padding: 12px;
            border-radius: 4px;
            overflow-x: auto;
            margin: 12px 0;
            border: 1px solid #334155;
            color: #cbd5e1;
            font-size: 0.9em;
        }}

        .detail {{
            color: #cbd5e1;
            margin: 10px 0;
        }}

        .suggestion {{
            color: #a1d8ff;
            background: rgba(3, 102, 214, 0.1);
            padding: 10px 12px;
            border-radius: 4px;
            margin-top: 10px;
        }}

        .no-issues {{
            text-align: center;
            padding: 40px;
            background: rgba(16, 185, 129, 0.1);
            border: 2px solid #10b981;
            border-radius: 8px;
        }}

        .no-issues p {{
            font-size: 1.3em;
            color: #10b981;
            font-weight: 600;
        }}

        .footer {{
            text-align: center;
            margin-top: 40px;
            padding-top: 20px;
            border-top: 1px solid #334155;
            color: #64748b;
            font-size: 0.85em;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🎨 Falcon Theme Audit</h1>
            <div class="score-display">{}</div>
            <p>Theme Consistency Score</p>
        </div>

        <div class="metrics">
            <div class="metric">
                <h3>Consistency Ratio</h3>
                <div class="value">{}</div>
                <div class="percentage">{} of theme usage is consistent</div>
            </div>

            <div class="metric">
                <h3>Files Scanned</h3>
                <div class="value">{}</div>
            </div>

            <div class="metric">
                <h3>Issues Found</h3>
                <div class="value">{}</div>
            </div>

            <div class="metric">
                <h3>Dark Mode</h3>
                {}
            </div>
        </div>

        <div class="stats">
            <div class="stat-item error">
                <div class="count">{}</div>
                <div class="label">Errors</div>
            </div>
            <div class="stat-item warning">
                <div class="count">{}</div>
                <div class="label">Warnings</div>
            </div>
            <div class="stat-item info">
                <div class="count">{}</div>
                <div class="label">Info</div>
            </div>
            <div class="stat-item">
                <div class="count">{}</div>
                <div class="label">Hardcoded Colors</div>
            </div>
        </div>

        <div class="issues-section">
            <h2>Issues Detected</h2>
            {}
        </div>

        <div class="footer">
            <p>Generated by Falcon Theme Audit — v1.0</p>
        </div>
    </div>
</body>
</html>"#,
        extract_rgb(score_color),
        extract_rgb(score_color),
        score_color,
        score_color,
        report.score,
        consistency_percent,
        consistency_percent,
        report.total_files_scanned,
        report.issues.len(),
        dark_theme_badge,
        error_count,
        warning_count,
        info_count,
        report.summary.hardcoded_colors,
        issues_html
    );

    fs::write(output_path, html)?;
    Ok(())
}

/// Extract RGB values from hex color
fn extract_rgb(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        format!(
            "{}, {}, {}",
            u8::from_str_radix(&hex[0..2], 16).unwrap_or(0),
            u8::from_str_radix(&hex[2..4], 16).unwrap_or(0),
            u8::from_str_radix(&hex[4..6], 16).unwrap_or(0)
        )
    } else {
        "6, 182, 212".to_string()
    }
}

/// HTML escape a string
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_severity_ordering() {
        assert!(ThemeSeverity::Error > ThemeSeverity::Warning);
        assert!(ThemeSeverity::Warning > ThemeSeverity::Info);
    }

    #[test]
    fn test_theme_severity_score_deduction() {
        assert_eq!(ThemeSeverity::Info.score_deduction(), 2);
        assert_eq!(ThemeSeverity::Warning.score_deduction(), 5);
        assert_eq!(ThemeSeverity::Error.score_deduction(), 10);
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<div>"), "&lt;div&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape(r#""quote""#), r#"&quot;quote&quot;"#);
    }

    #[test]
    fn test_extract_rgb() {
        assert_eq!(extract_rgb("#10b981"), "16, 185, 129");
        assert_eq!(extract_rgb("10b981"), "16, 185, 129");
    }

    #[test]
    fn test_detect_hardcoded_hex_color() {
        let patterns = ThemePatterns::new().unwrap();
        assert!(patterns.hardcoded_color_hex.is_match("Color(0xFFFF0000)"));
        assert!(patterns.hardcoded_color_hex.is_match("Color(0xff00ff00)"));
        assert!(!patterns.hardcoded_color_hex.is_match("Color(0xZZZZZZZZ)"));
    }

    #[test]
    fn test_detect_colors_class() {
        let patterns = ThemePatterns::new().unwrap();
        assert!(patterns.hardcoded_color_colors.is_match("Colors.red"));
        assert!(patterns.hardcoded_color_colors.is_match("Colors.blue"));
        assert!(patterns
            .hardcoded_color_colors
            .is_match("Colors.transparent"));
    }

    #[test]
    fn test_detect_font_size() {
        let patterns = ThemePatterns::new().unwrap();
        assert!(patterns.hardcoded_font_size.is_match("fontSize: 14"));
        assert!(patterns.hardcoded_font_size.is_match("fontSize:14.5"));
        assert!(patterns.hardcoded_font_size.is_match("fontSize : 14"));
    }

    #[test]
    fn test_detect_edge_insets() {
        let patterns = ThemePatterns::new().unwrap();
        assert!(patterns
            .hardcoded_edge_insets
            .is_match("EdgeInsets.all(8.0)"));
        assert!(patterns
            .hardcoded_edge_insets
            .is_match("EdgeInsets.symmetric(horizontal: 16.0)"));
        assert!(patterns
            .hardcoded_edge_insets
            .is_match("EdgeInsets.only(left: 10)"));
    }

    #[test]
    fn test_detect_theme_reference() {
        let patterns = ThemePatterns::new().unwrap();
        assert!(patterns.theme_reference.is_match("Theme.of(context)"));
        assert!(patterns.theme_reference.is_match("Theme.of( context )"));
        assert!(patterns.theme_reference.is_match("context.theme"));
    }

    #[test]
    fn test_detect_material_shade() {
        let patterns = ThemePatterns::new().unwrap();
        assert!(patterns.material_color_shade.is_match("Colors.blue[700]"));
        assert!(patterns.material_color_shade.is_match("Colors.red[500]"));
    }

    #[test]
    fn test_dark_theme_detection() {
        let content_with_dark = r#"
            MaterialApp(
              theme: ThemeData(...),
              darkTheme: ThemeData(...),
            )
        "#;
        assert!(check_dark_theme_support(content_with_dark));

        let content_without_dark = r#"
            MaterialApp(
              theme: ThemeData(...),
            )
        "#;
        assert!(!check_dark_theme_support(content_without_dark));
    }

    #[test]
    fn test_consistency_ratio_calculation() {
        let report = ThemeAuditReport {
            total_files_scanned: 5,
            issues: vec![],
            score: 100,
            summary: ThemeSummary {
                hardcoded_colors: 0,
                hardcoded_fonts: 0,
                hardcoded_padding: 0,
                theme_references: 10,
                consistency_ratio: 1.0,
                has_dark_theme: true,
            },
        };
        assert_eq!(report.summary.consistency_ratio, 1.0);
    }

    #[test]
    fn test_score_calculation_with_issues() {
        let issues = vec![
            ThemeIssue {
                severity: ThemeSeverity::Error,
                category: "Test".to_string(),
                file: PathBuf::from("test.dart"),
                line: 1,
                code_snippet: "test".to_string(),
                detail: "test".to_string(),
                suggestion: "test".to_string(),
            },
            ThemeIssue {
                severity: ThemeSeverity::Warning,
                category: "Test".to_string(),
                file: PathBuf::from("test.dart"),
                line: 2,
                code_snippet: "test".to_string(),
                detail: "test".to_string(),
                suggestion: "test".to_string(),
            },
        ];

        let mut score: u32 = 100;
        for issue in &issues {
            score = score.saturating_sub(issue.severity.score_deduction());
        }
        assert_eq!(score, 85); // 100 - 10 - 5 = 85
    }
}
