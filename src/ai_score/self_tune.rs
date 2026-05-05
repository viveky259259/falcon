use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const TUNE_FILE: &str = ".falcon-data/tune-history.json";

/// Record of how a rule performs over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTuneRecord {
    pub rule: String,
    pub trigger_count: usize,
    pub suppress_count: usize,
    pub fix_accepted: usize,
    pub fix_rejected: usize,
    pub signal_ratio: f64,
}

/// Tuning history for all rules.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuneHistory {
    pub records: HashMap<String, RuleTuneRecord>,
    pub snapshots: usize,
}

/// Recommendation for adjusting a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuneRecommendation {
    pub rule: String,
    pub action: TuneAction,
    pub reason: String,
    pub signal_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TuneAction {
    Downgrade,
    Disable,
    Upgrade,
    KeepAsIs,
}

impl std::fmt::Display for TuneAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TuneAction::Downgrade => write!(f, "downgrade"),
            TuneAction::Disable => write!(f, "disable"),
            TuneAction::Upgrade => write!(f, "upgrade"),
            TuneAction::KeepAsIs => write!(f, "keep"),
        }
    }
}

/// Load tuning history from disk.
pub fn load_tune_history(root: &Path) -> anyhow::Result<TuneHistory> {
    let path = root.join(TUNE_FILE);
    if !path.exists() {
        return Ok(TuneHistory::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let history: TuneHistory = serde_json::from_str(&content)?;
    Ok(history)
}

/// Save tuning history to disk.
pub fn save_tune_history(root: &Path, history: &TuneHistory) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let path = root.join(TUNE_FILE);
    let json = serde_json::to_string_pretty(history)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Record analysis results into tuning history.
pub fn record_analysis(root: &Path) -> anyhow::Result<TuneHistory> {
    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    let mut history = load_tune_history(root)?;
    history.snapshots += 1;

    let mut rule_counts: HashMap<String, usize> = HashMap::new();
    for issue in &report.issues {
        *rule_counts.entry(issue.rule.clone()).or_default() += 1;
    }

    let suppressions = crate::stability::suppression::load_suppressions(root).unwrap_or_default();
    let mut suppress_counts: HashMap<String, usize> = HashMap::new();
    for entry in &suppressions.entries {
        *suppress_counts.entry(entry.rule.clone()).or_default() += 1;
    }

    for (rule, count) in &rule_counts {
        let record = history
            .records
            .entry(rule.clone())
            .or_insert_with(|| RuleTuneRecord {
                rule: rule.clone(),
                trigger_count: 0,
                suppress_count: 0,
                fix_accepted: 0,
                fix_rejected: 0,
                signal_ratio: 1.0,
            });
        record.trigger_count += count;
        if let Some(sup) = suppress_counts.get(rule) {
            record.suppress_count += sup;
        }
        let total = record.trigger_count.max(1) as f64;
        record.signal_ratio = 1.0 - (record.suppress_count as f64 / total);
    }

    save_tune_history(root, &history)?;
    Ok(history)
}

/// Generate auto-tuning recommendations based on history.
pub fn generate_recommendations(history: &TuneHistory) -> Vec<TuneRecommendation> {
    let mut recs = Vec::new();

    for (rule, record) in &history.records {
        if record.signal_ratio < 0.3 && record.trigger_count > 10 {
            recs.push(TuneRecommendation {
                rule: rule.clone(),
                action: TuneAction::Disable,
                reason: format!(
                    "Signal ratio {:.0}% — {}/{} triggers were suppressed. Consider disabling.",
                    record.signal_ratio * 100.0,
                    record.suppress_count,
                    record.trigger_count
                ),
                signal_ratio: record.signal_ratio,
            });
        } else if record.signal_ratio < 0.5 && record.trigger_count > 5 {
            recs.push(TuneRecommendation {
                rule: rule.clone(),
                action: TuneAction::Downgrade,
                reason: format!(
                    "Signal ratio {:.0}% — consider downgrading from error to warning.",
                    record.signal_ratio * 100.0
                ),
                signal_ratio: record.signal_ratio,
            });
        } else if record.signal_ratio > 0.95 && record.trigger_count > 20 {
            recs.push(TuneRecommendation {
                rule: rule.clone(),
                action: TuneAction::Upgrade,
                reason: format!(
                    "Signal ratio {:.0}% with {} triggers — high-value rule, consider upgrading to error.",
                    record.signal_ratio * 100.0,
                    record.trigger_count
                ),
                signal_ratio: record.signal_ratio,
            });
        }
    }

    recs.sort_by(|a, b| {
        a.signal_ratio
            .partial_cmp(&b.signal_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    recs
}

/// Print tuning recommendations.
pub fn print_tune_recommendations(recs: &[TuneRecommendation], history: &TuneHistory) {
    println!();
    println!(
        "  {} Self-Tuning Recommendations",
        "falcon".bright_cyan().bold()
    );
    println!("  Based on {} analysis snapshots", history.snapshots);
    println!();

    if recs.is_empty() {
        println!(
            "  {} All rules are well-tuned — no changes recommended.",
            "✓".green().bold()
        );
        println!();
        return;
    }

    for rec in recs {
        let action_color = match rec.action {
            TuneAction::Disable => "DISABLE".red().bold(),
            TuneAction::Downgrade => "DOWNGRADE".yellow().bold(),
            TuneAction::Upgrade => "UPGRADE".green().bold(),
            TuneAction::KeepAsIs => "KEEP".dimmed(),
        };

        println!(
            "  {} {:<40} {}",
            action_color,
            rec.rule.bright_white(),
            rec.reason.dimmed()
        );
    }
    println!();
}
