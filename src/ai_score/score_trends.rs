use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

const SCORE_HISTORY_FILE: &str = ".falcon-data/score-history.json";

/// A snapshot of AI code quality score at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreSnapshot {
    pub timestamp: String,
    pub overall: u32,
    pub resource_safety: u32,
    pub error_handling: u32,
    pub type_safety: u32,
    pub security: u32,
    pub convention_match: u32,
    pub complexity: u32,
    pub file_count: usize,
    pub total_issues: usize,
    pub grade: String,
    pub git_commit: Option<String>,
}

/// History of score snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoreHistory {
    pub snapshots: Vec<ScoreSnapshot>,
}

/// Load score history from disk.
pub fn load_score_history(root: &Path) -> anyhow::Result<ScoreHistory> {
    let path = root.join(SCORE_HISTORY_FILE);
    if !path.exists() {
        return Ok(ScoreHistory::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let history: ScoreHistory = serde_json::from_str(&content)?;
    Ok(history)
}

/// Save score history to disk.
pub fn save_score_history(root: &Path, history: &ScoreHistory) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let path = root.join(SCORE_HISTORY_FILE);
    let json = serde_json::to_string_pretty(history)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Record a new score snapshot.
pub fn record_score(root: &Path) -> anyhow::Result<ScoreSnapshot> {
    let score = crate::ai_score::score::calculate_ai_score(root)?;

    let timestamp = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        });

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

    let snapshot = ScoreSnapshot {
        timestamp,
        overall: score.overall,
        resource_safety: score.resource_safety.score,
        error_handling: score.error_handling.score,
        type_safety: score.type_safety.score,
        security: score.security.score,
        convention_match: score.convention_match.score,
        complexity: score.complexity.score,
        file_count: score.file_count,
        total_issues: score.total_issues,
        grade: score.grade.to_string(),
        git_commit,
    };

    let mut history = load_score_history(root)?;
    history.snapshots.push(snapshot.clone());
    save_score_history(root, &history)?;

    Ok(snapshot)
}

/// Score delta between two snapshots.
#[derive(Debug)]
pub struct ScoreDelta {
    pub overall: i32,
    pub resource_safety: i32,
    pub error_handling: i32,
    pub type_safety: i32,
    pub security: i32,
    pub convention_match: i32,
    pub complexity: i32,
    pub issue_delta: i32,
}

/// Compare two score snapshots.
pub fn compare_scores(old: &ScoreSnapshot, new: &ScoreSnapshot) -> ScoreDelta {
    ScoreDelta {
        overall: new.overall as i32 - old.overall as i32,
        resource_safety: new.resource_safety as i32 - old.resource_safety as i32,
        error_handling: new.error_handling as i32 - old.error_handling as i32,
        type_safety: new.type_safety as i32 - old.type_safety as i32,
        security: new.security as i32 - old.security as i32,
        convention_match: new.convention_match as i32 - old.convention_match as i32,
        complexity: new.complexity as i32 - old.complexity as i32,
        issue_delta: new.total_issues as i32 - old.total_issues as i32,
    }
}

/// Print score history.
pub fn print_score_history(history: &ScoreHistory, last: usize) {
    println!();
    println!(
        "  {} AI Score Trends ({} snapshots)",
        "falcon".bright_cyan().bold(),
        history.snapshots.len()
    );
    println!();

    if history.snapshots.is_empty() {
        println!("  No score history yet. Run: falcon score-track");
        println!();
        return;
    }

    println!(
        "  {:<20} {:<6} {:<5} {:<5} {:<5} {:<5} {:<5} {:<5} {:<7} {:<7}",
        "Timestamp", "Score", "Res", "Err", "Type", "Sec", "Conv", "Cplx", "Issues", "Commit"
    );
    println!("  {}", "─".repeat(85));

    for snap in history.snapshots.iter().rev().take(last) {
        let score_color = if snap.overall >= 85 {
            snap.overall.to_string().bright_green()
        } else if snap.overall >= 70 {
            snap.overall.to_string().yellow()
        } else {
            snap.overall.to_string().red()
        };

        let commit = snap.git_commit.as_deref().unwrap_or("—");
        println!(
            "  {:<20} {:<6} {:<5} {:<5} {:<5} {:<5} {:<5} {:<5} {:<7} {:<7}",
            snap.timestamp,
            score_color,
            snap.resource_safety,
            snap.error_handling,
            snap.type_safety,
            snap.security,
            snap.convention_match,
            snap.complexity,
            snap.total_issues,
            commit
        );
    }

    if history.snapshots.len() >= 2 {
        let latest = &history.snapshots[history.snapshots.len() - 1];
        let previous = &history.snapshots[history.snapshots.len() - 2];
        let delta = compare_scores(previous, latest);

        println!();
        let trend = if delta.overall > 0 {
            format!("↑ +{}", delta.overall).bright_green()
        } else if delta.overall < 0 {
            format!("↓ {}", delta.overall).red()
        } else {
            "→ no change".dimmed()
        };
        println!("  Trend: {} (vs. previous snapshot)", trend);
    }

    println!();
}
