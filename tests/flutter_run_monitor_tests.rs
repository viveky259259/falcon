//! Hermetic tests for the `falcon run` live monitor.
//!
//! These cover the parts of `flutter_run::monitor` that don't require a real
//! Flutter VM service: VM URI parsing, the snapshot formatter, the
//! `FlutterRunConfig` defaults, and the "disabled" no-op behaviour of
//! `spawn_monitor`.

use falcon::flutter_run::monitor::{
    format_snapshot, parse_vm_service_uri, spawn_monitor, Snapshot, DEFAULT_MONITOR_INTERVAL_SECS,
};
use falcon::flutter_run::{FlutterError, FlutterRunConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn parses_dart_vm_service_announcement_http() {
    let line = "A Dart VM Service on iPhone 15 is available at: http://127.0.0.1:64591/abc=/";
    assert_eq!(
        parse_vm_service_uri(line).as_deref(),
        Some("http://127.0.0.1:64591/abc=/")
    );
}

#[test]
fn parses_debug_service_listening_ws() {
    let line = "Debug service listening on ws://127.0.0.1:64591/abc=/ws";
    assert_eq!(
        parse_vm_service_uri(line).as_deref(),
        Some("ws://127.0.0.1:64591/abc=/ws")
    );
}

#[test]
fn parses_observatory_announcement_http() {
    let line = "An Observatory debugger and profiler on Pixel 7 is available at: http://127.0.0.1:8181/xyz=/";
    assert_eq!(
        parse_vm_service_uri(line).as_deref(),
        Some("http://127.0.0.1:8181/xyz=/")
    );
}

#[test]
fn default_config_enables_monitor_with_30s_interval() {
    let cfg = FlutterRunConfig::new(".");
    assert_eq!(cfg.monitor_interval, Duration::from_secs(30));
    assert_eq!(DEFAULT_MONITOR_INTERVAL_SECS, 30);
    assert!(cfg.monitor_enabled);
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
    assert!(s.contains("[falcon @"));
    assert!(s.contains("memory="));
    assert!(s.contains("issues="));
    assert!(s.contains("30s"));
    assert!(s.contains("+1 new"));
}

#[test]
fn snapshot_formatter_waiting_state_when_no_vm_attached() {
    let snap = Snapshot::waiting(30, 0, 0);
    let s = format_snapshot(&snap, false);
    assert!(s.contains("VM service not yet attached"));
}

#[test]
fn spawn_monitor_returns_none_when_disabled() {
    let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
    let handle = spawn_monitor(false, Duration::from_secs(30), errors);
    assert!(handle.is_none());
}
