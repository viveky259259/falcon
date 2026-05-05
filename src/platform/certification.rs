//! Falcon Certification — badge system for projects and analysts.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Certification levels for projects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CertLevel {
    Bronze,
    Silver,
    Gold,
    Platinum,
}

impl std::fmt::Display for CertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bronze => write!(f, "Bronze"),
            Self::Silver => write!(f, "Silver"),
            Self::Gold => write!(f, "Gold"),
            Self::Platinum => write!(f, "Platinum"),
        }
    }
}

/// Certification result for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificationResult {
    pub project: String,
    pub level: Option<CertLevel>,
    pub score: u32,
    pub criteria: Vec<CertCriterion>,
    pub badge_markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertCriterion {
    pub name: String,
    pub required: String,
    pub actual: String,
    pub passed: bool,
}

/// Evaluate a project for Falcon certification.
pub fn evaluate_certification(root: &Path) -> anyhow::Result<CertificationResult> {
    let score = crate::ai_score::score::calculate_ai_score(root)?;
    let project = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    let errors = report.error_count();
    let vuln_count = report
        .issues
        .iter()
        .filter(|i| i.rule == "avoid-hardcoded-credentials")
        .count();
    let dispose_count = report
        .issues
        .iter()
        .filter(|i| i.rule == "ensure-dispose-lifecycle")
        .count();

    let criteria = vec![
        CertCriterion {
            name: "AI Score ≥ 60".to_string(),
            required: "60+".to_string(),
            actual: format!("{}", score.overall),
            passed: score.overall >= 60,
        },
        CertCriterion {
            name: "AI Score ≥ 75".to_string(),
            required: "75+".to_string(),
            actual: format!("{}", score.overall),
            passed: score.overall >= 75,
        },
        CertCriterion {
            name: "AI Score ≥ 85".to_string(),
            required: "85+".to_string(),
            actual: format!("{}", score.overall),
            passed: score.overall >= 85,
        },
        CertCriterion {
            name: "AI Score ≥ 95".to_string(),
            required: "95+".to_string(),
            actual: format!("{}", score.overall),
            passed: score.overall >= 95,
        },
        CertCriterion {
            name: "Zero errors".to_string(),
            required: "0".to_string(),
            actual: format!("{}", errors),
            passed: errors == 0,
        },
        CertCriterion {
            name: "No hardcoded credentials".to_string(),
            required: "0".to_string(),
            actual: format!("{}", vuln_count),
            passed: vuln_count == 0,
        },
        CertCriterion {
            name: "All controllers disposed".to_string(),
            required: "0 violations".to_string(),
            actual: format!("{}", dispose_count),
            passed: dispose_count == 0,
        },
    ];

    let level = if score.overall >= 95 && errors == 0 && vuln_count == 0 && dispose_count == 0 {
        Some(CertLevel::Platinum)
    } else if score.overall >= 85 && errors == 0 && vuln_count == 0 {
        Some(CertLevel::Gold)
    } else if score.overall >= 75 && vuln_count == 0 {
        Some(CertLevel::Silver)
    } else if score.overall >= 60 {
        Some(CertLevel::Bronze)
    } else {
        None
    };

    let badge_color = match &level {
        Some(CertLevel::Platinum) => "brightgreen",
        Some(CertLevel::Gold) => "gold",
        Some(CertLevel::Silver) => "silver",
        Some(CertLevel::Bronze) => "orange",
        None => "red",
    };
    let badge_text = level
        .as_ref()
        .map_or("Not Certified".to_string(), |l| format!("Certified_{}", l));
    let badge_markdown = format!(
        "![Falcon Certified](https://img.shields.io/badge/Falcon-{}-{})",
        badge_text, badge_color
    );

    Ok(CertificationResult {
        project,
        level,
        score: score.overall,
        criteria,
        badge_markdown,
    })
}

/// Print certification result.
pub fn print_certification(result: &CertificationResult) {
    println!();
    println!(
        "  {} Certification — {}",
        "falcon".bright_cyan().bold(),
        result.project.bright_white().bold()
    );
    println!();

    match &result.level {
        Some(level) => {
            let color = match level {
                CertLevel::Platinum => "PLATINUM".bright_green().bold(),
                CertLevel::Gold => "GOLD".yellow().bold(),
                CertLevel::Silver => "SILVER".white().bold(),
                CertLevel::Bronze => "BRONZE".red(),
            };
            println!(
                "  🏆 Falcon Certified: {} (Score: {}/100)",
                color, result.score
            );
        }
        None => {
            println!(
                "  {} Not yet certified (Score: {}/100)",
                "✗".red(),
                result.score
            );
        }
    }

    println!();
    for c in &result.criteria {
        let icon = if c.passed { "✓".green() } else { "✗".red() };
        println!(
            "  {} {:<35} required: {:<8} actual: {}",
            icon, c.name, c.required, c.actual
        );
    }

    println!();
    println!("  Badge: {}", result.badge_markdown);
    println!();
}
