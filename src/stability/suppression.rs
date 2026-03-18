use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

const SUPPRESSION_FILE: &str = ".falcon-data/suppressions.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppressionEntry {
    pub rule: String,
    pub file: String,
    pub line: Option<usize>,
    pub reason: String,
    pub category: SuppressionCategory,
    pub created_at: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SuppressionCategory {
    FalsePositive,
    WontFix,
    Acknowledged,
    Deferred,
}

impl std::fmt::Display for SuppressionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuppressionCategory::FalsePositive => write!(f, "false-positive"),
            SuppressionCategory::WontFix => write!(f, "wont-fix"),
            SuppressionCategory::Acknowledged => write!(f, "acknowledged"),
            SuppressionCategory::Deferred => write!(f, "deferred"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SuppressionDatabase {
    pub entries: Vec<SuppressionEntry>,
}

/// Load suppressions from disk.
pub fn load_suppressions(root: &Path) -> anyhow::Result<SuppressionDatabase> {
    let path = root.join(SUPPRESSION_FILE);
    if !path.exists() {
        return Ok(SuppressionDatabase::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let db: SuppressionDatabase = serde_json::from_str(&content)?;
    Ok(db)
}

/// Save suppressions to disk.
pub fn save_suppressions(root: &Path, db: &SuppressionDatabase) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;

    let path = root.join(SUPPRESSION_FILE);
    let json = serde_json::to_string_pretty(db)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Add a suppression entry.
pub fn add_suppression(
    root: &Path,
    rule: &str,
    file: &str,
    line: Option<usize>,
    reason: &str,
    category: SuppressionCategory,
) -> anyhow::Result<()> {
    let mut db = load_suppressions(root)?;

    let timestamp = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    db.entries.push(SuppressionEntry {
        rule: rule.to_string(),
        file: file.to_string(),
        line,
        reason: reason.to_string(),
        category,
        created_at: timestamp,
        created_by: None,
    });

    save_suppressions(root, &db)?;
    Ok(())
}

/// Get suppression statistics.
pub fn suppression_stats(db: &SuppressionDatabase) -> SuppressionStats {
    let total = db.entries.len();
    let false_positives = db
        .entries
        .iter()
        .filter(|e| e.category == SuppressionCategory::FalsePositive)
        .count();
    let wont_fix = db
        .entries
        .iter()
        .filter(|e| e.category == SuppressionCategory::WontFix)
        .count();
    let acknowledged = db
        .entries
        .iter()
        .filter(|e| e.category == SuppressionCategory::Acknowledged)
        .count();
    let deferred = db
        .entries
        .iter()
        .filter(|e| e.category == SuppressionCategory::Deferred)
        .count();

    let mut by_rule: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for entry in &db.entries {
        *by_rule.entry(entry.rule.clone()).or_default() += 1;
    }
    let mut top_suppressed_rules: Vec<(String, usize)> = by_rule.into_iter().collect();
    top_suppressed_rules.sort_by(|a, b| b.1.cmp(&a.1));
    top_suppressed_rules.truncate(10);

    let false_positive_rate = if total > 0 {
        false_positives as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    SuppressionStats {
        total,
        false_positives,
        wont_fix,
        acknowledged,
        deferred,
        false_positive_rate,
        top_suppressed_rules,
    }
}

#[derive(Debug)]
pub struct SuppressionStats {
    pub total: usize,
    pub false_positives: usize,
    pub wont_fix: usize,
    pub acknowledged: usize,
    pub deferred: usize,
    pub false_positive_rate: f64,
    pub top_suppressed_rules: Vec<(String, usize)>,
}

pub fn print_suppression_stats(stats: &SuppressionStats) {
    println!();
    println!(
        "  {} Suppression Database",
        "falcon".bright_cyan().bold()
    );
    println!();

    println!("  Total suppressions: {}", stats.total.to_string().bright_white());
    println!(
        "    False positives:  {} ({:.1}%)",
        stats.false_positives.to_string().red(),
        stats.false_positive_rate
    );
    println!(
        "    Won't fix:        {}",
        stats.wont_fix.to_string().yellow()
    );
    println!(
        "    Acknowledged:     {}",
        stats.acknowledged.to_string().blue()
    );
    println!(
        "    Deferred:         {}",
        stats.deferred.to_string().dimmed()
    );

    if !stats.top_suppressed_rules.is_empty() {
        println!();
        println!("  Most suppressed rules:");
        for (rule, count) in &stats.top_suppressed_rules {
            println!(
                "    {:<40} {}",
                rule.bright_white(),
                count.to_string().red()
            );
        }
    }

    println!();
}

pub fn print_suppression_list(db: &SuppressionDatabase) {
    println!();
    println!(
        "  {} Suppression Entries ({} total)",
        "falcon".bright_cyan().bold(),
        db.entries.len()
    );
    println!();

    if db.entries.is_empty() {
        println!("  No suppressions recorded.");
        println!();
        return;
    }

    for entry in &db.entries {
        let cat_color = match entry.category {
            SuppressionCategory::FalsePositive => "FP".red(),
            SuppressionCategory::WontFix => "WF".yellow(),
            SuppressionCategory::Acknowledged => "AK".blue(),
            SuppressionCategory::Deferred => "DF".dimmed(),
        };
        let location = match entry.line {
            Some(l) => format!("{}:{}", entry.file, l),
            None => entry.file.clone(),
        };
        println!(
            "  {} {:<40} {} — {}",
            cat_color,
            entry.rule.bright_white(),
            location.dimmed(),
            entry.reason
        );
    }
    println!();
}
