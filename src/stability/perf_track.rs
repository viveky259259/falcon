use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

const PERF_HISTORY_FILE: &str = ".falcon-data/perf-history.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfSnapshot {
    pub timestamp: String,
    pub falcon_version: String,
    pub file_count: usize,
    pub total_lines: usize,
    pub analysis_time_ms: u128,
    pub files_per_second: f64,
    pub lines_per_second: f64,
    pub issue_count: usize,
    pub git_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerfHistory {
    pub snapshots: Vec<PerfSnapshot>,
}

/// Capture a performance snapshot for the project.
pub fn capture_perf_snapshot(root: &Path) -> anyhow::Result<PerfSnapshot> {
    let benchmark = crate::benchmark::run_benchmark(root)?;

    let git_commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    let timestamp = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        });

    Ok(PerfSnapshot {
        timestamp,
        falcon_version: env!("CARGO_PKG_VERSION").to_string(),
        file_count: benchmark.file_count,
        total_lines: benchmark.total_lines,
        analysis_time_ms: benchmark.total_time_ms,
        files_per_second: benchmark.files_per_second,
        lines_per_second: benchmark.lines_per_second,
        issue_count: benchmark.issue_count,
        git_commit,
    })
}

/// Save a perf snapshot to history.
pub fn save_perf_snapshot(root: &Path, snapshot: &PerfSnapshot) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;

    let history_path = root.join(PERF_HISTORY_FILE);
    let mut history = load_perf_history(root)?;
    history.snapshots.push(snapshot.clone());

    let json = serde_json::to_string_pretty(&history)?;
    std::fs::write(&history_path, json)?;
    Ok(())
}

/// Load perf history.
pub fn load_perf_history(root: &Path) -> anyhow::Result<PerfHistory> {
    let history_path = root.join(PERF_HISTORY_FILE);
    if !history_path.exists() {
        return Ok(PerfHistory::default());
    }
    let content = std::fs::read_to_string(&history_path)?;
    let history: PerfHistory = serde_json::from_str(&content)?;
    Ok(history)
}

/// Check for performance regression.
pub fn check_regression(history: &PerfHistory) -> Option<PerfRegression> {
    if history.snapshots.len() < 2 {
        return None;
    }

    let latest = &history.snapshots[history.snapshots.len() - 1];
    let previous = &history.snapshots[history.snapshots.len() - 2];

    if previous.analysis_time_ms == 0 {
        return None;
    }

    let time_change_pct = ((latest.analysis_time_ms as f64 - previous.analysis_time_ms as f64)
        / previous.analysis_time_ms as f64)
        * 100.0;

    let throughput_change_pct = if previous.lines_per_second > 0.0 {
        ((latest.lines_per_second - previous.lines_per_second) / previous.lines_per_second) * 100.0
    } else {
        0.0
    };

    let is_regression = time_change_pct > 20.0;

    Some(PerfRegression {
        time_change_pct,
        throughput_change_pct,
        is_regression,
        previous_ms: previous.analysis_time_ms,
        current_ms: latest.analysis_time_ms,
    })
}

#[derive(Debug)]
pub struct PerfRegression {
    pub time_change_pct: f64,
    pub throughput_change_pct: f64,
    pub is_regression: bool,
    pub previous_ms: u128,
    pub current_ms: u128,
}

pub fn print_perf_history(history: &PerfHistory, last_n: usize) {
    println!();
    println!("  {} Performance History", "falcon".bright_cyan().bold());
    println!();

    let snapshots = if history.snapshots.len() > last_n {
        &history.snapshots[history.snapshots.len() - last_n..]
    } else {
        &history.snapshots
    };

    if snapshots.is_empty() {
        println!("  No performance data recorded yet.");
        println!("  Run: falcon perf-track <path>");
        println!();
        return;
    }

    println!(
        "  {:<22} {:>8} {:>8} {:>10} {:>12} {:>10}",
        "Timestamp".bright_white().bold(),
        "Files".dimmed(),
        "Lines".dimmed(),
        "Time".dimmed(),
        "Lines/sec".dimmed(),
        "Commit".dimmed()
    );
    println!("  {}", "─".repeat(76));

    for snap in snapshots {
        let commit = snap.git_commit.as_deref().unwrap_or("-");
        println!(
            "  {:<22} {:>8} {:>8} {:>8}ms {:>12.0} {:>10}",
            snap.timestamp.bright_white(),
            snap.file_count,
            snap.total_lines,
            snap.analysis_time_ms,
            snap.lines_per_second,
            commit.dimmed()
        );
    }

    if let Some(regression) = check_regression(history) {
        println!();
        if regression.is_regression {
            println!(
                "  {} Performance regression detected: {:.1}% slower ({} ms → {} ms)",
                "⚠".red().bold(),
                regression.time_change_pct,
                regression.previous_ms,
                regression.current_ms
            );
        } else if regression.time_change_pct < -5.0 {
            println!(
                "  {} Performance improved: {:.1}% faster ({} ms → {} ms)",
                "✓".green().bold(),
                -regression.time_change_pct,
                regression.previous_ms,
                regression.current_ms
            );
        } else {
            println!(
                "  {} Performance stable: {:.1}% change",
                "✓".green().bold(),
                regression.time_change_pct
            );
        }
    }

    println!();
}
