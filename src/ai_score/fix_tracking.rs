//! Fix Effectiveness Tracking — track which auto-fixes teams accept vs reject.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const FIX_HISTORY_FILE: &str = ".falcon-data/fix-history.json";

/// Record of a fix application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRecord {
    pub rule: String,
    pub file: String,
    pub timestamp: String,
    pub outcome: FixOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FixOutcome {
    Accepted,
    Rejected,
    Modified,
}

impl std::fmt::Display for FixOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixOutcome::Accepted => write!(f, "accepted"),
            FixOutcome::Rejected => write!(f, "rejected"),
            FixOutcome::Modified => write!(f, "modified"),
        }
    }
}

/// Fix history database.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FixHistory {
    pub records: Vec<FixRecord>,
}

/// Per-rule fix effectiveness stats.
#[derive(Debug, Clone)]
pub struct FixEffectiveness {
    pub rule: String,
    pub total_fixes: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub modified: usize,
    pub acceptance_rate: f64,
}

/// Load fix history.
pub fn load_fix_history(root: &Path) -> anyhow::Result<FixHistory> {
    let path = root.join(FIX_HISTORY_FILE);
    if !path.exists() {
        return Ok(FixHistory::default());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Save fix history.
pub fn save_fix_history(root: &Path, history: &FixHistory) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let path = root.join(FIX_HISTORY_FILE);
    let json = serde_json::to_string_pretty(history)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Record a fix outcome.
pub fn record_fix(root: &Path, rule: &str, file: &str, outcome: FixOutcome) -> anyhow::Result<()> {
    let mut history = load_fix_history(root)?;

    let timestamp = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        });

    history.records.push(FixRecord {
        rule: rule.to_string(),
        file: file.to_string(),
        timestamp,
        outcome,
    });

    save_fix_history(root, &history)
}

/// Compute fix effectiveness per rule.
pub fn compute_effectiveness(history: &FixHistory) -> Vec<FixEffectiveness> {
    let mut by_rule: HashMap<String, (usize, usize, usize)> = HashMap::new();

    for record in &history.records {
        let entry = by_rule.entry(record.rule.clone()).or_default();
        match record.outcome {
            FixOutcome::Accepted => entry.0 += 1,
            FixOutcome::Rejected => entry.1 += 1,
            FixOutcome::Modified => entry.2 += 1,
        }
    }

    let mut results: Vec<FixEffectiveness> = by_rule
        .into_iter()
        .map(|(rule, (accepted, rejected, modified))| {
            let total = accepted + rejected + modified;
            let rate = if total > 0 {
                accepted as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            FixEffectiveness {
                rule,
                total_fixes: total,
                accepted,
                rejected,
                modified,
                acceptance_rate: rate,
            }
        })
        .collect();

    results.sort_by(|a, b| b.total_fixes.cmp(&a.total_fixes));
    results
}

/// Print fix effectiveness report.
pub fn print_fix_effectiveness(effectiveness: &[FixEffectiveness]) {
    println!();
    println!(
        "  {} Fix Effectiveness Tracking",
        "falcon".bright_cyan().bold()
    );
    println!();

    if effectiveness.is_empty() {
        println!("  No fix history yet. Record with: falcon fix-track --rule <rule> --outcome accepted|rejected");
        println!();
        return;
    }

    println!(
        "  {:<35} {:<8} {:<10} {:<10} {:<10} {:<12}",
        "Rule", "Total", "Accepted", "Rejected", "Modified", "Rate"
    );
    println!("  {}", "─".repeat(85));

    for eff in effectiveness {
        let rate_color = if eff.acceptance_rate >= 80.0 {
            format!("{:.0}%", eff.acceptance_rate).green()
        } else if eff.acceptance_rate >= 50.0 {
            format!("{:.0}%", eff.acceptance_rate).yellow()
        } else {
            format!("{:.0}%", eff.acceptance_rate).red()
        };

        println!(
            "  {:<35} {:<8} {:<10} {:<10} {:<10} {:<12}",
            eff.rule.bright_white(),
            eff.total_fixes,
            eff.accepted.to_string().green(),
            eff.rejected.to_string().red(),
            eff.modified,
            rate_color
        );
    }
    println!();
}
