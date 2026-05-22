//! Live 30s monitor for `falcon run`.
//!
//! While `flutter run` streams its own log lines, this monitor periodically
//! samples the Dart VM Service (once a URI has been parsed from flutter's
//! stdout) and prints a compact, visually-distinct snapshot to stderr so the
//! developer can see memory and issue trends without leaving `falcon run`.
//!
//! Design constraints:
//! - `flutter_run::run_flutter_app` is sync (`Command::spawn` + thread-pumped
//!   stdout/stderr). We deliberately keep that sync; the monitor lives on its
//!   own OS thread with its own single-threaded tokio runtime and talks to the
//!   parent via a `crossbeam`-flavoured `mpsc::channel`.
//! - The monitor is best-effort: if the VM service isn't attached yet, or if a
//!   metric is unavailable (web builds, paused isolates, etc.), the snapshot
//!   degrades gracefully to `n/a` rather than failing the run.
//! - On flutter exit we send a shutdown signal so the monitor thread doesn't
//!   hang on a vanished VM service.

use crate::flutter_run::FlutterError;
use anyhow::Result;
use colored::Colorize;
use regex::Regex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Minimum allowed monitor interval (seconds). Anything lower would drown the
/// terminal in metrics and put pressure on the VM service.
pub const MIN_MONITOR_INTERVAL_SECS: u64 = 5;
/// Maximum allowed monitor interval (seconds). Beyond 10 minutes the snapshots
/// stop being useful as a "live" view.
pub const MAX_MONITOR_INTERVAL_SECS: u64 = 600;
/// Default monitor interval — non-negotiable 30 seconds.
pub const DEFAULT_MONITOR_INTERVAL_SECS: u64 = 30;

// ─────────────────────────────────────────────────────────────────────────────
// VM service URI parsing
// ─────────────────────────────────────────────────────────────────────────────

fn vm_uri_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Match either:
        //   "...available at: http(s)://host:port/TOKEN=/"
        //   "...listening on ws(s)://host:port/TOKEN=/ws"
        // Tolerant: host can be IPv4, localhost, or any non-whitespace hostname.
        Regex::new(r"(?P<uri>(?:https?|wss?)://[^\s]+)").expect("vm uri regex compiles")
    })
}

/// Parse a Dart VM Service URI from a single line of `flutter run` output.
///
/// Returns `Some(uri)` when the line contains a recognizable VM service /
/// Observatory announcement. Returns `None` otherwise.
pub fn parse_vm_service_uri(line: &str) -> Option<String> {
    let line = line.trim();
    // Only consider lines that look like an announcement to avoid grabbing
    // unrelated URLs (e.g. a stack trace pointing at a package on pub.dev).
    let is_announcement = line.contains("Dart VM Service")
        || line.contains("Observatory debugger")
        || line.contains("Debug service listening");
    if !is_announcement {
        return None;
    }

    let caps = vm_uri_regex().captures(line)?;
    let uri = caps.name("uri")?.as_str().trim_end_matches(['.', ',']);
    Some(uri.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Snapshot formatter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub elapsed_secs: u64,
    pub memory_mb: Option<f64>,
    pub memory_delta_mb: Option<f64>,
    pub issues_total: usize,
    pub issues_delta: usize,
    pub dropped_frames_pct: Option<f64>,
    pub frames_per_sec: Option<f64>,
}

impl Snapshot {
    pub fn waiting(elapsed_secs: u64, issues_total: usize, issues_delta: usize) -> Self {
        Self {
            elapsed_secs,
            memory_mb: None,
            memory_delta_mb: None,
            issues_total,
            issues_delta,
            dropped_frames_pct: None,
            frames_per_sec: None,
        }
    }
}

/// Format a memory value with optional delta. `n/a` when unavailable.
fn fmt_memory(mb: Option<f64>, delta: Option<f64>) -> String {
    match mb {
        Some(v) => match delta {
            Some(d) if d.abs() >= 0.5 => {
                let sign = if d >= 0.0 { "+" } else { "" };
                format!("memory={:.0}MB ({}{}MB)", v, sign, d.round() as i64)
            }
            _ => format!("memory={:.0}MB", v),
        },
        None => "memory=n/a".to_string(),
    }
}

fn fmt_issues(total: usize, delta: usize) -> String {
    if delta > 0 {
        format!("issues={} (+{} new)", total, delta)
    } else {
        format!("issues={}", total)
    }
}

fn fmt_frames(fps: Option<f64>, dropped_pct: Option<f64>) -> String {
    match (fps, dropped_pct) {
        (Some(f), Some(d)) => format!("frames={:.0}fps dropped={:.0}%", f, d),
        (Some(f), None) => format!("frames={:.0}fps", f),
        (None, Some(d)) => format!("dropped={:.0}%", d),
        (None, None) => String::new(),
    }
}

/// Render a snapshot as a single line for stderr. Colored cyan prefix; if a
/// memory threshold is breached the prefix tints yellow/red.
pub fn format_snapshot(snap: &Snapshot, vm_attached: bool) -> String {
    let prefix_plain = format!("[falcon @ {}s]", snap.elapsed_secs);
    let mem = snap.memory_mb.unwrap_or(0.0);
    let prefix = if !vm_attached {
        prefix_plain.bright_cyan().to_string()
    } else if mem >= 350.0 || snap.issues_total >= 10 {
        prefix_plain.bright_red().bold().to_string()
    } else if mem >= 200.0 || snap.issues_delta >= 1 {
        prefix_plain.yellow().bold().to_string()
    } else {
        prefix_plain.bright_cyan().bold().to_string()
    };

    if !vm_attached {
        return format!(
            "{} VM service not yet attached — waiting…  {}",
            prefix,
            fmt_issues(snap.issues_total, snap.issues_delta)
        );
    }

    let mut parts = vec![
        fmt_memory(snap.memory_mb, snap.memory_delta_mb),
        fmt_issues(snap.issues_total, snap.issues_delta),
    ];
    let frames = fmt_frames(snap.frames_per_sec, snap.dropped_frames_pct);
    if !frames.is_empty() {
        parts.push(frames);
    }
    format!("{} {}", prefix, parts.join("  "))
}

// ─────────────────────────────────────────────────────────────────────────────
// Monitor handle
// ─────────────────────────────────────────────────────────────────────────────

/// Messages sent from the main thread to the monitor thread.
#[derive(Debug)]
enum MonitorMsg {
    /// VM service URI captured from flutter stdout — connect now.
    VmServiceUri(String),
    /// `flutter run` exited — shut down cleanly.
    Shutdown,
}

/// Handle returned by `spawn_monitor`. Drop it (or call `shutdown`) to stop
/// the background thread. The handle is `Send + Sync` so it can sit alongside
/// the stdout/stderr pumping threads.
pub struct MonitorHandle {
    tx: Sender<MonitorMsg>,
    join: Option<JoinHandle<()>>,
    uri_sent: Arc<Mutex<bool>>,
}

impl MonitorHandle {
    /// Forward a VM service URI if one hasn't been forwarded yet. Subsequent
    /// calls are no-ops (Flutter prints the URI again after hot reload, etc.).
    pub fn notify_vm_uri(&self, uri: String) {
        let mut sent = self.uri_sent.lock().unwrap();
        if *sent {
            return;
        }
        if self.tx.send(MonitorMsg::VmServiceUri(uri)).is_ok() {
            *sent = true;
        }
    }

    /// Ask the monitor to stop and join its thread.
    pub fn shutdown(mut self) {
        let _ = self.tx.send(MonitorMsg::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        if self.join.is_some() {
            let _ = self.tx.send(MonitorMsg::Shutdown);
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
}

/// Spawn the monitor thread.
///
/// Returns `None` when `enabled` is `false` — the caller can treat that as a
/// "do nothing" handle and the run is byte-for-byte identical to today.
pub fn spawn_monitor(
    enabled: bool,
    interval: Duration,
    errors: Arc<Mutex<Vec<FlutterError>>>,
) -> Option<MonitorHandle> {
    if !enabled {
        return None;
    }

    let (tx, rx) = mpsc::channel::<MonitorMsg>();
    let uri_sent = Arc::new(Mutex::new(false));

    let join = std::thread::Builder::new()
        .name("falcon-run-monitor".into())
        .spawn(move || {
            // Each monitor owns its own current-thread tokio runtime so we
            // don't have to make the caller async.
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!(
                        "  {} falcon monitor could not start a tokio runtime: {}",
                        "!".yellow(),
                        e
                    );
                    return;
                }
            };
            rt.block_on(monitor_loop(rx, interval, errors));
        })
        .ok()?;

    Some(MonitorHandle {
        tx,
        join: Some(join),
        uri_sent,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Monitor loop
// ─────────────────────────────────────────────────────────────────────────────

async fn monitor_loop(
    rx: Receiver<MonitorMsg>,
    interval: Duration,
    errors: Arc<Mutex<Vec<FlutterError>>>,
) {
    let start = Instant::now();
    let mut vm_uri: Option<String> = None;
    let mut client: Option<crate::runtime::connection::VmServiceClient> = None;
    let mut last_memory_mb: Option<f64> = None;
    let mut last_issue_count: usize = 0;
    let mut next_tick = Instant::now() + interval;

    loop {
        // Drain any pending messages without blocking.
        loop {
            match rx.try_recv() {
                Ok(MonitorMsg::VmServiceUri(uri)) => {
                    vm_uri = Some(uri);
                }
                Ok(MonitorMsg::Shutdown) => return,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        // Wait until either the next tick or a new message arrives. We poll
        // every 250ms so a Shutdown message is honoured promptly.
        if Instant::now() < next_tick {
            let poll = Duration::from_millis(250);
            match rx.recv_timeout(poll.min(next_tick.saturating_duration_since(Instant::now()))) {
                Ok(MonitorMsg::VmServiceUri(uri)) => {
                    vm_uri = Some(uri);
                    continue;
                }
                Ok(MonitorMsg::Shutdown) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        next_tick = Instant::now() + interval;

        // Lazy connect to the VM service the first time we have a URI.
        if client.is_none() {
            if let Some(uri) = vm_uri.clone() {
                match connect_with_timeout(&uri).await {
                    Ok(c) => client = Some(c),
                    Err(e) => {
                        eprintln!(
                            "  {} falcon monitor could not connect to VM service: {}",
                            "!".yellow(),
                            e
                        );
                        // Keep trying on the next tick.
                    }
                }
            }
        }

        // Build a snapshot.
        let issues_total = errors.lock().map(|g| g.len()).unwrap_or(0);
        let issues_delta = issues_total.saturating_sub(last_issue_count);
        let elapsed_secs = start.elapsed().as_secs();

        let snap = if let Some(c) = client.as_ref() {
            let memory_mb = sample_memory(c).await;
            let delta = match (memory_mb, last_memory_mb) {
                (Some(now), Some(prev)) => Some(now - prev),
                _ => None,
            };
            if memory_mb.is_some() {
                last_memory_mb = memory_mb;
            }
            Snapshot {
                elapsed_secs,
                memory_mb,
                memory_delta_mb: delta,
                issues_total,
                issues_delta,
                dropped_frames_pct: None,
                frames_per_sec: None,
            }
        } else {
            Snapshot::waiting(elapsed_secs, issues_total, issues_delta)
        };

        eprintln!("{}", format_snapshot(&snap, client.is_some()));
        last_issue_count = issues_total;
    }
}

async fn connect_with_timeout(uri: &str) -> Result<crate::runtime::connection::VmServiceClient> {
    let client = tokio::time::timeout(
        Duration::from_secs(15),
        crate::runtime::connection::VmServiceClient::connect(uri),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out connecting to VM service after 15s"))??;
    // Best-effort extension enablement — ignore failure (web builds restrict it).
    let _ = client.enable_extensions().await;
    Ok(client)
}

async fn sample_memory(client: &crate::runtime::connection::VmServiceClient) -> Option<f64> {
    // Don't use collect_memory_report — it pulls allocation profile and process
    // memory, which are heavyweight and unavailable on web. A direct
    // getMemoryUsage call is enough for the live monitor.
    match tokio::time::timeout(Duration::from_secs(5), client.get_memory_usage()).await {
        Ok(Ok(usage)) => Some(usage.heap_usage_mb()),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_dart_vm_service_announcement() {
        let line = "A Dart VM Service on iPhone 15 is available at: http://127.0.0.1:64591/abc=/";
        assert_eq!(
            parse_vm_service_uri(line).as_deref(),
            Some("http://127.0.0.1:64591/abc=/")
        );
    }

    #[test]
    fn parses_ws_debug_service_announcement() {
        let line = "Debug service listening on ws://127.0.0.1:64591/abc=/ws";
        assert_eq!(
            parse_vm_service_uri(line).as_deref(),
            Some("ws://127.0.0.1:64591/abc=/ws")
        );
    }

    #[test]
    fn parses_observatory_announcement() {
        let line = "An Observatory debugger and profiler on Pixel 7 is available at: http://127.0.0.1:8181/xyz=/";
        assert_eq!(
            parse_vm_service_uri(line).as_deref(),
            Some("http://127.0.0.1:8181/xyz=/")
        );
    }

    #[test]
    fn ignores_unrelated_lines_with_urls() {
        let line = "Compiler message: see https://docs.flutter.dev/foo for details";
        assert_eq!(parse_vm_service_uri(line), None);
    }

    #[test]
    fn snapshot_formatter_contains_expected_substrings() {
        let snap = Snapshot {
            elapsed_secs: 30,
            memory_mb: Some(142.0),
            memory_delta_mb: Some(12.0),
            issues_total: 3,
            issues_delta: 1,
            dropped_frames_pct: Some(2.0),
            frames_per_sec: Some(58.0),
        };
        let s = format_snapshot(&snap, true);
        assert!(s.contains("[falcon @"), "missing prefix: {s}");
        assert!(s.contains("30s"), "missing elapsed seconds: {s}");
        assert!(s.contains("memory=142MB"), "missing memory: {s}");
        assert!(s.contains("+12MB"), "missing memory delta: {s}");
        assert!(s.contains("issues=3"), "missing issue total: {s}");
        assert!(s.contains("+1 new"), "missing issue delta: {s}");
    }

    #[test]
    fn snapshot_formatter_waiting_state_when_no_vm() {
        let snap = Snapshot::waiting(30, 0, 0);
        let s = format_snapshot(&snap, false);
        assert!(s.contains("VM service not yet attached"), "got: {s}");
        assert!(s.contains("[falcon @ 30s]"), "got: {s}");
    }

    #[test]
    fn spawn_monitor_returns_none_when_disabled() {
        let errors = Arc::new(Mutex::new(Vec::<FlutterError>::new()));
        let handle = spawn_monitor(false, Duration::from_secs(30), errors);
        assert!(handle.is_none(), "expected no monitor thread when disabled");
    }

    #[test]
    fn memory_formatter_handles_missing_metric() {
        assert_eq!(fmt_memory(None, None), "memory=n/a");
        assert_eq!(fmt_memory(Some(100.0), None), "memory=100MB");
        assert_eq!(fmt_memory(Some(100.0), Some(-12.0)), "memory=100MB (-12MB)");
    }
}
