//! Console (CLI) output for the runtime report.

use super::{RuntimeReport, RuntimeSeverity};
use colored::Colorize;

/// Print the full runtime report to stdout.
pub fn print_report(report: &RuntimeReport) {
    println!();

    // ── Overall score ────────────────────────────────────────────────
    let score_color = match report.overall_score {
        90..=100 => "green",
        70..=89 => "yellow",
        _ => "red",
    };
    println!(
        "  {} {} {}",
        "Runtime Health Score:".bold(),
        format!("{}/100 (Grade: {})", report.overall_score, report.grade)
            .color(score_color)
            .bold(),
        score_bar(report.overall_score)
    );
    println!();

    // ── Dimension breakdown ──────────────────────────────────────────
    print_dimension("Memory", report.scores.memory);
    print_dimension("Rendering", report.scores.rendering);
    print_dimension("Network", report.scores.network);
    print_dimension("CPU", report.scores.cpu);
    print_dimension("Stability", report.scores.stability);
    println!();

    // ── Memory summary ───────────────────────────────────────────────
    println!("{}", "  ── Memory ──".bright_blue().bold());
    println!(
        "    Heap: {:.1} MB avg │ {:.1} MB peak │ Growth: {:.1} MB {}",
        report.memory.avg_heap_mb,
        report.memory.max_heap_mb,
        report.memory.heap_growth_mb,
        if report.memory.monotonic_growth {
            "⚠ monotonic".bright_red().to_string()
        } else {
            "✓".green().to_string()
        }
    );
    if report.memory.peak_external_mb > 0.1 {
        println!(
            "    External: {:.1} MB peak",
            report.memory.peak_external_mb
        );
    }
    println!();

    // ── Rendering summary ────────────────────────────────────────────
    println!("{}", "  ── Rendering ──".bright_blue().bold());
    println!(
        "    Frames: {} total │ {} dropped ({:.1}%)",
        report.rendering.total_frames,
        report.rendering.dropped_frames,
        report.rendering.dropped_pct
    );
    println!(
        "    Build:  {:.1} ms avg │ {:.0} ms max",
        report.rendering.avg_build_ms, report.rendering.max_build_ms
    );
    println!(
        "    Raster: {:.1} ms avg │ {:.0} ms max",
        report.rendering.avg_raster_ms, report.rendering.max_raster_ms
    );
    if report.rendering.jank_events > 0 {
        println!(
            "    Jank events: {}",
            report.rendering.jank_events.to_string().bright_yellow()
        );
    }
    println!();

    // ── Network summary ──────────────────────────────────────────────
    if report.network.total_requests > 0 {
        println!("{}", "  ── Network ──".bright_blue().bold());
        println!(
            "    Requests: {} total │ {} failed",
            report.network.total_requests, report.network.failed_requests
        );
        println!(
            "    Latency: {:.0} ms avg │ {:.0} ms max",
            report.network.avg_latency_ms, report.network.max_latency_ms
        );
        println!(
            "    Received: {:.1} KB",
            report.network.total_bytes_received as f64 / 1024.0
        );
        println!();
    }

    // ── CPU summary ──────────────────────────────────────────────────
    if report.cpu.total_samples > 0 {
        println!("{}", "  ── CPU ──".bright_blue().bold());
        println!(
            "    Estimated usage: {:.0}% │ {} samples",
            report.cpu.estimated_usage_pct, report.cpu.total_samples
        );
        if !report.cpu.top_functions.is_empty() {
            println!("    Hot functions:");
            for (name, count) in report.cpu.top_functions.iter().take(5) {
                println!("      {} ({}×)", name.bright_white(), count);
            }
        }
        println!();
    }

    // ── Issues ───────────────────────────────────────────────────────
    if !report.issues.is_empty() {
        println!("{}", "  ── Issues ──".bright_blue().bold());
        println!();
        for issue in &report.issues {
            let (icon, color) = match issue.severity {
                RuntimeSeverity::Error => ("✗", "red"),
                RuntimeSeverity::Warning => ("⚠", "yellow"),
                RuntimeSeverity::Info => ("ℹ", "cyan"),
            };
            println!(
                "  {} {} [{}] {}",
                icon.color(color).bold(),
                issue.title.color(color),
                issue.category.dimmed(),
                ""
            );
            println!("    {}", issue.detail.dimmed());
            println!(
                "    {} {}",
                "→".bright_cyan(),
                issue.suggestion.bright_white()
            );
            println!();
        }
    }

    // ── Summary line ─────────────────────────────────────────────────
    println!(
        "  {} {} error(s), {} warning(s), {} info(s) — collected over {:.1}s ({} snapshots)",
        "Summary:".bold(),
        report.error_count().to_string().bright_red(),
        report.warning_count().to_string().bright_yellow(),
        report.info_count().to_string().bright_cyan(),
        report.duration_secs,
        report.snapshot_count,
    );
    println!();
}

fn print_dimension(name: &str, score: u32) {
    let bar = score_bar(score);
    let color = match score {
        90..=100 => "green",
        70..=89 => "yellow",
        _ => "red",
    };
    println!(
        "    {:<14} {:>3}/100 {}",
        name,
        score.to_string().color(color),
        bar
    );
}

fn score_bar(score: u32) -> String {
    let filled = (score as usize) / 5;
    let empty = 20 - filled;
    format!(
        "{}{}",
        "█".repeat(filled).green(),
        "░".repeat(empty).dimmed()
    )
}
