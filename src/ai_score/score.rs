use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Letter grade for an AI code quality score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Grade {
    A,
    B,
    C,
    D,
    F,
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Grade::A => write!(f, "A"),
            Grade::B => write!(f, "B"),
            Grade::C => write!(f, "C"),
            Grade::D => write!(f, "D"),
            Grade::F => write!(f, "F"),
        }
    }
}

/// AI Code Quality Score with 6-dimension breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCodeScore {
    pub overall: u32,
    pub resource_safety: DimensionScore,
    pub error_handling: DimensionScore,
    pub type_safety: DimensionScore,
    pub security: DimensionScore,
    pub convention_match: DimensionScore,
    pub complexity: DimensionScore,
    pub file_count: usize,
    pub total_issues: usize,
    pub grade: Grade,
}

/// Score for a single dimension with detected findings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub score: u32,
    pub findings: Vec<String>,
}

impl DimensionScore {
    fn new(score: u32, findings: Vec<String>) -> Self {
        Self {
            score: score.min(100),
            findings,
        }
    }
}

/// Pre-computed issue counts by rule name, built in a single pass.
struct IssueCounts {
    counts: HashMap<String, usize>,
}

impl IssueCounts {
    fn from_issues(issues: &[crate::reporters::Issue]) -> Self {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for issue in issues {
            *counts.entry(issue.rule.clone()).or_default() += 1;
        }
        Self { counts }
    }

    fn get(&self, rule: &str) -> usize {
        self.counts.get(rule).copied().unwrap_or(0)
    }
}

/// Calculate AI Code Quality Score for a project at `root`.
/// Returns an error if the path doesn't exist or config is invalid.
pub fn calculate_ai_score(root: &Path) -> anyhow::Result<AiCodeScore> {
    if !root.exists() {
        anyhow::bail!("Path does not exist: {}", root.display());
    }

    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    score_from_report(&report)
}

/// Calculate AI Code Quality Score from a pre-existing analysis report.
/// Avoids duplicate analysis when the caller already has results.
pub fn score_from_report(report: &crate::reporters::AnalysisReport) -> anyhow::Result<AiCodeScore> {
    let issues = &report.issues;
    let file_count = report.file_count;
    let ic = IssueCounts::from_issues(issues);

    let resource_safety = score_resource_safety(&ic);
    let error_handling = score_error_handling(&ic);
    let type_safety = score_type_safety(&ic, file_count);
    let security = score_security(&ic);
    let convention_match = score_convention(&ic, file_count);
    let complexity = score_complexity(&ic, file_count);

    let overall = (resource_safety.score as f64 * 0.20
        + error_handling.score as f64 * 0.25
        + type_safety.score as f64 * 0.15
        + security.score as f64 * 0.15
        + convention_match.score as f64 * 0.10
        + complexity.score as f64 * 0.15) as u32;

    let grade = match overall {
        90..=100 => Grade::A,
        80..=89 => Grade::B,
        70..=79 => Grade::C,
        60..=69 => Grade::D,
        _ => Grade::F,
    };

    Ok(AiCodeScore {
        overall,
        resource_safety,
        error_handling,
        type_safety,
        security,
        convention_match,
        complexity,
        file_count,
        total_issues: issues.len(),
        grade,
    })
}

fn score_resource_safety(ic: &IssueCounts) -> DimensionScore {
    let mut findings = Vec::new();

    let dispose_count = ic.get("ensure-dispose-lifecycle");
    let stream_count = ic.get("ensure-stream-subscription-cancel");

    if dispose_count > 0 {
        findings.push(format!(
            "{} undisposed controllers/FocusNodes",
            dispose_count
        ));
    }
    if stream_count > 0 {
        findings.push(format!("{} uncancelled stream subscriptions", stream_count));
    }

    let penalty = (dispose_count + stream_count) * 5;
    let score = 100u32.saturating_sub(penalty as u32);
    DimensionScore::new(score, findings)
}

fn score_error_handling(ic: &IssueCounts) -> DimensionScore {
    let mut findings = Vec::new();

    let empty_catch = ic.get("avoid-empty-catch");
    let unawaited = ic.get("avoid-unawaited-futures");
    let generic_catch = ic.get("prefer-specific-catch-type");
    let throw_in_catch = ic.get("avoid-throw-in-catch");

    if empty_catch > 0 {
        findings.push(format!("{} empty catch blocks", empty_catch));
    }
    if unawaited > 0 {
        findings.push(format!("{} unawaited futures", unawaited));
    }
    if generic_catch > 0 {
        findings.push(format!("{} generic catch clauses", generic_catch));
    }
    if throw_in_catch > 0 {
        findings.push(format!("{} throw-in-catch violations", throw_in_catch));
    }

    let penalty = empty_catch * 8 + unawaited * 3 + generic_catch * 2 + throw_in_catch * 4;
    let score = 100u32.saturating_sub(penalty as u32);
    DimensionScore::new(score, findings)
}

fn score_type_safety(ic: &IssueCounts, file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let dynamic_count = ic.get("avoid-dynamic");
    let type_assert = ic.get("avoid-unnecessary-type-assertions");
    let type_cast = ic.get("avoid-unnecessary-type-casts");

    if dynamic_count > 0 {
        findings.push(format!("{} uses of 'dynamic' type", dynamic_count));
    }
    if type_assert > 0 {
        findings.push(format!("{} unnecessary type assertions", type_assert));
    }
    if type_cast > 0 {
        findings.push(format!("{} unnecessary type casts", type_cast));
    }

    let per_file = if file_count > 0 {
        dynamic_count as f64 / file_count as f64
    } else {
        0.0
    };
    let penalty = (per_file * 30.0).min(60.0) as u32 + (type_assert + type_cast) as u32;
    let score = 100u32.saturating_sub(penalty);
    DimensionScore::new(score, findings)
}

fn score_security(ic: &IssueCounts) -> DimensionScore {
    let mut findings = Vec::new();

    let creds = ic.get("avoid-hardcoded-credentials");
    let print_prod = ic.get("avoid-print-in-production");

    if creds > 0 {
        findings.push(format!("{} hardcoded credentials", creds));
    }
    if print_prod > 0 {
        findings.push(format!("{} print() calls in production code", print_prod));
    }

    let penalty = creds * 25 + print_prod * 2;
    let score = 100u32.saturating_sub(penalty as u32);
    DimensionScore::new(score, findings)
}

fn score_convention(ic: &IssueCounts, file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let naming = ic.get("prefer-correct-identifier-length");
    let file_name = ic.get("prefer-match-file-name");
    let trailing_comma = ic.get("prefer-trailing-comma");
    let bool_params = ic.get("prefer-named-boolean-parameters");

    if naming > 0 {
        findings.push(format!("{} identifier length issues", naming));
    }
    if file_name > 0 {
        findings.push(format!("{} file naming mismatches", file_name));
    }
    if trailing_comma > 0 {
        findings.push(format!("{} missing trailing commas", trailing_comma));
    }

    let per_file = if file_count > 0 {
        (naming + file_name + bool_params) as f64 / file_count as f64
    } else {
        0.0
    };
    let penalty = (per_file * 20.0).min(50.0) as u32;
    let score = 100u32.saturating_sub(penalty);
    DimensionScore::new(score, findings)
}

fn score_complexity(ic: &IssueCounts, file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let long_fn = ic.get("avoid-long-functions");
    let nested = ic.get("avoid-nested-conditionals");
    let long_params = ic.get("avoid-long-parameter-list");
    let widget_nesting = ic.get("avoid-excessive-widget-nesting");

    if long_fn > 0 {
        findings.push(format!("{} overly long functions", long_fn));
    }
    if nested > 0 {
        findings.push(format!("{} deeply nested conditionals", nested));
    }
    if long_params > 0 {
        findings.push(format!(
            "{} functions with too many parameters",
            long_params
        ));
    }
    if widget_nesting > 0 {
        findings.push(format!(
            "{} excessively nested widget trees",
            widget_nesting
        ));
    }

    let per_file = if file_count > 0 {
        (long_fn + nested + long_params + widget_nesting) as f64 / file_count as f64
    } else {
        0.0
    };
    let penalty = (per_file * 25.0).min(60.0) as u32;
    let score = 100u32.saturating_sub(penalty);
    DimensionScore::new(score, findings)
}

/// Print an AI Code Quality Score to the console.
pub fn print_ai_score(score: &AiCodeScore) {
    println!();
    println!("  {} AI Code Quality Score", "falcon".bright_cyan().bold());
    println!();

    let grade_color = match score.grade {
        Grade::A => score.overall.to_string().bright_green().bold(),
        Grade::B => score.overall.to_string().green().bold(),
        Grade::C => score.overall.to_string().yellow().bold(),
        Grade::D => score.overall.to_string().red(),
        Grade::F => score.overall.to_string().bright_red().bold(),
    };

    println!(
        "  AI Code Quality Score: {}/100 (Grade: {})",
        grade_color,
        match score.grade {
            Grade::A => "A".bright_green().bold(),
            Grade::B => "B".green().bold(),
            Grade::C => "C".yellow().bold(),
            Grade::D => "D".red(),
            Grade::F => "F".bright_red().bold(),
        }
    );
    println!();

    print_dimension("Resource Safety", &score.resource_safety, 0.20);
    print_dimension("Error Handling", &score.error_handling, 0.25);
    print_dimension("Type Safety", &score.type_safety, 0.15);
    print_dimension("Security", &score.security, 0.15);
    print_dimension("Convention Match", &score.convention_match, 0.10);
    print_dimension("Complexity", &score.complexity, 0.15);

    println!();
    println!(
        "  {} files analyzed, {} total issues",
        score.file_count, score.total_issues
    );

    println!();
    if score.overall >= 85 {
        println!(
            "  {} Production Ready — qualifies for Falcon Certified badge",
            "✓".green().bold()
        );
    } else if score.overall >= 70 {
        println!(
            "  {} Near Production Ready — address findings above to reach 85+",
            "▸".yellow()
        );
    } else {
        println!(
            "  {} Needs Improvement — focus on error handling and resource safety first",
            "⚠".red()
        );
    }
    println!();
}

fn print_dimension(name: &str, dim: &DimensionScore, weight: f64) {
    let bar_len = (dim.score as f64 / 5.0) as usize;
    let bar_full = "█".repeat(bar_len);
    let bar_empty = "░".repeat(20 - bar_len);

    let score_color = if dim.score >= 80 {
        format!("{:>3}/100", dim.score).green()
    } else if dim.score >= 60 {
        format!("{:>3}/100", dim.score).yellow()
    } else {
        format!("{:>3}/100", dim.score).red()
    };

    println!(
        "    {:<20} {} {}{} ({:.0}% weight)",
        name,
        score_color,
        bar_full.bright_cyan(),
        bar_empty.dimmed(),
        weight * 100.0
    );

    for finding in &dim.findings {
        println!(
            "                         {} {}",
            "·".dimmed(),
            finding.dimmed()
        );
    }
}

/// Generate a shields.io badge markdown string for README embedding.
pub fn generate_badge(score: &AiCodeScore) -> String {
    let color = match score.overall {
        90..=100 => "brightgreen",
        80..=89 => "green",
        70..=79 => "yellow",
        60..=69 => "orange",
        _ => "red",
    };
    format!(
        "![Falcon AI Score](https://img.shields.io/badge/Falcon_AI_Score-{}/100-{})",
        score.overall, color
    )
}
