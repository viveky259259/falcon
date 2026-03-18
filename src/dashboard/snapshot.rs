use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const DATA_DIR: &str = ".falcon-data";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSnapshot {
    pub timestamp: String,
    pub commit_hash: Option<String>,
    pub commit_message: Option<String>,
    pub branch: Option<String>,
    pub file_count: usize,
    pub total_lines: usize,
    pub health_score: f64,
    pub issues: IssueSummary,
    pub metrics_summary: MetricsSummary,
    pub rule_counts: HashMap<String, usize>,
    pub per_package: Vec<PackageSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub avg_cyclomatic: f64,
    pub max_cyclomatic: u32,
    pub avg_maintainability: f64,
    pub avg_lines_per_file: f64,
    pub god_file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSnapshot {
    pub name: String,
    pub file_count: usize,
    pub health_score: f64,
    pub issue_count: usize,
}

impl AnalysisSnapshot {
    pub fn capture(
        report: &crate::reporters::AnalysisReport,
        root: &Path,
    ) -> Self {
        let commit_info = get_git_info(root);
        let total_lines: usize = report
            .metrics
            .iter()
            .map(|(_, m)| m.file_lines_of_code as usize)
            .sum();

        let mut rule_counts: HashMap<String, usize> = HashMap::new();
        for issue in &report.issues {
            *rule_counts.entry(issue.rule.clone()).or_default() += 1;
        }

        let (avg_cc, max_cc, avg_mi) = compute_metrics_summary(&report.metrics);

        let god_file_count = report
            .metrics
            .iter()
            .filter(|(_, m)| m.file_lines_of_code > 500)
            .count();

        let avg_lines = if report.file_count > 0 {
            total_lines as f64 / report.file_count as f64
        } else {
            0.0
        };

        let health = compute_health(avg_mi, avg_cc, god_file_count, report.file_count);

        AnalysisSnapshot {
            timestamp: chrono_now(),
            commit_hash: commit_info.0,
            commit_message: commit_info.1,
            branch: commit_info.2,
            file_count: report.file_count,
            total_lines,
            health_score: health,
            issues: IssueSummary {
                errors: report.error_count(),
                warnings: report.warning_count(),
                info: report.info_count(),
                total: report.issues.len(),
            },
            metrics_summary: MetricsSummary {
                avg_cyclomatic: avg_cc,
                max_cyclomatic: max_cc,
                avg_maintainability: avg_mi,
                avg_lines_per_file: avg_lines,
                god_file_count,
            },
            rule_counts,
            per_package: Vec::new(),
        }
    }
}

fn compute_metrics_summary(
    metrics: &[(PathBuf, crate::metrics::MetricsResults)],
) -> (f64, u32, f64) {
    let mut total_cc = 0u64;
    let mut max_cc = 0u32;
    let mut total_mi = 0.0f64;
    let mut func_count = 0usize;

    for (_, m) in metrics {
        for f in &m.functions {
            total_cc += f.cyclomatic_complexity as u64;
            if f.cyclomatic_complexity > max_cc {
                max_cc = f.cyclomatic_complexity;
            }
            total_mi += f.maintainability_index;
            func_count += 1;
        }
    }

    let avg_cc = if func_count > 0 {
        total_cc as f64 / func_count as f64
    } else {
        0.0
    };
    let avg_mi = if func_count > 0 {
        total_mi / func_count as f64
    } else {
        100.0
    };

    (avg_cc, max_cc, avg_mi)
}

fn compute_health(avg_mi: f64, avg_cc: f64, god_files: usize, total_files: usize) -> f64 {
    let mi_score = (avg_mi / 100.0 * 40.0).min(40.0);
    let cc_score = ((20.0 - avg_cc) / 20.0 * 30.0).clamp(0.0, 30.0);
    let god_ratio = if total_files > 0 {
        god_files as f64 / total_files as f64
    } else {
        0.0
    };
    let structure_score = ((1.0 - god_ratio) * 30.0).clamp(0.0, 30.0);
    (mi_score + cc_score + structure_score).clamp(0.0, 100.0)
}

fn get_git_info(root: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let hash = std::process::Command::new("git")
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

    let message = std::process::Command::new("git")
        .args(["log", "-1", "--format=%s"])
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

    let branch = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
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

    (hash, message, branch)
}

fn chrono_now() -> String {
    let output = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

/// Save a snapshot to .falcon-data/history.json.
pub fn save_snapshot(root: &Path, snapshot: &AnalysisSnapshot) -> anyhow::Result<PathBuf> {
    let data_dir = root.join(DATA_DIR);
    std::fs::create_dir_all(&data_dir)?;

    let history_path = data_dir.join("history.json");
    let mut history = load_history(root)?;
    history.push(snapshot.clone());

    if history.len() > 500 {
        history.drain(0..history.len() - 500);
    }

    let json = serde_json::to_string_pretty(&history)?;
    std::fs::write(&history_path, json)?;

    Ok(history_path)
}

/// Load snapshot history from .falcon-data/history.json.
pub fn load_history(root: &Path) -> anyhow::Result<Vec<AnalysisSnapshot>> {
    let path = root.join(DATA_DIR).join("history.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let history: Vec<AnalysisSnapshot> = serde_json::from_str(&content)?;
    Ok(history)
}
