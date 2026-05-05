use colored::Colorize;
use std::path::Path;
use std::time::Instant;

#[derive(Debug)]
pub struct BenchmarkResult {
    pub file_count: usize,
    pub total_lines: usize,
    pub parse_time_ms: u128,
    pub analysis_time_ms: u128,
    pub total_time_ms: u128,
    pub files_per_second: f64,
    pub lines_per_second: f64,
    pub issue_count: usize,
    pub peak_memory_estimate_mb: f64,
}

/// Run a performance benchmark on a project.
pub fn run_benchmark(root: &Path) -> anyhow::Result<BenchmarkResult> {
    let config = crate::config::FalconConfig::load(root)?;
    let exclude_patterns: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let files: Vec<std::path::PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !exclude_patterns.iter().any(|p| p.matches_path(rel))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let file_count = files.len();

    let parse_start = Instant::now();
    let mut total_lines = 0usize;
    let mut sources: Vec<(std::path::PathBuf, String)> = Vec::new();
    for file in &files {
        if let Ok(source) = std::fs::read_to_string(file) {
            total_lines += source.lines().count();
            sources.push((file.clone(), source));
        }
    }
    let parse_time = parse_start.elapsed();

    let analysis_start = Instant::now();
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;
    let analysis_time = analysis_start.elapsed();

    let total_time = parse_time + analysis_time;

    let files_per_second = if total_time.as_secs_f64() > 0.0 {
        file_count as f64 / total_time.as_secs_f64()
    } else {
        file_count as f64
    };

    let lines_per_second = if total_time.as_secs_f64() > 0.0 {
        total_lines as f64 / total_time.as_secs_f64()
    } else {
        total_lines as f64
    };

    let peak_memory_estimate_mb = (total_lines as f64 * 0.001) + (file_count as f64 * 0.05);

    Ok(BenchmarkResult {
        file_count,
        total_lines,
        parse_time_ms: parse_time.as_millis(),
        analysis_time_ms: analysis_time.as_millis(),
        total_time_ms: total_time.as_millis(),
        files_per_second,
        lines_per_second,
        issue_count: report.issues.len(),
        peak_memory_estimate_mb,
    })
}

pub fn print_benchmark(result: &BenchmarkResult) {
    println!();
    println!("  {} Performance Benchmark", "falcon".bright_cyan().bold());
    println!();

    println!("  Codebase:");
    println!(
        "    Files:        {}",
        result.file_count.to_string().bright_white()
    );
    println!(
        "    Lines:        {}",
        result.total_lines.to_string().bright_white()
    );
    println!(
        "    Issues found: {}",
        result.issue_count.to_string().bright_white()
    );
    println!();

    println!("  Timing:");
    println!(
        "    File I/O:     {} ms",
        result.parse_time_ms.to_string().bright_green()
    );
    println!(
        "    Analysis:     {} ms",
        result.analysis_time_ms.to_string().bright_green()
    );
    println!(
        "    Total:        {} ms",
        result.total_time_ms.to_string().bright_green().bold()
    );
    println!();

    println!("  Throughput:");
    println!("    Files/sec:    {:.0}", result.files_per_second);
    println!("    Lines/sec:    {:.0}", result.lines_per_second);
    println!(
        "    Memory est:   ~{:.1} MB",
        result.peak_memory_estimate_mb
    );
    println!();

    let under_5s = result.total_time_ms < 5000;
    if under_5s {
        println!(
            "  {} Analysis completed in under 5 seconds — production target met.",
            "✓".green().bold()
        );
    } else {
        println!(
            "  {} Analysis took {:.1}s — target is < 5s for production.",
            "⚠".yellow(),
            result.total_time_ms as f64 / 1000.0
        );
    }
    println!();
}
