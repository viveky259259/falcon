//! Build Optimizer — analyze build configuration, asset sizes,
//! and provide optimization recommendations.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub asset_analysis: AssetAnalysis,
    pub pubspec_analysis: PubspecAnalysis,
    pub recommendations: Vec<BuildRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAnalysis {
    pub total_assets: usize,
    pub total_size_kb: u64,
    pub large_assets: Vec<(String, u64)>,
    pub image_count: usize,
    pub font_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubspecAnalysis {
    pub has_flutter_section: bool,
    pub uses_deferred_components: bool,
    pub has_tree_shake_icons: bool,
    pub platform_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRecommendation {
    pub category: String,
    pub recommendation: String,
    pub impact: String,
}

/// Analyze build configuration and assets.
pub fn analyze_build(root: &Path) -> anyhow::Result<BuildReport> {
    let asset_analysis = analyze_assets(root);
    let pubspec_analysis = analyze_pubspec(root);

    let mut recommendations = Vec::new();

    for (name, size_kb) in &asset_analysis.large_assets {
        if *size_kb > 500 {
            recommendations.push(BuildRecommendation {
                category: "Assets".to_string(),
                recommendation: format!(
                    "'{}' is {}KB — compress or use a lower resolution",
                    name, size_kb
                ),
                impact: "Reduces APK/IPA size".to_string(),
            });
        }
    }

    if asset_analysis.image_count > 50 {
        recommendations.push(BuildRecommendation {
            category: "Assets".to_string(),
            recommendation:
                "50+ images bundled — consider lazy loading or CDN for non-critical images"
                    .to_string(),
            impact: "Reduces initial download size".to_string(),
        });
    }

    if !pubspec_analysis.has_tree_shake_icons {
        recommendations.push(BuildRecommendation {
            category: "Build Config".to_string(),
            recommendation: "Enable tree-shaking for icons: flutter: uses-material-design: true with --tree-shake-icons".to_string(),
            impact: "Removes unused Material icons (~1MB savings)".to_string(),
        });
    }

    if !pubspec_analysis.uses_deferred_components {
        recommendations.push(BuildRecommendation {
            category: "Build Config".to_string(),
            recommendation: "Consider deferred components for large apps — loads features on demand".to_string(),
            impact: "Reduces initial app size".to_string(),
        });
    }

    let dart_files: usize = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .count();

    if dart_files > 200 {
        recommendations.push(BuildRecommendation {
            category: "Code Size".to_string(),
            recommendation: format!(
                "{} Dart files — consider code splitting with deferred imports",
                dart_files
            ),
            impact: "Faster cold start on web, smaller initial payload".to_string(),
        });
    }

    Ok(BuildReport {
        asset_analysis,
        pubspec_analysis,
        recommendations,
    })
}

fn analyze_assets(root: &Path) -> AssetAnalysis {
    let assets_dir = root.join("assets");
    let mut total = 0;
    let mut total_size: u64 = 0;
    let mut large = Vec::new();
    let mut images = 0;
    let mut fonts = 0;

    if assets_dir.exists() {
        for entry in walkdir::WalkDir::new(&assets_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            total += 1;
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0) / 1024;
            total_size += size;

            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            match ext {
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => images += 1,
                "ttf" | "otf" | "woff" | "woff2" => fonts += 1,
                _ => {}
            }

            if size > 200 {
                let name = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .to_string();
                large.push((name, size));
            }
        }
    }

    large.sort_by(|a, b| b.1.cmp(&a.1));

    AssetAnalysis {
        total_assets: total,
        total_size_kb: total_size,
        large_assets: large,
        image_count: images,
        font_count: fonts,
    }
}

fn analyze_pubspec(root: &Path) -> PubspecAnalysis {
    let pubspec = root.join("pubspec.yaml");
    let content = std::fs::read_to_string(&pubspec).unwrap_or_default();

    PubspecAnalysis {
        has_flutter_section: content.contains("flutter:"),
        uses_deferred_components: content.contains("deferred-components"),
        has_tree_shake_icons: content.contains("tree-shake-icons")
            || content.contains("uses-material-design: true"),
        platform_count: {
            let mut count = 0;
            if root.join("android").exists() {
                count += 1;
            }
            if root.join("ios").exists() {
                count += 1;
            }
            if root.join("web").exists() {
                count += 1;
            }
            if root.join("macos").exists() {
                count += 1;
            }
            if root.join("linux").exists() {
                count += 1;
            }
            if root.join("windows").exists() {
                count += 1;
            }
            count
        },
    }
}

/// Print build optimization report.
pub fn print_build_report(report: &BuildReport) {
    println!();
    println!("  {} Build Optimizer", "falcon manage".bright_cyan().bold());
    println!();

    println!(
        "  Assets: {} files ({} KB total, {} images, {} fonts)",
        report.asset_analysis.total_assets,
        report.asset_analysis.total_size_kb,
        report.asset_analysis.image_count,
        report.asset_analysis.font_count
    );
    println!("  Platforms: {}", report.pubspec_analysis.platform_count);

    if !report.asset_analysis.large_assets.is_empty() {
        println!();
        println!("  {} Large assets:", "▸".yellow());
        for (name, size) in report.asset_analysis.large_assets.iter().take(5) {
            println!("    {} {} ({}KB)", "·".dimmed(), name.bright_white(), size);
        }
    }

    if !report.recommendations.is_empty() {
        println!();
        println!("  {} Recommendations:", "▸".green());
        for r in &report.recommendations {
            println!(
                "    {} [{}] {}",
                "→".green(),
                r.category.bright_cyan(),
                r.recommendation
            );
            println!("       Impact: {}", r.impact.dimmed());
        }
    } else {
        println!();
        println!("  {} Build configuration looks good!", "✓".green().bold());
    }

    println!();
}
