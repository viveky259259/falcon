//! Asset Optimizer — Analyzes Flutter project assets for optimization opportunities.
//!
//! This module provides comprehensive asset auditing for Flutter projects, detecting:
//! - Oversized images exceeding configurable thresholds
//! - Unused assets declared in pubspec.yaml but never referenced in code
//! - Missing WebP alternatives for large image files
//! - Duplicate assets identified by content hash
//! - Unoptimized SVGs with embedded fonts or unnecessary metadata
//!
//! # Example
//!
//! ```text
//! let report = audit_assets(Path::new("."))?;
//! print_asset_report(&report);
//! write_asset_html_report(&report, Path::new("asset_audit.html"))?;
//! ```

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Configuration for asset audit thresholds
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Maximum file size in bytes before flagging as oversized (default: 200KB)
    pub oversized_threshold_bytes: u64,
    /// Extensions to scan for images
    pub image_extensions: Vec<&'static str>,
    /// Extensions that should have WebP alternatives
    pub webp_conversion_candidates: Vec<&'static str>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            oversized_threshold_bytes: 200 * 1024, // 200KB
            image_extensions: vec!["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"],
            webp_conversion_candidates: vec!["png", "jpg", "jpeg"],
        }
    }
}

/// Severity level for asset issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AssetSeverity {
    Info,
    Warning,
    Error,
}

impl AssetSeverity {
    fn score_deduction(&self) -> u32 {
        match self {
            AssetSeverity::Info => 2,
            AssetSeverity::Warning => 5,
            AssetSeverity::Error => 10,
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            AssetSeverity::Info => "ℹ",
            AssetSeverity::Warning => "⚠",
            AssetSeverity::Error => "✕",
        }
    }
}

/// Detailed issue with an asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIssue {
    pub severity: AssetSeverity,
    pub category: String,
    pub file: PathBuf,
    pub detail: String,
    pub suggestion: String,
    pub savings_bytes: u64,
}

/// Complete audit report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAuditReport {
    pub total_assets: usize,
    pub total_size_bytes: u64,
    pub issues: Vec<AssetIssue>,
    pub potential_savings_bytes: u64,
    pub score: u32,
}

/// Parse pubspec.yaml to extract asset declarations
fn parse_pubspec_assets(project_path: &Path) -> Result<HashSet<PathBuf>> {
    let pubspec_path = project_path.join("pubspec.yaml");
    let mut assets = HashSet::new();

    if !pubspec_path.exists() {
        return Ok(assets);
    }

    let content = fs::read_to_string(&pubspec_path)?;
    let lines = content.lines().collect::<Vec<_>>();

    let mut in_assets_section = false;
    for line in lines {
        let trimmed = line.trim();

        // Detect flutter.assets section
        if trimmed.starts_with("flutter:") {
            in_assets_section = false;
        } else if trimmed.starts_with("assets:") && !in_assets_section {
            in_assets_section = true;
            continue;
        }

        if in_assets_section {
            // End of assets section (next non-asset line)
            if !trimmed.is_empty() && !trimmed.starts_with('-') && !trimmed.starts_with('#') {
                break;
            }

            // Parse asset lines (format: "  - assets/path/")
            if trimmed.starts_with('-') {
                let asset = trimmed.strip_prefix('-').unwrap_or("").trim();
                if !asset.is_empty() {
                    assets.insert(PathBuf::from(asset));
                }
            }
        }
    }

    Ok(assets)
}

/// Collect all asset files from the project
fn collect_assets(project_path: &Path, config: &AuditConfig) -> Result<Vec<(PathBuf, u64)>> {
    let mut assets = Vec::new();
    let asset_exts = config
        .image_extensions
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<HashSet<_>>();

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Skip common exclusions
        let path_str = path.to_string_lossy();
        if ["/.git/", "/build/", "/.dart_tool/", "/packages/"]
            .iter()
            .any(|excl| path_str.contains(excl))
        {
            continue;
        }

        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                if asset_exts.contains(&ext_str.to_lowercase()) {
                    if let Ok(metadata) = fs::metadata(path) {
                        let relative_path = path
                            .strip_prefix(project_path)
                            .unwrap_or(path)
                            .to_path_buf();
                        assets.push((relative_path, metadata.len()));
                    }
                }
            }
        }
    }

    Ok(assets)
}

/// Find assets referenced in Dart source code
fn find_referenced_assets(project_path: &Path) -> Result<HashSet<PathBuf>> {
    let mut referenced = HashSet::new();

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Only check Dart files
        if path.extension().is_none_or(|ext| ext != "dart") {
            continue;
        }

        if let Ok(content) = fs::read_to_string(path) {
            // Simple pattern matching for asset references
            // Matches: 'assets/...', "assets/...", Image.asset('assets/...')
            for line in content.lines() {
                // Skip comments
                if line.trim().starts_with("//") {
                    continue;
                }

                // Extract string literals with 'assets' paths
                extract_asset_references(line, &mut referenced);
            }
        }
    }

    Ok(referenced)
}

/// Extract asset paths from a code line
fn extract_asset_references(line: &str, referenced: &mut HashSet<PathBuf>) {
    // Match patterns like 'assets/...' or "assets/..."
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut current_str = String::new();
    let mut escape_next = false;

    for ch in line.chars() {
        if escape_next {
            escape_next = false;
            if in_single_quote || in_double_quote {
                current_str.push(ch);
            }
            continue;
        }

        match ch {
            '\\' if in_single_quote || in_double_quote => {
                escape_next = true;
            }
            '\'' if !in_double_quote => {
                if in_single_quote {
                    if current_str.starts_with("assets/") {
                        referenced.insert(PathBuf::from(current_str.clone()));
                    }
                    current_str.clear();
                }
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                if in_double_quote {
                    if current_str.starts_with("assets/") {
                        referenced.insert(PathBuf::from(current_str.clone()));
                    }
                    current_str.clear();
                }
                in_double_quote = !in_double_quote;
            }
            _ if in_single_quote || in_double_quote => {
                current_str.push(ch);
            }
            _ => {}
        }
    }
}

/// Compute hash of first 4KB of file for duplicate detection
fn compute_file_hash(path: &Path) -> Result<u64> {
    let file = fs::File::open(path)?;
    let mut hash: u64 = 0;
    let mut buffer = [0u8; 4096];

    let bytes_read = std::io::Read::read(&mut &file, &mut buffer)?;
    for &byte in &buffer[..bytes_read] {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }

    Ok(hash)
}

/// Find duplicate assets by content hash
fn find_duplicates(project_path: &Path, assets: &[(PathBuf, u64)]) -> Result<Vec<Vec<PathBuf>>> {
    let mut hash_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for (path, _) in assets {
        let full_path = project_path.join(path);
        if let Ok(hash) = compute_file_hash(&full_path) {
            hash_map.entry(hash).or_default().push(path.clone());
        }
    }

    let duplicates = hash_map
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();

    Ok(duplicates)
}

/// Check for unoptimized SVGs
fn check_svg_optimization(
    project_path: &Path,
    svg_paths: &[PathBuf],
) -> Result<Vec<(PathBuf, String)>> {
    let mut unoptimized = Vec::new();

    for path in svg_paths {
        let full_path = project_path.join(path);
        if let Ok(content) = fs::read_to_string(&full_path) {
            let mut issues = Vec::new();

            if content.contains("<defs>") || content.contains("<font") {
                issues.push("Contains embedded fonts or defs".to_string());
            }

            if content.contains("<?xml") && content.len() > 500 {
                issues.push("May contain unnecessary metadata".to_string());
            }

            if content.contains("DOCTYPE") {
                issues.push("Contains DOCTYPE declaration".to_string());
            }

            if !issues.is_empty() {
                unoptimized.push((path.clone(), issues.join(", ")));
            }
        }
    }

    Ok(unoptimized)
}

/// Main audit function with default configuration.
pub fn audit_assets(path: &Path) -> Result<AssetAuditReport> {
    audit_assets_with_threshold(
        path,
        AuditConfig::default().oversized_threshold_bytes / 1024,
    )
}

/// Main audit function with a custom oversized-image threshold (KB).
pub fn audit_assets_with_threshold(
    path: &Path,
    size_threshold_kb: u64,
) -> Result<AssetAuditReport> {
    let config = AuditConfig {
        oversized_threshold_bytes: size_threshold_kb * 1024,
        ..AuditConfig::default()
    };

    // Parse pubspec.yaml for declared assets
    let declared_assets = parse_pubspec_assets(path)?;

    // Collect all image files
    let assets = collect_assets(path, &config)?;

    // Find referenced assets in code
    let referenced_assets = find_referenced_assets(path)?;

    // Compute total size
    let total_size_bytes: u64 = assets.iter().map(|(_, size)| size).sum();
    let total_assets = assets.len();

    // Separate assets by type
    let mut oversized = Vec::new();
    let mut svg_paths = Vec::new();
    let mut webp_candidates = Vec::new();

    for (path, size) in &assets {
        let path_str = path.to_string_lossy();

        if *size > config.oversized_threshold_bytes {
            oversized.push((path.clone(), *size));
        }

        if path_str.ends_with(".svg") {
            svg_paths.push(path.clone());
        }

        // Check for WebP conversion candidates
        for ext in &config.webp_conversion_candidates {
            if path_str.ends_with(ext) && *size > 50 * 1024 {
                webp_candidates.push((path.clone(), *size));
            }
        }
    }

    // Find unused assets
    let mut unused = Vec::new();
    for declared in &declared_assets {
        let mut found = false;

        // Check if declared asset is referenced
        for referenced in &referenced_assets {
            if referenced
                .to_string_lossy()
                .contains(declared.to_string_lossy().as_ref())
            {
                found = true;
                break;
            }
        }

        if !found {
            // Check if any actual file matches this declaration
            let full_path = path.join(declared);
            if full_path.exists() && full_path.is_file() {
                unused.push(
                    full_path
                        .strip_prefix(path)
                        .unwrap_or(&full_path)
                        .to_path_buf(),
                );
            }
        }
    }

    // Find duplicates
    let duplicates = find_duplicates(path, &assets)?;

    // Check SVG optimization
    let unoptimized_svgs = check_svg_optimization(path, &svg_paths)?;

    // Build issues
    let mut issues = Vec::new();
    let mut potential_savings = 0u64;

    // Oversized images
    for (file, size) in oversized {
        let savings = (size as f64 * 0.3) as u64; // Assume 30% savings
        potential_savings += savings;
        issues.push(AssetIssue {
            severity: AssetSeverity::Warning,
            category: "Oversized".to_string(),
            file: file.clone(),
            detail: format!("{} — {} bytes", file.display(), size),
            suggestion: format!(
                "Optimize or compress this image (could save ~{} bytes)",
                savings
            ),
            savings_bytes: savings,
        });
    }

    // Unused assets
    for file in unused {
        potential_savings += fs::metadata(path.join(&file)).map(|m| m.len()).unwrap_or(0);
        issues.push(AssetIssue {
            severity: AssetSeverity::Info,
            category: "Unused".to_string(),
            file: file.clone(),
            detail: "Declared in pubspec.yaml but never referenced".to_string(),
            suggestion: "Remove from pubspec.yaml and filesystem if no longer needed".to_string(),
            savings_bytes: fs::metadata(path.join(&file)).map(|m| m.len()).unwrap_or(0),
        });
    }

    // Missing WebP alternatives
    for (file, size) in webp_candidates {
        let webp_path = file.with_extension("webp");
        if !assets.iter().any(|(p, _)| p == &webp_path) {
            let savings = (size as f64 * 0.4) as u64; // Assume 40% savings with WebP
            potential_savings += savings;
            issues.push(AssetIssue {
                severity: AssetSeverity::Warning,
                category: "No WebP Alternative".to_string(),
                file: file.clone(),
                detail: format!("{} — {} bytes (no WebP version)", file.display(), size),
                suggestion: "Convert to WebP format for better compression (~40% smaller)"
                    .to_string(),
                savings_bytes: savings,
            });
        }
    }

    // Duplicates
    for duplicate_group in duplicates {
        if duplicate_group.len() > 1 {
            if let Ok(size) = fs::metadata(path.join(&duplicate_group[0])).map(|m| m.len()) {
                let savings = size * (duplicate_group.len() - 1) as u64;
                potential_savings += savings;

                for dup_file in &duplicate_group {
                    issues.push(AssetIssue {
                        severity: AssetSeverity::Info,
                        category: "Duplicate Asset".to_string(),
                        file: dup_file.clone(),
                        detail: format!(
                            "Duplicate of {} ({} bytes)",
                            duplicate_group[0].display(),
                            size
                        ),
                        suggestion: "Remove duplicates and use symlinks or shared reference"
                            .to_string(),
                        savings_bytes: size,
                    });
                }
            }
        }
    }

    // Unoptimized SVGs
    for (file, issues_str) in unoptimized_svgs {
        potential_savings += 500; // Minimal savings, mainly for cleanup
        issues.push(AssetIssue {
            severity: AssetSeverity::Info,
            category: "Unoptimized SVG".to_string(),
            file: file.clone(),
            detail: format!("{} — {}", file.display(), issues_str),
            suggestion: "Run through an SVG optimizer (SVGO) to remove metadata and reduce size"
                .to_string(),
            savings_bytes: 500,
        });
    }

    // Calculate score
    let mut score = 100u32;
    for issue in &issues {
        let deduction = issue.severity.score_deduction();
        score = score.saturating_sub(deduction);
    }

    Ok(AssetAuditReport {
        total_assets,
        total_size_bytes,
        issues,
        potential_savings_bytes: potential_savings,
        score,
    })
}

/// Print formatted asset report to console
pub fn print_asset_report(report: &AssetAuditReport) {
    println!();
    println!(
        "{}",
        "╔════════════════════════════════════════════════════════╗".bright_cyan()
    );
    println!(
        "{}",
        "║          FALCON Asset Audit Report                    ║".bright_cyan()
    );
    println!(
        "{}",
        "╚════════════════════════════════════════════════════════╝".bright_cyan()
    );
    println!();

    // Summary cards
    println!(
        "  📦 {} assets  │  {} MB total",
        report.total_assets.to_string().bold(),
        (report.total_size_bytes / 1024 / 1024).to_string().bold()
    );
    println!(
        "  💾 {} potential savings  │  {} issues found",
        format_bytes(report.potential_savings_bytes).bold(),
        report.issues.len().to_string().bold()
    );
    println!("  🎯 Health Score: {}", format_score(report.score));
    println!();

    if report.issues.is_empty() {
        println!("  {} All assets optimized!", "✓".bright_green().bold());
        println!();
        return;
    }

    // Group issues by category
    let mut by_category: HashMap<String, Vec<&AssetIssue>> = HashMap::new();
    for issue in &report.issues {
        by_category
            .entry(issue.category.clone())
            .or_default()
            .push(issue);
    }

    // Print by category
    for (category, mut cat_issues) in by_category {
        cat_issues.sort_by_key(|i| std::cmp::Reverse(i.savings_bytes));

        let total_savings: u64 = cat_issues.iter().map(|i| i.savings_bytes).sum();
        println!(
            "  {} {}",
            "──".dimmed(),
            format!(
                "{} ({} issues, {} savings)",
                category,
                cat_issues.len(),
                format_bytes(total_savings)
            )
            .bright_white()
            .bold()
        );

        for (idx, issue) in cat_issues.iter().take(3).enumerate() {
            let severity_str = match issue.severity {
                AssetSeverity::Error => issue.severity.symbol().bright_red(),
                AssetSeverity::Warning => issue.severity.symbol().bright_yellow(),
                AssetSeverity::Info => issue.severity.symbol().bright_blue(),
            };

            println!(
                "    {} {} — {}",
                severity_str,
                issue.file.display(),
                issue.detail
            );

            if idx == 2 && cat_issues.len() > 3 {
                println!(
                    "    {} {} more issues in this category",
                    "└".dimmed(),
                    (cat_issues.len() - 3).to_string().dimmed()
                );
                break;
            }
        }
        println!();
    }

    println!(
        "  {} Run with --html to generate detailed report",
        "→".dimmed()
    );
    println!();
}

/// Format bytes to human-readable string
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}

/// Format score with color
fn format_score(score: u32) -> String {
    match score {
        90..=100 => format!("{}/100", score).bright_green().bold().to_string(),
        70..=89 => format!("{}/100", score).bright_yellow().bold().to_string(),
        _ => format!("{}/100", score).bright_red().bold().to_string(),
    }
}

/// Generate an HTML report for the asset audit
pub fn write_asset_html_report(report: &AssetAuditReport, output_path: &Path) -> Result<()> {
    let html = generate_html_report(report);
    fs::write(output_path, html)?;
    Ok(())
}

/// Generate HTML report content
fn generate_html_report(report: &AssetAuditReport) -> String {
    let health_score = report.score;
    let score_color = if health_score >= 90 {
        "#10b981"
    } else if health_score >= 70 {
        "#f59e0b"
    } else {
        "#ef4444"
    };

    let mut issues_by_category: HashMap<String, Vec<&AssetIssue>> = HashMap::new();
    for issue in &report.issues {
        issues_by_category
            .entry(issue.category.clone())
            .or_default()
            .push(issue);
    }

    let error_count = report
        .issues
        .iter()
        .filter(|i| i.severity == AssetSeverity::Error)
        .count();
    let warning_count = report
        .issues
        .iter()
        .filter(|i| i.severity == AssetSeverity::Warning)
        .count();
    let info_count = report
        .issues
        .iter()
        .filter(|i| i.severity == AssetSeverity::Info)
        .count();

    let issues_html = if report.issues.is_empty() {
        "<div style='text-align: center; padding: 40px; color: #10b981;'><p style='font-size: 18px;'>✓ All assets are optimized!</p></div>".to_string()
    } else {
        let mut html = String::new();
        for (category, mut issues) in issues_by_category {
            issues.sort_by_key(|i| std::cmp::Reverse(i.savings_bytes));
            let total_savings: u64 = issues.iter().map(|i| i.savings_bytes).sum();

            html.push_str(&format!(
                "<div style='margin-bottom: 24px;'>\
                <h3 style='margin: 0 0 16px 0; color: #e4e4ef; font-size: 16px;'>{} ({} issues, {} savings)</h3>\
                <table style='width: 100%; border-collapse: collapse;'>\
                <thead><tr style='border-bottom: 1px solid #2a2a3a;'>\
                <th style='text-align: left; padding: 12px; color: #8888a0;'>File</th>\
                <th style='text-align: left; padding: 12px; color: #8888a0;'>Severity</th>\
                <th style='text-align: left; padding: 12px; color: #8888a0;'>Detail</th>\
                <th style='text-align: right; padding: 12px; color: #8888a0;'>Savings</th>\
                </tr></thead><tbody>",
                category, issues.len(), format_bytes(total_savings)
            ));

            for (idx, issue) in issues.iter().enumerate() {
                let severity_badge = match issue.severity {
                    AssetSeverity::Error => "<span style='background: #fee2e2; color: #991b1b; padding: 4px 8px; border-radius: 4px; font-size: 12px;'>Error</span>",
                    AssetSeverity::Warning => "<span style='background: #fef3c7; color: #92400e; padding: 4px 8px; border-radius: 4px; font-size: 12px;'>Warning</span>",
                    AssetSeverity::Info => "<span style='background: #dbeafe; color: #1e40af; padding: 4px 8px; border-radius: 4px; font-size: 12px;'>Info</span>",
                };

                let bg = if idx % 2 == 0 {
                    "rgba(255, 255, 255, .02)"
                } else {
                    "transparent"
                };

                html.push_str(&format!(
                    "<tr style='background: {}; border-bottom: 1px solid #2a2a3a;'>\
                    <td style='padding: 12px; color: #e4e4ef; font-family: monospace; font-size: 12px;'>{}</td>\
                    <td style='padding: 12px;'>{}</td>\
                    <td style='padding: 12px; color: #8888a0;'>{}</td>\
                    <td style='padding: 12px; text-align: right; color: #e4e4ef;'>{}</td>\
                    </tr>",
                    bg,
                    issue.file.display(),
                    severity_badge,
                    issue.suggestion,
                    format_bytes(issue.savings_bytes)
                ));
            }

            html.push_str("</tbody></table></div>");
        }
        html
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Falcon Asset Audit Report</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', 'Oxygen', 'Ubuntu', 'Cantarell', 'Fira Sans', 'Droid Sans', 'Helvetica Neue', sans-serif;
            background: #0f0f14;
            color: #e4e4ef;
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
            border-bottom: 1px solid #2a2a3a;
            padding-bottom: 40px;
        }}

        .header h1 {{
            font-size: 36px;
            margin-bottom: 8px;
            background: linear-gradient(135deg, #6366f1, #818cf8);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }}

        .header p {{
            color: #8888a0;
            font-size: 16px;
        }}

        .summary-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 16px;
            margin-bottom: 40px;
        }}

        .summary-card {{
            background: #1a1a24;
            border: 1px solid #2a2a3a;
            border-radius: 8px;
            padding: 20px;
            text-align: center;
        }}

        .summary-card .label {{
            color: #8888a0;
            font-size: 12px;
            text-transform: uppercase;
            letter-spacing: 1px;
            margin-bottom: 8px;
        }}

        .summary-card .value {{
            font-size: 28px;
            font-weight: 700;
            color: #e4e4ef;
        }}

        .score-card {{
            background: linear-gradient(135deg, rgba({}, 0.1), rgba({}, 0.05));
            border: 2px solid {};
            text-align: center;
        }}

        .score-card .value {{
            color: {};
        }}

        .issues-section {{
            margin-top: 40px;
        }}

        .issues-section h2 {{
            font-size: 20px;
            margin-bottom: 24px;
            color: #e4e4ef;
        }}

        table {{
            width: 100%;
            border-collapse: collapse;
        }}

        th {{
            background: #16161e;
            padding: 12px;
            text-align: left;
            color: #8888a0;
            font-size: 12px;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            border-bottom: 2px solid #2a2a3a;
        }}

        td {{
            padding: 12px;
            border-bottom: 1px solid #2a2a3a;
        }}

        tr:hover {{
            background: rgba(99, 102, 241, 0.04);
        }}

        .severity-error {{
            background: #fee2e2;
            color: #991b1b;
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 12px;
            font-weight: 600;
        }}

        .severity-warning {{
            background: #fef3c7;
            color: #92400e;
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 12px;
            font-weight: 600;
        }}

        .severity-info {{
            background: #dbeafe;
            color: #1e40af;
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 12px;
            font-weight: 600;
        }}

        .file-path {{
            font-family: 'Fira Code', 'JetBrains Mono', monospace;
            font-size: 12px;
            color: #818cf8;
            word-break: break-all;
        }}

        .footer {{
            text-align: center;
            margin-top: 40px;
            padding-top: 20px;
            border-top: 1px solid #2a2a3a;
            color: #5c5c72;
            font-size: 12px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Asset Audit Report</h1>
            <p>Comprehensive analysis of your Flutter project's assets</p>
        </div>

        <div class="summary-grid">
            <div class="summary-card">
                <div class="label">Total Assets</div>
                <div class="value">{}</div>
            </div>
            <div class="summary-card">
                <div class="label">Total Size</div>
                <div class="value">{}</div>
            </div>
            <div class="summary-card">
                <div class="label">Issues Found</div>
                <div class="value">{}</div>
            </div>
            <div class="summary-card">
                <div class="label">Potential Savings</div>
                <div class="value">{}</div>
            </div>
            <div class="summary-card score-card" style="grid-column: 1 / -1;">
                <div class="label">Health Score</div>
                <div class="value">{}/100</div>
            </div>
        </div>

        <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; margin-bottom: 40px;">
            <div class="summary-card">
                <div class="label">Errors</div>
                <div class="value" style="color: #ef4444;">{}</div>
            </div>
            <div class="summary-card">
                <div class="label">Warnings</div>
                <div class="value" style="color: #f59e0b;">{}</div>
            </div>
            <div class="summary-card">
                <div class="label">Info</div>
                <div class="value" style="color: #6366f1;">{}</div>
            </div>
        </div>

        <div class="issues-section">
            <h2>Asset Issues</h2>
            {}
        </div>

        <div class="footer">
            <p>Generated by Falcon Asset Audit — v1.0</p>
        </div>
    </div>
</body>
</html>"#,
        extract_rgb(score_color),
        extract_rgb(score_color),
        score_color,
        score_color,
        report.total_assets,
        format_bytes(report.total_size_bytes),
        report.issues.len(),
        format_bytes(report.potential_savings_bytes),
        report.score,
        error_count,
        warning_count,
        info_count,
        issues_html
    )
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
        "99, 102, 241".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_severity_ordering() {
        assert!(AssetSeverity::Error > AssetSeverity::Warning);
        assert!(AssetSeverity::Warning > AssetSeverity::Info);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_extract_asset_references() {
        let mut referenced = HashSet::new();
        extract_asset_references("Image.asset('assets/images/logo.png')", &mut referenced);
        assert!(referenced.contains(&PathBuf::from("assets/images/logo.png")));
    }

    #[test]
    fn test_extract_asset_references_double_quotes() {
        let mut referenced = HashSet::new();
        extract_asset_references(r#"Image.asset("assets/images/icon.png")"#, &mut referenced);
        assert!(referenced.contains(&PathBuf::from("assets/images/icon.png")));
    }

    #[test]
    fn test_extract_asset_references_ignores_non_assets() {
        let mut referenced = HashSet::new();
        extract_asset_references("String text = 'some random text'", &mut referenced);
        assert!(referenced.is_empty());
    }

    #[test]
    fn test_format_score_green() {
        let score = format_score(95);
        assert!(score.contains("95"));
    }

    #[test]
    fn test_format_score_yellow() {
        let score = format_score(75);
        assert!(score.contains("75"));
    }

    #[test]
    fn test_format_score_red() {
        let score = format_score(50);
        assert!(score.contains("50"));
    }
}
