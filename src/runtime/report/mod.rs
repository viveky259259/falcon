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
    use serde_json::json;

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
}
