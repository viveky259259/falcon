use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    pub grade: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub score: u32,
    pub findings: Vec<String>,
}

impl DimensionScore {
    fn new(score: u32, findings: Vec<String>) -> Self {
        Self { score: score.min(100), findings }
    }
}

/// Calculate AI Code Quality Score for a project.
pub fn calculate_ai_score(root: &Path) -> anyhow::Result<AiCodeScore> {
    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    let issues = &report.issues;
    let file_count = report.file_count;

    let resource_safety = score_resource_safety(issues, file_count);
    let error_handling = score_error_handling(issues, file_count);
    let type_safety = score_type_safety(issues, file_count);
    let security = score_security(issues);
    let convention_match = score_convention(issues, file_count);
    let complexity = score_complexity(issues, file_count);

    let overall = (resource_safety.score as f64 * 0.20
        + error_handling.score as f64 * 0.25
        + type_safety.score as f64 * 0.15
        + security.score as f64 * 0.15
        + convention_match.score as f64 * 0.10
        + complexity.score as f64 * 0.15) as u32;

    let grade = match overall {
        90..=100 => "A".to_string(),
        80..=89 => "B".to_string(),
        70..=79 => "C".to_string(),
        60..=69 => "D".to_string(),
        _ => "F".to_string(),
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

fn score_resource_safety(issues: &[crate::reporters::Issue], _file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let dispose_count = issues.iter().filter(|i| i.rule == "ensure-dispose-lifecycle").count();
    let stream_count = issues.iter().filter(|i| i.rule == "ensure-stream-subscription-cancel").count();

    if dispose_count > 0 {
        findings.push(format!("{} undisposed controllers/FocusNodes", dispose_count));
    }
    if stream_count > 0 {
        findings.push(format!("{} uncancelled stream subscriptions", stream_count));
    }

    let penalty = (dispose_count + stream_count) * 5;
    let score = 100u32.saturating_sub(penalty as u32);
    DimensionScore::new(score, findings)
}

fn score_error_handling(issues: &[crate::reporters::Issue], _file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let empty_catch = issues.iter().filter(|i| i.rule == "avoid-empty-catch").count();
    let unawaited = issues.iter().filter(|i| i.rule == "avoid-unawaited-futures").count();
    let generic_catch = issues.iter().filter(|i| i.rule == "prefer-specific-catch-type").count();
    let throw_in_catch = issues.iter().filter(|i| i.rule == "avoid-throw-in-catch").count();

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

fn score_type_safety(issues: &[crate::reporters::Issue], file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let dynamic_count = issues.iter().filter(|i| i.rule == "avoid-dynamic").count();
    let type_assert = issues.iter().filter(|i| i.rule == "avoid-unnecessary-type-assertions").count();
    let type_cast = issues.iter().filter(|i| i.rule == "avoid-unnecessary-type-casts").count();

    if dynamic_count > 0 {
        findings.push(format!("{} uses of 'dynamic' type", dynamic_count));
    }
    if type_assert > 0 {
        findings.push(format!("{} unnecessary type assertions", type_assert));
    }
    if type_cast > 0 {
        findings.push(format!("{} unnecessary type casts", type_cast));
    }

    let per_file = if file_count > 0 { dynamic_count as f64 / file_count as f64 } else { 0.0 };
    let penalty = (per_file * 30.0).min(60.0) as u32 + (type_assert + type_cast) as u32;
    let score = 100u32.saturating_sub(penalty);
    DimensionScore::new(score, findings)
}

fn score_security(issues: &[crate::reporters::Issue]) -> DimensionScore {
    let mut findings = Vec::new();

    let creds = issues.iter().filter(|i| i.rule == "avoid-hardcoded-credentials").count();
    let print_prod = issues.iter().filter(|i| i.rule == "avoid-print-in-production").count();

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

fn score_convention(issues: &[crate::reporters::Issue], file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let naming = issues.iter().filter(|i| i.rule == "prefer-correct-identifier-length").count();
    let file_name = issues.iter().filter(|i| i.rule == "prefer-match-file-name").count();
    let trailing_comma = issues.iter().filter(|i| i.rule == "prefer-trailing-comma").count();
    let bool_params = issues.iter().filter(|i| i.rule == "prefer-named-boolean-parameters").count();

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

fn score_complexity(issues: &[crate::reporters::Issue], file_count: usize) -> DimensionScore {
    let mut findings = Vec::new();

    let long_fn = issues.iter().filter(|i| i.rule == "avoid-long-functions").count();
    let nested = issues.iter().filter(|i| i.rule == "avoid-nested-conditionals").count();
    let long_params = issues.iter().filter(|i| i.rule == "avoid-long-parameter-list").count();
    let widget_nesting = issues.iter().filter(|i| i.rule == "avoid-excessive-widget-nesting").count();

    if long_fn > 0 {
        findings.push(format!("{} overly long functions", long_fn));
    }
    if nested > 0 {
        findings.push(format!("{} deeply nested conditionals", nested));
    }
    if long_params > 0 {
        findings.push(format!("{} functions with too many parameters", long_params));
    }
    if widget_nesting > 0 {
        findings.push(format!("{} excessively nested widget trees", widget_nesting));
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

pub fn print_ai_score(score: &AiCodeScore) {
    println!();
    println!(
        "  {} AI Code Quality Score",
        "falcon".bright_cyan().bold()
    );
    println!();

    let grade_color = match score.grade.as_str() {
        "A" => score.overall.to_string().bright_green().bold(),
        "B" => score.overall.to_string().green().bold(),
        "C" => score.overall.to_string().yellow().bold(),
        "D" => score.overall.to_string().red(),
        _ => score.overall.to_string().bright_red().bold(),
    };

    println!(
        "  AI Code Quality Score: {}/100 (Grade: {})",
        grade_color,
        match score.grade.as_str() {
            "A" => "A".bright_green().bold(),
            "B" => "B".green().bold(),
            "C" => "C".yellow().bold(),
            "D" => "D".red(),
            _ => "F".bright_red().bold(),
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
        score.file_count,
        score.total_issues
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
        println!("                         {} {}", "·".dimmed(), finding.dimmed());
    }
}

/// Generate a badge string for README embedding.
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
