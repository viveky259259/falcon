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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_snapshot(analysis_time_ms: u128, lines_per_second: f64) -> PerfSnapshot {
        PerfSnapshot {
            timestamp: "2024-01-01T00:00:00".to_string(),
            falcon_version: "0.0.0".to_string(),
            file_count: 10,
            total_lines: 1000,
            analysis_time_ms,
            files_per_second: 100.0,
            lines_per_second,
            issue_count: 5,
            git_commit: None,
        }
    }

    fn make_snapshot_with_commit(ms: u128, commit: &str) -> PerfSnapshot {
        let mut s = make_snapshot(ms, 1000.0);
        s.git_commit = Some(commit.to_string());
        s
    }

    // ── PerfHistory::default ─────────────────────────────────────────────────

    #[test]
    fn test_perf_history_default_empty() {
        let h = PerfHistory::default();
        assert!(h.snapshots.is_empty());
    }

    #[test]
    fn test_perf_history_default_twice_independent() {
        let a = PerfHistory::default();
        let b = PerfHistory::default();
        assert_eq!(a.snapshots.len(), b.snapshots.len());
    }

    // ── save_perf_snapshot ───────────────────────────────────────────────────

    #[test]
    fn test_save_creates_data_dir_and_file() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let snap = make_snapshot(500, 2000.0);
        save_perf_snapshot(root, &snap).unwrap();

        let path = root.join(".falcon-data/perf-history.json");
        assert!(path.exists(), "history file should exist after save");
    }

    #[test]
    fn test_save_writes_valid_json() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let snap = make_snapshot(123, 999.0);
        save_perf_snapshot(root, &snap).unwrap();

        let content = std::fs::read_to_string(root.join(".falcon-data/perf-history.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("snapshots").is_some());
    }

    #[test]
    fn test_save_appends_on_second_call() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        save_perf_snapshot(root, &make_snapshot(100, 500.0)).unwrap();
        save_perf_snapshot(root, &make_snapshot(200, 600.0)).unwrap();

        let history = load_perf_history(root).unwrap();
        assert_eq!(history.snapshots.len(), 2);
    }

    #[test]
    fn test_save_preserves_snapshot_fields() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let snap = make_snapshot_with_commit(750, "abc1234");
        save_perf_snapshot(root, &snap).unwrap();

        let history = load_perf_history(root).unwrap();
        let loaded = &history.snapshots[0];
        assert_eq!(loaded.analysis_time_ms, 750);
        assert_eq!(loaded.git_commit.as_deref(), Some("abc1234"));
        assert_eq!(loaded.file_count, 10);
        assert_eq!(loaded.total_lines, 1000);
    }

    #[test]
    fn test_save_multiple_snapshots_order_preserved() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        for ms in [10u128, 20, 30, 40, 50] {
            save_perf_snapshot(root, &make_snapshot(ms, 1000.0)).unwrap();
        }
        let history = load_perf_history(root).unwrap();
        assert_eq!(history.snapshots.len(), 5);
        let times: Vec<u128> = history
            .snapshots
            .iter()
            .map(|s| s.analysis_time_ms)
            .collect();
        assert_eq!(times, vec![10, 20, 30, 40, 50]);
    }

    // ── load_perf_history ────────────────────────────────────────────────────

    #[test]
    fn test_load_returns_default_when_file_absent() {
        let tmp = TempDir::new().unwrap();
        let history = load_perf_history(tmp.path()).unwrap();
        assert!(history.snapshots.is_empty());
    }

    #[test]
    fn test_load_reads_saved_history() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let snap = make_snapshot(300, 1500.0);
        save_perf_snapshot(root, &snap).unwrap();

        let loaded = load_perf_history(root).unwrap();
        assert_eq!(loaded.snapshots.len(), 1);
        assert_eq!(loaded.snapshots[0].analysis_time_ms, 300);
        assert!((loaded.snapshots[0].lines_per_second - 1500.0).abs() < 1e-6);
    }

    #[test]
    fn test_load_invalid_json_returns_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".falcon-data")).unwrap();
        std::fs::write(root.join(".falcon-data/perf-history.json"), b"not json").unwrap();

        let result = load_perf_history(root);
        assert!(result.is_err(), "should fail on invalid JSON");
    }

    #[test]
    fn test_load_empty_snapshots_array() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".falcon-data")).unwrap();
        std::fs::write(
            root.join(".falcon-data/perf-history.json"),
            b"{\"snapshots\":[]}",
        )
        .unwrap();

        let history = load_perf_history(root).unwrap();
        assert!(history.snapshots.is_empty());
    }

    #[test]
    fn test_load_snapshot_with_null_git_commit() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let mut snap = make_snapshot(200, 800.0);
        snap.git_commit = None;
        save_perf_snapshot(root, &snap).unwrap();

        let history = load_perf_history(root).unwrap();
        assert!(history.snapshots[0].git_commit.is_none());
    }

    // ── check_regression ────────────────────────────────────────────────────

    #[test]
    fn test_check_regression_empty_history_returns_none() {
        let history = PerfHistory::default();
        assert!(check_regression(&history).is_none());
    }

    #[test]
    fn test_check_regression_single_snapshot_returns_none() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(100, 1000.0));
        assert!(check_regression(&h).is_none());
    }

    #[test]
    fn test_check_regression_previous_zero_ms_returns_none() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(0, 1000.0));
        h.snapshots.push(make_snapshot(200, 500.0));
        assert!(check_regression(&h).is_none());
    }

    #[test]
    fn test_check_regression_detects_regression_above_20pct() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(100, 2000.0));
        h.snapshots.push(make_snapshot(125, 1600.0)); // 25% slower
        let result = check_regression(&h).unwrap();
        assert!(result.is_regression, "25% increase should be a regression");
        assert!((result.time_change_pct - 25.0).abs() < 1e-6);
    }

    #[test]
    fn test_check_regression_no_regression_within_20pct() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(100, 2000.0));
        h.snapshots.push(make_snapshot(115, 1740.0)); // 15% slower — within threshold
        let result = check_regression(&h).unwrap();
        assert!(!result.is_regression);
        assert!((result.time_change_pct - 15.0).abs() < 1e-6);
    }

    #[test]
    fn test_check_regression_exactly_20pct_not_regression() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(100, 2000.0));
        h.snapshots.push(make_snapshot(120, 1666.0)); // exactly 20%
        let result = check_regression(&h).unwrap();
        // boundary: 20.0 is NOT > 20.0
        assert!(!result.is_regression);
    }

    #[test]
    fn test_check_regression_improvement() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(200, 1000.0));
        h.snapshots.push(make_snapshot(100, 2000.0)); // 50% faster
        let result = check_regression(&h).unwrap();
        assert!(!result.is_regression);
        assert!(result.time_change_pct < 0.0, "time should have decreased");
    }

    #[test]
    fn test_check_regression_uses_last_two_snapshots() {
        let mut h = PerfHistory::default();
        // Three snapshots — only last two matter
        h.snapshots.push(make_snapshot(50, 5000.0));
        h.snapshots.push(make_snapshot(100, 2000.0));
        h.snapshots.push(make_snapshot(130, 1538.0)); // 30% slower than middle
        let result = check_regression(&h).unwrap();
        assert!(result.is_regression);
        assert_eq!(result.previous_ms, 100);
        assert_eq!(result.current_ms, 130);
    }

    #[test]
    fn test_check_regression_throughput_change_pct_positive_when_slower() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(100, 2000.0));
        h.snapshots.push(make_snapshot(150, 1000.0)); // throughput halved
        let result = check_regression(&h).unwrap();
        assert!(result.throughput_change_pct < 0.0, "throughput should drop");
    }

    #[test]
    fn test_check_regression_zero_previous_throughput_gives_zero_pct() {
        let mut h = PerfHistory::default();
        h.snapshots.push(make_snapshot(100, 0.0)); // previous lines_per_second = 0
        h.snapshots.push(make_snapshot(200, 1000.0));
        let result = check_regression(&h).unwrap();
        assert!((result.throughput_change_pct - 0.0).abs() < 1e-6);
    }
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
