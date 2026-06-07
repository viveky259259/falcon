use colored::Colorize;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Debug)]
pub struct CompareResult {
    pub falcon_issues: usize,
    pub falcon_time_ms: u128,
    pub dart_analyze_issues: usize,
    pub dart_analyze_time_ms: u128,
    pub falcon_unique: Vec<String>,
    pub overlap_categories: Vec<(String, usize)>,
    pub file_count: usize,
}

/// Compare Falcon analysis with `dart analyze` on the same project.
pub fn compare_with_dart_analyze(root: &Path) -> anyhow::Result<CompareResult> {
    let config = crate::config::FalconConfig::load(root)?;

    let falcon_start = Instant::now();
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;
    let falcon_time = falcon_start.elapsed();

    let falcon_issues = report.issues.len();
    let file_count = report.file_count;

    let mut rule_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for issue in &report.issues {
        *rule_counts.entry(issue.rule.clone()).or_default() += 1;
    }

    let dart_start = Instant::now();
    let dart_output = Command::new("dart")
        .args(["analyze", "--format", "json"])
        .current_dir(root)
        .output();
    let dart_time = dart_start.elapsed();

    let dart_analyze_issues = match dart_output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            count_dart_analyze_issues(&stdout)
        }
        Err(_) => 0,
    };

    let falcon_unique = vec![
        "avoid-empty-catch".to_string(),
        "avoid-print-in-production".to_string(),
        "avoid-hardcoded-credentials".to_string(),
        "ensure-dispose-lifecycle".to_string(),
        "avoid-unawaited-futures".to_string(),
        "ensure-stream-subscription-cancel".to_string(),
        "prefer-specific-catch-type".to_string(),
        "avoid-excessive-widget-nesting".to_string(),
        "cognitive-complexity".to_string(),
        "widget-rebuild-detection".to_string(),
        "layer-enforcement".to_string(),
        "codebase-intelligence".to_string(),
    ];

    let mut overlap_categories: Vec<(String, usize)> = rule_counts.into_iter().collect();
    overlap_categories.sort_by_key(|e| std::cmp::Reverse(e.1));
    overlap_categories.truncate(15);

    Ok(CompareResult {
        falcon_issues,
        falcon_time_ms: falcon_time.as_millis(),
        dart_analyze_issues,
        dart_analyze_time_ms: dart_time.as_millis(),
        falcon_unique,
        overlap_categories,
        file_count,
    })
}

fn count_dart_analyze_issues(output: &str) -> usize {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(output) {
        if let Some(diagnostics) = parsed.get("diagnostics").and_then(|d| d.as_array()) {
            return diagnostics.len();
        }
    }
    output
        .lines()
        .filter(|l| l.contains("info") || l.contains("warning") || l.contains("error"))
        .count()
}

pub fn print_compare_result(result: &CompareResult) {
    println!();
    println!("  {} Falcon vs dart analyze", "falcon".bright_cyan().bold());
    println!();

    println!("  Codebase: {} files", result.file_count);
    println!();

    println!("  ┌─────────────────────┬──────────────┬──────────────┐");
    println!(
        "  │                     │ {} │ {} │",
        "Falcon".bright_cyan().bold(),
        "dart analyze".bright_yellow().bold(),
    );
    println!("  ├─────────────────────┼──────────────┼──────────────┤");
    println!(
        "  │ Issues found        │ {:<12} │ {:<12} │",
        result.falcon_issues, result.dart_analyze_issues,
    );
    println!(
        "  │ Time                │ {:<10} ms │ {:<10} ms │",
        result.falcon_time_ms, result.dart_analyze_time_ms,
    );
    println!("  └─────────────────────┴──────────────┴──────────────┘");
    println!();

    let speed_ratio = if result.dart_analyze_time_ms > 0 {
        result.dart_analyze_time_ms as f64 / result.falcon_time_ms.max(1) as f64
    } else {
        1.0
    };

    if speed_ratio > 1.0 {
        println!(
            "  {} Falcon is {:.1}x faster than dart analyze",
            "⚡".bright_yellow(),
            speed_ratio
        );
    }

    if result.falcon_issues > result.dart_analyze_issues {
        println!(
            "  {} Falcon found {} more issues",
            "🔍".bright_cyan(),
            result.falcon_issues - result.dart_analyze_issues
        );
    }

    println!();
    println!("  Falcon-exclusive capabilities:");
    for cap in &result.falcon_unique {
        println!("    {} {}", "+".green(), cap.bright_white());
    }

    println!();
    println!("  Top issues by rule:");
    for (rule, count) in &result.overlap_categories {
        let bar_len = (*count as f64 / result.overlap_categories[0].1 as f64 * 20.0) as usize;
        let bar = "█".repeat(bar_len.max(1));
        println!(
            "    {:<45} {} {}",
            rule.bright_white(),
            bar.bright_cyan(),
            count
        );
    }
    println!();
}
