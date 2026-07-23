//! AI-Tool Profiling — analyze per-tool error rates from the benchmark database.

use super::benchmark_db::{compute_tool_stats, BenchmarkDatabase};
use colored::Colorize;

/// Per-tool error profile showing which rules each AI tool struggles with.
#[derive(Debug, Clone)]
pub struct ToolProfile {
    pub tool: String,
    pub projects: usize,
    pub avg_score: f64,
    pub top_violations: Vec<(String, f64)>,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
}

/// Build profiles for each AI tool from the benchmark database.
pub fn build_tool_profiles(db: &BenchmarkDatabase) -> Vec<ToolProfile> {
    let stats = compute_tool_stats(db);

    stats
        .iter()
        .map(|s| {
            let weaknesses = if s.avg_score < 50.0 {
                vec!["Overall code quality needs improvement".to_string()]
            } else if s.avg_score < 70.0 {
                vec!["Error handling and resource management".to_string()]
            } else {
                vec![]
            };

            let strengths = if s.avg_score >= 85.0 {
                vec!["Production-ready code quality".to_string()]
            } else if s.avg_score >= 70.0 {
                vec!["Good baseline quality".to_string()]
            } else {
                vec![]
            };

            ToolProfile {
                tool: s.tool.clone(),
                projects: s.projects_analyzed,
                avg_score: s.avg_score,
                top_violations: vec![],
                strengths,
                weaknesses,
            }
        })
        .collect()
}

/// Print AI tool profiles.
pub fn print_tool_profiles(profiles: &[ToolProfile]) {
    println!();
    println!("  {} AI Tool Profiling", "falcon".bright_cyan().bold());
    println!();

    if profiles.is_empty() {
        println!(
            "  No benchmark data yet. Record benchmarks with: falcon x benchmark-db --tool <name>"
        );
        println!();
        return;
    }

    for profile in profiles {
        let score_color = if profile.avg_score >= 85.0 {
            format!("{:.0}", profile.avg_score).bright_green()
        } else if profile.avg_score >= 60.0 {
            format!("{:.0}", profile.avg_score).yellow()
        } else {
            format!("{:.0}", profile.avg_score).red()
        };

        println!(
            "  {} ({} projects, avg score: {})",
            profile.tool.bright_white().bold(),
            profile.projects,
            score_color
        );

        for s in &profile.strengths {
            println!("    {} {}", "+".green(), s);
        }
        for w in &profile.weaknesses {
            println!("    {} {}", "!".red(), w);
        }
        println!();
    }
}
