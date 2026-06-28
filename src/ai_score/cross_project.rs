//! Cross-Project Learning — aggregate conventions and patterns across multiple
//! projects to build a shared knowledge base that improves detection for all.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

const LEARNING_DB_FILE: &str = ".falcon-data/cross-project-db.json";

/// Convention data extracted from a single project (anonymized).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProfile {
    pub project_id: String,
    pub timestamp: String,
    pub file_count: usize,
    pub architecture: String,
    pub state_management: Option<String>,
    pub naming_convention: String,
    pub error_handling_style: String,
    pub ai_score: Option<u32>,
    pub rule_violations: HashMap<String, usize>,
    pub top_patterns: Vec<String>,
}

/// Aggregated knowledge across projects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearningDatabase {
    pub profiles: Vec<ProjectProfile>,
    pub convention_frequencies: HashMap<String, usize>,
    pub rule_violation_totals: HashMap<String, usize>,
    pub architecture_distribution: HashMap<String, usize>,
    pub state_mgmt_distribution: HashMap<String, usize>,
}

/// Aggregated insights derived from the learning database.
#[derive(Debug, Clone)]
pub struct CrossProjectInsights {
    pub total_projects: usize,
    pub most_common_architecture: String,
    pub most_common_state_mgmt: String,
    pub most_violated_rules: Vec<(String, usize)>,
    pub avg_score: f64,
    pub convention_recommendations: Vec<String>,
}

/// Load the learning database.
pub fn load_learning_db(root: &Path) -> anyhow::Result<LearningDatabase> {
    let path = root.join(LEARNING_DB_FILE);
    if !path.exists() {
        return Ok(LearningDatabase::default());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Save the learning database.
pub fn save_learning_db(root: &Path, db: &LearningDatabase) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let path = root.join(LEARNING_DB_FILE);
    let json = serde_json::to_string_pretty(db)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Record a project into the learning database (anonymized).
pub fn record_project(db_root: &Path, project_root: &Path) -> anyhow::Result<ProjectProfile> {
    let conventions = crate::ai_score::convention::detect_conventions(project_root)?;

    let config = crate::config::FalconConfig::load(project_root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(project_root)?;
    let score = crate::ai_score::score::score_from_report(&report).ok();

    let mut rule_violations: HashMap<String, usize> = HashMap::new();
    for issue in &report.issues {
        *rule_violations.entry(issue.rule.clone()).or_default() += 1;
    }

    let error_style = if conventions.error_handling.uses_result_type {
        "Result"
    } else if conventions.error_handling.uses_either {
        "Either"
    } else {
        "try-catch"
    };

    let project_id = format!("proj-{:08x}", {
        let full_path = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        let path_str = full_path.to_string_lossy();
        let mut hash: u32 = 0;
        for b in path_str.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u32);
        }
        hash
    });

    let timestamp = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        });

    let mut top_violations: Vec<(String, usize)> = rule_violations
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    top_violations.sort_by_key(|(_, count)| Reverse(*count));

    let profile = ProjectProfile {
        project_id,
        timestamp,
        file_count: report.file_count,
        architecture: conventions.architecture.pattern.clone(),
        state_management: conventions.state_management.clone(),
        naming_convention: conventions.naming.file_naming.clone(),
        error_handling_style: error_style.to_string(),
        ai_score: score.map(|s| s.overall),
        rule_violations,
        top_patterns: top_violations
            .iter()
            .take(5)
            .map(|(r, _)| r.clone())
            .collect(),
    };

    let mut db = load_learning_db(db_root)?;
    db.profiles.retain(|p| p.project_id != profile.project_id);
    db.profiles.push(profile.clone());
    rebuild_aggregates(&mut db);
    save_learning_db(db_root, &db)?;

    Ok(profile)
}

fn rebuild_aggregates(db: &mut LearningDatabase) {
    db.convention_frequencies.clear();
    db.rule_violation_totals.clear();
    db.architecture_distribution.clear();
    db.state_mgmt_distribution.clear();

    for profile in &db.profiles {
        *db.architecture_distribution
            .entry(profile.architecture.clone())
            .or_default() += 1;
        if let Some(ref sm) = profile.state_management {
            *db.state_mgmt_distribution.entry(sm.clone()).or_default() += 1;
        }
        *db.convention_frequencies
            .entry(profile.naming_convention.clone())
            .or_default() += 1;

        for (rule, count) in &profile.rule_violations {
            *db.rule_violation_totals.entry(rule.clone()).or_default() += count;
        }
    }
}

/// Derive insights from the learning database.
pub fn derive_insights(db: &LearningDatabase) -> CrossProjectInsights {
    let total = db.profiles.len();

    let most_arch = db
        .architecture_distribution
        .iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let most_sm = db
        .state_mgmt_distribution
        .iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let mut top_rules: Vec<(String, usize)> = db
        .rule_violation_totals
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    top_rules.sort_by_key(|(_, count)| Reverse(*count));
    top_rules.truncate(10);

    let scores: Vec<f64> = db
        .profiles
        .iter()
        .filter_map(|p| p.ai_score.map(|s| s as f64))
        .collect();
    let avg = if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    };

    let mut recs = Vec::new();
    if db.architecture_distribution.len() > 1 {
        recs.push(format!(
            "Most teams use {} — consider standardizing",
            most_arch
        ));
    }
    if let Some((top_rule, count)) = top_rules.first() {
        recs.push(format!(
            "'{}' is the most common violation ({} total) — consider team training",
            top_rule, count
        ));
    }
    if avg < 70.0 && total > 0 {
        recs.push(
            "Average score below 70 — focus on error handling and resource safety".to_string(),
        );
    }

    CrossProjectInsights {
        total_projects: total,
        most_common_architecture: most_arch,
        most_common_state_mgmt: most_sm,
        most_violated_rules: top_rules,
        avg_score: avg,
        convention_recommendations: recs,
    }
}

/// Print cross-project insights.
pub fn print_insights(insights: &CrossProjectInsights) {
    println!();
    println!("  {} Cross-Project Learning", "falcon".bright_cyan().bold());
    println!();

    if insights.total_projects == 0 {
        println!("  No project data yet. Record with: falcon learn --project /path/to/project");
        println!();
        return;
    }

    println!(
        "  Projects analyzed:       {}",
        insights.total_projects.to_string().bright_white()
    );
    println!(
        "  Dominant architecture:   {}",
        insights.most_common_architecture.bright_white().bold()
    );
    println!(
        "  Dominant state mgmt:     {}",
        insights.most_common_state_mgmt.bright_white().bold()
    );
    println!("  Average AI score:        {:.0}/100", insights.avg_score);

    if !insights.most_violated_rules.is_empty() {
        println!();
        println!("  Most common violations across projects:");
        for (rule, count) in &insights.most_violated_rules {
            println!(
                "    {:<40} {}x",
                rule.bright_white(),
                count.to_string().red()
            );
        }
    }

    if !insights.convention_recommendations.is_empty() {
        println!();
        println!("  {} Recommendations:", "▸".bright_cyan());
        for rec in &insights.convention_recommendations {
            println!("    {} {}", "→".green(), rec);
        }
    }

    println!();
}
