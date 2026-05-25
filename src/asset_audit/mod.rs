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
//! ```ignore
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

    fn color_name(&self) -> &'static str {
        match self {
            AssetSeverity::Info => "blue",
            AssetSeverity::Warning => "yellow",
            AssetSeverity::Error => "red",
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

/// Internal structure for tracking assets by category
#[derive(Debug)]
struct AssetAnalysis {
    oversized: Vec<(PathBuf, u64)>,
    unused: Vec<PathBuf>,
    no_webp: Vec<(PathBuf, u64)>,
    duplicates: Vec<Vec<PathBuf>>,
    unoptimized_svgs: Vec<(PathBuf, String)>,
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
        if path.extension().map_or(true, |ext| ext != "dart") {
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
            hash_map
                .entry(hash)
                .or_insert_with(Vec::new)
                .push(path.clone());
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
    let mut config = AuditConfig::default();
    config.oversized_threshold_bytes = size_threshold_kb * 1024;

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
            detail: format!("Declared in pubspec.yaml but never referenced"),
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
        "  {} {} assets  │  {} MB total",
        "📦".to_string(),
        report.total_assets.to_string().bold(),
        (report.total_size_bytes / 1024 / 1024).to_string().bold()
    );
    println!(
        "  {} {} potential savings  │  {} issues found",
        "💾".to_string(),
        format_bytes(report.potential_savings_bytes).bold(),
        report.issues.len().to_string().bold()
    );
    println!(
        "  {} Health Score: {}",
        "🎯".to_string(),
        format_score(report.score)
    );
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
            .or_insert_with(Vec::new)
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
            .or_insert_with(Vec::new)
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
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ────────────────────────────────────────────────────────────────

    fn make_file(dir: &Path, rel: &str, content: &[u8]) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&full).unwrap();
        f.write_all(content).unwrap();
    }

    // ── AuditConfig::default ──────────────────────────────────────────────────

    #[test]
    fn test_audit_config_default_threshold() {
        let cfg = AuditConfig::default();
        assert_eq!(cfg.oversized_threshold_bytes, 200 * 1024);
    }

    #[test]
    fn test_audit_config_default_image_extensions() {
        let cfg = AuditConfig::default();
        assert!(cfg.image_extensions.contains(&"png"));
        assert!(cfg.image_extensions.contains(&"jpg"));
        assert!(cfg.image_extensions.contains(&"svg"));
        assert!(cfg.image_extensions.contains(&"webp"));
    }

    #[test]
    fn test_audit_config_default_webp_candidates() {
        let cfg = AuditConfig::default();
        assert!(cfg.webp_conversion_candidates.contains(&"png"));
        assert!(cfg.webp_conversion_candidates.contains(&"jpg"));
        assert!(cfg.webp_conversion_candidates.contains(&"jpeg"));
    }

    // ── AssetSeverity methods ─────────────────────────────────────────────────

    #[test]
    fn test_asset_severity_ordering() {
        assert!(AssetSeverity::Error > AssetSeverity::Warning);
        assert!(AssetSeverity::Warning > AssetSeverity::Info);
    }

    #[test]
    fn test_asset_severity_score_deduction() {
        assert_eq!(AssetSeverity::Info.score_deduction(), 2);
        assert_eq!(AssetSeverity::Warning.score_deduction(), 5);
        assert_eq!(AssetSeverity::Error.score_deduction(), 10);
    }

    #[test]
    fn test_asset_severity_color_name() {
        assert_eq!(AssetSeverity::Info.color_name(), "blue");
        assert_eq!(AssetSeverity::Warning.color_name(), "yellow");
        assert_eq!(AssetSeverity::Error.color_name(), "red");
    }

    #[test]
    fn test_asset_severity_symbol() {
        assert_eq!(AssetSeverity::Info.symbol(), "ℹ");
        assert_eq!(AssetSeverity::Warning.symbol(), "⚠");
        assert_eq!(AssetSeverity::Error.symbol(), "✕");
    }

    #[test]
    fn test_asset_severity_equality() {
        assert_eq!(AssetSeverity::Info, AssetSeverity::Info);
        assert_ne!(AssetSeverity::Info, AssetSeverity::Warning);
    }

    // ── format_bytes ──────────────────────────────────────────────────────────

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn test_format_bytes_boundary_kb() {
        // 1023 bytes is still in B range
        assert!(format_bytes(1023).ends_with(" B"));
        // 1024 bytes is exactly 1.0 KB
        assert_eq!(format_bytes(1024), "1.0 KB");
    }

    #[test]
    fn test_format_bytes_large_mb() {
        let result = format_bytes(5 * 1024 * 1024);
        assert!(result.contains("MB"));
        assert!(result.contains("5.0"));
    }

    #[test]
    fn test_format_bytes_fractional_kb() {
        // 1536 bytes = 1.5 KB
        let result = format_bytes(1536);
        assert!(result.contains("KB"));
        assert!(result.contains("1.5"));
    }

    // ── format_score ─────────────────────────────────────────────────────────

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

    #[test]
    fn test_format_score_boundary_90() {
        // 90 is the lower bound for green
        let score = format_score(90);
        assert!(score.contains("90"));
    }

    #[test]
    fn test_format_score_boundary_70() {
        // 70 is the lower bound for yellow
        let score = format_score(70);
        assert!(score.contains("70"));
    }

    #[test]
    fn test_format_score_zero() {
        let score = format_score(0);
        assert!(score.contains("0"));
    }

    #[test]
    fn test_format_score_100() {
        let score = format_score(100);
        assert!(score.contains("100"));
    }

    // ── extract_rgb ───────────────────────────────────────────────────────────

    #[test]
    fn test_extract_rgb_green() {
        assert_eq!(extract_rgb("#10b981"), "16, 185, 129");
    }

    #[test]
    fn test_extract_rgb_without_hash() {
        assert_eq!(extract_rgb("ff0000"), "255, 0, 0");
    }

    #[test]
    fn test_extract_rgb_black() {
        assert_eq!(extract_rgb("#000000"), "0, 0, 0");
    }

    #[test]
    fn test_extract_rgb_white() {
        assert_eq!(extract_rgb("#ffffff"), "255, 255, 255");
    }

    #[test]
    fn test_extract_rgb_invalid_falls_back() {
        // Non-6-char hex falls back to the default purple
        let result = extract_rgb("#abc");
        assert_eq!(result, "99, 102, 241");
    }

    #[test]
    fn test_extract_rgb_empty_falls_back() {
        let result = extract_rgb("");
        assert_eq!(result, "99, 102, 241");
    }

    // ── extract_asset_references ──────────────────────────────────────────────

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
    fn test_extract_asset_references_multiple_on_same_line() {
        let mut referenced = HashSet::new();
        extract_asset_references(
            "var a = 'assets/a.png'; var b = 'assets/b.png';",
            &mut referenced,
        );
        assert!(referenced.contains(&PathBuf::from("assets/a.png")));
        assert!(referenced.contains(&PathBuf::from("assets/b.png")));
    }

    #[test]
    fn test_extract_asset_references_escape_sequence() {
        // An escaped backslash inside a string should not break parsing
        let mut referenced = HashSet::new();
        extract_asset_references(r"var x = 'assets/img\\test.png';", &mut referenced);
        // The path is collected (backslash is part of the string)
        assert!(!referenced.is_empty());
    }

    #[test]
    fn test_extract_asset_references_empty_string_ignored() {
        let mut referenced = HashSet::new();
        extract_asset_references("var x = '';", &mut referenced);
        assert!(referenced.is_empty());
    }

    #[test]
    fn test_extract_asset_references_non_asset_string_ignored() {
        let mut referenced = HashSet::new();
        extract_asset_references("var x = 'images/logo.png';", &mut referenced);
        // Does NOT start with 'assets/' so should be ignored
        assert!(referenced.is_empty());
    }

    // ── parse_pubspec_assets ──────────────────────────────────────────────────

    #[test]
    fn test_parse_pubspec_assets_no_pubspec() {
        let dir = TempDir::new().unwrap();
        let result = parse_pubspec_assets(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pubspec_assets_with_assets() {
        let dir = TempDir::new().unwrap();
        let pubspec = "name: myapp\nflutter:\n  assets:\n    - assets/logo.png\n    - assets/icon.png\n";
        make_file(dir.path(), "pubspec.yaml", pubspec.as_bytes());
        let result = parse_pubspec_assets(dir.path()).unwrap();
        assert!(result.contains(&PathBuf::from("assets/logo.png")));
        assert!(result.contains(&PathBuf::from("assets/icon.png")));
    }

    #[test]
    fn test_parse_pubspec_assets_empty_assets_section() {
        let dir = TempDir::new().unwrap();
        let pubspec = "name: myapp\nflutter:\n  uses-material-design: true\n";
        make_file(dir.path(), "pubspec.yaml", pubspec.as_bytes());
        let result = parse_pubspec_assets(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pubspec_assets_comment_lines_skipped() {
        let dir = TempDir::new().unwrap();
        let pubspec =
            "name: myapp\nflutter:\n  assets:\n    # a comment\n    - assets/real.png\n";
        make_file(dir.path(), "pubspec.yaml", pubspec.as_bytes());
        let result = parse_pubspec_assets(dir.path()).unwrap();
        assert!(result.contains(&PathBuf::from("assets/real.png")));
        // Comment lines should not produce paths
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_pubspec_assets_section_ends_at_non_dash_line() {
        let dir = TempDir::new().unwrap();
        // After the assets section ends we have another key
        let pubspec =
            "name: myapp\nflutter:\n  assets:\n    - assets/img.png\n  uses-material-design: true\n";
        make_file(dir.path(), "pubspec.yaml", pubspec.as_bytes());
        let result = parse_pubspec_assets(dir.path()).unwrap();
        assert!(result.contains(&PathBuf::from("assets/img.png")));
        assert_eq!(result.len(), 1);
    }

    // ── collect_assets ────────────────────────────────────────────────────────

    #[test]
    fn test_collect_assets_finds_png() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "assets/logo.png", b"\x89PNG\r\n\x1a\n");
        let cfg = AuditConfig::default();
        let assets = collect_assets(dir.path(), &cfg).unwrap();
        assert!(!assets.is_empty());
        let names: Vec<_> = assets.iter().map(|(p, _)| p.to_string_lossy().to_string()).collect();
        assert!(names.iter().any(|n| n.contains("logo.png")));
    }

    #[test]
    fn test_collect_assets_skips_build_dir() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "build/outputs/logo.png", b"\x89PNG");
        let cfg = AuditConfig::default();
        let assets = collect_assets(dir.path(), &cfg).unwrap();
        // File inside /build/ must be excluded
        assert!(assets.is_empty());
    }

    #[test]
    fn test_collect_assets_skips_git_dir() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), ".git/objects/logo.png", b"\x89PNG");
        let cfg = AuditConfig::default();
        let assets = collect_assets(dir.path(), &cfg).unwrap();
        assert!(assets.is_empty());
    }

    #[test]
    fn test_collect_assets_ignores_dart_files() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "lib/main.dart", b"void main() {}");
        let cfg = AuditConfig::default();
        let assets = collect_assets(dir.path(), &cfg).unwrap();
        assert!(assets.is_empty());
    }

    #[test]
    fn test_collect_assets_multiple_extensions() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "assets/a.png", b"\x89PNG");
        make_file(dir.path(), "assets/b.jpg", b"\xff\xd8\xff");
        make_file(dir.path(), "assets/c.svg", b"<svg></svg>");
        let cfg = AuditConfig::default();
        let assets = collect_assets(dir.path(), &cfg).unwrap();
        assert_eq!(assets.len(), 3);
    }

    // ── find_referenced_assets ────────────────────────────────────────────────

    #[test]
    fn test_find_referenced_assets_from_dart_file() {
        let dir = TempDir::new().unwrap();
        let dart_src = "Widget build(ctx) => Image.asset('assets/images/logo.png');";
        make_file(dir.path(), "lib/main.dart", dart_src.as_bytes());
        let referenced = find_referenced_assets(dir.path()).unwrap();
        assert!(referenced.contains(&PathBuf::from("assets/images/logo.png")));
    }

    #[test]
    fn test_find_referenced_assets_skips_comments() {
        let dir = TempDir::new().unwrap();
        let dart_src = "// Image.asset('assets/hidden.png')\nvar x = 1;";
        make_file(dir.path(), "lib/widget.dart", dart_src.as_bytes());
        let referenced = find_referenced_assets(dir.path()).unwrap();
        // Commented-out references should NOT be collected
        assert!(!referenced.contains(&PathBuf::from("assets/hidden.png")));
    }

    #[test]
    fn test_find_referenced_assets_non_dart_files_ignored() {
        let dir = TempDir::new().unwrap();
        // A yaml file with an asset path should not contribute references
        make_file(
            dir.path(),
            "config.yaml",
            b"path: 'assets/background.png'",
        );
        let referenced = find_referenced_assets(dir.path()).unwrap();
        assert!(referenced.is_empty());
    }

    #[test]
    fn test_find_referenced_assets_empty_dir() {
        let dir = TempDir::new().unwrap();
        let referenced = find_referenced_assets(dir.path()).unwrap();
        assert!(referenced.is_empty());
    }

    // ── compute_file_hash ─────────────────────────────────────────────────────

    #[test]
    fn test_compute_file_hash_returns_ok() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "test.bin", b"hello world");
        let hash = compute_file_hash(&dir.path().join("test.bin"));
        assert!(hash.is_ok());
    }

    #[test]
    fn test_compute_file_hash_same_content_same_hash() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.bin", b"identical content");
        make_file(dir.path(), "b.bin", b"identical content");
        let ha = compute_file_hash(&dir.path().join("a.bin")).unwrap();
        let hb = compute_file_hash(&dir.path().join("b.bin")).unwrap();
        assert_eq!(ha, hb);
    }

    #[test]
    fn test_compute_file_hash_different_content_different_hash() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.bin", b"content A");
        make_file(dir.path(), "b.bin", b"content B");
        let ha = compute_file_hash(&dir.path().join("a.bin")).unwrap();
        let hb = compute_file_hash(&dir.path().join("b.bin")).unwrap();
        assert_ne!(ha, hb);
    }

    #[test]
    fn test_compute_file_hash_empty_file() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "empty.bin", b"");
        let hash = compute_file_hash(&dir.path().join("empty.bin"));
        assert!(hash.is_ok());
        assert_eq!(hash.unwrap(), 0);
    }

    #[test]
    fn test_compute_file_hash_nonexistent_returns_err() {
        let dir = TempDir::new().unwrap();
        let result = compute_file_hash(&dir.path().join("no_such_file.bin"));
        assert!(result.is_err());
    }

    // ── find_duplicates ───────────────────────────────────────────────────────

    #[test]
    fn test_find_duplicates_identical_files() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "assets/a.png", b"\x89PNG same content");
        make_file(dir.path(), "assets/b.png", b"\x89PNG same content");
        let assets = vec![
            (PathBuf::from("assets/a.png"), 18u64),
            (PathBuf::from("assets/b.png"), 18u64),
        ];
        let dups = find_duplicates(dir.path(), &assets).unwrap();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].len(), 2);
    }

    #[test]
    fn test_find_duplicates_no_duplicates() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "assets/a.png", b"content A");
        make_file(dir.path(), "assets/b.png", b"content B");
        let assets = vec![
            (PathBuf::from("assets/a.png"), 9u64),
            (PathBuf::from("assets/b.png"), 9u64),
        ];
        let dups = find_duplicates(dir.path(), &assets).unwrap();
        assert!(dups.is_empty());
    }

    #[test]
    fn test_find_duplicates_empty_asset_list() {
        let dir = TempDir::new().unwrap();
        let dups = find_duplicates(dir.path(), &[]).unwrap();
        assert!(dups.is_empty());
    }

    // ── check_svg_optimization ────────────────────────────────────────────────

    #[test]
    fn test_check_svg_optimization_clean_svg() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "assets/icon.svg", b"<svg><circle/></svg>");
        let paths = vec![PathBuf::from("assets/icon.svg")];
        let result = check_svg_optimization(dir.path(), &paths).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_check_svg_optimization_embedded_font() {
        let dir = TempDir::new().unwrap();
        let svg = b"<svg><defs><font-face/></font></defs></svg>";
        make_file(dir.path(), "assets/font.svg", svg);
        let paths = vec![PathBuf::from("assets/font.svg")];
        let result = check_svg_optimization(dir.path(), &paths).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].1.contains("embedded fonts"));
    }

    #[test]
    fn test_check_svg_optimization_doctype_flag() {
        let dir = TempDir::new().unwrap();
        let svg = b"<!DOCTYPE svg PUBLIC><svg></svg>";
        make_file(dir.path(), "assets/legacy.svg", svg);
        let paths = vec![PathBuf::from("assets/legacy.svg")];
        let result = check_svg_optimization(dir.path(), &paths).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].1.contains("DOCTYPE"));
    }

    #[test]
    fn test_check_svg_optimization_xml_metadata_large() {
        let dir = TempDir::new().unwrap();
        // Make content > 500 bytes containing <?xml
        let mut svg = String::from("<?xml version=\"1.0\"?><svg>");
        for _ in 0..50 {
            svg.push_str("<rect width=\"10\" height=\"10\"/>");
        }
        svg.push_str("</svg>");
        make_file(dir.path(), "assets/meta.svg", svg.as_bytes());
        let paths = vec![PathBuf::from("assets/meta.svg")];
        let result = check_svg_optimization(dir.path(), &paths).unwrap();
        assert!(!result.is_empty());
        assert!(result[0].1.contains("metadata"));
    }

    #[test]
    fn test_check_svg_optimization_missing_file_skipped() {
        let dir = TempDir::new().unwrap();
        // Path doesn't exist — should be silently skipped, not panic
        let paths = vec![PathBuf::from("assets/missing.svg")];
        let result = check_svg_optimization(dir.path(), &paths).unwrap();
        assert!(result.is_empty());
    }

    // ── audit_assets / audit_assets_with_threshold ────────────────────────────

    #[test]
    fn test_audit_assets_empty_project() {
        let dir = TempDir::new().unwrap();
        let report = audit_assets(dir.path()).unwrap();
        assert_eq!(report.total_assets, 0);
        assert_eq!(report.total_size_bytes, 0);
        assert!(report.issues.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn test_audit_assets_with_threshold_custom_kb() {
        let dir = TempDir::new().unwrap();
        // Create a 10 KB image — below 200 KB default but above 5 KB custom threshold
        let data = vec![0u8; 10 * 1024];
        make_file(dir.path(), "assets/img.png", &data);
        // Use a 5 KB threshold — should flag the file as oversized
        let report = audit_assets_with_threshold(dir.path(), 5).unwrap();
        let oversized: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.category == "Oversized")
            .collect();
        assert!(!oversized.is_empty());
    }

    #[test]
    fn test_audit_assets_counts_files_and_size() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "assets/a.png", b"\x89PNG data");
        make_file(dir.path(), "assets/b.png", b"\x89PNG data2");
        let report = audit_assets(dir.path()).unwrap();
        assert_eq!(report.total_assets, 2);
        assert!(report.total_size_bytes > 0);
    }

    #[test]
    fn test_audit_assets_score_decrements_on_issues() {
        let dir = TempDir::new().unwrap();
        // Oversized image triggers a Warning (5 deduction each)
        let data = vec![0u8; 300 * 1024]; // 300 KB > 200 KB threshold
        make_file(dir.path(), "assets/big.png", &data);
        let report = audit_assets(dir.path()).unwrap();
        assert!(report.score < 100);
    }

    #[test]
    fn test_audit_assets_unused_declared_asset() {
        let dir = TempDir::new().unwrap();
        // Declare asset in pubspec but do NOT create a Dart reference
        let pubspec =
            "name: app\nflutter:\n  assets:\n    - assets/unused.png\n";
        make_file(dir.path(), "pubspec.yaml", pubspec.as_bytes());
        // Create the actual file so it is considered "exists"
        make_file(dir.path(), "assets/unused.png", b"\x89PNG");
        let report = audit_assets(dir.path()).unwrap();
        let unused: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.category == "Unused")
            .collect();
        assert!(!unused.is_empty());
    }

    #[test]
    fn test_audit_assets_duplicate_detection() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "assets/copy1.png", b"\x89PNG identical");
        make_file(dir.path(), "assets/copy2.png", b"\x89PNG identical");
        let report = audit_assets(dir.path()).unwrap();
        let dups: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.category == "Duplicate Asset")
            .collect();
        assert!(!dups.is_empty());
    }

    #[test]
    fn test_audit_assets_webp_candidate_flagged() {
        let dir = TempDir::new().unwrap();
        // A PNG > 50 KB with no webp sibling triggers "No WebP Alternative"
        let data = vec![0u8; 60 * 1024];
        make_file(dir.path(), "assets/banner.png", &data);
        let report = audit_assets(dir.path()).unwrap();
        let no_webp: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.category == "No WebP Alternative")
            .collect();
        assert!(!no_webp.is_empty());
    }

    #[test]
    fn test_audit_assets_webp_candidate_not_flagged_when_webp_exists() {
        let dir = TempDir::new().unwrap();
        // A PNG > 50 KB AND a matching .webp file — should not be flagged
        let data = vec![0u8; 60 * 1024];
        make_file(dir.path(), "assets/banner.png", &data);
        make_file(dir.path(), "assets/banner.webp", &data);
        let report = audit_assets(dir.path()).unwrap();
        let no_webp: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.category == "No WebP Alternative")
            .collect();
        assert!(no_webp.is_empty());
    }

    #[test]
    fn test_audit_assets_svg_unoptimized_issue() {
        let dir = TempDir::new().unwrap();
        let svg = b"<svg><defs></defs></svg>";
        make_file(dir.path(), "assets/icon.svg", svg);
        let report = audit_assets(dir.path()).unwrap();
        let svg_issues: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.category == "Unoptimized SVG")
            .collect();
        assert!(!svg_issues.is_empty());
    }

    #[test]
    fn test_audit_assets_potential_savings_nonzero_for_issues() {
        let dir = TempDir::new().unwrap();
        let data = vec![0u8; 300 * 1024];
        make_file(dir.path(), "assets/bigimg.png", &data);
        let report = audit_assets(dir.path()).unwrap();
        assert!(report.potential_savings_bytes > 0);
    }

    // ── write_asset_html_report / generate_html_report ────────────────────────

    #[test]
    fn test_write_asset_html_report_creates_file() {
        let dir = TempDir::new().unwrap();
        let report = AssetAuditReport {
            total_assets: 0,
            total_size_bytes: 0,
            issues: vec![],
            potential_savings_bytes: 0,
            score: 100,
        };
        let out = dir.path().join("report.html");
        write_asset_html_report(&report, &out).unwrap();
        assert!(out.exists());
    }

    #[test]
    fn test_write_asset_html_report_contains_score() {
        let dir = TempDir::new().unwrap();
        let report = AssetAuditReport {
            total_assets: 3,
            total_size_bytes: 1024,
            issues: vec![],
            potential_savings_bytes: 0,
            score: 87,
        };
        let out = dir.path().join("report.html");
        write_asset_html_report(&report, &out).unwrap();
        let contents = fs::read_to_string(&out).unwrap();
        assert!(contents.contains("87"));
    }

    #[test]
    fn test_generate_html_report_no_issues_message() {
        let report = AssetAuditReport {
            total_assets: 0,
            total_size_bytes: 0,
            issues: vec![],
            potential_savings_bytes: 0,
            score: 100,
        };
        let html = generate_html_report(&report);
        assert!(html.contains("All assets are optimized"));
    }

    #[test]
    fn test_generate_html_report_contains_issue_category() {
        let issue = AssetIssue {
            severity: AssetSeverity::Warning,
            category: "Oversized".to_string(),
            file: PathBuf::from("assets/big.png"),
            detail: "Too large".to_string(),
            suggestion: "Compress it".to_string(),
            savings_bytes: 1024,
        };
        let report = AssetAuditReport {
            total_assets: 1,
            total_size_bytes: 5000,
            issues: vec![issue],
            potential_savings_bytes: 1024,
            score: 95,
        };
        let html = generate_html_report(&report);
        assert!(html.contains("Oversized"));
        assert!(html.contains("big.png"));
    }

    #[test]
    fn test_generate_html_report_score_color_green() {
        let report = AssetAuditReport {
            total_assets: 0,
            total_size_bytes: 0,
            issues: vec![],
            potential_savings_bytes: 0,
            score: 95,
        };
        let html = generate_html_report(&report);
        // Green color hex
        assert!(html.contains("#10b981"));
    }

    #[test]
    fn test_generate_html_report_score_color_yellow() {
        let mut issues = Vec::new();
        // Create enough warnings to push score into 70-89 range
        for i in 0..4 {
            issues.push(AssetIssue {
                severity: AssetSeverity::Warning,
                category: "Oversized".to_string(),
                file: PathBuf::from(format!("assets/img{}.png", i)),
                detail: "too big".to_string(),
                suggestion: "compress".to_string(),
                savings_bytes: 0,
            });
        }
        let report = AssetAuditReport {
            total_assets: 4,
            total_size_bytes: 1000,
            issues,
            potential_savings_bytes: 0,
            score: 80, // Manually set for simplicity
        };
        let html = generate_html_report(&report);
        assert!(html.contains("#f59e0b"));
    }

    #[test]
    fn test_generate_html_report_score_color_red() {
        let report = AssetAuditReport {
            total_assets: 0,
            total_size_bytes: 0,
            issues: vec![],
            potential_savings_bytes: 0,
            score: 60,
        };
        let html = generate_html_report(&report);
        assert!(html.contains("#ef4444"));
    }

    #[test]
    fn test_generate_html_report_severity_counts() {
        let issues = vec![
            AssetIssue {
                severity: AssetSeverity::Error,
                category: "Cat".to_string(),
                file: PathBuf::from("a.png"),
                detail: "d".to_string(),
                suggestion: "s".to_string(),
                savings_bytes: 0,
            },
            AssetIssue {
                severity: AssetSeverity::Warning,
                category: "Cat".to_string(),
                file: PathBuf::from("b.png"),
                detail: "d".to_string(),
                suggestion: "s".to_string(),
                savings_bytes: 0,
            },
            AssetIssue {
                severity: AssetSeverity::Info,
                category: "Cat".to_string(),
                file: PathBuf::from("c.png"),
                detail: "d".to_string(),
                suggestion: "s".to_string(),
                savings_bytes: 0,
            },
        ];
        let report = AssetAuditReport {
            total_assets: 3,
            total_size_bytes: 0,
            issues,
            potential_savings_bytes: 0,
            score: 83,
        };
        let html = generate_html_report(&report);
        // All three severity badges should appear
        assert!(html.contains("Error"));
        assert!(html.contains("Warning"));
        assert!(html.contains("Info"));
    }
}
