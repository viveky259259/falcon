use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const BENCHMARK_DB_FILE: &str = ".falcon-data/benchmark-db.json";

/// A recorded project analysis with tool attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBenchmark {
    pub project_name: String,
    pub ai_tool: String,
    pub score: u32,
    pub grade: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub total_issues: usize,
    pub file_count: usize,
    pub timestamp: String,
}

/// Aggregated statistics per AI tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStats {
    pub tool: String,
    pub projects_analyzed: usize,
    pub avg_score: f64,
    pub min_score: u32,
    pub max_score: u32,
    pub avg_issues_per_file: f64,
    pub common_issues: Vec<(String, usize)>,
}

/// The benchmark database.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkDatabase {
    pub entries: Vec<ProjectBenchmark>,
}

/// Load the benchmark database.
pub fn load_benchmark_db(root: &Path) -> anyhow::Result<BenchmarkDatabase> {
    let path = root.join(BENCHMARK_DB_FILE);
    if !path.exists() {
        return Ok(BenchmarkDatabase::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let db: BenchmarkDatabase = serde_json::from_str(&content)?;
    Ok(db)
}

/// Save the benchmark database.
pub fn save_benchmark_db(root: &Path, db: &BenchmarkDatabase) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let path = root.join(BENCHMARK_DB_FILE);
    let json = serde_json::to_string_pretty(db)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Record a project analysis into the benchmark database.
pub fn record_benchmark(
    root: &Path,
    project_name: &str,
    ai_tool: &str,
) -> anyhow::Result<ProjectBenchmark> {
    let score = crate::ai_score::score::calculate_ai_score(root)?;

    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;

    let timestamp = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        });

    let entry = ProjectBenchmark {
        project_name: project_name.to_string(),
        ai_tool: ai_tool.to_string(),
        score: score.overall,
        grade: score.grade.to_string(),
        error_count: report.error_count(),
        warning_count: report.warning_count(),
        total_issues: report.issues.len(),
        file_count: report.file_count,
        timestamp,
    };

    let mut db = load_benchmark_db(root)?;
    db.entries.push(entry.clone());
    save_benchmark_db(root, &db)?;

    Ok(entry)
}

/// Compute per-tool statistics from the benchmark database.
pub fn compute_tool_stats(db: &BenchmarkDatabase) -> Vec<ToolStats> {
    let mut by_tool: HashMap<String, Vec<&ProjectBenchmark>> = HashMap::new();
    for entry in &db.entries {
        by_tool
            .entry(entry.ai_tool.clone())
            .or_default()
            .push(entry);
    }

    let mut stats: Vec<ToolStats> = by_tool
        .into_iter()
        .map(|(tool, entries)| {
            let count = entries.len();
            let avg = entries.iter().map(|e| e.score as f64).sum::<f64>() / count as f64;
            let min = entries.iter().map(|e| e.score).min().unwrap_or(0);
            let max = entries.iter().map(|e| e.score).max().unwrap_or(0);

            let total_files: usize = entries.iter().map(|e| e.file_count).sum();
            let total_issues: usize = entries.iter().map(|e| e.total_issues).sum();
            let avg_ipf = if total_files > 0 {
                total_issues as f64 / total_files as f64
            } else {
                0.0
            };

            ToolStats {
                tool,
                projects_analyzed: count,
                avg_score: avg,
                min_score: min,
                max_score: max,
                avg_issues_per_file: avg_ipf,
                common_issues: vec![],
            }
        })
        .collect();

    stats.sort_by(|a, b| {
        b.avg_score
            .partial_cmp(&a.avg_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    stats
}

/// Print benchmark database summary.
pub fn print_benchmark_summary(stats: &[ToolStats]) {
    println!();
    println!(
        "  {} AI Tool Benchmark Database",
        "falcon".bright_cyan().bold()
    );
    println!();

    if stats.is_empty() {
        println!("  No benchmarks recorded yet. Run: falcon benchmark-db --tool <tool-name>");
        println!();
        return;
    }

    println!(
        "  {:<15} {:<10} {:<10} {:<10} {:<10} {:<12}",
        "Tool", "Projects", "Avg Score", "Min", "Max", "Issues/File"
    );
    println!("  {}", "─".repeat(67));

    for s in stats {
        let avg_color = if s.avg_score >= 80.0 {
            format!("{:.0}", s.avg_score).bright_green()
        } else if s.avg_score >= 60.0 {
            format!("{:.0}", s.avg_score).yellow()
        } else {
            format!("{:.0}", s.avg_score).red()
        };

        println!(
            "  {:<15} {:<10} {:<10} {:<10} {:<10} {:<12.1}",
            s.tool.bright_white(),
            s.projects_analyzed,
            avg_color,
            s.min_score,
            s.max_score,
            s.avg_issues_per_file,
        );
    }
    println!();
}
