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
    let _ = client.clear_cpu_samples().await;
    tokio::time::sleep(duration).await;
    let cpu_samples = client.get_cpu_samples().await.unwrap_or(Value::Null);
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

pub async fn collect_logging_report(
    client: &VmServiceClient,
    vm_service_uri: &str,
    duration: Duration,
) -> Result<LoggingToolReport> {
    let events = client
        .collect_stream_events(
            &["Stdout", "Stderr", "Logging", "GC", "Extension"],
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
    let mut function_counts: HashMap<String, u64> = HashMap::new();
    if let Some(samples) = cpu_samples["samples"].as_array() {
        for sample in samples {
            if let Some(stack) = sample["stack"].as_array() {
                if let Some(top_frame) = stack.first() {
                    let name = top_frame["function"]["name"]
                        .as_str()
                        .unwrap_or("<unknown>")
                        .to_string();
                    *function_counts.entry(name).or_insert(0) += 1;
                }
            }
        }
    }

    let mut hot_functions = function_counts
        .into_iter()
        .map(|(name, samples)| HotFunction { name, samples })
        .collect::<Vec<_>>();
    hot_functions.sort_by(|a, b| b.samples.cmp(&a.samples));
    hot_functions.truncate(DEFAULT_TOP_HOT_FUNCTIONS_LIMIT);

    CpuSummaryData {
        sample_count: cpu_samples["sampleCount"].as_u64().unwrap_or(0),
        sample_period_micros: cpu_samples["samplePeriod"].as_u64(),
        max_stack_depth: cpu_samples["maxStackDepth"].as_u64(),
        hot_functions,
    }
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
}
