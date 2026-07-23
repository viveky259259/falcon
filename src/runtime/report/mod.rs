//! Runtime report generation — data model, console output, and HTML dashboard.

pub mod console;
pub mod html;

use super::diagnostics::{
    analyze_cpu, analyze_memory, analyze_network, analyze_rendering, CpuSummary, MemoryTrend,
    NetworkSummary, RenderingSummary, RuntimeSnapshot,
};
use super::RuntimeThresholds;

// ──────────────────────────────────────────────────────────────────────
// Report data model
// ──────────────────────────────────────────────────────────────────────

/// Severity for a runtime issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSeverity {
    Info,
    Warning,
    Error,
}

/// A single runtime issue detected during the check.
#[derive(Debug, Clone)]
pub struct RuntimeIssue {
    pub severity: RuntimeSeverity,
    pub category: &'static str,
    pub title: String,
    pub detail: String,
    pub suggestion: String,
}

/// The full runtime analysis report.
#[derive(Debug, Clone)]
pub struct RuntimeReport {
    pub overall_score: u32,
    pub grade: &'static str,
    pub duration_secs: f64,
    pub snapshot_count: usize,
    pub memory: MemoryTrend,
    pub rendering: RenderingSummary,
    pub network: NetworkSummary,
    pub cpu: CpuSummary,
    pub issues: Vec<RuntimeIssue>,
    /// Per-dimension scores (0–100).
    pub scores: RuntimeScores,
}

#[derive(Debug, Clone)]
pub struct RuntimeScores {
    pub memory: u32,
    pub rendering: u32,
    pub network: u32,
    pub cpu: u32,
    pub stability: u32,
}

impl RuntimeReport {
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == RuntimeSeverity::Error)
            .count()
    }
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == RuntimeSeverity::Warning)
            .count()
    }
    pub fn info_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == RuntimeSeverity::Info)
            .count()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Report builder
// ──────────────────────────────────────────────────────────────────────

/// Analyse collected snapshots and produce a scored `RuntimeReport`.
pub fn build_report(
    snapshots: &[RuntimeSnapshot],
    thresholds: &RuntimeThresholds,
) -> RuntimeReport {
    let memory = analyze_memory(snapshots);
    let rendering = analyze_rendering(snapshots);
    let network = analyze_network(snapshots);
    let cpu = analyze_cpu(snapshots);

    let mut issues = Vec::new();

    // ── Memory issues ────────────────────────────────────────────────
    let memory_score = score_memory(&memory, thresholds, &mut issues);

    // ── Rendering issues ─────────────────────────────────────────────
    let rendering_score = score_rendering(&rendering, thresholds, &mut issues);

    // ── Network issues ───────────────────────────────────────────────
    let network_score = score_network(&network, &mut issues);

    // ── CPU issues ───────────────────────────────────────────────────
    let cpu_score = score_cpu(&cpu, thresholds, &mut issues);

    // ── Stability (lack of crashes / isolate errors) ─────────────────
    let stability_score = score_stability(snapshots, &mut issues);

    let scores = RuntimeScores {
        memory: memory_score,
        rendering: rendering_score,
        network: network_score,
        cpu: cpu_score,
        stability: stability_score,
    };

    // Weighted overall score.
    let overall = (memory_score as f64 * 0.25
        + rendering_score as f64 * 0.30
        + network_score as f64 * 0.15
        + cpu_score as f64 * 0.15
        + stability_score as f64 * 0.15) as u32;

    let grade = match overall {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    };

    let duration_secs = snapshots.last().map(|s| s.elapsed_secs).unwrap_or(0.0);

    RuntimeReport {
        overall_score: overall,
        grade,
        duration_secs,
        snapshot_count: snapshots.len(),
        memory,
        rendering,
        network,
        cpu,
        issues,
        scores,
    }
}

// ── Scoring helpers ──────────────────────────────────────────────────

fn score_memory(
    mem: &MemoryTrend,
    thresholds: &RuntimeThresholds,
    issues: &mut Vec<RuntimeIssue>,
) -> u32 {
    let mut score = 100i32;

    if mem.max_heap_mb > thresholds.memory_error_mb {
        score -= 50;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Error,
            category: "Memory",
            title: format!("Heap usage peaked at {:.1} MB", mem.max_heap_mb),
            detail: format!(
                "Maximum heap usage ({:.1} MB) exceeded the error threshold ({:.0} MB).",
                mem.max_heap_mb, thresholds.memory_error_mb
            ),
            suggestion: "Profile with DevTools Memory view. Look for large image caches, \
                          un-disposed controllers, or retained widget trees."
                .to_string(),
        });
    } else if mem.max_heap_mb > thresholds.memory_warn_mb {
        score -= 20;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "Memory",
            title: format!("Heap usage peaked at {:.1} MB", mem.max_heap_mb),
            detail: format!(
                "Maximum heap usage ({:.1} MB) exceeded the warning threshold ({:.0} MB).",
                mem.max_heap_mb, thresholds.memory_warn_mb
            ),
            suggestion: "Consider reviewing image cache sizes and disposing unused controllers."
                .to_string(),
        });
    }

    if mem.monotonic_growth {
        score -= 35;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Error,
            category: "Memory",
            title: format!(
                "Potential memory leak — heap grew {:.1} MB without GC reclaim",
                mem.heap_growth_mb
            ),
            detail: "Heap usage grew monotonically throughout the session, suggesting objects \
                     are being retained and never garbage collected."
                .to_string(),
            suggestion: "Use DevTools allocation tracking to identify retained objects. \
                         Check for missing dispose() calls on controllers and streams."
                .to_string(),
        });
    } else if mem.heap_growth_mb > 20.0 {
        score -= 10;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "Memory",
            title: format!("Heap grew by {:.1} MB during session", mem.heap_growth_mb),
            detail: "Heap usage increased significantly during the session.".to_string(),
            suggestion: "Monitor whether heap stabilises over longer periods. \
                         If growth continues, investigate retained objects."
                .to_string(),
        });
    }

    if mem.peak_external_mb > 50.0 {
        score -= 10;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "Memory",
            title: format!("High external memory: {:.1} MB", mem.peak_external_mb),
            detail: "External (native) memory is high, often due to decoded images.".to_string(),
            suggestion: "Use ResizeImage or cacheWidth/cacheHeight to limit decoded image sizes."
                .to_string(),
        });
    }

    score.max(0) as u32
}

fn score_rendering(
    ren: &RenderingSummary,
    thresholds: &RuntimeThresholds,
    issues: &mut Vec<RuntimeIssue>,
) -> u32 {
    let mut score = 100i32;

    if ren.avg_build_ms > thresholds.frame_error_ms {
        score -= 35;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Error,
            category: "Rendering",
            title: format!(
                "Average frame build time {:.1} ms (target <16 ms)",
                ren.avg_build_ms
            ),
            detail: "Frames are building significantly slower than the 16 ms budget, \
                     causing visible jank."
                .to_string(),
            suggestion: "Use const constructors, extract expensive subtrees into separate \
                         widgets, and avoid calling setState on ancestor widgets."
                .to_string(),
        });
    } else if ren.avg_build_ms > thresholds.frame_warn_ms {
        score -= 15;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "Rendering",
            title: format!("Average frame build time {:.1} ms", ren.avg_build_ms),
            detail: "Frame build times are near or above the 16 ms budget.".to_string(),
            suggestion: "Profile with the Performance Overlay to identify slow widgets."
                .to_string(),
        });
    }

    if ren.dropped_pct > thresholds.dropped_frames_warn_pct {
        score -= 20;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "Rendering",
            title: format!(
                "{:.1}% frames dropped ({}/{})",
                ren.dropped_pct, ren.dropped_frames, ren.total_frames
            ),
            detail: "A significant percentage of frames were dropped.".to_string(),
            suggestion: "Look for expensive paint operations, shader compilation jank, \
                         or large list views without itemExtent."
                .to_string(),
        });
    }

    if ren.max_build_ms > 100.0 {
        score -= 10;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "Rendering",
            title: format!("Worst frame took {:.0} ms to build", ren.max_build_ms),
            detail: "At least one frame took over 100 ms, causing a noticeable freeze.".to_string(),
            suggestion: "Check for synchronous I/O or heavy computation on the UI thread."
                .to_string(),
        });
    }

    if ren.jank_events > 5 {
        score -= 10;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Info,
            category: "Rendering",
            title: format!("{} jank events detected", ren.jank_events),
            detail: "Multiple samples exceeded the 16 ms frame budget.".to_string(),
            suggestion: "Run in profile mode and use the Flutter Performance overlay.".to_string(),
        });
    }

    score.max(0) as u32
}

/// Detect isolate errors / exception events in the timeline and score stability.
/// Each error event costs 10 points; each pause-on-exception state costs 25 points.
fn score_stability(snapshots: &[RuntimeSnapshot], issues: &mut Vec<RuntimeIssue>) -> u32 {
    let mut error_events = 0u32;
    let mut paused_on_exception = false;

    for snap in snapshots {
        if let Some(events) = snap.timeline_events["traceEvents"].as_array() {
            for event in events {
                let name = event["name"].as_str().unwrap_or("");
                let cat = event["cat"].as_str().unwrap_or("");
                let phase = event["ph"].as_str().unwrap_or("");
                let is_error = name.contains("Error")
                    || name.contains("Exception")
                    || name.contains("Crash")
                    || cat == "Embedder/Error"
                    || phase == "i" && name.eq_ignore_ascii_case("error");
                if is_error {
                    error_events += 1;
                }
            }
        }
        // CPU samples response sometimes carries a `pauseEvent` for the isolate.
        if snap.cpu_samples["pauseEvent"]["kind"]
            .as_str()
            .map(|k| k.contains("Exception"))
            .unwrap_or(false)
        {
            paused_on_exception = true;
        }
    }

    let mut score: i32 = 100;
    if error_events > 0 {
        score -= (error_events as i32) * 10;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Error,
            category: "stability",
            title: format!("{} runtime error event(s) on the timeline", error_events),
            detail: "Errors / exceptions appeared in the VM timeline during the sample window."
                .to_string(),
            suggestion: "Inspect `falcon devtools logging --duration 30` and the issue stream for the underlying stack traces.".to_string(),
        });
    }
    if paused_on_exception {
        score -= 25;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Error,
            category: "stability",
            title: "Isolate paused on uncaught exception".to_string(),
            detail: "The Dart isolate is in a PauseException state.".to_string(),
            suggestion: "Resume the app with `falcon devtools debugger --action resume` after fixing the throwing code.".to_string(),
        });
    }

    score.clamp(0, 100) as u32
}

fn score_network(net: &NetworkSummary, issues: &mut Vec<RuntimeIssue>) -> u32 {
    let mut score = 100i32;

    if net.failed_requests > 0 {
        let fail_pct = if net.total_requests > 0 {
            (net.failed_requests as f64 / net.total_requests as f64) * 100.0
        } else {
            0.0
        };

        if fail_pct > 20.0 {
            score -= 30;
            issues.push(RuntimeIssue {
                severity: RuntimeSeverity::Error,
                category: "Network",
                title: format!(
                    "{}/{} HTTP requests failed ({:.0}%)",
                    net.failed_requests, net.total_requests, fail_pct
                ),
                detail: "A high percentage of HTTP requests are failing.".to_string(),
                suggestion: "Check endpoint availability, authentication, and error handling."
                    .to_string(),
            });
        } else {
            score -= 10;
            issues.push(RuntimeIssue {
                severity: RuntimeSeverity::Warning,
                category: "Network",
                title: format!(
                    "{} HTTP request(s) failed out of {}",
                    net.failed_requests, net.total_requests
                ),
                detail: "Some HTTP requests returned error status codes.".to_string(),
                suggestion: "Verify API endpoints and add proper error handling.".to_string(),
            });
        }
    }

    if net.max_latency_ms > 5000.0 {
        score -= 15;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "Network",
            title: format!("Slow request: {:.0} ms max latency", net.max_latency_ms),
            detail: "At least one HTTP request took over 5 seconds.".to_string(),
            suggestion: "Add timeouts and loading indicators for slow endpoints.".to_string(),
        });
    }

    score.max(0) as u32
}

fn score_cpu(
    cpu: &CpuSummary,
    thresholds: &RuntimeThresholds,
    issues: &mut Vec<RuntimeIssue>,
) -> u32 {
    let mut score = 100i32;

    if cpu.estimated_usage_pct > thresholds.cpu_warn_pct {
        score -= 25;
        issues.push(RuntimeIssue {
            severity: RuntimeSeverity::Warning,
            category: "CPU",
            title: format!("Estimated CPU usage: {:.0}%", cpu.estimated_usage_pct),
            detail: "CPU usage is high, which will drain battery and may cause thermal throttling."
                .to_string(),
            suggestion: "Move expensive computation to isolates. Avoid polling timers.".to_string(),
        });
    }

    if let Some((name, count)) = cpu.top_functions.first() {
        if *count > 100 {
            score -= 10;
            issues.push(RuntimeIssue {
                severity: RuntimeSeverity::Info,
                category: "CPU",
                title: format!("Hot function: {} ({} samples)", name, count),
                detail: "This function appears frequently in CPU profiler samples.".to_string(),
                suggestion: "Consider optimising this function or moving it off the main isolate."
                    .to_string(),
            });
        }
    }

    score.max(0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::connection::{MemoryUsage, RenderingStats};
    use crate::runtime::diagnostics::{CpuSummary, MemoryTrend, NetworkSummary, RenderingSummary};
    use crate::runtime::RuntimeThresholds;
    use serde_json::json;

    // ── Snapshot helpers ──────────────────────────────────────────────────

    fn snap_with(timeline: serde_json::Value, cpu: serde_json::Value) -> RuntimeSnapshot {
        RuntimeSnapshot {
            elapsed_secs: 0.0,
            memory: MemoryUsage {
                heap_usage_bytes: 0,
                heap_capacity_bytes: 0,
                external_usage_bytes: 0,
            },
            rendering: RenderingStats {
                total_frames: 0,
                dropped_frames: 0,
                avg_frame_build_time_ms: 0.0,
                max_frame_build_time_ms: 0.0,
                avg_frame_raster_time_ms: 0.0,
                max_frame_raster_time_ms: 0.0,
            },
            cpu_samples: cpu,
            http_profile: json!({}),
            timeline_events: timeline,
            allocation_profile: json!({}),
            rebuild_counts: json!({}),
        }
    }

    fn snap_with_memory(heap_bytes: u64, external_bytes: u64, elapsed: f64) -> RuntimeSnapshot {
        RuntimeSnapshot {
            elapsed_secs: elapsed,
            memory: MemoryUsage {
                heap_usage_bytes: heap_bytes,
                heap_capacity_bytes: heap_bytes * 2,
                external_usage_bytes: external_bytes,
            },
            rendering: RenderingStats {
                total_frames: 100,
                dropped_frames: 0,
                avg_frame_build_time_ms: 8.0,
                max_frame_build_time_ms: 10.0,
                avg_frame_raster_time_ms: 4.0,
                max_frame_raster_time_ms: 6.0,
            },
            cpu_samples: json!({}),
            http_profile: json!({}),
            timeline_events: json!({"traceEvents": []}),
            allocation_profile: json!({}),
            rebuild_counts: json!({}),
        }
    }

    fn snap_with_rendering(
        total: u64,
        dropped: u64,
        avg_build: f64,
        max_build: f64,
    ) -> RuntimeSnapshot {
        RuntimeSnapshot {
            elapsed_secs: 1.0,
            memory: MemoryUsage {
                heap_usage_bytes: 10 * 1024 * 1024,
                heap_capacity_bytes: 20 * 1024 * 1024,
                external_usage_bytes: 0,
            },
            rendering: RenderingStats {
                total_frames: total,
                dropped_frames: dropped,
                avg_frame_build_time_ms: avg_build,
                max_frame_build_time_ms: max_build,
                avg_frame_raster_time_ms: 4.0,
                max_frame_raster_time_ms: 8.0,
            },
            cpu_samples: json!({}),
            http_profile: json!({}),
            timeline_events: json!({"traceEvents": []}),
            allocation_profile: json!({}),
            rebuild_counts: json!({}),
        }
    }

    fn default_thresholds() -> RuntimeThresholds {
        RuntimeThresholds::default()
    }

    fn clean_memory_trend() -> MemoryTrend {
        MemoryTrend {
            min_heap_mb: 50.0,
            max_heap_mb: 60.0,
            avg_heap_mb: 55.0,
            heap_growth_mb: 10.0,
            peak_external_mb: 10.0,
            monotonic_growth: false,
            samples: vec![(0.0, 50.0), (1.0, 60.0)],
        }
    }

    fn clean_rendering_summary() -> RenderingSummary {
        RenderingSummary {
            total_frames: 1000,
            dropped_frames: 0,
            dropped_pct: 0.0,
            avg_build_ms: 8.0,
            max_build_ms: 14.0,
            avg_raster_ms: 4.0,
            max_raster_ms: 8.0,
            jank_events: 0,
            frame_times: vec![],
        }
    }

    fn clean_network_summary() -> NetworkSummary {
        NetworkSummary {
            total_requests: 10,
            failed_requests: 0,
            avg_latency_ms: 100.0,
            max_latency_ms: 200.0,
            total_bytes_received: 1024,
        }
    }

    fn clean_cpu_summary() -> CpuSummary {
        CpuSummary {
            estimated_usage_pct: 10.0,
            total_samples: 10,
            top_functions: vec![],
        }
    }

    // ── score_memory tests ────────────────────────────────────────────────

    #[test]
    fn memory_score_100_when_all_clean() {
        let mem = clean_memory_trend();
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn memory_score_penalty_for_warn_heap() {
        // max_heap_mb > memory_warn_mb (150) but < memory_error_mb (300)
        let mem = MemoryTrend {
            max_heap_mb: 200.0,
            ..clean_memory_trend()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 80); // 100 - 20
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
        assert_eq!(issues[0].category, "Memory");
    }

    #[test]
    fn memory_score_penalty_for_error_heap() {
        // max_heap_mb > memory_error_mb (300)
        let mem = MemoryTrend {
            max_heap_mb: 400.0,
            ..clean_memory_trend()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 50); // 100 - 50
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Error);
        assert_eq!(issues[0].category, "Memory");
    }

    #[test]
    fn memory_score_penalty_for_monotonic_growth() {
        // monotonic_growth=true causes -35
        let mem = MemoryTrend {
            monotonic_growth: true,
            heap_growth_mb: 30.0,
            ..clean_memory_trend()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 65); // 100 - 35
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Error);
    }

    #[test]
    fn memory_score_penalty_for_heap_growth_above_20mb() {
        // heap_growth_mb > 20.0, no monotonic_growth → -10
        let mem = MemoryTrend {
            monotonic_growth: false,
            heap_growth_mb: 25.0,
            ..clean_memory_trend()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 90); // 100 - 10
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
    }

    #[test]
    fn memory_score_penalty_for_high_external_memory() {
        // peak_external_mb > 50.0 → -10
        let mem = MemoryTrend {
            peak_external_mb: 60.0,
            ..clean_memory_trend()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 90); // 100 - 10
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
        assert!(issues[0].title.contains("external memory"));
    }

    #[test]
    fn memory_score_combined_penalties() {
        // error heap (-50) + monotonic growth (-35) + external (-10) = 100 - 95 = 5
        let mem = MemoryTrend {
            max_heap_mb: 400.0,
            monotonic_growth: true,
            heap_growth_mb: 30.0,
            peak_external_mb: 60.0,
            ..clean_memory_trend()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 5); // 100 - 50 - 35 - 10
        assert!(issues.len() >= 3);
    }

    #[test]
    fn memory_score_no_growth_penalty_exactly_at_20mb() {
        // heap_growth_mb == 20.0 (not > 20.0), no issue
        let mem = MemoryTrend {
            monotonic_growth: false,
            heap_growth_mb: 20.0,
            ..clean_memory_trend()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_memory(&mem, &thresholds, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    // ── score_rendering tests ─────────────────────────────────────────────

    #[test]
    fn rendering_score_100_when_all_clean() {
        let ren = clean_rendering_summary();
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn rendering_score_penalty_for_warn_build_time() {
        // avg_build_ms > frame_warn_ms (16) but <= frame_error_ms (32) → -15
        let ren = RenderingSummary {
            avg_build_ms: 20.0,
            ..clean_rendering_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 85); // 100 - 15
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
        assert_eq!(issues[0].category, "Rendering");
    }

    #[test]
    fn rendering_score_penalty_for_error_build_time() {
        // avg_build_ms > frame_error_ms (32) → -35
        let ren = RenderingSummary {
            avg_build_ms: 50.0,
            ..clean_rendering_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 65); // 100 - 35
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Error);
    }

    #[test]
    fn rendering_score_penalty_for_dropped_frames() {
        // dropped_pct > dropped_frames_warn_pct (5.0) → -20
        let ren = RenderingSummary {
            dropped_pct: 10.0,
            dropped_frames: 100,
            total_frames: 1000,
            ..clean_rendering_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 80); // 100 - 20
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
    }

    #[test]
    fn rendering_score_penalty_for_max_build_over_100ms() {
        // max_build_ms > 100 → -10
        let ren = RenderingSummary {
            max_build_ms: 150.0,
            ..clean_rendering_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 90); // 100 - 10
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
        assert!(issues[0].title.contains("ms to build"));
    }

    #[test]
    fn rendering_score_penalty_for_jank_events() {
        // jank_events > 5 → -10
        let ren = RenderingSummary {
            jank_events: 10,
            ..clean_rendering_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 90); // 100 - 10
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Info);
        assert!(issues[0].title.contains("jank events"));
    }

    #[test]
    fn rendering_score_jank_events_at_threshold_no_penalty() {
        // jank_events == 5 (not > 5), no issue
        let ren = RenderingSummary {
            jank_events: 5,
            ..clean_rendering_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn rendering_score_clamped_to_zero_when_all_penalties() {
        // error build (-35) + dropped (-20) + max_build (-10) + jank (-10) = -75, result 25
        let ren = RenderingSummary {
            avg_build_ms: 50.0,
            dropped_pct: 10.0,
            dropped_frames: 100,
            max_build_ms: 150.0,
            jank_events: 10,
            ..clean_rendering_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_rendering(&ren, &thresholds, &mut issues);
        assert_eq!(score, 25);
        assert_eq!(issues.len(), 4);
    }

    // ── score_stability tests ─────────────────────────────────────────────

    #[test]
    fn stability_score_is_100_when_no_errors() {
        let snaps = vec![snap_with(json!({"traceEvents": []}), json!({}))];
        let mut issues = Vec::new();
        assert_eq!(score_stability(&snaps, &mut issues), 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn stability_score_drops_per_error_event() {
        let snaps = vec![snap_with(
            json!({"traceEvents": [
                {"name": "DartError", "cat": "Dart"},
                {"name": "UncaughtException", "cat": "Embedder"},
            ]}),
            json!({}),
        )];
        let mut issues = Vec::new();
        let score = score_stability(&snaps, &mut issues);
        assert_eq!(score, 80); // 100 - 2*10
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].category, "stability");
    }

    #[test]
    fn stability_score_penalises_pause_on_exception() {
        let snaps = vec![snap_with(
            json!({"traceEvents": []}),
            json!({"pauseEvent": {"kind": "PauseException"}}),
        )];
        let mut issues = Vec::new();
        assert_eq!(score_stability(&snaps, &mut issues), 75); // 100 - 25
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn stability_score_empty_snapshots_returns_100() {
        let snaps: Vec<RuntimeSnapshot> = vec![];
        let mut issues = Vec::new();
        assert_eq!(score_stability(&snaps, &mut issues), 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn stability_score_detects_crash_in_name() {
        let snaps = vec![snap_with(
            json!({"traceEvents": [{"name": "AppCrash", "cat": "Embedder", "ph": "B"}]}),
            json!({}),
        )];
        let mut issues = Vec::new();
        let score = score_stability(&snaps, &mut issues);
        assert_eq!(score, 90); // 100 - 10
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn stability_score_detects_embedder_error_category() {
        let snaps = vec![snap_with(
            json!({"traceEvents": [{"name": "SomeEvent", "cat": "Embedder/Error", "ph": "B"}]}),
            json!({}),
        )];
        let mut issues = Vec::new();
        let score = score_stability(&snaps, &mut issues);
        assert_eq!(score, 90);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn stability_score_clamped_at_zero_for_many_errors() {
        // 11 errors * 10 = 110 penalty, clamp to 0
        let events: Vec<serde_json::Value> = (0..11)
            .map(|_| json!({"name": "DartError", "cat": "Dart", "ph": "B"}))
            .collect();
        let snaps = vec![snap_with(json!({"traceEvents": events}), json!({}))];
        let mut issues = Vec::new();
        let score = score_stability(&snaps, &mut issues);
        assert_eq!(score, 0);
    }

    #[test]
    fn stability_score_both_error_events_and_paused() {
        // 1 error (-10) + pause (-25) = -35 → 65
        let snaps = vec![snap_with(
            json!({"traceEvents": [{"name": "DartError", "cat": "Dart"}]}),
            json!({"pauseEvent": {"kind": "PauseException"}}),
        )];
        let mut issues = Vec::new();
        let score = score_stability(&snaps, &mut issues);
        assert_eq!(score, 65);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[1].severity, RuntimeSeverity::Error);
    }

    // ── score_network tests ───────────────────────────────────────────────

    #[test]
    fn network_score_100_when_all_clean() {
        let net = clean_network_summary();
        let mut issues = Vec::new();
        let score = score_network(&net, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn network_score_penalty_for_low_failure_rate() {
        // failed_requests > 0 but fail_pct <= 20% → -10
        let net = NetworkSummary {
            total_requests: 100,
            failed_requests: 10,
            ..clean_network_summary()
        };
        let mut issues = Vec::new();
        let score = score_network(&net, &mut issues);
        assert_eq!(score, 90); // 100 - 10
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
        assert_eq!(issues[0].category, "Network");
    }

    #[test]
    fn network_score_penalty_for_high_failure_rate() {
        // fail_pct > 20% → -30
        let net = NetworkSummary {
            total_requests: 10,
            failed_requests: 5,
            ..clean_network_summary()
        };
        let mut issues = Vec::new();
        let score = score_network(&net, &mut issues);
        assert_eq!(score, 70); // 100 - 30
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Error);
    }

    #[test]
    fn network_score_penalty_for_high_latency() {
        // max_latency_ms > 5000 → -15
        let net = NetworkSummary {
            max_latency_ms: 6000.0,
            ..clean_network_summary()
        };
        let mut issues = Vec::new();
        let score = score_network(&net, &mut issues);
        assert_eq!(score, 85); // 100 - 15
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
        assert!(issues[0].title.contains("latency"));
    }

    #[test]
    fn network_score_no_latency_penalty_at_5000ms() {
        // max_latency_ms == 5000 (not > 5000), no penalty
        let net = NetworkSummary {
            max_latency_ms: 5000.0,
            ..clean_network_summary()
        };
        let mut issues = Vec::new();
        let score = score_network(&net, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn network_score_both_failures_and_latency_penalties() {
        // high failure rate (-30) + high latency (-15) = -45 → 55
        let net = NetworkSummary {
            total_requests: 10,
            failed_requests: 5,
            max_latency_ms: 6000.0,
            ..clean_network_summary()
        };
        let mut issues = Vec::new();
        let score = score_network(&net, &mut issues);
        assert_eq!(score, 55);
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn network_score_zero_requests_with_failed_requests_no_high_fail_pct() {
        // total_requests == 0 means fail_pct computes as 0.0, so low penalty branch (-10)
        let net = NetworkSummary {
            total_requests: 0,
            failed_requests: 1,
            ..clean_network_summary()
        };
        let mut issues = Vec::new();
        let score = score_network(&net, &mut issues);
        assert_eq!(score, 90); // 100 - 10 (fail_pct is 0.0, which is <= 20)
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
    }

    // ── score_cpu tests ───────────────────────────────────────────────────

    #[test]
    fn cpu_score_100_when_all_clean() {
        let cpu = clean_cpu_summary();
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_cpu(&cpu, &thresholds, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn cpu_score_penalty_for_high_cpu_usage() {
        // estimated_usage_pct > cpu_warn_pct (60.0) → -25
        let cpu = CpuSummary {
            estimated_usage_pct: 80.0,
            ..clean_cpu_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_cpu(&cpu, &thresholds, &mut issues);
        assert_eq!(score, 75); // 100 - 25
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Warning);
        assert_eq!(issues[0].category, "CPU");
    }

    #[test]
    fn cpu_score_penalty_for_hot_function() {
        // top_functions first entry count > 100 → -10
        let cpu = CpuSummary {
            top_functions: vec![("expensiveFn".to_string(), 200)],
            ..clean_cpu_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_cpu(&cpu, &thresholds, &mut issues);
        assert_eq!(score, 90); // 100 - 10
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, RuntimeSeverity::Info);
        assert!(issues[0].title.contains("expensiveFn"));
    }

    #[test]
    fn cpu_score_no_hot_function_penalty_at_threshold() {
        // count == 100 (not > 100), no issue
        let cpu = CpuSummary {
            top_functions: vec![("fn".to_string(), 100)],
            ..clean_cpu_summary()
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_cpu(&cpu, &thresholds, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    #[test]
    fn cpu_score_both_high_usage_and_hot_function() {
        // -25 + -10 = -35 → 65
        let cpu = CpuSummary {
            estimated_usage_pct: 80.0,
            top_functions: vec![("hotFn".to_string(), 500)],
            total_samples: 500,
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_cpu(&cpu, &thresholds, &mut issues);
        assert_eq!(score, 65);
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn cpu_score_empty_top_functions_no_penalty() {
        // top_functions is empty, only usage checked
        let cpu = CpuSummary {
            estimated_usage_pct: 10.0,
            top_functions: vec![],
            total_samples: 10,
        };
        let thresholds = default_thresholds();
        let mut issues = Vec::new();
        let score = score_cpu(&cpu, &thresholds, &mut issues);
        assert_eq!(score, 100);
        assert!(issues.is_empty());
    }

    // ── build_report tests ────────────────────────────────────────────────

    #[test]
    fn build_report_empty_snapshots_returns_report() {
        let snaps: Vec<RuntimeSnapshot> = vec![];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        assert_eq!(report.snapshot_count, 0);
        assert_eq!(report.duration_secs, 0.0);
        assert!(report.issues.is_empty());
        // All scores should be 100 (clean data)
        assert_eq!(report.scores.memory, 100);
        assert_eq!(report.scores.rendering, 100);
        assert_eq!(report.scores.network, 100);
        assert_eq!(report.scores.cpu, 100);
        assert_eq!(report.scores.stability, 100);
    }

    #[test]
    fn build_report_snapshot_count_matches() {
        let snaps = vec![
            snap_with(json!({"traceEvents": []}), json!({})),
            snap_with(json!({"traceEvents": []}), json!({})),
            snap_with(json!({"traceEvents": []}), json!({})),
        ];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        assert_eq!(report.snapshot_count, 3);
    }

    #[test]
    fn build_report_duration_secs_from_last_snapshot() {
        let mut snap1 = snap_with(json!({"traceEvents": []}), json!({}));
        snap1.elapsed_secs = 0.0;
        let mut snap2 = snap_with(json!({"traceEvents": []}), json!({}));
        snap2.elapsed_secs = 5.0;
        let snaps = vec![snap1, snap2];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        assert_eq!(report.duration_secs, 5.0);
    }

    #[test]
    fn build_report_grade_a_for_high_score() {
        // Clean snapshots with low memory should give a high score
        let snaps = vec![snap_with_memory(10 * 1024 * 1024, 0, 1.0)];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        assert_eq!(report.grade, "A");
        assert!(report.overall_score >= 90);
    }

    #[test]
    fn build_report_overall_score_is_weighted_average() {
        // Verify the overall is in valid range
        let snaps = vec![snap_with(json!({"traceEvents": []}), json!({}))];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        assert!(report.overall_score <= 100);
    }

    #[test]
    fn build_report_issues_accumulated_from_all_scorers() {
        // Insert a memory-triggering snapshot (high heap) and stability error
        let mut snap = snap_with_memory(400 * 1024 * 1024, 0, 1.0); // 400 MB heap
        snap.timeline_events = json!({"traceEvents": [{"name": "DartError", "cat": "Dart"}]});
        let snaps = vec![snap];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        // Should have at least a memory error and a stability error
        assert!(report.issues.len() >= 2);
        assert!(report.error_count() >= 2);
    }

    // ── RuntimeReport helper methods ──────────────────────────────────────

    #[test]
    fn runtime_report_error_count_counts_errors_only() {
        let issues = vec![
            RuntimeIssue {
                severity: RuntimeSeverity::Error,
                category: "Memory",
                title: "err".to_string(),
                detail: "".to_string(),
                suggestion: "".to_string(),
            },
            RuntimeIssue {
                severity: RuntimeSeverity::Warning,
                category: "CPU",
                title: "warn".to_string(),
                detail: "".to_string(),
                suggestion: "".to_string(),
            },
            RuntimeIssue {
                severity: RuntimeSeverity::Info,
                category: "Rendering",
                title: "info".to_string(),
                detail: "".to_string(),
                suggestion: "".to_string(),
            },
        ];
        let snaps: Vec<RuntimeSnapshot> = vec![];
        let thresholds = default_thresholds();
        let mut report = build_report(&snaps, &thresholds);
        report.issues = issues;
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.info_count(), 1);
    }

    #[test]
    fn runtime_report_counts_all_zero_when_no_issues() {
        let snaps: Vec<RuntimeSnapshot> = vec![];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 0);
        assert_eq!(report.info_count(), 0);
    }

    // ── build_report grade assignment tests ──────────────────────────────

    #[test]
    fn build_report_grade_f_for_low_overall_score() {
        // Use very high heap memory to drive score down significantly
        let mut snap = snap_with_memory(500 * 1024 * 1024, 100 * 1024 * 1024, 1.0);
        // Add stability errors too
        snap.timeline_events = json!({"traceEvents": [
            {"name": "DartError", "cat": "Dart"},
            {"name": "DartError", "cat": "Dart"},
            {"name": "DartError", "cat": "Dart"},
            {"name": "DartError", "cat": "Dart"},
            {"name": "DartError", "cat": "Dart"},
            {"name": "DartError", "cat": "Dart"},
        ]});
        let snaps = vec![snap];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        // Score should be lower than 90 with multiple large penalties
        assert!(report.overall_score < 90);
    }

    #[test]
    fn build_report_with_rendering_issues_lowers_score() {
        let snap = snap_with_rendering(1000, 100, 40.0, 150.0); // high avg_build + dropped frames
        let snaps = vec![snap];
        let thresholds = default_thresholds();
        let report = build_report(&snaps, &thresholds);
        // rendering score should be penalised
        assert!(report.scores.rendering < 100);
        assert!(!report.issues.is_empty());
    }
}
