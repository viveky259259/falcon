//! Tests for the runtime analysis module.
//!
//! These tests exercise the diagnostic analysis and report-building logic
//! using synthetic snapshots (no actual VM Service connection required).

use falcon::runtime::connection::{MemoryUsage, RenderingStats};
use falcon::runtime::diagnostics::{
    analyze_cpu, analyze_memory, analyze_network, analyze_rendering, RuntimeSnapshot,
};
use falcon::runtime::report::{build_report, RuntimeSeverity};
use falcon::runtime::RuntimeThresholds;
use serde_json::json;

/// Helper to create a synthetic snapshot with given heap and frame values.
fn make_snapshot(elapsed: f64, heap_bytes: u64, build_ms: f64, dropped: u64) -> RuntimeSnapshot {
    RuntimeSnapshot {
        elapsed_secs: elapsed,
        memory: MemoryUsage {
            heap_usage_bytes: heap_bytes,
            heap_capacity_bytes: heap_bytes * 2,
            external_usage_bytes: 0,
        },
        rendering: RenderingStats {
            total_frames: 60,
            dropped_frames: dropped,
            avg_frame_build_time_ms: build_ms,
            max_frame_build_time_ms: build_ms * 1.5,
            avg_frame_raster_time_ms: build_ms * 0.8,
            max_frame_raster_time_ms: build_ms * 1.2,
        },
        cpu_samples: json!({}),
        http_profile: json!({}),
        timeline_events: json!({}),
        allocation_profile: json!({}),
        rebuild_counts: json!({}),
    }
}

fn mb(mb: f64) -> u64 {
    (mb * 1024.0 * 1024.0) as u64
}

// ── Memory analysis tests ────────────────────────────────────────────

#[test]
fn test_memory_trend_healthy() {
    let snapshots = vec![
        make_snapshot(0.0, mb(50.0), 8.0, 0),
        make_snapshot(1.0, mb(52.0), 8.0, 0),
        make_snapshot(2.0, mb(48.0), 8.0, 0), // GC reclaim
        make_snapshot(3.0, mb(51.0), 8.0, 0),
    ];

    let trend = analyze_memory(&snapshots);
    assert!(
        !trend.monotonic_growth,
        "Should not flag monotonic growth with GC reclaim"
    );
    assert!(trend.avg_heap_mb > 48.0 && trend.avg_heap_mb < 53.0);
    assert_eq!(trend.samples.len(), 4);
}

#[test]
fn test_memory_trend_leak_detected() {
    // Monotonically increasing heap with no GC reclaim.
    let snapshots = vec![
        make_snapshot(0.0, mb(50.0), 8.0, 0),
        make_snapshot(1.0, mb(55.0), 8.0, 0),
        make_snapshot(2.0, mb(60.0), 8.0, 0),
        make_snapshot(3.0, mb(66.0), 8.0, 0),
    ];

    let trend = analyze_memory(&snapshots);
    assert!(
        trend.monotonic_growth,
        "Should detect monotonic heap growth"
    );
    assert!(trend.heap_growth_mb > 15.0);
}

#[test]
fn test_memory_empty_snapshots() {
    let trend = analyze_memory(&[]);
    assert_eq!(trend.samples.len(), 0);
    assert_eq!(trend.avg_heap_mb, 0.0);
}

// ── Rendering analysis tests ─────────────────────────────────────────

#[test]
fn test_rendering_healthy() {
    let snapshots = vec![
        make_snapshot(0.0, mb(50.0), 8.0, 0),
        make_snapshot(1.0, mb(50.0), 9.0, 0),
    ];

    let summary = analyze_rendering(&snapshots);
    assert!(summary.avg_build_ms < 16.0, "Avg build should be healthy");
    assert_eq!(summary.jank_events, 0);
}

#[test]
fn test_rendering_jank_detected() {
    let snapshots = vec![
        make_snapshot(0.0, mb(50.0), 20.0, 3),
        make_snapshot(1.0, mb(50.0), 22.0, 5),
        make_snapshot(2.0, mb(50.0), 25.0, 8),
    ];

    let summary = analyze_rendering(&snapshots);
    assert!(summary.avg_build_ms > 16.0);
    assert!(summary.jank_events > 0, "Should detect jank events");
}

// ── Network analysis tests ───────────────────────────────────────────

#[test]
fn test_network_no_requests() {
    let snapshots = vec![make_snapshot(0.0, mb(50.0), 8.0, 0)];
    let summary = analyze_network(&snapshots);
    assert_eq!(summary.total_requests, 0);
    assert_eq!(summary.failed_requests, 0);
}

#[test]
fn test_network_with_failures() {
    let mut snap = make_snapshot(0.0, mb(50.0), 8.0, 0);
    snap.http_profile = json!({
        "requests": [
            { "startTime": 1000, "endTime": 2000, "response": { "statusCode": 200, "contentLength": 1024 } },
            { "startTime": 3000, "endTime": 8000, "response": { "statusCode": 500, "contentLength": 0 } },
        ]
    });

    let summary = analyze_network(&[snap]);
    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.failed_requests, 1);
}

// ── CPU analysis tests ───────────────────────────────────────────────

#[test]
fn test_cpu_no_samples() {
    let snapshots = vec![make_snapshot(0.0, mb(50.0), 8.0, 0)];
    let summary = analyze_cpu(&snapshots);
    assert_eq!(summary.total_samples, 0);
    assert!(summary.top_functions.is_empty());
}

// ── Full report tests ────────────────────────────────────────────────

#[test]
fn test_report_healthy_app() {
    let snapshots = vec![
        make_snapshot(0.0, mb(50.0), 8.0, 0),
        make_snapshot(1.0, mb(52.0), 9.0, 0),
        make_snapshot(2.0, mb(48.0), 8.5, 0),
    ];

    let thresholds = RuntimeThresholds::default();
    let report = build_report(&snapshots, &thresholds);

    assert!(
        report.overall_score >= 80,
        "Healthy app should score well: got {}",
        report.overall_score
    );
    assert_eq!(
        report.error_count(),
        0,
        "No errors expected for healthy app"
    );
    assert_eq!(report.snapshot_count, 3);
}

#[test]
fn test_report_memory_leak() {
    let snapshots = vec![
        make_snapshot(0.0, mb(100.0), 8.0, 0),
        make_snapshot(1.0, mb(150.0), 8.0, 0),
        make_snapshot(2.0, mb(200.0), 8.0, 0),
        make_snapshot(3.0, mb(250.0), 8.0, 0),
        make_snapshot(4.0, mb(310.0), 8.0, 0), // over error threshold
    ];

    let thresholds = RuntimeThresholds::default();
    let report = build_report(&snapshots, &thresholds);

    assert!(report.overall_score < 80, "Leaking app should score lower");
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.category == "Memory" && i.severity == RuntimeSeverity::Error),
        "Should have a memory error"
    );
}

#[test]
fn test_report_janky_rendering() {
    let snapshots = vec![
        make_snapshot(0.0, mb(50.0), 35.0, 10),
        make_snapshot(1.0, mb(50.0), 40.0, 15),
        make_snapshot(2.0, mb(50.0), 38.0, 12),
    ];

    let thresholds = RuntimeThresholds::default();
    let report = build_report(&snapshots, &thresholds);

    assert!(
        report.issues.iter().any(|i| i.category == "Rendering"),
        "Should detect rendering issues"
    );
}

#[test]
fn test_html_report_generation() {
    let snapshots = vec![
        make_snapshot(0.0, mb(50.0), 8.0, 0),
        make_snapshot(1.0, mb(55.0), 10.0, 1),
    ];

    let thresholds = RuntimeThresholds::default();
    let report = build_report(&snapshots, &thresholds);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    falcon::runtime::write_html_report(&report, tmp.path()).unwrap();

    let html = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(html.contains("Falcon Runtime Report"));
    assert!(html.contains("Memory"));
    assert!(html.contains("Chart.js"));
}
