use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::ai_score::provenance::{analyze_project_provenance, summarize_provenance};
use crate::ai_score::score::score_from_report;
use crate::config::Severity;

/// Comprehensive AI code quality report combining score, provenance, and recommendations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReport {
    pub project_name: String,
    pub ai_score: crate::ai_score::score::AiCodeScore,
    pub provenance: ProvenanceStats,
    pub top_issues: Vec<IssueSummary>,
    pub recommendations: Vec<String>,
}

/// Code provenance breakdown by percentage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceStats {
    pub total_files: usize,
    pub human_pct: f64,
    pub ai_pct: f64,
    pub codegen_pct: f64,
    pub unknown_pct: f64,
}

/// Summary of a single rule's issue count and severity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub rule: String,
    pub count: usize,
    pub severity: Severity,
}

/// Generate a comprehensive AI code quality report.
/// Runs analysis once and shares results between score and issue counting.
pub fn generate_ai_report(root: &Path) -> anyhow::Result<AiReport> {
    if !root.exists() {
        anyhow::bail!("Path does not exist: {}", root.display());
    }

    let project_name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let analysis = falcon.analyze(root)?;

    let score = score_from_report(&analysis)?;

    let prov_results = analyze_project_provenance(root)?;
    let prov_summary = summarize_provenance(&prov_results);
    let total = prov_summary.total_files.max(1) as f64;

    let provenance = ProvenanceStats {
        total_files: prov_summary.total_files,
        human_pct: prov_summary.human_files as f64 / total * 100.0,
        ai_pct: prov_summary.ai_files as f64 / total * 100.0,
        codegen_pct: prov_summary.codegen_files as f64 / total * 100.0,
        unknown_pct: prov_summary.unknown_files as f64 / total * 100.0,
    };

    let mut rule_counts: HashMap<String, (usize, Severity)> = HashMap::new();
    for issue in &analysis.issues {
        let entry = rule_counts
            .entry(issue.rule.clone())
            .or_insert((0, issue.severity));
        entry.0 += 1;
        if issue.severity as u8 > entry.1 as u8 {
            entry.1 = issue.severity;
        }
    }

    let mut top_issues: Vec<IssueSummary> = rule_counts
        .into_iter()
        .map(|(rule, (count, severity))| IssueSummary {
            rule,
            count,
            severity,
        })
        .collect();
    top_issues.sort_by(|a, b| b.count.cmp(&a.count));
    top_issues.truncate(10);

    let recommendations = generate_recommendations(&score, &provenance);

    Ok(AiReport {
        project_name,
        ai_score: score,
        provenance,
        top_issues,
        recommendations,
    })
}

fn generate_recommendations(
    score: &crate::ai_score::score::AiCodeScore,
    provenance: &ProvenanceStats,
) -> Vec<String> {
    let mut recs = Vec::new();

    if score.error_handling.score < 70 {
        recs.push(
            "Error Handling needs attention — replace empty catch blocks with proper error handling, and ensure all Futures are awaited.".to_string()
        );
    }
    if score.resource_safety.score < 80 {
        recs.push(
            "Resource Safety: ensure all controllers, FocusNodes, and StreamSubscriptions are properly disposed/cancelled.".to_string()
        );
    }
    if score.security.score < 90 {
        recs.push(
            "Security: remove hardcoded credentials and replace print() calls with a logging framework.".to_string()
        );
    }
    if score.type_safety.score < 80 {
        recs.push(
            "Type Safety: reduce usage of 'dynamic' type — use explicit types or generics instead."
                .to_string(),
        );
    }
    if score.complexity.score < 70 {
        recs.push(
            "Complexity: break down long functions and deeply nested widget trees into smaller, focused components.".to_string()
        );
    }
    if provenance.ai_pct > 30.0 {
        recs.push(format!(
            "AI Provenance: {:.0}% of files show AI-generation patterns — run 'falcon ai-score' after each AI session to catch common pitfalls.",
            provenance.ai_pct
        ));
    }
    if recs.is_empty() {
        recs.push("Excellent code quality — maintain current standards!".to_string());
    }
    recs
}

/// Print a formatted AI report to the console.
pub fn print_ai_report(report: &AiReport) {
    println!();
    println!(
        "  {} State of AI-Generated Flutter Code",
        "falcon".bright_cyan().bold()
    );
    println!("  {}", "─".repeat(50));
    println!("  Project: {}", report.project_name.bright_white().bold());
    println!();

    println!("  {} Overall Score", "▸".bright_cyan());
    crate::ai_score::score::print_ai_score(&report.ai_score);

    println!("  {} Code Provenance", "▸".bright_cyan());
    println!("    Human-written:   {:.1}%", report.provenance.human_pct);
    println!("    AI-generated:    {:.1}%", report.provenance.ai_pct);
    println!("    Code-generated:  {:.1}%", report.provenance.codegen_pct);
    println!("    Unknown:         {:.1}%", report.provenance.unknown_pct);

    if !report.top_issues.is_empty() {
        println!();
        println!("  {} Top Issues", "▸".bright_cyan());
        for (i, issue) in report.top_issues.iter().enumerate() {
            println!(
                "    {}. {} ({}x)",
                i + 1,
                issue.rule.yellow(),
                issue.count.to_string().bright_white()
            );
        }
    }

    if !report.recommendations.is_empty() {
        println!();
        println!("  {} Recommendations", "▸".bright_cyan());
        for rec in &report.recommendations {
            println!("    {} {}", "→".green(), rec);
        }
    }

    println!();
}

/// Generate a markdown report suitable for blog posts or documentation.
pub fn generate_markdown_report(report: &AiReport) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# State of AI-Generated Flutter Code — {}\n\n",
        report.project_name
    ));
    md.push_str(&format!(
        "**AI Code Quality Score: {}/100 (Grade: {})**\n\n",
        report.ai_score.overall, report.ai_score.grade
    ));

    md.push_str("## Score Breakdown\n\n");
    md.push_str("| Dimension | Score | Weight |\n");
    md.push_str("|---|---|---|\n");
    md.push_str(&format!(
        "| Resource Safety | {}/100 | 20% |\n",
        report.ai_score.resource_safety.score
    ));
    md.push_str(&format!(
        "| Error Handling | {}/100 | 25% |\n",
        report.ai_score.error_handling.score
    ));
    md.push_str(&format!(
        "| Type Safety | {}/100 | 15% |\n",
        report.ai_score.type_safety.score
    ));
    md.push_str(&format!(
        "| Security | {}/100 | 15% |\n",
        report.ai_score.security.score
    ));
    md.push_str(&format!(
        "| Convention Match | {}/100 | 10% |\n",
        report.ai_score.convention_match.score
    ));
    md.push_str(&format!(
        "| Complexity | {}/100 | 15% |\n",
        report.ai_score.complexity.score
    ));

    md.push_str("\n## Code Provenance\n\n");
    md.push_str(&format!(
        "- Human-written: {:.1}%\n",
        report.provenance.human_pct
    ));
    md.push_str(&format!(
        "- AI-generated: {:.1}%\n",
        report.provenance.ai_pct
    ));
    md.push_str(&format!(
        "- Code-generated: {:.1}%\n",
        report.provenance.codegen_pct
    ));

    if !report.top_issues.is_empty() {
        md.push_str("\n## Top Issues\n\n");
        md.push_str("| Rule | Occurrences |\n");
        md.push_str("|---|---|\n");
        for issue in &report.top_issues {
            md.push_str(&format!("| {} | {} |\n", issue.rule, issue.count));
        }
    }

    if !report.recommendations.is_empty() {
        md.push_str("\n## Recommendations\n\n");
        for rec in &report.recommendations {
            md.push_str(&format!("- {}\n", rec));
        }
    }

    md.push_str("\n---\n*Generated by Falcon v1.3 — AI Code Quality Narrative*\n");
    md
}
