use colored::Colorize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RepoAnalysis {
    pub name: String,
    pub file_count: usize,
    pub issue_count: usize,
    pub top_rules: Vec<(String, usize)>,
    pub health_score: f64,
    pub analysis_time_ms: u128,
}

#[derive(Debug)]
pub struct ShowcaseReport {
    pub repos: Vec<RepoAnalysis>,
    pub total_files: usize,
    pub total_issues: usize,
    pub most_common_rules: Vec<(String, usize)>,
    pub avg_health_score: f64,
}

/// Analyze a local project directory for the showcase.
pub fn analyze_local_project(path: &Path, name: &str) -> anyhow::Result<RepoAnalysis> {
    let config = crate::config::FalconConfig::load(path)?;
    let start = std::time::Instant::now();
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(path)?;
    let elapsed = start.elapsed();

    let mut rule_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for issue in &report.issues {
        *rule_counts.entry(issue.rule.clone()).or_default() += 1;
    }

    let mut top_rules: Vec<(String, usize)> = rule_counts.into_iter().collect();
    top_rules.sort_by(|a, b| b.1.cmp(&a.1));
    top_rules.truncate(5);

    let issue_count = report.issues.len();
    let file_count = report.file_count;

    let health_score = if file_count > 0 {
        let issues_per_file = issue_count as f64 / file_count as f64;
        (100.0 - issues_per_file * 5.0).max(0.0).min(100.0)
    } else {
        100.0
    };

    Ok(RepoAnalysis {
        name: name.to_string(),
        file_count,
        issue_count,
        top_rules,
        health_score,
        analysis_time_ms: elapsed.as_millis(),
    })
}

/// Aggregate multiple repo analyses into a showcase report.
pub fn generate_showcase_report(repos: Vec<RepoAnalysis>) -> ShowcaseReport {
    let total_files: usize = repos.iter().map(|r| r.file_count).sum();
    let total_issues: usize = repos.iter().map(|r| r.issue_count).sum();
    let avg_health = if repos.is_empty() {
        0.0
    } else {
        repos.iter().map(|r| r.health_score).sum::<f64>() / repos.len() as f64
    };

    let mut all_rules: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for repo in &repos {
        for (rule, count) in &repo.top_rules {
            *all_rules.entry(rule.clone()).or_default() += count;
        }
    }
    let mut most_common: Vec<(String, usize)> = all_rules.into_iter().collect();
    most_common.sort_by(|a, b| b.1.cmp(&a.1));
    most_common.truncate(10);

    ShowcaseReport {
        repos,
        total_files,
        total_issues,
        most_common_rules: most_common,
        avg_health_score: avg_health,
    }
}

pub fn print_showcase_report(report: &ShowcaseReport) {
    println!();
    println!(
        "  {} Showcase Analysis Report",
        "falcon".bright_cyan().bold()
    );
    println!();

    println!("  Summary:");
    println!(
        "    Projects analyzed: {}",
        report.repos.len().to_string().bright_white()
    );
    println!(
        "    Total files:       {}",
        report.total_files.to_string().bright_white()
    );
    println!(
        "    Total issues:      {}",
        report.total_issues.to_string().bright_white()
    );
    println!(
        "    Avg health score:  {:.1}%",
        report.avg_health_score
    );
    println!();

    println!("  Per-project breakdown:");
    println!(
        "  {:<35} {:>8} {:>8} {:>8} {:>10}",
        "Project".bright_white().bold(),
        "Files".dimmed(),
        "Issues".dimmed(),
        "Health".dimmed(),
        "Time".dimmed()
    );
    println!("  {}", "─".repeat(75));

    for repo in &report.repos {
        let health_color = if repo.health_score >= 80.0 {
            format!("{:.0}%", repo.health_score).green()
        } else if repo.health_score >= 60.0 {
            format!("{:.0}%", repo.health_score).yellow()
        } else {
            format!("{:.0}%", repo.health_score).red()
        };

        println!(
            "  {:<35} {:>8} {:>8} {:>8} {:>8}ms",
            repo.name.bright_white(),
            repo.file_count,
            repo.issue_count,
            health_color,
            repo.analysis_time_ms,
        );
    }
    println!();

    println!("  Most common issues across all projects:");
    for (rule, count) in &report.most_common_rules {
        let bar_len = if !report.most_common_rules.is_empty() {
            (*count as f64 / report.most_common_rules[0].1 as f64 * 20.0) as usize
        } else {
            1
        };
        let bar = "█".repeat(bar_len.max(1));
        println!(
            "    {:<40} {} {}",
            rule.bright_white(),
            bar.bright_cyan(),
            count
        );
    }
    println!();
}

/// Generate a markdown report.
pub fn generate_markdown_report(report: &ShowcaseReport) -> String {
    let mut md = String::new();
    md.push_str("# Falcon Showcase — Flutter Project Analysis\n\n");
    md.push_str(&format!(
        "**{}** projects analyzed | **{}** files | **{}** issues found\n\n",
        report.repos.len(),
        report.total_files,
        report.total_issues
    ));
    md.push_str(&format!(
        "Average health score: **{:.1}%**\n\n",
        report.avg_health_score
    ));

    md.push_str("## Per-Project Breakdown\n\n");
    md.push_str("| Project | Files | Issues | Health | Time |\n");
    md.push_str("|---------|------:|-------:|-------:|-----:|\n");
    for repo in &report.repos {
        md.push_str(&format!(
            "| {} | {} | {} | {:.0    }% board| {}ms |\n",
            repo.name, repo.file_count, repo.issue_count, repo.health_score, repo.analysis_time_ms
        ));
    }
    md.push('\n');

    md.push_str("## Most Common Issues\n\n");
    md.push_str("| Rule | Count 

|\n");
    md.push_str("|------|------:|\n");
    for (rule, count) in &report.most_common_rules {
        md.push_str(&format!("| `{}` | {} |\n", rule, count));
    }

    md
}
