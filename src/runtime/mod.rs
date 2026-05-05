//! Runtime analysis module for Falcon.
//!
//! Connects to a running Flutter app via the Dart VM Service Protocol,
//! collects runtime diagnostics (memory, rendering, network, CPU),
//! and produces both CLI and HTML reports.

pub mod connection;
pub mod diagnostics;
pub mod live;
pub mod report;
pub mod tools;

use anyhow::Result;
use connection::VmServiceClient;
use diagnostics::{DiagnosticCollector, RuntimeSnapshot};
use report::RuntimeReport;
use std::time::{Duration, Instant};

/// Configuration for a runtime check session.
#[derive(Debug, Clone)]
pub struct RuntimeCheckConfig {
    /// Path to the Flutter project (used for `flutter run`).
    pub project_path: std::path::PathBuf,
    /// Duration to collect diagnostics before generating the report.
    pub duration: Duration,
    /// Whether to launch the app or attach to an already-running one.
    pub attach_uri: Option<String>,
    /// Output path for HTML report.
    pub html_output: Option<std::path::PathBuf>,
    /// Thresholds for issue detection.
    pub thresholds: RuntimeThresholds,
}

/// Tunable thresholds that determine when a metric becomes a warning or error.
#[derive(Debug, Clone)]
pub struct RuntimeThresholds {
    /// Heap usage in MB that triggers a warning.
    pub memory_warn_mb: f64,
    /// Heap usage in MB that triggers an error.
    pub memory_error_mb: f64,
    /// Frame build time (ms) that triggers a jank warning.
    pub frame_warn_ms: f64,
    /// Frame build time (ms) that triggers a jank error.
    pub frame_error_ms: f64,
    /// Dropped frame percentage that triggers a warning.
    pub dropped_frames_warn_pct: f64,
    /// CPU usage percentage that triggers a warning.
    pub cpu_warn_pct: f64,
}

impl Default for RuntimeThresholds {
    fn default() -> Self {
        Self {
            memory_warn_mb: 150.0,
            memory_error_mb: 300.0,
            frame_warn_ms: 16.0,
            frame_error_ms: 32.0,
            dropped_frames_warn_pct: 5.0,
            cpu_warn_pct: 60.0,
        }
    }
}

/// Main entry point: run a full runtime check session.
///
/// 1. Launches `flutter run` (or attaches to the given URI).
/// 2. Connects to the Dart VM Service.
/// 3. Enables diagnostics extensions (memory, rendering, etc.).
/// 4. Collects snapshots for the configured duration.
/// 5. Analyses snapshots and produces a `RuntimeReport`.
pub async fn run_runtime_check(config: &RuntimeCheckConfig) -> Result<RuntimeReport> {
    use colored::Colorize;

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════╗".bright_cyan()
    );
    println!(
        "{}",
        "║         FALCON Runtime Check                    ║".bright_cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════╝".bright_cyan()
    );
    println!();

    // ── Step 1: Connect ──────────────────────────────────────────────
    let vm_uri = if let Some(ref uri) = config.attach_uri {
        eprintln!(
            "  {} Attaching to VM Service at {}",
            "▸".bright_cyan(),
            uri.bright_white()
        );
        uri.clone()
    } else {
        eprintln!(
            "  {} Launching flutter run in {}",
            "▸".bright_cyan(),
            config.project_path.display().to_string().bright_white()
        );
        connection::launch_flutter_run(&config.project_path).await?
    };

    let client = VmServiceClient::connect(&vm_uri).await?;
    eprintln!("  {} Connected to Dart VM Service", "✓".green().bold());

    // ── Step 2: Enable extensions ────────────────────────────────────
    client.enable_extensions().await?;
    eprintln!(
        "  {} Enabled runtime extensions (memory, rendering, network)",
        "✓".green().bold()
    );

    // ── Step 3: Collect snapshots ────────────────────────────────────
    let collector = DiagnosticCollector::new(client);
    let sample_interval = Duration::from_secs(1);
    let num_samples = (config.duration.as_secs() / sample_interval.as_secs()).max(1);

    eprintln!(
        "  {} Collecting {} samples over {} seconds …",
        "▸".bright_cyan(),
        num_samples.to_string().bright_white(),
        config.duration.as_secs().to_string().bright_white()
    );

    let start = Instant::now();
    let mut snapshots: Vec<RuntimeSnapshot> = Vec::new();

    for i in 0..num_samples {
        let snap = collector.collect_snapshot().await?;
        snapshots.push(snap);

        if i < num_samples - 1 {
            tokio::time::sleep(sample_interval).await;
        }

        // Progress indicator every 5 seconds.
        let elapsed = start.elapsed().as_secs();
        if elapsed > 0 && elapsed % 5 == 0 && i > 0 {
            eprint!(".");
        }
    }
    eprintln!();

    if let Some(last_snapshot) = snapshots.last_mut() {
        collector.enrich_snapshot(last_snapshot).await;
    }

    eprintln!(
        "  {} Collected {} snapshots in {:.1}s",
        "✓".green().bold(),
        snapshots.len(),
        start.elapsed().as_secs_f64()
    );

    // ── Step 4: Analyse & build report ───────────────────────────────
    let report = report::build_report(&snapshots, &config.thresholds);

    Ok(report)
}

/// Print the runtime report to the console.
pub fn print_console_report(report: &RuntimeReport) {
    report::console::print_report(report);
}

/// Write an HTML dashboard report to disk.
pub fn write_html_report(report: &RuntimeReport, path: &std::path::Path) -> Result<()> {
    report::html::write_report(report, path)
}
