//! Console (CLI) output for the runtime report.

use std::fmt::Write;

use super::{RuntimeReport, RuntimeSeverity};
use colored::Colorize;

/// Print the full runtime report to stdout.
pub fn print_report(report: &RuntimeReport) {
    print!("{}", format_report(report));
}

/// Format the full runtime report as a String (pure, no I/O).
fn format_report(report: &RuntimeReport) -> String {
    let mut out = String::new();

    out.push('\n');

    // ── Overall score ────────────────────────────────────────────────
    let score_color = match report.overall_score {
        90..=100 => "green",
        70..=89 => "yellow",
        _ => "red",
    };
    writeln!(
        out,
        "  {} {} {}",
        "Runtime Health Score:".bold(),
        format!("{}/100 (Grade: {})", report.overall_score, report.grade)
            .color(score_color)
            .bold(),
        score_bar(report.overall_score)
    )
    .unwrap();
    out.push('\n');

    // ── Dimension breakdown ──────────────────────────────────────────
    writeln!(out, "{}", format_dimension("Memory", report.scores.memory)).unwrap();
    writeln!(out, "{}", format_dimension("Rendering", report.scores.rendering)).unwrap();
    writeln!(out, "{}", format_dimension("Network", report.scores.network)).unwrap();
    writeln!(out, "{}", format_dimension("CPU", report.scores.cpu)).unwrap();
    writeln!(out, "{}", format_dimension("Stability", report.scores.stability)).unwrap();
    out.push('\n');

    // ── Memory summary ───────────────────────────────────────────────
    writeln!(out, "{}", "  ── Memory ──".bright_blue().bold()).unwrap();
    writeln!(
        out,
        "    Heap: {:.1} MB avg │ {:.1} MB peak │ Growth: {:.1} MB {}",
        report.memory.avg_heap_mb,
        report.memory.max_heap_mb,
        report.memory.heap_growth_mb,
        if report.memory.monotonic_growth {
            "⚠ monotonic".bright_red().to_string()
        } else {
            "✓".green().to_string()
        }
    )
    .unwrap();
    if report.memory.peak_external_mb > 0.1 {
        writeln!(
            out,
            "    External: {:.1} MB peak",
            report.memory.peak_external_mb
        )
        .unwrap();
    }
    out.push('\n');

    // ── Rendering summary ────────────────────────────────────────────
    writeln!(out, "{}", "  ── Rendering ──".bright_blue().bold()).unwrap();
    writeln!(
        out,
        "    Frames: {} total │ {} dropped ({:.1}%)",
        report.rendering.total_frames,
        report.rendering.dropped_frames,
        report.rendering.dropped_pct
    )
    .unwrap();
    writeln!(
        out,
        "    Build:  {:.1} ms avg │ {:.0} ms max",
        report.rendering.avg_build_ms, report.rendering.max_build_ms
    )
    .unwrap();
    writeln!(
        out,
        "    Raster: {:.1} ms avg │ {:.0} ms max",
        report.rendering.avg_raster_ms, report.rendering.max_raster_ms
    )
    .unwrap();
    if report.rendering.jank_events > 0 {
        writeln!(
            out,
            "    Jank events: {}",
            report.rendering.jank_events.to_string().bright_yellow()
        )
        .unwrap();
    }
    out.push('\n');

    // ── Network summary ──────────────────────────────────────────────
    if report.network.total_requests > 0 {
        writeln!(out, "{}", "  ── Network ──".bright_blue().bold()).unwrap();
        writeln!(
            out,
            "    Requests: {} total │ {} failed",
            report.network.total_requests, report.network.failed_requests
        )
        .unwrap();
        writeln!(
            out,
            "    Latency: {:.0} ms avg │ {:.0} ms max",
            report.network.avg_latency_ms, report.network.max_latency_ms
        )
        .unwrap();
        writeln!(
            out,
            "    Received: {:.1} KB",
            report.network.total_bytes_received as f64 / 1024.0
        )
        .unwrap();
        out.push('\n');
    }

    // ── CPU summary ──────────────────────────────────────────────────
    if report.cpu.total_samples > 0 {
        writeln!(out, "{}", "  ── CPU ──".bright_blue().bold()).unwrap();
        writeln!(
            out,
            "    Estimated usage: {:.0}% │ {} samples",
            report.cpu.estimated_usage_pct, report.cpu.total_samples
        )
        .unwrap();
        if !report.cpu.top_functions.is_empty() {
            writeln!(out, "    Hot functions:").unwrap();
            for (name, count) in report.cpu.top_functions.iter().take(5) {
                writeln!(out, "      {} ({}×)", name.bright_white(), count).unwrap();
            }
        }
        out.push('\n');
    }

    // ── Issues ───────────────────────────────────────────────────────
    if !report.issues.is_empty() {
        writeln!(out, "{}", "  ── Issues ──".bright_blue().bold()).unwrap();
        out.push('\n');
        for issue in &report.issues {
            let (icon, color) = match issue.severity {
                RuntimeSeverity::Error => ("✗", "red"),
                RuntimeSeverity::Warning => ("⚠", "yellow"),
                RuntimeSeverity::Info => ("ℹ", "cyan"),
            };
            writeln!(
                out,
                "  {} {} [{}] {}",
                icon.color(color).bold(),
                issue.title.color(color),
                issue.category.dimmed(),
                ""
            )
            .unwrap();
            writeln!(out, "    {}", issue.detail.dimmed()).unwrap();
            writeln!(
                out,
                "    {} {}",
                "→".bright_cyan(),
                issue.suggestion.bright_white()
            )
            .unwrap();
            out.push('\n');
        }
    }

    // ── Summary line ─────────────────────────────────────────────────
    writeln!(
        out,
        "  {} {} error(s), {} warning(s), {} info(s) — collected over {:.1}s ({} snapshots)",
        "Summary:".bold(),
        report.error_count().to_string().bright_red(),
        report.warning_count().to_string().bright_yellow(),
        report.info_count().to_string().bright_cyan(),
        report.duration_secs,
        report.snapshot_count,
    )
    .unwrap();
    out.push('\n');

    out
}

/// Format a single dimension line as a String (pure, no trailing newline).
fn format_dimension(name: &str, score: u32) -> String {
    let bar = score_bar(score);
    let color = match score {
        90..=100 => "green",
        70..=89 => "yellow",
        _ => "red",
    };
    format!(
        "    {:<14} {:>3}/100 {}",
        name,
        score.to_string().color(color),
        bar
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::diagnostics::{CpuSummary, MemoryTrend, NetworkSummary, RenderingSummary};
    use crate::runtime::report::{RuntimeIssue, RuntimeScores, RuntimeSeverity};
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn disable_colors() {
        INIT.call_once(|| {
            colored::control::set_override(false);
        });
    }

    fn sample_report() -> RuntimeReport {
        RuntimeReport {
            overall_score: 95,
            grade: "A",
            duration_secs: 12.3,
            snapshot_count: 7,
            memory: MemoryTrend {
                min_heap_mb: 10.0,
                max_heap_mb: 50.0,
                avg_heap_mb: 25.0,
                heap_growth_mb: 5.0,
                peak_external_mb: 0.0,
                monotonic_growth: false,
                samples: vec![],
            },
            rendering: RenderingSummary {
                total_frames: 1000,
                dropped_frames: 0,
                dropped_pct: 0.0,
                avg_build_ms: 3.0,
                max_build_ms: 8.0,
                avg_raster_ms: 2.0,
                max_raster_ms: 5.0,
                jank_events: 0,
                frame_times: vec![],
            },
            network: NetworkSummary {
                total_requests: 0,
                failed_requests: 0,
                avg_latency_ms: 0.0,
                max_latency_ms: 0.0,
                total_bytes_received: 0,
            },
            cpu: CpuSummary {
                estimated_usage_pct: 0.0,
                total_samples: 0,
                top_functions: vec![],
            },
            issues: vec![],
            scores: RuntimeScores {
                memory: 95,
                rendering: 95,
                network: 100,
                cpu: 100,
                stability: 100,
            },
        }
    }

    #[test]
    fn score_bar_full_score_is_all_filled() {
        disable_colors();
        let bar = score_bar(100);
        assert_eq!(bar.matches('█').count(), 20, "bar: {bar}");
    }

    #[test]
    fn score_bar_zero_score_is_all_empty() {
        disable_colors();
        let bar = score_bar(0);
        assert_eq!(bar.matches('░').count(), 20, "bar: {bar}");
    }

    #[test]
    fn score_bar_mid_score_has_mixed_blocks() {
        disable_colors();
        let bar = score_bar(50);
        assert_eq!(bar.matches('█').count(), 10, "bar: {bar}");
        assert_eq!(bar.matches('░').count(), 10, "bar: {bar}");
    }

    #[test]
    fn format_dimension_green_band_for_high_score() {
        disable_colors();
        let line = format_dimension("Memory", 95);
        assert!(line.contains("Memory"), "line: {line}");
        assert!(line.contains("95/100"), "line: {line}");
        assert!(line.contains('█') || line.contains('░'), "line: {line}");
    }

    #[test]
    fn format_dimension_yellow_band_for_mid_score() {
        disable_colors();
        let line = format_dimension("CPU", 80);
        assert!(line.contains("80/100"), "line: {line}");
    }

    #[test]
    fn format_dimension_red_band_for_low_score() {
        disable_colors();
        let line = format_dimension("Stability", 50);
        assert!(line.contains("50/100"), "line: {line}");
    }

    #[test]
    fn format_report_includes_overall_score_and_grade() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(out.contains("Runtime Health Score:"), "out: {out}");
        assert!(out.contains("95/100"), "out: {out}");
        assert!(out.contains("Grade: A"), "out: {out}");
    }

    #[test]
    fn format_report_includes_all_dimension_lines() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(out.contains("Memory"), "out: {out}");
        assert!(out.contains("Rendering"), "out: {out}");
        assert!(out.contains("Network"), "out: {out}");
        assert!(out.contains("CPU"), "out: {out}");
        assert!(out.contains("Stability"), "out: {out}");
    }

    #[test]
    fn format_report_includes_memory_section() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(out.contains("── Memory ──"), "out: {out}");
        assert!(out.contains("Heap:"), "out: {out}");
        assert!(out.contains("25.0 MB avg"), "out: {out}");
        assert!(out.contains("50.0 MB peak"), "out: {out}");
    }

    #[test]
    fn format_report_memory_monotonic_growth_marker() {
        disable_colors();
        let mut report = sample_report();
        report.memory.monotonic_growth = true;
        let out = format_report(&report);
        assert!(out.contains("monotonic"), "out: {out}");
    }

    #[test]
    fn format_report_memory_external_section_when_peak_high() {
        disable_colors();
        let mut report = sample_report();
        report.memory.peak_external_mb = 5.5;
        let out = format_report(&report);
        assert!(out.contains("External:"), "out: {out}");
        assert!(out.contains("5.5 MB peak"), "out: {out}");
    }

    #[test]
    fn format_report_memory_external_section_absent_when_peak_zero() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(!out.contains("External:"), "out: {out}");
    }

    #[test]
    fn format_report_rendering_jank_section_present_when_jank_events_nonzero() {
        disable_colors();
        let mut report = sample_report();
        report.rendering.jank_events = 3;
        let out = format_report(&report);
        assert!(out.contains("Jank events:"), "out: {out}");
    }

    #[test]
    fn format_report_rendering_jank_section_absent_when_jank_events_zero() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(!out.contains("Jank events:"), "out: {out}");
    }

    #[test]
    fn format_report_network_section_present_when_requests_nonzero() {
        disable_colors();
        let mut report = sample_report();
        report.network.total_requests = 10;
        let out = format_report(&report);
        assert!(out.contains("── Network ──"), "out: {out}");
        assert!(out.contains("Requests:"), "out: {out}");
    }

    #[test]
    fn format_report_network_section_absent_when_no_requests() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(!out.contains("── Network ──"), "out: {out}");
    }

    #[test]
    fn format_report_cpu_section_present_when_samples_nonzero() {
        disable_colors();
        let mut report = sample_report();
        report.cpu.total_samples = 100;
        let out = format_report(&report);
        assert!(out.contains("── CPU ──"), "out: {out}");
    }

    #[test]
    fn format_report_cpu_section_absent_when_no_samples() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(!out.contains("── CPU ──"), "out: {out}");
    }

    #[test]
    fn format_report_cpu_hot_functions_listed() {
        disable_colors();
        let mut report = sample_report();
        report.cpu.total_samples = 100;
        report.cpu.top_functions = vec![("foo".into(), 42)];
        let out = format_report(&report);
        assert!(out.contains("foo"), "out: {out}");
        assert!(out.contains("42×"), "out: {out}");
    }

    #[test]
    fn format_report_issues_section_includes_all_severities() {
        disable_colors();
        let mut report = sample_report();
        report.issues = vec![
            RuntimeIssue {
                severity: RuntimeSeverity::Error,
                category: "mem",
                title: "Leak detected".into(),
                detail: "growing heap".into(),
                suggestion: "investigate".into(),
            },
            RuntimeIssue {
                severity: RuntimeSeverity::Warning,
                category: "mem",
                title: "High usage warning".into(),
                detail: "memory is high".into(),
                suggestion: "reduce allocations".into(),
            },
            RuntimeIssue {
                severity: RuntimeSeverity::Info,
                category: "mem",
                title: "Info notice".into(),
                detail: "informational".into(),
                suggestion: "no action needed".into(),
            },
        ];
        let out = format_report(&report);
        assert!(out.contains("── Issues ──"), "out: {out}");
        assert!(out.contains("Leak detected"), "out: {out}");
        assert!(out.contains("growing heap"), "out: {out}");
        assert!(out.contains("investigate"), "out: {out}");
        assert!(out.contains("High usage warning"), "out: {out}");
        assert!(out.contains("memory is high"), "out: {out}");
        assert!(out.contains("reduce allocations"), "out: {out}");
        assert!(out.contains("Info notice"), "out: {out}");
        assert!(out.contains("informational"), "out: {out}");
        assert!(out.contains("no action needed"), "out: {out}");
    }

    #[test]
    fn format_report_issues_section_absent_when_empty() {
        disable_colors();
        let report = sample_report();
        let out = format_report(&report);
        assert!(!out.contains("── Issues ──"), "out: {out}");
    }

    #[test]
    fn format_report_summary_line_includes_counts_and_duration() {
        disable_colors();
        let mut report = sample_report();
        report.duration_secs = 12.3;
        report.snapshot_count = 7;
        report.issues = vec![
            RuntimeIssue {
                severity: RuntimeSeverity::Error,
                category: "test",
                title: "E".into(),
                detail: "d".into(),
                suggestion: "s".into(),
            },
            RuntimeIssue {
                severity: RuntimeSeverity::Warning,
                category: "test",
                title: "W".into(),
                detail: "d".into(),
                suggestion: "s".into(),
            },
            RuntimeIssue {
                severity: RuntimeSeverity::Info,
                category: "test",
                title: "I".into(),
                detail: "d".into(),
                suggestion: "s".into(),
            },
        ];
        let out = format_report(&report);
        assert!(out.contains("Summary:"), "out: {out}");
        assert!(out.contains("1 error(s)"), "out: {out}");
        assert!(out.contains("1 warning(s)"), "out: {out}");
        assert!(out.contains("1 info(s)"), "out: {out}");
        assert!(out.contains("12.3s"), "out: {out}");
        assert!(out.contains("7 snapshots"), "out: {out}");
    }
}
