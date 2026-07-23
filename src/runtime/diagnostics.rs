//! Diagnostic collectors that sample the Dart VM Service at regular intervals
//! and produce `RuntimeSnapshot` values.

use super::connection::{MemoryUsage, RenderingStats, VmServiceClient};
use anyhow::Result;
use serde_json::Value;

/// A single point-in-time snapshot of all runtime diagnostics.
#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    /// Timestamp relative to the first snapshot (seconds).
    pub elapsed_secs: f64,
    /// Heap and external memory usage.
    pub memory: MemoryUsage,
    /// Frame rendering statistics.
    pub rendering: RenderingStats,
    /// CPU profiler samples (raw JSON — interpreted later).
    pub cpu_samples: Value,
    /// HTTP profile (raw JSON — interpreted later).
    pub http_profile: Value,
    /// VM timeline events (raw JSON — interpreted later).
    pub timeline_events: Value,
    /// Allocation profile per class (raw JSON).
    pub allocation_profile: Value,
    /// Widget rebuild counts (raw JSON).
    pub rebuild_counts: Value,
}

/// Collects a full runtime snapshot from the VM Service.
pub struct DiagnosticCollector {
    client: VmServiceClient,
    start: std::time::Instant,
}

impl DiagnosticCollector {
    pub fn new(client: VmServiceClient) -> Self {
        Self {
            client,
            start: std::time::Instant::now(),
        }
    }

    /// Collect a lightweight snapshot suitable for repeated polling.
    pub async fn collect_snapshot(&self) -> Result<RuntimeSnapshot> {
        let memory = self.client.get_memory_usage().await?;
        let rendering = self.client.get_rendering_stats().await?;
        let http_profile = self.client.get_http_timeline().await.unwrap_or_default();
        let rebuilds = self.client.get_rebuild_counts().await.unwrap_or_default();

        Ok(RuntimeSnapshot {
            elapsed_secs: self.start.elapsed().as_secs_f64(),
            memory,
            rendering,
            cpu_samples: Value::Null,
            http_profile,
            timeline_events: Value::Null,
            allocation_profile: Value::Null,
            rebuild_counts: rebuilds,
        })
    }

    /// Populate the last snapshot with expensive one-shot diagnostics.
    pub async fn enrich_snapshot(&self, snapshot: &mut RuntimeSnapshot) {
        snapshot.cpu_samples = self.client.get_cpu_samples().await.unwrap_or_default();
        snapshot.timeline_events = self.client.get_vm_timeline().await.unwrap_or_default();
        snapshot.allocation_profile = self
            .client
            .get_allocation_profile()
            .await
            .unwrap_or_default();
    }
}

// ──────────────────────────────────────────────────────────────────────
// Higher-level analysis helpers used by the report builder
// ──────────────────────────────────────────────────────────────────────

/// Computed memory trend from a series of snapshots.
#[derive(Debug, Clone)]
pub struct MemoryTrend {
    pub min_heap_mb: f64,
    pub max_heap_mb: f64,
    pub avg_heap_mb: f64,
    pub heap_growth_mb: f64,
    pub peak_external_mb: f64,
    /// True if heap grows monotonically (potential leak).
    pub monotonic_growth: bool,
    /// Per-sample heap values for charting.
    pub samples: Vec<(f64, f64)>, // (elapsed_secs, heap_mb)
}

/// Computed rendering summary.
#[derive(Debug, Clone)]
pub struct RenderingSummary {
    pub total_frames: u64,
    pub dropped_frames: u64,
    pub dropped_pct: f64,
    pub avg_build_ms: f64,
    pub max_build_ms: f64,
    pub avg_raster_ms: f64,
    pub max_raster_ms: f64,
    pub jank_events: u64,
    /// Per-sample frame time for charting.
    pub frame_times: Vec<(f64, f64)>, // (elapsed_secs, build_ms)
}

/// Computed network summary.
#[derive(Debug, Clone)]
pub struct NetworkSummary {
    pub total_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: f64,
    pub max_latency_ms: f64,
    pub total_bytes_received: u64,
}

/// Computed CPU summary.
#[derive(Debug, Clone)]
pub struct CpuSummary {
    pub estimated_usage_pct: f64,
    pub total_samples: u64,
    pub top_functions: Vec<(String, u64)>,
}

/// Extract a `MemoryTrend` from a series of snapshots.
pub fn analyze_memory(snapshots: &[RuntimeSnapshot]) -> MemoryTrend {
    if snapshots.is_empty() {
        return MemoryTrend {
            min_heap_mb: 0.0,
            max_heap_mb: 0.0,
            avg_heap_mb: 0.0,
            heap_growth_mb: 0.0,
            peak_external_mb: 0.0,
            monotonic_growth: false,
            samples: Vec::new(),
        };
    }

    let heaps: Vec<f64> = snapshots.iter().map(|s| s.memory.heap_usage_mb()).collect();
    let externals: Vec<f64> = snapshots
        .iter()
        .map(|s| s.memory.external_usage_mb())
        .collect();

    let min = heaps.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = heaps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg = heaps.iter().sum::<f64>() / heaps.len() as f64;
    let growth = heaps.last().unwrap_or(&0.0) - heaps.first().unwrap_or(&0.0);
    let peak_ext = externals.iter().cloned().fold(0.0f64, f64::max);

    // Check monotonic growth (allow small dips from GC).
    let monotonic = heaps.windows(2).filter(|w| w[1] < w[0] - 0.5).count() == 0 && growth > 5.0;

    let samples = snapshots
        .iter()
        .map(|s| (s.elapsed_secs, s.memory.heap_usage_mb()))
        .collect();

    MemoryTrend {
        min_heap_mb: min,
        max_heap_mb: max,
        avg_heap_mb: avg,
        heap_growth_mb: growth,
        peak_external_mb: peak_ext,
        monotonic_growth: monotonic,
        samples,
    }
}

/// Extract a `RenderingSummary` from a series of snapshots.
pub fn analyze_rendering(snapshots: &[RuntimeSnapshot]) -> RenderingSummary {
    if snapshots.is_empty() {
        return RenderingSummary {
            total_frames: 0,
            dropped_frames: 0,
            dropped_pct: 0.0,
            avg_build_ms: 0.0,
            max_build_ms: 0.0,
            avg_raster_ms: 0.0,
            max_raster_ms: 0.0,
            jank_events: 0,
            frame_times: Vec::new(),
        };
    }

    // Use the latest snapshot's cumulative stats.
    let last = snapshots.last().unwrap();
    let first = snapshots.first().unwrap();

    let total = last
        .rendering
        .total_frames
        .saturating_sub(first.rendering.total_frames)
        .max(last.rendering.total_frames);
    let dropped = last
        .rendering
        .dropped_frames
        .saturating_sub(first.rendering.dropped_frames)
        .max(last.rendering.dropped_frames);
    let dropped_pct = if total > 0 {
        (dropped as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let avg_build: f64 = snapshots
        .iter()
        .map(|s| s.rendering.avg_frame_build_time_ms)
        .sum::<f64>()
        / snapshots.len() as f64;
    let max_build = snapshots
        .iter()
        .map(|s| s.rendering.max_frame_build_time_ms)
        .fold(0.0f64, f64::max);
    let avg_raster: f64 = snapshots
        .iter()
        .map(|s| s.rendering.avg_frame_raster_time_ms)
        .sum::<f64>()
        / snapshots.len() as f64;
    let max_raster = snapshots
        .iter()
        .map(|s| s.rendering.max_frame_raster_time_ms)
        .fold(0.0f64, f64::max);

    // Count jank events (frames > 16ms budget).
    let jank = snapshots
        .iter()
        .filter(|s| s.rendering.max_frame_build_time_ms > 16.0)
        .count() as u64;

    let frame_times = snapshots
        .iter()
        .map(|s| (s.elapsed_secs, s.rendering.avg_frame_build_time_ms))
        .collect();

    RenderingSummary {
        total_frames: total,
        dropped_frames: dropped,
        dropped_pct,
        avg_build_ms: avg_build,
        max_build_ms: max_build,
        avg_raster_ms: avg_raster,
        max_raster_ms: max_raster,
        jank_events: jank,
        frame_times,
    }
}

/// Extract a `NetworkSummary` from a series of snapshots.
pub fn analyze_network(snapshots: &[RuntimeSnapshot]) -> NetworkSummary {
    // Use the latest snapshot's HTTP profile.
    let last = snapshots.last();
    let profile = last.map(|s| &s.http_profile);

    let requests = profile
        .and_then(|p| p["requests"].as_array())
        .map(|arr| arr.len() as u64)
        .unwrap_or(0);

    let failed = profile
        .and_then(|p| p["requests"].as_array())
        .map(|arr| {
            arr.iter()
                .filter(|r| {
                    r["response"]["statusCode"]
                        .as_u64()
                        .is_none_or(|c| c >= 400)
                })
                .count() as u64
        })
        .unwrap_or(0);

    let latencies: Vec<f64> = profile
        .and_then(|p| p["requests"].as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r["endTime"].as_f64().zip(r["startTime"].as_f64()))
                .map(|(end, start)| (end - start) / 1000.0) // micros → ms
                .collect()
        })
        .unwrap_or_default();

    let avg_lat = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<f64>() / latencies.len() as f64
    };
    let max_lat = latencies.iter().cloned().fold(0.0f64, f64::max);

    let total_bytes = profile
        .and_then(|p| p["requests"].as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r["response"]["contentLength"].as_u64())
                .sum()
        })
        .unwrap_or(0);

    NetworkSummary {
        total_requests: requests,
        failed_requests: failed,
        avg_latency_ms: avg_lat,
        max_latency_ms: max_lat,
        total_bytes_received: total_bytes,
    }
}

/// Extract a `CpuSummary` from a series of snapshots.
pub fn analyze_cpu(snapshots: &[RuntimeSnapshot]) -> CpuSummary {
    let last = snapshots.last();
    let samples_data = last.map(|s| &s.cpu_samples);

    let total_samples = samples_data
        .and_then(|d| d["sampleCount"].as_u64())
        .unwrap_or(0);

    // Extract top functions from the stack traces.
    let mut func_counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    if let Some(data) = samples_data {
        if let Some(samples) = data["samples"].as_array() {
            for sample in samples {
                if let Some(stack) = sample["stack"].as_array() {
                    if let Some(top_frame) = stack.first() {
                        let name = top_frame["function"]["name"]
                            .as_str()
                            .unwrap_or("<unknown>")
                            .to_string();
                        *func_counts.entry(name).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let mut top_functions: Vec<(String, u64)> = func_counts.into_iter().collect();
    top_functions.sort_by_key(|e| std::cmp::Reverse(e.1));
    top_functions.truncate(10);

    // Rough CPU estimate based on sample density.
    let estimated_pct = if total_samples > 0 {
        // Heuristic: samples / expected idle rate
        (total_samples as f64 / 100.0).min(100.0)
    } else {
        0.0
    };

    CpuSummary {
        estimated_usage_pct: estimated_pct,
        total_samples,
        top_functions,
    }
}
