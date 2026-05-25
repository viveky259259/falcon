use super::connection::{launch_flutter_run, VmServiceClient};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use colored::Colorize;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

const DEFAULT_TOP_ALLOCATIONS_LIMIT: usize = 10;
const DEFAULT_TOP_PROCESS_BUCKETS_LIMIT: usize = 10;
const DEFAULT_TOP_REQUESTS_LIMIT: usize = 20;
const DEFAULT_TOP_SOCKETS_LIMIT: usize = 20;
const DEFAULT_TOP_TIMELINE_COUNTS_LIMIT: usize = 10;
const DEFAULT_TOP_TIMELINE_DURATIONS_LIMIT: usize = 10;
const DEFAULT_TOP_HOT_FUNCTIONS_LIMIT: usize = 10;
const DEFAULT_STACK_FRAME_LIMIT: usize = 20;
const DEFAULT_LOG_EVENT_LIMIT: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct AllocationEntry {
    pub class: String,
    pub instances_current: u64,
    pub bytes_current_mb: f64,
    pub accumulated_size_mb: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessBucketEntry {
    pub name: String,
    pub description: Option<String>,
    pub size_mb: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub heap_usage_mb: f64,
    pub heap_capacity_mb: f64,
    pub external_usage_mb: f64,
    pub top_process_buckets: Vec<ProcessBucketEntry>,
    pub top_allocations: Vec<AllocationEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkRequestSummary {
    pub method: Option<String>,
    pub uri: Option<String>,
    pub status: Option<i64>,
    pub duration_ms: Option<f64>,
    pub content_length: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SocketSummary {
    pub id: Option<String>,
    pub address: Option<String>,
    pub port: Option<u64>,
    pub socket_type: Option<String>,
    pub status: Option<String>,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub last_read_time: Option<i64>,
    pub last_write_time: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub duration_secs: u64,
    pub total_requests: usize,
    pub failed_requests: usize,
    pub total_bytes_received: u64,
    pub max_latency_ms: f64,
    pub socket_count: usize,
    pub total_socket_read_bytes: u64,
    pub total_socket_write_bytes: u64,
    pub requests: Vec<NetworkRequestSummary>,
    pub sockets: Vec<SocketSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineDuration {
    pub name: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub duration_secs: u64,
    pub timeline_event_count: usize,
    pub top_event_counts: Vec<TimelineCount>,
    pub top_duration_events: Vec<TimelineDuration>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotFunction {
    pub name: String,
    pub samples: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfilerToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub duration_secs: u64,
    pub sample_count: u64,
    pub sample_period_micros: Option<u64>,
    pub max_stack_depth: Option<u64>,
    pub hot_functions: Vec<HotFunction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebuggerFrame {
    pub function: Option<String>,
    pub code: Option<String>,
    pub script_uri: Option<String>,
    pub token_pos: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebuggerToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub isolate_name: Option<String>,
    pub is_system_isolate: bool,
    pub pause_event_kind: Option<String>,
    pub exception_pause_mode: Option<String>,
    pub frame_count: usize,
    pub frames: Vec<DebuggerFrame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub stream: String,
    pub kind: String,
    pub timestamp_micros: Option<i64>,
    pub message: Option<String>,
    pub logger_name: Option<String>,
    pub level: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoggingToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub duration_secs: u64,
    pub total_events: usize,
    pub stream_counts: Vec<TimelineCount>,
    pub entries: Vec<LogEntry>,
}

#[derive(Debug, Clone, Copy)]
pub enum DebuggerAction {
    Pause,
    Resume,
    StepOver,
    StepIn,
    StepOut,
}

#[derive(Debug, Clone, Serialize)]
pub struct RebuildEntry {
    pub widget: String,
    pub count: u64,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RebuildsToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub total_widgets: usize,
    pub total_rebuilds: u64,
    pub top_widgets: Vec<RebuildEntry>,
    pub stats_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectorNode {
    pub widget: String,
    pub description: Option<String>,
    pub creation_location: Option<String>,
    pub child_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectorToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub total_nodes: usize,
    pub max_depth: usize,
    pub root: Option<InspectorNode>,
    pub top_widgets: Vec<RebuildEntry>,
    pub selected: Option<InspectorNode>,
}

#[derive(Debug, Clone, Copy)]
pub enum ReloadMode {
    HotReload,
    HotRestart,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReloadToolReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub mode: String,
    pub success: bool,
    pub reason: Option<String>,
    pub reloaded_libraries: Option<u64>,
    pub elapsed_ms: u128,
    pub reassembled: bool,
}

pub async fn connect_client(
    project_path: &Path,
    attach_uri: Option<&str>,
) -> Result<(String, VmServiceClient)> {
    let vm_uri = match attach_uri {
        Some(uri) => uri.to_string(),
        None => launch_flutter_run(project_path).await?,
    };
    let client = VmServiceClient::connect(&vm_uri).await?;
    client.enable_extensions().await?;
    Ok((vm_uri, client))
}

pub async fn collect_memory_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
) -> Result<MemoryToolReport> {
    let memory = client.get_memory_usage().await?;
    let process_memory = client
        .get_process_memory_usage()
        .await
        .unwrap_or(Value::Null);
    let allocation_profile = client.get_allocation_profile().await.unwrap_or(Value::Null);

    Ok(MemoryToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        heap_usage_mb: memory.heap_usage_mb(),
        heap_capacity_mb: memory.heap_capacity_mb(),
        external_usage_mb: memory.external_usage_mb(),
        top_process_buckets: extract_process_buckets(
            &process_memory,
            DEFAULT_TOP_PROCESS_BUCKETS_LIMIT,
        ),
        top_allocations: extract_allocations(&allocation_profile, DEFAULT_TOP_ALLOCATIONS_LIMIT),
    })
}

pub async fn collect_network_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
    duration: Duration,
) -> Result<NetworkToolReport> {
    let _ = client.enable_http_timeline_logging(true).await;
    let _ = client.enable_socket_profiling(true).await;
    let _ = client.clear_http_profile().await;
    let _ = client.clear_socket_profile().await;

    tokio::time::sleep(duration).await;

    let http_profile = client.get_http_timeline().await.unwrap_or(Value::Null);
    let socket_profile = client.get_socket_profile().await.unwrap_or(Value::Null);
    let network_summary = summarize_network(&http_profile, &socket_profile);

    Ok(NetworkToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        duration_secs: duration.as_secs(),
        total_requests: network_summary.total_requests,
        failed_requests: network_summary.failed_requests,
        total_bytes_received: network_summary.total_bytes_received,
        max_latency_ms: network_summary.max_latency_ms,
        socket_count: network_summary.socket_count,
        total_socket_read_bytes: network_summary.total_socket_read_bytes,
        total_socket_write_bytes: network_summary.total_socket_write_bytes,
        requests: network_summary.requests,
        sockets: network_summary.sockets,
    })
}

pub async fn collect_performance_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
    duration: Duration,
) -> Result<PerformanceToolReport> {
    let _ = client
        .set_vm_timeline_flags(&["Compiler", "Dart", "Embedder", "GC", "Isolate", "VM"])
        .await;
    let _ = client.clear_vm_timeline().await;
    let start = client.get_vm_timeline_micros().await?;
    tokio::time::sleep(duration).await;
    let end = client.get_vm_timeline_micros().await?;
    let timeline = client
        .get_vm_timeline_range(start, end.saturating_sub(start))
        .await
        .unwrap_or(Value::Null);

    let performance_summary = summarize_performance(&timeline);

    Ok(PerformanceToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        duration_secs: duration.as_secs(),
        timeline_event_count: performance_summary.timeline_event_count,
        top_event_counts: performance_summary.top_event_counts,
        top_duration_events: performance_summary.top_duration_events,
    })
}

pub async fn collect_profiler_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
    duration: Duration,
) -> Result<ProfilerToolReport> {
    let _ = client.set_flag("profiler", "true").await;
    let _ = client.set_flag("profile_period", "250").await;
    let _ = client.clear_cpu_samples().await;
    tokio::time::sleep(duration).await;
    let cpu_samples = client.get_cpu_samples().await?;
    let cpu_summary = summarize_cpu_samples(&cpu_samples);

    Ok(ProfilerToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        duration_secs: duration.as_secs(),
        sample_count: cpu_summary.sample_count,
        sample_period_micros: cpu_summary.sample_period_micros,
        max_stack_depth: cpu_summary.max_stack_depth,
        hot_functions: cpu_summary.hot_functions,
    })
}

pub async fn collect_debugger_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
    action: Option<DebuggerAction>,
) -> Result<DebuggerToolReport> {
    if let Some(action) = action {
        match action {
            DebuggerAction::Pause => {
                let _ = client.pause().await;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            DebuggerAction::Resume => {
                let _ = client.resume(None).await;
            }
            DebuggerAction::StepOver => {
                let _ = client.resume(Some("Over")).await;
            }
            DebuggerAction::StepIn => {
                let _ = client.resume(Some("Into")).await;
            }
            DebuggerAction::StepOut => {
                let _ = client.resume(Some("Out")).await;
            }
        }
    }

    let isolate = client.get_isolate().await?;
    let paused = isolate["pauseEvent"]["kind"]
        .as_str()
        .map(|kind| kind != "Resume")
        .unwrap_or(false);
    let stack = if paused {
        client.get_stack(Some(DEFAULT_STACK_FRAME_LIMIT)).await.ok()
    } else {
        None
    };
    let debugger_summary = summarize_debugger(&isolate, stack.as_ref());

    Ok(DebuggerToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        isolate_name: debugger_summary.isolate_name,
        is_system_isolate: debugger_summary.is_system_isolate,
        pause_event_kind: debugger_summary.pause_event_kind,
        exception_pause_mode: debugger_summary.exception_pause_mode,
        frame_count: debugger_summary.frame_count,
        frames: debugger_summary.frames,
    })
}

pub async fn collect_rebuilds_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
) -> Result<RebuildsToolReport> {
    let _ = client
        .set_flag("ext.flutter.profileWidgetBuilds", "true")
        .await;
    let raw = client.get_rebuild_counts().await.unwrap_or(Value::Null);
    let summary = summarize_rebuilds(&raw);
    Ok(RebuildsToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        total_widgets: summary.total_widgets,
        total_rebuilds: summary.total_rebuilds,
        top_widgets: summary.top_widgets,
        stats_enabled: summary.stats_enabled,
    })
}

pub async fn collect_inspector_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
) -> Result<InspectorToolReport> {
    let tree = client.get_root_widget_tree().await.unwrap_or(Value::Null);
    let selected = client.get_selected_widget().await.unwrap_or(Value::Null);
    let rebuilds = client.get_rebuild_counts().await.unwrap_or(Value::Null);

    let tree_summary = summarize_inspector_tree(&tree);
    let selected_summary = summarize_inspector_node(&selected);
    let rebuild_summary = summarize_rebuilds(&rebuilds);

    Ok(InspectorToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        total_nodes: tree_summary.total_nodes,
        max_depth: tree_summary.max_depth,
        root: tree_summary.root,
        top_widgets: rebuild_summary.top_widgets,
        selected: selected_summary,
    })
}

pub async fn collect_reload_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
    mode: ReloadMode,
) -> Result<ReloadToolReport> {
    let start = std::time::Instant::now();
    let (force, pause) = match mode {
        ReloadMode::HotReload => (false, false),
        ReloadMode::HotRestart => (true, false),
    };
    let resp = client.reload_sources(force, pause).await?;
    let success = resp["success"].as_bool().unwrap_or(false);
    let reason = resp["notices"]
        .as_array()
        .and_then(|n| n.first())
        .and_then(|n| n["message"].as_str())
        .map(|s| s.to_string())
        .or_else(|| resp["reason"].as_str().map(|s| s.to_string()));
    let reloaded_libraries = resp["details"]["loadedLibraryCount"]
        .as_u64()
        .or_else(|| resp["loadedLibraryCount"].as_u64());

    let mut reassembled = false;
    if success {
        if client.flutter_reassemble().await.is_ok() {
            reassembled = true;
        }
    }

    Ok(ReloadToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        mode: match mode {
            ReloadMode::HotReload => "reload".into(),
            ReloadMode::HotRestart => "restart".into(),
        },
        success,
        reason,
        reloaded_libraries,
        elapsed_ms: start.elapsed().as_millis(),
        reassembled,
    })
}

pub async fn collect_logging_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
    duration: Duration,
) -> Result<LoggingToolReport> {
    let events = client
        .collect_stream_events(
            &["Stdout", "Stderr", "Logging", "GC", "Extension", "Timer"],
            duration,
            DEFAULT_LOG_EVENT_LIMIT,
        )
        .await?;
    let logging_summary = summarize_logging(&events);

    Ok(LoggingToolReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        duration_secs: duration.as_secs(),
        total_events: logging_summary.total_events,
        stream_counts: logging_summary.stream_counts,
        entries: logging_summary.entries,
    })
}

pub fn print_memory_report(report: &MemoryToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!(
        "{} {:.2} MB",
        "Heap usage:".bright_cyan(),
        report.heap_usage_mb
    );
    println!(
        "{} {:.2} MB",
        "Heap capacity:".bright_cyan(),
        report.heap_capacity_mb
    );
    println!(
        "{} {:.2} MB",
        "External usage:".bright_cyan(),
        report.external_usage_mb
    );

    if !report.top_process_buckets.is_empty() {
        println!();
        println!("{}", "Top process memory buckets".bright_white().bold());
        for entry in &report.top_process_buckets {
            match &entry.description {
                Some(description) => {
                    println!(
                        "  - {}: {:.2} MB ({})",
                        entry.name, entry.size_mb, description
                    );
                }
                None => println!("  - {}: {:.2} MB", entry.name, entry.size_mb),
            }
        }
    }

    if !report.top_allocations.is_empty() {
        println!();
        println!("{}", "Top allocations".bright_white().bold());
        for entry in &report.top_allocations {
            println!(
                "  - {}: {:.2} MB current, {} instances",
                entry.class, entry.bytes_current_mb, entry.instances_current
            );
        }
    }
}

pub fn print_network_report(report: &NetworkToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!("{} {}s", "Duration:".bright_cyan(), report.duration_secs);
    println!(
        "{} {}",
        "HTTP requests:".bright_cyan(),
        report.total_requests
    );
    println!(
        "{} {}",
        "Failed requests:".bright_cyan(),
        report.failed_requests
    );
    println!(
        "{} {}",
        "Socket entries:".bright_cyan(),
        report.socket_count
    );
    println!(
        "{} {} bytes",
        "Received bytes:".bright_cyan(),
        report.total_bytes_received
    );
    println!(
        "{} {} bytes",
        "Socket read bytes:".bright_cyan(),
        report.total_socket_read_bytes
    );
    println!(
        "{} {} bytes",
        "Socket write bytes:".bright_cyan(),
        report.total_socket_write_bytes
    );
    println!(
        "{} {:.2} ms",
        "Max latency:".bright_cyan(),
        report.max_latency_ms
    );
    if !report.requests.is_empty() {
        println!();
        println!("{}", "Slowest requests".bright_white().bold());
        for request in &report.requests {
            println!(
                "  - {} {} [{}] {:.2} ms",
                request.method.as_deref().unwrap_or("?"),
                request.uri.as_deref().unwrap_or("?"),
                request.status.unwrap_or(0),
                request.duration_ms.unwrap_or_default()
            );
        }
    }
    if !report.sockets.is_empty() {
        println!();
        println!("{}", "Busiest sockets".bright_white().bold());
        for socket in &report.sockets {
            println!(
                "  - {}:{} [{}] read={} write={} status={}",
                socket.address.as_deref().unwrap_or("?"),
                socket.port.unwrap_or(0),
                socket.socket_type.as_deref().unwrap_or("?"),
                socket.read_bytes,
                socket.write_bytes,
                socket.status.as_deref().unwrap_or("?")
            );
        }
    }
}

pub fn print_performance_report(report: &PerformanceToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!("{} {}s", "Duration:".bright_cyan(), report.duration_secs);
    println!(
        "{} {}",
        "Timeline events:".bright_cyan(),
        report.timeline_event_count
    );
    if !report.top_event_counts.is_empty() {
        println!();
        println!("{}", "Top event counts".bright_white().bold());
        for event in &report.top_event_counts {
            println!("  - {}: {}", event.name, event.count);
        }
    }
    if !report.top_duration_events.is_empty() {
        println!();
        println!("{}", "Slowest events".bright_white().bold());
        for event in &report.top_duration_events {
            println!("  - {}: {:.2} ms", event.name, event.duration_ms);
        }
    }
}

pub fn print_profiler_report(report: &ProfilerToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!("{} {}s", "Duration:".bright_cyan(), report.duration_secs);
    println!("{} {}", "Sample count:".bright_cyan(), report.sample_count);
    if let Some(sample_period_micros) = report.sample_period_micros {
        println!(
            "{} {} µs",
            "Sample period:".bright_cyan(),
            sample_period_micros
        );
    }
    if let Some(max_stack_depth) = report.max_stack_depth {
        println!("{} {}", "Max stack depth:".bright_cyan(), max_stack_depth);
    }
    if !report.hot_functions.is_empty() {
        println!();
        println!("{}", "Hot functions".bright_white().bold());
        for function in &report.hot_functions {
            println!("  - {}: {}", function.name, function.samples);
        }
    }
}

pub fn print_debugger_report(report: &DebuggerToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    if let Some(name) = &report.isolate_name {
        println!("{} {}", "Isolate name:".bright_cyan(), name);
    }
    println!(
        "{} {}",
        "System isolate:".bright_cyan(),
        report.is_system_isolate
    );
    if let Some(kind) = &report.pause_event_kind {
        println!("{} {}", "Pause event:".bright_cyan(), kind);
    }
    if let Some(mode) = &report.exception_pause_mode {
        println!("{} {}", "Exception pause mode:".bright_cyan(), mode);
    }
    println!("{} {}", "Stack frames:".bright_cyan(), report.frame_count);
    if !report.frames.is_empty() {
        println!();
        println!("{}", "Top stack frames".bright_white().bold());
        for frame in &report.frames {
            println!(
                "  - {} ({})",
                frame.function.as_deref().unwrap_or("<unknown>"),
                frame.script_uri.as_deref().unwrap_or("?")
            );
        }
    }
}

pub fn print_rebuilds_report(report: &RebuildsToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    if !report.stats_enabled {
        println!(
            "{}",
            "  Note: rebuild profiling is off — start the app with `--profile-widget-builds` or call `WidgetsBinding.instance.deferFirstFrame` in profile mode for accurate counts."
                .yellow()
        );
    }
    println!(
        "{} {}",
        "Tracked widgets:".bright_cyan(),
        report.total_widgets
    );
    println!(
        "{} {}",
        "Total rebuilds:".bright_cyan(),
        report.total_rebuilds
    );
    if !report.top_widgets.is_empty() {
        println!();
        println!("{}", "Hot rebuilders".bright_white().bold());
        for entry in &report.top_widgets {
            match &entry.location {
                Some(loc) => println!("  - {} × {}  ({})", entry.widget, entry.count, loc),
                None => println!("  - {} × {}", entry.widget, entry.count),
            }
        }
    }
}

pub fn print_inspector_report(report: &InspectorToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!(
        "{} {}",
        "Widget tree nodes:".bright_cyan(),
        report.total_nodes
    );
    println!("{} {}", "Max depth:".bright_cyan(), report.max_depth);
    if let Some(root) = &report.root {
        println!(
            "{} {} ({} children)",
            "Root:".bright_cyan(),
            root.widget,
            root.child_count
        );
        if let Some(loc) = &root.creation_location {
            println!("  at {}", loc.dimmed());
        }
    }
    if let Some(selected) = &report.selected {
        println!();
        println!("{}", "Selected widget".bright_white().bold());
        println!("  - {}", selected.widget);
        if let Some(desc) = &selected.description {
            println!("    {}", desc.dimmed());
        }
        if let Some(loc) = &selected.creation_location {
            println!("    at {}", loc.dimmed());
        }
    }
    if !report.top_widgets.is_empty() {
        println!();
        println!("{}", "Top rebuilders".bright_white().bold());
        for entry in &report.top_widgets {
            println!("  - {} × {}", entry.widget, entry.count);
        }
    }
}

pub fn print_reload_report(report: &ReloadToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!("{} {}", "Mode:".bright_cyan(), report.mode);
    if report.success {
        println!(
            "{} {} in {} ms",
            "Result:".bright_cyan(),
            "success".green().bold(),
            report.elapsed_ms
        );
    } else {
        println!(
            "{} {} in {} ms",
            "Result:".bright_cyan(),
            "failed".red().bold(),
            report.elapsed_ms
        );
    }
    if let Some(reason) = &report.reason {
        println!("{} {}", "Reason:".bright_cyan(), reason);
    }
    if let Some(libs) = report.reloaded_libraries {
        println!("{} {}", "Libraries reloaded:".bright_cyan(), libs);
    }
    println!(
        "{} {}",
        "Widgets reassembled:".bright_cyan(),
        report.reassembled
    );
    if report.mode == "restart" {
        println!(
            "{}",
            "  Note: full hot restart (state reset) requires `flutter run` to be the controlling process. \
This forces a reload of all sources and reassembles the widget tree, but constructor state is preserved.".dimmed()
        );
    }
}

pub fn print_logging_report(report: &LoggingToolReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!("{} {}s", "Duration:".bright_cyan(), report.duration_secs);
    println!("{} {}", "Total events:".bright_cyan(), report.total_events);
    if !report.stream_counts.is_empty() {
        println!();
        println!("{}", "Stream counts".bright_white().bold());
        for count in &report.stream_counts {
            println!("  - {}: {}", count.name, count.count);
        }
    }
    if !report.entries.is_empty() {
        println!();
        println!("{}", "Recent log entries".bright_white().bold());
        for entry in &report.entries {
            println!(
                "  - [{}] {}",
                entry.stream,
                entry.message.as_deref().unwrap_or("<no message>")
            );
        }
    }
}

#[derive(Debug)]
struct NetworkSummaryData {
    total_requests: usize,
    failed_requests: usize,
    total_bytes_received: u64,
    max_latency_ms: f64,
    socket_count: usize,
    total_socket_read_bytes: u64,
    total_socket_write_bytes: u64,
    requests: Vec<NetworkRequestSummary>,
    sockets: Vec<SocketSummary>,
}

#[derive(Debug)]
struct PerformanceSummaryData {
    timeline_event_count: usize,
    top_event_counts: Vec<TimelineCount>,
    top_duration_events: Vec<TimelineDuration>,
}

#[derive(Debug)]
struct CpuSummaryData {
    sample_count: u64,
    sample_period_micros: Option<u64>,
    max_stack_depth: Option<u64>,
    hot_functions: Vec<HotFunction>,
}

#[derive(Debug)]
struct DebuggerSummaryData {
    isolate_name: Option<String>,
    is_system_isolate: bool,
    pause_event_kind: Option<String>,
    exception_pause_mode: Option<String>,
    frame_count: usize,
    frames: Vec<DebuggerFrame>,
}

#[derive(Debug)]
struct LoggingSummaryData {
    total_events: usize,
    stream_counts: Vec<TimelineCount>,
    entries: Vec<LogEntry>,
}

fn extract_process_buckets(process_memory: &Value, limit: usize) -> Vec<ProcessBucketEntry> {
    let mut buckets = process_memory["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(ProcessBucketEntry {
                name: item["name"].as_str()?.to_string(),
                description: item["description"].as_str().map(ToString::to_string),
                size_mb: bytes_to_mb(item["size"].as_u64().unwrap_or(0)),
            })
        })
        .collect::<Vec<_>>();

    buckets.sort_by(|a, b| b.size_mb.partial_cmp(&a.size_mb).unwrap_or(Ordering::Equal));
    buckets.truncate(limit);
    buckets
}

fn extract_allocations(allocation_profile: &Value, limit: usize) -> Vec<AllocationEntry> {
    let mut allocations = allocation_profile["members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AllocationEntry {
                class: item["class"]["name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                instances_current: item["instancesCurrent"].as_u64().unwrap_or(0),
                bytes_current_mb: bytes_to_mb(item["bytesCurrent"].as_u64().unwrap_or(0)),
                accumulated_size_mb: bytes_to_mb(item["accumulatedSize"].as_u64().unwrap_or(0)),
            })
        })
        .filter(|entry| entry.bytes_current_mb > 0.0)
        .collect::<Vec<_>>();

    allocations.sort_by(|a, b| {
        b.bytes_current_mb
            .partial_cmp(&a.bytes_current_mb)
            .unwrap_or(Ordering::Equal)
    });
    allocations.truncate(limit);
    allocations
}

fn summarize_network(http_profile: &Value, socket_profile: &Value) -> NetworkSummaryData {
    let requests = http_profile["requests"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut request_summaries = requests
        .iter()
        .map(|request| NetworkRequestSummary {
            method: request["method"].as_str().map(ToString::to_string),
            uri: request["uri"].as_str().map(ToString::to_string),
            status: request["response"]["statusCode"]
                .as_i64()
                .or_else(|| request["response"]["statusCode"].as_u64().map(|s| s as i64)),
            duration_ms: duration_ms(request),
            content_length: request["response"]["contentLength"].as_u64(),
        })
        .collect::<Vec<_>>();

    request_summaries.sort_by(|a, b| {
        b.duration_ms
            .unwrap_or_default()
            .partial_cmp(&a.duration_ms.unwrap_or_default())
            .unwrap_or(Ordering::Equal)
    });
    request_summaries.truncate(DEFAULT_TOP_REQUESTS_LIMIT);

    let failed_requests = requests
        .iter()
        .filter(|request| request["response"]["statusCode"].as_i64().unwrap_or(200) >= 400)
        .count();
    let total_bytes_received = requests
        .iter()
        .filter_map(|request| request["response"]["contentLength"].as_u64())
        .sum();
    let max_latency_ms = request_summaries
        .iter()
        .filter_map(|request| request.duration_ms)
        .fold(0.0f64, f64::max);

    let socket_values = socket_profile["sockets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut sockets = socket_values
        .iter()
        .map(|socket| SocketSummary {
            id: socket["id"].as_str().map(ToString::to_string),
            address: socket["address"].as_str().map(ToString::to_string),
            port: socket["port"].as_u64(),
            socket_type: socket["socketType"].as_str().map(ToString::to_string),
            status: Some(
                if socket["endTime"].is_null() {
                    "Open"
                } else {
                    "Closed"
                }
                .to_string(),
            ),
            read_bytes: socket["readBytes"].as_u64().unwrap_or(0),
            write_bytes: socket["writeBytes"].as_u64().unwrap_or(0),
            last_read_time: socket["lastReadTime"]
                .as_i64()
                .or_else(|| socket["lastReadTime"].as_u64().map(|v| v as i64)),
            last_write_time: socket["lastWriteTime"]
                .as_i64()
                .or_else(|| socket["lastWriteTime"].as_u64().map(|v| v as i64)),
        })
        .collect::<Vec<_>>();
    sockets.sort_by(|a, b| {
        let a_bytes = a.read_bytes + a.write_bytes;
        let b_bytes = b.read_bytes + b.write_bytes;
        b_bytes.cmp(&a_bytes)
    });
    sockets.truncate(DEFAULT_TOP_SOCKETS_LIMIT);

    let total_socket_read_bytes = socket_values
        .iter()
        .filter_map(|socket| socket["readBytes"].as_u64())
        .sum();
    let total_socket_write_bytes = socket_values
        .iter()
        .filter_map(|socket| socket["writeBytes"].as_u64())
        .sum();

    NetworkSummaryData {
        total_requests: requests.len(),
        failed_requests,
        total_bytes_received,
        max_latency_ms,
        socket_count: socket_values.len(),
        total_socket_read_bytes,
        total_socket_write_bytes,
        requests: request_summaries,
        sockets,
    }
}

fn summarize_performance(timeline: &Value) -> PerformanceSummaryData {
    let trace_events = timeline["traceEvents"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut durations = Vec::new();
    for event in &trace_events {
        let name = event["name"].as_str().unwrap_or("unknown").to_string();
        *counts.entry(name.clone()).or_insert(0) += 1;
        if let Some(dur) = event["dur"]
            .as_i64()
            .or_else(|| event["dur"].as_u64().map(|v| v as i64))
        {
            durations.push(TimelineDuration {
                name,
                duration_ms: dur as f64 / 1000.0,
            });
        }
    }

    let mut top_event_counts = counts
        .into_iter()
        .map(|(name, count)| TimelineCount { name, count })
        .collect::<Vec<_>>();
    top_event_counts.sort_by(|a, b| b.count.cmp(&a.count));
    top_event_counts.truncate(DEFAULT_TOP_TIMELINE_COUNTS_LIMIT);

    durations.sort_by(|a, b| {
        b.duration_ms
            .partial_cmp(&a.duration_ms)
            .unwrap_or(Ordering::Equal)
    });
    durations.truncate(DEFAULT_TOP_TIMELINE_DURATIONS_LIMIT);

    PerformanceSummaryData {
        timeline_event_count: trace_events.len(),
        top_event_counts,
        top_duration_events: durations,
    }
}

fn summarize_cpu_samples(cpu_samples: &Value) -> CpuSummaryData {
    let functions = cpu_samples["functions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut function_counts: HashMap<String, u64> = HashMap::new();
    if let Some(samples) = cpu_samples["samples"].as_array() {
        for sample in samples {
            let resolved_name = sample["stack"]
                .as_array()
                .and_then(|stack| stack.first())
                .and_then(|top_frame| {
                    frame_index_from_value(top_frame)
                        .and_then(|index| resolve_profile_function_name(&functions, index))
                        .or_else(|| {
                            top_frame["function"]["name"]
                                .as_str()
                                .or_else(|| top_frame["name"].as_str())
                                .map(ToString::to_string)
                        })
                })
                .or_else(|| sample["vmTag"].as_str().map(ToString::to_string));
            // Skip samples with neither a resolvable top frame nor a vmTag —
            // counting them as "<unknown>" pollutes hot-function rankings and
            // makes ordering non-deterministic when ties form.
            let Some(name) = resolved_name else { continue };
            *function_counts.entry(name).or_insert(0) += 1;
        }
    }

    let mut hot_functions = function_counts
        .into_iter()
        .map(|(name, samples)| HotFunction { name, samples })
        .collect::<Vec<_>>();
    hot_functions.sort_by(|a, b| b.samples.cmp(&a.samples));
    hot_functions.truncate(DEFAULT_TOP_HOT_FUNCTIONS_LIMIT);

    CpuSummaryData {
        sample_count: cpu_samples["sampleCount"].as_u64().unwrap_or_else(|| {
            cpu_samples["samples"]
                .as_array()
                .map_or(0, |samples| samples.len() as u64)
        }),
        sample_period_micros: cpu_samples["samplePeriod"].as_u64(),
        max_stack_depth: cpu_samples["maxStackDepth"].as_u64().or_else(|| {
            cpu_samples["samples"].as_array().and_then(|samples| {
                samples
                    .iter()
                    .filter_map(|sample| sample["stack"].as_array().map(|stack| stack.len() as u64))
                    .max()
            })
        }),
        hot_functions,
    }
}

fn frame_index_from_value(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .map(|index| index as usize)
        .or_else(|| value.as_i64().map(|index| index as usize))
}

fn resolve_profile_function_name(functions: &[Value], index: usize) -> Option<String> {
    let function = functions.get(index)?;
    function["function"]["name"]
        .as_str()
        .or_else(|| function["name"].as_str())
        .map(ToString::to_string)
}

fn summarize_debugger(isolate: &Value, stack: Option<&Value>) -> DebuggerSummaryData {
    let frames = stack
        .and_then(|stack| stack["frames"].as_array())
        .into_iter()
        .flatten()
        .map(|frame| DebuggerFrame {
            function: frame["function"]["name"].as_str().map(ToString::to_string),
            code: frame["code"]["name"].as_str().map(ToString::to_string),
            script_uri: frame["location"]["script"]["uri"]
                .as_str()
                .map(ToString::to_string),
            token_pos: frame["location"]["tokenPos"]
                .as_i64()
                .or_else(|| frame["location"]["tokenPos"].as_u64().map(|v| v as i64)),
        })
        .collect::<Vec<_>>();

    DebuggerSummaryData {
        isolate_name: isolate["name"].as_str().map(ToString::to_string),
        is_system_isolate: isolate["isSystemIsolate"].as_bool().unwrap_or(false),
        pause_event_kind: isolate["pauseEvent"]["kind"]
            .as_str()
            .map(ToString::to_string),
        exception_pause_mode: isolate["exceptionPauseMode"]
            .as_str()
            .map(ToString::to_string),
        frame_count: frames.len(),
        frames,
    }
}

fn summarize_logging(events: &[Value]) -> LoggingSummaryData {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut entries = events.iter().map(extract_log_entry).collect::<Vec<_>>();

    for entry in &entries {
        *counts.entry(entry.stream.clone()).or_insert(0) += 1;
    }

    entries.sort_by(|a, b| {
        b.timestamp_micros
            .unwrap_or_default()
            .cmp(&a.timestamp_micros.unwrap_or_default())
    });

    let mut stream_counts = counts
        .into_iter()
        .map(|(name, count)| TimelineCount { name, count })
        .collect::<Vec<_>>();
    stream_counts.sort_by(|a, b| b.count.cmp(&a.count));

    LoggingSummaryData {
        total_events: events.len(),
        stream_counts,
        entries,
    }
}

fn extract_log_entry(params: &Value) -> LogEntry {
    let stream = params["streamId"].as_str().unwrap_or("unknown").to_string();
    let event = &params["event"];
    let timestamp_micros = event["timestamp"]
        .as_i64()
        .or_else(|| event["timestamp"].as_u64().map(|v| v as i64));

    let (kind, message, logger_name, level) = match stream.as_str() {
        "Stdout" | "Stderr" => (
            stream.to_lowercase(),
            decode_event_bytes(event["bytes"].as_str()),
            None,
            None,
        ),
        "Logging" => (
            event["kind"].as_str().unwrap_or("logging").to_string(),
            event["logRecord"]["message"]["valueAsString"]
                .as_str()
                .map(ToString::to_string)
                .or_else(|| {
                    event["logRecord"]["message"]
                        .as_str()
                        .map(ToString::to_string)
                }),
            event["logRecord"]["loggerName"]
                .as_str()
                .map(ToString::to_string),
            event["logRecord"]["level"]
                .as_i64()
                .or_else(|| event["logRecord"]["level"].as_u64().map(|v| v as i64)),
        ),
        _ => (
            event["kind"].as_str().unwrap_or(&stream).to_string(),
            event["extensionData"]["data"]
                .as_str()
                .map(ToString::to_string)
                .or_else(|| Some(event.to_string())),
            None,
            None,
        ),
    };

    LogEntry {
        stream,
        kind,
        timestamp_micros,
        message,
        logger_name,
        level,
    }
}

fn decode_event_bytes(bytes: Option<&str>) -> Option<String> {
    let bytes = bytes?;
    match STANDARD.decode(bytes) {
        Ok(decoded) => Some(String::from_utf8_lossy(&decoded).to_string()),
        Err(_) => Some(bytes.to_string()),
    }
}

fn duration_ms(request: &Value) -> Option<f64> {
    match (
        request["startTime"]
            .as_i64()
            .or_else(|| request["startTime"].as_u64().map(|v| v as i64)),
        request["endTime"]
            .as_i64()
            .or_else(|| request["endTime"].as_u64().map(|v| v as i64)),
    ) {
        (Some(start), Some(end)) if end >= start => Some((end - start) as f64 / 1000.0),
        _ => None,
    }
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[derive(Debug)]
struct RebuildSummary {
    total_widgets: usize,
    total_rebuilds: u64,
    top_widgets: Vec<RebuildEntry>,
    stats_enabled: bool,
}

fn summarize_rebuilds(raw: &Value) -> RebuildSummary {
    // The inspector returns either {"events": [{"name", "count", "location"}, ...]}
    // or {"data": [...]} depending on Flutter version. Be lenient about shape.
    let entries_array = raw["events"]
        .as_array()
        .or_else(|| raw["data"].as_array())
        .or_else(|| raw["counts"].as_array());

    let mut entries: Vec<RebuildEntry> = Vec::new();
    let mut stats_enabled = false;
    let mut total_rebuilds: u64 = 0;

    if let Some(arr) = entries_array {
        stats_enabled = !arr.is_empty();
        for item in arr {
            let widget = item["name"]
                .as_str()
                .or_else(|| item["widget"].as_str())
                .unwrap_or("<unknown>")
                .to_string();
            let count = item["count"].as_u64().unwrap_or(0);
            total_rebuilds += count;
            let location = item["location"]["file"]
                .as_str()
                .map(|f| {
                    let line = item["location"]["line"].as_i64().unwrap_or(0);
                    format!("{}:{}", f, line)
                })
                .or_else(|| item["location"].as_str().map(String::from));
            entries.push(RebuildEntry {
                widget,
                count,
                location,
            });
        }
    } else if raw.is_object() {
        // Fallback: object keyed by widget name → count
        if let Some(obj) = raw.as_object() {
            stats_enabled = !obj.is_empty();
            for (k, v) in obj {
                let count = v.as_u64().unwrap_or(0);
                total_rebuilds += count;
                entries.push(RebuildEntry {
                    widget: k.clone(),
                    count,
                    location: None,
                });
            }
        }
    }

    entries.sort_by(|a, b| b.count.cmp(&a.count));
    let total_widgets = entries.len();
    entries.truncate(DEFAULT_TOP_HOT_FUNCTIONS_LIMIT);

    RebuildSummary {
        total_widgets,
        total_rebuilds,
        top_widgets: entries,
        stats_enabled,
    }
}

#[derive(Debug)]
struct InspectorTreeSummary {
    total_nodes: usize,
    max_depth: usize,
    root: Option<InspectorNode>,
}

fn summarize_inspector_tree(tree: &Value) -> InspectorTreeSummary {
    let root_value = if tree.get("type").is_some() || tree.get("children").is_some() {
        tree
    } else if let Some(result) = tree.get("result") {
        result
    } else {
        tree
    };

    if root_value.is_null() || !root_value.is_object() {
        return InspectorTreeSummary {
            total_nodes: 0,
            max_depth: 0,
            root: None,
        };
    }

    let mut total_nodes = 0usize;
    let mut max_depth = 0usize;
    walk_inspector(root_value, 1, &mut total_nodes, &mut max_depth);

    InspectorTreeSummary {
        total_nodes,
        max_depth,
        root: summarize_inspector_node(root_value),
    }
}

fn walk_inspector(node: &Value, depth: usize, total: &mut usize, max_depth: &mut usize) {
    if !node.is_object() {
        return;
    }
    *total += 1;
    if depth > *max_depth {
        *max_depth = depth;
    }
    if let Some(children) = node["children"].as_array() {
        for child in children {
            walk_inspector(child, depth + 1, total, max_depth);
        }
    }
}

fn summarize_inspector_node(node: &Value) -> Option<InspectorNode> {
    if node.is_null() || !node.is_object() {
        return None;
    }
    let widget = node["type"]
        .as_str()
        .or_else(|| node["widgetRuntimeType"].as_str())
        .or_else(|| node["description"].as_str())
        .unwrap_or("<unknown>")
        .to_string();
    let description = node["description"].as_str().map(String::from);
    let creation_location = node["creationLocation"]["file"]
        .as_str()
        .map(|file| {
            let line = node["creationLocation"]["line"].as_i64().unwrap_or(0);
            format!("{}:{}", file, line)
        })
        .or_else(|| {
            node["createdByLocalProject"]
                .as_bool()
                .filter(|b| *b)
                .and_then(|_| node["creationLocation"].as_str().map(String::from))
        });
    let child_count = node["children"].as_array().map(|c| c.len()).unwrap_or(0);
    Some(InspectorNode {
        widget,
        description,
        creation_location,
        child_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_process_buckets_sorts_and_truncates() {
        let buckets = extract_process_buckets(
            &json!({
                "items": [
                    { "name": "A", "description": "alpha", "size": 10 },
                    { "name": "B", "size": 30 },
                    { "name": "C", "size": 20 },
                    { "description": "missing-name", "size": 99 }
                ]
            }),
            2,
        );

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].name, "B");
        assert_eq!(buckets[1].name, "C");
    }

    #[test]
    fn summarize_network_handles_missing_durations_and_socket_totals() {
        let summary = summarize_network(
            &json!({
                "requests": [
                    {
                        "method": "GET",
                        "uri": "https://a.test",
                        "startTime": 0,
                        "endTime": 5000,
                        "response": { "statusCode": 500, "contentLength": 25 }
                    },
                    {
                        "method": "POST",
                        "uri": "https://b.test",
                        "startTime": 10,
                        "response": { "statusCode": 200, "contentLength": 5 }
                    }
                ]
            }),
            &json!({
                "sockets": [
                    { "id": "s1", "address": "1.1.1.1", "port": 443, "socketType": "tcp", "readBytes": 5, "writeBytes": 7, "endTime": null },
                    { "id": "s2", "address": "2.2.2.2", "port": 80, "socketType": "ws", "readBytes": 50, "writeBytes": 20, "endTime": 1 }
                ]
            }),
        );

        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.failed_requests, 1);
        assert_eq!(summary.total_bytes_received, 30);
        assert_eq!(summary.max_latency_ms, 5.0);
        assert_eq!(summary.socket_count, 2);
        assert_eq!(summary.total_socket_read_bytes, 55);
        assert_eq!(summary.total_socket_write_bytes, 27);
        assert_eq!(summary.sockets[0].id.as_deref(), Some("s2"));
        assert_eq!(summary.requests[0].uri.as_deref(), Some("https://a.test"));
    }

    #[test]
    fn summarize_performance_counts_unknown_and_slowest_events() {
        let summary = summarize_performance(&json!({
            "traceEvents": [
                { "name": "Frame", "dur": 3000 },
                { "name": "Frame", "dur": 1000 },
                { "dur": 9000 }
            ]
        }));

        assert_eq!(summary.timeline_event_count, 3);
        assert_eq!(summary.top_event_counts[0].name, "Frame");
        assert_eq!(summary.top_event_counts[0].count, 2);
        assert_eq!(summary.top_duration_events[0].duration_ms, 9.0);
    }

    #[test]
    fn summarize_cpu_samples_ignores_missing_stack_and_counts_top_frames() {
        let summary = summarize_cpu_samples(&json!({
            "sampleCount": 3,
            "samplePeriod": 1000,
            "maxStackDepth": 128,
            "samples": [
                { "stack": [ { "function": { "name": "foo" } } ] },
                { "stack": [ { "function": { "name": "foo" } } ] },
                { "stack": [ { "function": { "name": "bar" } } ] },
                { "stack": [] },
                {}
            ]
        }));

        assert_eq!(summary.sample_count, 3);
        assert_eq!(summary.sample_period_micros, Some(1000));
        assert_eq!(summary.hot_functions[0].name, "foo");
        assert_eq!(summary.hot_functions[0].samples, 2);
    }

    #[test]
    fn summarize_cpu_samples_resolves_function_indexes_from_profile_table() {
        let summary = summarize_cpu_samples(&json!({
            "sampleCount": 2,
            "samplePeriod": 250,
            "functions": [
                { "function": { "name": "root" } },
                { "function": { "name": "renderFrame" } },
                { "function": { "name": "fetchRepo" } }
            ],
            "samples": [
                { "stack": [2, 1, 0], "vmTag": "Dart" },
                { "stack": [2, 0], "vmTag": "Dart" }
            ]
        }));

        assert_eq!(summary.sample_count, 2);
        assert_eq!(summary.sample_period_micros, Some(250));
        assert_eq!(summary.max_stack_depth, Some(3));
        assert_eq!(summary.hot_functions[0].name, "fetchRepo");
        assert_eq!(summary.hot_functions[0].samples, 2);
    }

    #[test]
    fn summarize_debugger_extracts_frames_for_paused_isolate() {
        let summary = summarize_debugger(
            &json!({
                "name": "main",
                "isSystemIsolate": false,
                "pauseEvent": { "kind": "PauseBreakpoint" },
                "exceptionPauseMode": "Unhandled"
            }),
            Some(&json!({
                "frames": [
                    {
                        "function": { "name": "main" },
                        "code": { "name": "main" },
                        "location": { "script": { "uri": "package:app/main.dart" }, "tokenPos": 42 }
                    }
                ]
            })),
        );

        assert_eq!(summary.isolate_name.as_deref(), Some("main"));
        assert_eq!(summary.pause_event_kind.as_deref(), Some("PauseBreakpoint"));
        assert_eq!(summary.frame_count, 1);
        assert_eq!(
            summary.frames[0].script_uri.as_deref(),
            Some("package:app/main.dart")
        );
    }

    #[test]
    fn summarize_logging_decodes_stdout_and_logging_events() {
        let summary = summarize_logging(&[
            json!({
                "streamId": "Stdout",
                "event": { "timestamp": 2, "bytes": "aGVsbG8K" }
            }),
            json!({
                "streamId": "Logging",
                "event": {
                    "timestamp": 3,
                    "kind": "Logging",
                    "logRecord": {
                        "message": { "valueAsString": "developer log" },
                        "loggerName": "app.logger",
                        "level": 800
                    }
                }
            }),
            json!({
                "streamId": "Stderr",
                "event": { "timestamp": 1, "bytes": "%%%not-base64%%%" }
            }),
        ]);

        assert_eq!(summary.total_events, 3);
        assert_eq!(summary.stream_counts[0].count, 1);
        assert_eq!(summary.entries[0].message.as_deref(), Some("developer log"));
        assert_eq!(summary.entries[1].message.as_deref(), Some("hello\n"));
        assert_eq!(
            summary.entries[2].message.as_deref(),
            Some("%%%not-base64%%%")
        );
    }

    // ── extract_process_buckets ──────────────────────────────────────────────

    #[test]
    fn extract_process_buckets_empty_items_returns_empty() {
        let buckets = extract_process_buckets(&json!({ "items": [] }), 10);
        assert!(buckets.is_empty());
    }

    #[test]
    fn extract_process_buckets_missing_items_returns_empty() {
        let buckets = extract_process_buckets(&json!({}), 10);
        assert!(buckets.is_empty());
    }

    #[test]
    fn extract_process_buckets_limit_zero_returns_empty() {
        let buckets = extract_process_buckets(
            &json!({ "items": [{ "name": "X", "size": 100 }] }),
            0,
        );
        assert!(buckets.is_empty());
    }

    #[test]
    fn extract_process_buckets_optional_description_preserved() {
        let buckets = extract_process_buckets(
            &json!({ "items": [{ "name": "heap", "description": "dart heap", "size": 1048576 }] }),
            5,
        );
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].description.as_deref(), Some("dart heap"));
        assert!((buckets[0].size_mb - 1.0).abs() < 1e-6);
    }

    #[test]
    fn extract_process_buckets_no_description_is_none() {
        let buckets = extract_process_buckets(
            &json!({ "items": [{ "name": "code", "size": 0 }] }),
            5,
        );
        assert_eq!(buckets.len(), 1);
        assert!(buckets[0].description.is_none());
    }

    // ── extract_allocations ──────────────────────────────────────────────────

    #[test]
    fn extract_allocations_filters_zero_bytes_entries() {
        let allocs = extract_allocations(
            &json!({
                "members": [
                    { "class": { "name": "Empty" }, "instancesCurrent": 5, "bytesCurrent": 0, "accumulatedSize": 0 },
                    { "class": { "name": "Big" }, "instancesCurrent": 1, "bytesCurrent": 2097152, "accumulatedSize": 4194304 }
                ]
            }),
            10,
        );
        assert_eq!(allocs.len(), 1);
        assert_eq!(allocs[0].class, "Big");
        assert!((allocs[0].bytes_current_mb - 2.0).abs() < 1e-6);
        assert!((allocs[0].accumulated_size_mb - 4.0).abs() < 1e-6);
    }

    #[test]
    fn extract_allocations_empty_members_returns_empty() {
        let allocs = extract_allocations(&json!({ "members": [] }), 10);
        assert!(allocs.is_empty());
    }

    #[test]
    fn extract_allocations_missing_members_returns_empty() {
        let allocs = extract_allocations(&json!({}), 10);
        assert!(allocs.is_empty());
    }

    #[test]
    fn extract_allocations_sorts_by_bytes_current_desc() {
        let allocs = extract_allocations(
            &json!({
                "members": [
                    { "class": { "name": "Small" }, "instancesCurrent": 1, "bytesCurrent": 1048576, "accumulatedSize": 0 },
                    { "class": { "name": "Large" }, "instancesCurrent": 1, "bytesCurrent": 3145728, "accumulatedSize": 0 }
                ]
            }),
            10,
        );
        assert_eq!(allocs[0].class, "Large");
        assert_eq!(allocs[1].class, "Small");
    }

    #[test]
    fn extract_allocations_missing_class_name_uses_unknown() {
        let allocs = extract_allocations(
            &json!({
                "members": [
                    { "instancesCurrent": 2, "bytesCurrent": 1048576, "accumulatedSize": 0 }
                ]
            }),
            10,
        );
        assert_eq!(allocs.len(), 1);
        assert_eq!(allocs[0].class, "unknown");
    }

    #[test]
    fn extract_allocations_truncates_to_limit() {
        let members: Vec<serde_json::Value> = (1u64..=5)
            .map(|i| {
                json!({ "class": { "name": format!("C{}", i) }, "instancesCurrent": 1, "bytesCurrent": i * 1048576, "accumulatedSize": 0 })
            })
            .collect();
        let allocs = extract_allocations(&json!({ "members": members }), 2);
        assert_eq!(allocs.len(), 2);
    }

    // ── summarize_network (additional branches) ──────────────────────────────

    #[test]
    fn summarize_network_empty_inputs_returns_zero_totals() {
        let summary = summarize_network(&json!({}), &json!({}));
        assert_eq!(summary.total_requests, 0);
        assert_eq!(summary.failed_requests, 0);
        assert_eq!(summary.socket_count, 0);
        assert_eq!(summary.max_latency_ms, 0.0);
    }

    #[test]
    fn summarize_network_open_socket_status_is_open() {
        let summary = summarize_network(
            &json!({ "requests": [] }),
            &json!({
                "sockets": [
                    { "id": "sock1", "address": "localhost", "port": 8080, "socketType": "tcp",
                      "readBytes": 10, "writeBytes": 5, "endTime": null }
                ]
            }),
        );
        assert_eq!(summary.socket_count, 1);
        assert_eq!(summary.sockets[0].status.as_deref(), Some("Open"));
    }

    #[test]
    fn summarize_network_closed_socket_status_is_closed() {
        let summary = summarize_network(
            &json!({ "requests": [] }),
            &json!({
                "sockets": [
                    { "id": "sock2", "address": "remote", "port": 443, "socketType": "tls",
                      "readBytes": 0, "writeBytes": 0, "endTime": 9999 }
                ]
            }),
        );
        assert_eq!(summary.sockets[0].status.as_deref(), Some("Closed"));
    }

    #[test]
    fn summarize_network_sockets_sorted_by_total_bytes_desc() {
        let summary = summarize_network(
            &json!({ "requests": [] }),
            &json!({
                "sockets": [
                    { "id": "small", "readBytes": 1, "writeBytes": 1, "endTime": null },
                    { "id": "big",   "readBytes": 100, "writeBytes": 200, "endTime": null }
                ]
            }),
        );
        assert_eq!(summary.sockets[0].id.as_deref(), Some("big"));
    }

    #[test]
    fn summarize_network_request_without_end_time_has_no_duration() {
        let summary = summarize_network(
            &json!({
                "requests": [
                    { "method": "GET", "uri": "http://x.test", "startTime": 100,
                      "response": { "statusCode": 200, "contentLength": 50 } }
                ]
            }),
            &json!({ "sockets": [] }),
        );
        assert!(summary.requests[0].duration_ms.is_none());
    }

    // ── summarize_performance ────────────────────────────────────────────────

    #[test]
    fn summarize_performance_empty_trace_events_returns_zeroes() {
        let summary = summarize_performance(&json!({ "traceEvents": [] }));
        assert_eq!(summary.timeline_event_count, 0);
        assert!(summary.top_event_counts.is_empty());
        assert!(summary.top_duration_events.is_empty());
    }

    #[test]
    fn summarize_performance_missing_trace_events_returns_zeroes() {
        let summary = summarize_performance(&json!({}));
        assert_eq!(summary.timeline_event_count, 0);
    }

    #[test]
    fn summarize_performance_events_without_dur_excluded_from_durations() {
        let summary = summarize_performance(&json!({
            "traceEvents": [
                { "name": "NoDur" },
                { "name": "WithDur", "dur": 2000 }
            ]
        }));
        assert_eq!(summary.timeline_event_count, 2);
        assert_eq!(summary.top_duration_events.len(), 1);
        assert_eq!(summary.top_duration_events[0].duration_ms, 2.0);
    }

    #[test]
    fn summarize_performance_duration_converts_micros_to_ms() {
        let summary = summarize_performance(&json!({
            "traceEvents": [{ "name": "E", "dur": 1000 }]
        }));
        assert!((summary.top_duration_events[0].duration_ms - 1.0).abs() < 1e-9);
    }

    // ── summarize_cpu_samples ────────────────────────────────────────────────

    #[test]
    fn summarize_cpu_samples_empty_returns_zero_count() {
        let summary = summarize_cpu_samples(&json!({}));
        assert_eq!(summary.sample_count, 0);
        assert!(summary.sample_period_micros.is_none());
        assert!(summary.max_stack_depth.is_none());
        assert!(summary.hot_functions.is_empty());
    }

    #[test]
    fn summarize_cpu_samples_vmtag_fallback_when_no_stack() {
        let summary = summarize_cpu_samples(&json!({
            "samples": [
                { "vmTag": "Native" },
                { "vmTag": "Native" },
                { "vmTag": "GC" }
            ]
        }));
        // sample_count falls back to samples array length
        assert_eq!(summary.sample_count, 3);
        assert_eq!(summary.hot_functions[0].name, "Native");
        assert_eq!(summary.hot_functions[0].samples, 2);
    }

    #[test]
    fn summarize_cpu_samples_infers_max_stack_depth_from_samples() {
        let summary = summarize_cpu_samples(&json!({
            "samples": [
                { "stack": [0, 1, 2] },
                { "stack": [0] }
            ]
        }));
        assert_eq!(summary.max_stack_depth, Some(3));
    }

    #[test]
    fn summarize_cpu_samples_uses_name_field_fallback_in_top_frame() {
        let summary = summarize_cpu_samples(&json!({
            "samples": [
                { "stack": [ { "name": "fallback_fn" } ] }
            ]
        }));
        assert_eq!(summary.hot_functions[0].name, "fallback_fn");
    }

    #[test]
    fn summarize_cpu_samples_skips_sample_with_no_resolvable_name() {
        // stack has integer index 99 which is out of range, no vmTag either
        let summary = summarize_cpu_samples(&json!({
            "functions": [],
            "samples": [
                { "stack": [99] }
            ]
        }));
        assert!(summary.hot_functions.is_empty());
    }

    // ── frame_index_from_value ───────────────────────────────────────────────

    #[test]
    fn frame_index_from_value_u64_returns_some() {
        assert_eq!(frame_index_from_value(&json!(3u64)), Some(3));
    }

    #[test]
    fn frame_index_from_value_i64_returns_some() {
        assert_eq!(frame_index_from_value(&json!(7i64)), Some(7));
    }

    #[test]
    fn frame_index_from_value_string_returns_none() {
        assert_eq!(frame_index_from_value(&json!("not-a-number")), None);
    }

    #[test]
    fn frame_index_from_value_null_returns_none() {
        assert_eq!(frame_index_from_value(&json!(null)), None);
    }

    #[test]
    fn frame_index_from_value_zero_returns_zero() {
        assert_eq!(frame_index_from_value(&json!(0u64)), Some(0));
    }

    // ── resolve_profile_function_name ────────────────────────────────────────

    #[test]
    fn resolve_profile_function_name_happy_path() {
        let functions = vec![
            json!({ "function": { "name": "alpha" } }),
            json!({ "function": { "name": "beta" } }),
        ];
        assert_eq!(
            resolve_profile_function_name(&functions, 1),
            Some("beta".to_string())
        );
    }

    #[test]
    fn resolve_profile_function_name_out_of_bounds_returns_none() {
        let functions = vec![json!({ "function": { "name": "only" } })];
        assert_eq!(resolve_profile_function_name(&functions, 5), None);
    }

    #[test]
    fn resolve_profile_function_name_falls_back_to_name_field() {
        let functions = vec![json!({ "name": "top_level_fn" })];
        assert_eq!(
            resolve_profile_function_name(&functions, 0),
            Some("top_level_fn".to_string())
        );
    }

    #[test]
    fn resolve_profile_function_name_empty_slice_returns_none() {
        let functions: Vec<Value> = vec![];
        assert_eq!(resolve_profile_function_name(&functions, 0), None);
    }

    // ── summarize_debugger ───────────────────────────────────────────────────

    #[test]
    fn summarize_debugger_no_stack_has_zero_frames() {
        let summary = summarize_debugger(
            &json!({ "name": "main", "isSystemIsolate": false }),
            None,
        );
        assert_eq!(summary.frame_count, 0);
        assert!(summary.frames.is_empty());
    }

    #[test]
    fn summarize_debugger_system_isolate_flag() {
        let summary = summarize_debugger(
            &json!({ "name": "system", "isSystemIsolate": true }),
            None,
        );
        assert!(summary.is_system_isolate);
    }

    #[test]
    fn summarize_debugger_missing_isolate_name_is_none() {
        let summary = summarize_debugger(&json!({}), None);
        assert!(summary.isolate_name.is_none());
    }

    #[test]
    fn summarize_debugger_frame_fields_extracted() {
        let summary = summarize_debugger(
            &json!({ "name": "iso" }),
            Some(&json!({
                "frames": [{
                    "function": { "name": "buildWidget" },
                    "code": { "name": "buildWidget_code" },
                    "location": { "script": { "uri": "pkg:app/home.dart" }, "tokenPos": 100 }
                }]
            })),
        );
        assert_eq!(summary.frame_count, 1);
        assert_eq!(summary.frames[0].function.as_deref(), Some("buildWidget"));
        assert_eq!(summary.frames[0].code.as_deref(), Some("buildWidget_code"));
        assert_eq!(summary.frames[0].token_pos, Some(100));
    }

    #[test]
    fn summarize_debugger_exception_pause_mode_extracted() {
        let summary = summarize_debugger(
            &json!({ "exceptionPauseMode": "All" }),
            None,
        );
        assert_eq!(summary.exception_pause_mode.as_deref(), Some("All"));
    }

    // ── summarize_logging ────────────────────────────────────────────────────

    #[test]
    fn summarize_logging_empty_returns_zero_totals() {
        let summary = summarize_logging(&[]);
        assert_eq!(summary.total_events, 0);
        assert!(summary.stream_counts.is_empty());
        assert!(summary.entries.is_empty());
    }

    #[test]
    fn summarize_logging_entries_sorted_by_timestamp_desc() {
        let summary = summarize_logging(&[
            json!({ "streamId": "Stdout", "event": { "timestamp": 10, "bytes": "YQ==" } }),
            json!({ "streamId": "Stdout", "event": { "timestamp": 30, "bytes": "Yg==" } }),
            json!({ "streamId": "Stdout", "event": { "timestamp": 20, "bytes": "Yw==" } }),
        ]);
        assert_eq!(summary.entries[0].timestamp_micros, Some(30));
        assert_eq!(summary.entries[1].timestamp_micros, Some(20));
        assert_eq!(summary.entries[2].timestamp_micros, Some(10));
    }

    #[test]
    fn summarize_logging_stream_counts_aggregated() {
        let summary = summarize_logging(&[
            json!({ "streamId": "Stdout", "event": { "timestamp": 1, "bytes": "YQ==" } }),
            json!({ "streamId": "Stdout", "event": { "timestamp": 2, "bytes": "Yg==" } }),
            json!({ "streamId": "Stderr", "event": { "timestamp": 3, "bytes": "Yw==" } }),
        ]);
        let stdout_count = summary
            .stream_counts
            .iter()
            .find(|c| c.name == "Stdout")
            .map(|c| c.count)
            .unwrap_or(0);
        assert_eq!(stdout_count, 2);
    }

    // ── extract_log_entry ────────────────────────────────────────────────────

    #[test]
    fn extract_log_entry_stdout_decodes_bytes() {
        let entry = extract_log_entry(&json!({
            "streamId": "Stdout",
            "event": { "timestamp": 100, "bytes": "aGVsbG8=" }
        }));
        assert_eq!(entry.stream, "Stdout");
        assert_eq!(entry.kind, "stdout");
        assert_eq!(entry.message.as_deref(), Some("hello"));
        assert_eq!(entry.timestamp_micros, Some(100));
        assert!(entry.logger_name.is_none());
        assert!(entry.level.is_none());
    }

    #[test]
    fn extract_log_entry_stderr_kind_is_stderr() {
        let entry = extract_log_entry(&json!({
            "streamId": "Stderr",
            "event": { "timestamp": 50, "bytes": "ZXJy" }
        }));
        assert_eq!(entry.kind, "stderr");
    }

    #[test]
    fn extract_log_entry_logging_stream_extracts_log_record() {
        let entry = extract_log_entry(&json!({
            "streamId": "Logging",
            "event": {
                "timestamp": 200,
                "kind": "Logging",
                "logRecord": {
                    "message": { "valueAsString": "test msg" },
                    "loggerName": "my.logger",
                    "level": 500
                }
            }
        }));
        assert_eq!(entry.stream, "Logging");
        assert_eq!(entry.message.as_deref(), Some("test msg"));
        assert_eq!(entry.logger_name.as_deref(), Some("my.logger"));
        assert_eq!(entry.level, Some(500));
    }

    #[test]
    fn extract_log_entry_logging_message_string_fallback() {
        let entry = extract_log_entry(&json!({
            "streamId": "Logging",
            "event": {
                "kind": "Logging",
                "logRecord": {
                    "message": "plain string msg"
                }
            }
        }));
        assert_eq!(entry.message.as_deref(), Some("plain string msg"));
    }

    #[test]
    fn extract_log_entry_unknown_stream_uses_extension_data() {
        let entry = extract_log_entry(&json!({
            "streamId": "Extension",
            "event": {
                "kind": "Extension",
                "extensionData": { "data": "ext payload" }
            }
        }));
        assert_eq!(entry.stream, "Extension");
        assert_eq!(entry.kind, "Extension");
        assert_eq!(entry.message.as_deref(), Some("ext payload"));
    }

    #[test]
    fn extract_log_entry_unknown_stream_fallback_to_json() {
        let entry = extract_log_entry(&json!({
            "streamId": "Custom",
            "event": { "kind": "CustomEvent" }
        }));
        assert_eq!(entry.stream, "Custom");
        // Falls back to full event JSON string since no extensionData.data
        assert!(entry.message.is_some());
    }

    #[test]
    fn extract_log_entry_no_bytes_gives_none_message_for_stdout() {
        let entry = extract_log_entry(&json!({
            "streamId": "Stdout",
            "event": { "timestamp": 5 }
        }));
        assert!(entry.message.is_none());
    }

    // ── decode_event_bytes ───────────────────────────────────────────────────

    #[test]
    fn decode_event_bytes_valid_base64_returns_decoded_string() {
        let result = decode_event_bytes(Some("aGVsbG8="));
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn decode_event_bytes_invalid_base64_returns_raw_string() {
        let result = decode_event_bytes(Some("not!valid!base64!!!"));
        assert_eq!(result, Some("not!valid!base64!!!".to_string()));
    }

    #[test]
    fn decode_event_bytes_none_returns_none() {
        assert_eq!(decode_event_bytes(None), None);
    }

    #[test]
    fn decode_event_bytes_empty_string_returns_empty() {
        let result = decode_event_bytes(Some(""));
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn decode_event_bytes_newline_base64_returns_with_newline() {
        // "hello\n" in base64 is "aGVsbG8K"
        let result = decode_event_bytes(Some("aGVsbG8K"));
        assert_eq!(result, Some("hello\n".to_string()));
    }

    // ── duration_ms ──────────────────────────────────────────────────────────

    #[test]
    fn duration_ms_normal_returns_correct_value() {
        let result = duration_ms(&json!({ "startTime": 0, "endTime": 5000 }));
        assert_eq!(result, Some(5.0));
    }

    #[test]
    fn duration_ms_missing_end_returns_none() {
        let result = duration_ms(&json!({ "startTime": 1000 }));
        assert_eq!(result, None);
    }

    #[test]
    fn duration_ms_missing_start_returns_none() {
        let result = duration_ms(&json!({ "endTime": 5000 }));
        assert_eq!(result, None);
    }

    #[test]
    fn duration_ms_end_before_start_returns_none() {
        let result = duration_ms(&json!({ "startTime": 5000, "endTime": 1000 }));
        assert_eq!(result, None);
    }

    #[test]
    fn duration_ms_equal_start_and_end_returns_zero() {
        let result = duration_ms(&json!({ "startTime": 3000, "endTime": 3000 }));
        assert_eq!(result, Some(0.0));
    }

    #[test]
    fn duration_ms_large_values() {
        let result = duration_ms(&json!({ "startTime": 0, "endTime": 1_000_000 }));
        assert_eq!(result, Some(1000.0));
    }

    // ── bytes_to_mb ──────────────────────────────────────────────────────────

    #[test]
    fn bytes_to_mb_zero_returns_zero() {
        assert_eq!(bytes_to_mb(0), 0.0);
    }

    #[test]
    fn bytes_to_mb_one_mb_exact() {
        assert!((bytes_to_mb(1_048_576) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bytes_to_mb_half_mb() {
        assert!((bytes_to_mb(524_288) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn bytes_to_mb_large_value() {
        assert!((bytes_to_mb(1_073_741_824) - 1024.0).abs() < 1e-6);
    }

    // ── summarize_rebuilds ───────────────────────────────────────────────────

    #[test]
    fn summarize_rebuilds_events_array_path() {
        let raw = json!({
            "events": [
                { "name": "MyWidget", "count": 5, "location": { "file": "lib/main.dart", "line": 10 } },
                { "name": "OtherWidget", "count": 2 }
            ]
        });
        let summary = summarize_rebuilds(&raw);
        assert!(summary.stats_enabled);
        assert_eq!(summary.total_widgets, 2);
        assert_eq!(summary.total_rebuilds, 7);
        assert_eq!(summary.top_widgets[0].widget, "MyWidget");
        assert_eq!(summary.top_widgets[0].location.as_deref(), Some("lib/main.dart:10"));
    }

    #[test]
    fn summarize_rebuilds_data_array_fallback() {
        let raw = json!({
            "data": [
                { "name": "WidgetA", "count": 3 }
            ]
        });
        let summary = summarize_rebuilds(&raw);
        assert!(summary.stats_enabled);
        assert_eq!(summary.total_widgets, 1);
    }

    #[test]
    fn summarize_rebuilds_counts_array_fallback() {
        let raw = json!({
            "counts": [
                { "widget": "WidgetB", "count": 7 }
            ]
        });
        let summary = summarize_rebuilds(&raw);
        assert!(summary.stats_enabled);
        assert_eq!(summary.top_widgets[0].widget, "WidgetB");
    }

    #[test]
    fn summarize_rebuilds_object_fallback_keyed_by_name() {
        let raw = json!({
            "TextWidget": 4,
            "ButtonWidget": 9
        });
        let summary = summarize_rebuilds(&raw);
        assert!(summary.stats_enabled);
        assert_eq!(summary.total_widgets, 2);
        assert_eq!(summary.total_rebuilds, 13);
        // Sorted by count desc
        assert_eq!(summary.top_widgets[0].widget, "ButtonWidget");
    }

    #[test]
    fn summarize_rebuilds_empty_events_array_stats_disabled() {
        let raw = json!({ "events": [] });
        let summary = summarize_rebuilds(&raw);
        assert!(!summary.stats_enabled);
        assert_eq!(summary.total_widgets, 0);
        assert_eq!(summary.total_rebuilds, 0);
    }

    #[test]
    fn summarize_rebuilds_missing_name_uses_unknown() {
        let raw = json!({
            "events": [
                { "count": 3 }
            ]
        });
        let summary = summarize_rebuilds(&raw);
        assert_eq!(summary.top_widgets[0].widget, "<unknown>");
    }

    #[test]
    fn summarize_rebuilds_location_string_fallback() {
        let raw = json!({
            "events": [
                { "name": "W", "count": 1, "location": "lib/widget.dart:5" }
            ]
        });
        let summary = summarize_rebuilds(&raw);
        assert_eq!(
            summary.top_widgets[0].location.as_deref(),
            Some("lib/widget.dart:5")
        );
    }

    #[test]
    fn summarize_rebuilds_null_input_returns_empty() {
        let raw = json!(null);
        let summary = summarize_rebuilds(&raw);
        assert_eq!(summary.total_widgets, 0);
        assert!(!summary.stats_enabled);
    }

    // ── summarize_inspector_tree ─────────────────────────────────────────────

    #[test]
    fn summarize_inspector_tree_simple_root() {
        let tree = json!({
            "type": "MaterialApp",
            "description": "MaterialApp",
            "children": [
                { "type": "Scaffold", "children": [] }
            ]
        });
        let summary = summarize_inspector_tree(&tree);
        assert_eq!(summary.total_nodes, 2);
        assert_eq!(summary.max_depth, 2);
        assert!(summary.root.is_some());
        assert_eq!(summary.root.as_ref().unwrap().widget, "MaterialApp");
        assert_eq!(summary.root.as_ref().unwrap().child_count, 1);
    }

    #[test]
    fn summarize_inspector_tree_null_input_returns_empty() {
        let summary = summarize_inspector_tree(&json!(null));
        assert_eq!(summary.total_nodes, 0);
        assert_eq!(summary.max_depth, 0);
        assert!(summary.root.is_none());
    }

    #[test]
    fn summarize_inspector_tree_result_wrapper_unwrapped() {
        let tree = json!({
            "result": {
                "type": "Container",
                "children": []
            }
        });
        let summary = summarize_inspector_tree(&tree);
        assert_eq!(summary.total_nodes, 1);
        assert_eq!(summary.root.as_ref().unwrap().widget, "Container");
    }

    #[test]
    fn summarize_inspector_tree_no_children_single_node() {
        let tree = json!({ "type": "Text" });
        let summary = summarize_inspector_tree(&tree);
        assert_eq!(summary.total_nodes, 1);
        assert_eq!(summary.max_depth, 1);
    }

    #[test]
    fn summarize_inspector_tree_deep_nesting_tracks_max_depth() {
        // depth 3
        let tree = json!({
            "type": "A",
            "children": [{
                "type": "B",
                "children": [{
                    "type": "C",
                    "children": []
                }]
            }]
        });
        let summary = summarize_inspector_tree(&tree);
        assert_eq!(summary.total_nodes, 3);
        assert_eq!(summary.max_depth, 3);
    }

    // ── walk_inspector ───────────────────────────────────────────────────────

    #[test]
    fn walk_inspector_non_object_is_skipped() {
        let mut total = 0usize;
        let mut max_depth = 0usize;
        walk_inspector(&json!("not an object"), 1, &mut total, &mut max_depth);
        assert_eq!(total, 0);
        assert_eq!(max_depth, 0);
    }

    #[test]
    fn walk_inspector_counts_nodes_correctly() {
        let mut total = 0usize;
        let mut max_depth = 0usize;
        let node = json!({
            "type": "Root",
            "children": [
                { "type": "Child1" },
                { "type": "Child2", "children": [{ "type": "GrandChild" }] }
            ]
        });
        walk_inspector(&node, 1, &mut total, &mut max_depth);
        assert_eq!(total, 4);
        assert_eq!(max_depth, 3);
    }

    #[test]
    fn walk_inspector_empty_children_array_still_counts_root() {
        let mut total = 0usize;
        let mut max_depth = 0usize;
        walk_inspector(&json!({ "type": "Leaf", "children": [] }), 1, &mut total, &mut max_depth);
        assert_eq!(total, 1);
        assert_eq!(max_depth, 1);
    }

    // ── summarize_inspector_node ─────────────────────────────────────────────

    #[test]
    fn summarize_inspector_node_null_returns_none() {
        assert!(summarize_inspector_node(&json!(null)).is_none());
    }

    #[test]
    fn summarize_inspector_node_non_object_returns_none() {
        assert!(summarize_inspector_node(&json!("string")).is_none());
    }

    #[test]
    fn summarize_inspector_node_type_field_takes_priority() {
        let node = summarize_inspector_node(&json!({
            "type": "Container",
            "widgetRuntimeType": "WRT",
            "description": "desc"
        }))
        .unwrap();
        assert_eq!(node.widget, "Container");
    }

    #[test]
    fn summarize_inspector_node_runtime_type_fallback() {
        let node = summarize_inspector_node(&json!({
            "widgetRuntimeType": "MyWidget",
            "description": "d"
        }))
        .unwrap();
        assert_eq!(node.widget, "MyWidget");
    }

    #[test]
    fn summarize_inspector_node_description_fallback() {
        let node = summarize_inspector_node(&json!({
            "description": "FallbackWidget"
        }))
        .unwrap();
        assert_eq!(node.widget, "FallbackWidget");
    }

    #[test]
    fn summarize_inspector_node_unknown_fallback() {
        let node = summarize_inspector_node(&json!({})).unwrap();
        assert_eq!(node.widget, "<unknown>");
    }

    #[test]
    fn summarize_inspector_node_creation_location_file_and_line() {
        let node = summarize_inspector_node(&json!({
            "type": "W",
            "creationLocation": { "file": "lib/a.dart", "line": 42 }
        }))
        .unwrap();
        assert_eq!(
            node.creation_location.as_deref(),
            Some("lib/a.dart:42")
        );
    }

    #[test]
    fn summarize_inspector_node_creation_location_created_by_local_project() {
        let node = summarize_inspector_node(&json!({
            "type": "W",
            "createdByLocalProject": true,
            "creationLocation": "lib/b.dart:10"
        }))
        .unwrap();
        assert_eq!(node.creation_location.as_deref(), Some("lib/b.dart:10"));
    }

    #[test]
    fn summarize_inspector_node_child_count_from_children_array() {
        let node = summarize_inspector_node(&json!({
            "type": "Row",
            "children": [{}, {}, {}]
        }))
        .unwrap();
        assert_eq!(node.child_count, 3);
    }

    #[test]
    fn summarize_inspector_node_no_children_field_gives_zero() {
        let node = summarize_inspector_node(&json!({ "type": "Leaf" })).unwrap();
        assert_eq!(node.child_count, 0);
    }

    #[test]
    fn summarize_inspector_node_description_field_extracted() {
        let node = summarize_inspector_node(&json!({
            "type": "T",
            "description": "my desc"
        }))
        .unwrap();
        assert_eq!(node.description.as_deref(), Some("my desc"));
    }
}
