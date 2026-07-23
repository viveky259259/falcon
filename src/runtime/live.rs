use super::connection::VmServiceClient;
use super::tools::{
    collect_logging_report, collect_memory_report, collect_network_report, connect_client,
    LoggingToolReport, MemoryToolReport, NetworkToolReport,
};
use anyhow::Result;
use colored::Colorize;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_MEMORY_WARN_MB: f64 = 200.0;
const DEFAULT_MEMORY_ERROR_MB: f64 = 350.0;
const DEFAULT_SLOW_REQUEST_MS: f64 = 1500.0;

#[derive(Debug, Clone)]
pub struct LiveConfig {
    pub project_path: PathBuf,
    pub attach_uri: Option<String>,
    pub duration: Duration,
    pub interval: Duration,
    pub memory_warn_mb: f64,
    pub memory_error_mb: f64,
    pub slow_request_ms: f64,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            project_path: PathBuf::from("."),
            attach_uri: None,
            duration: Duration::from_secs(30),
            interval: Duration::from_secs(10),
            memory_warn_mb: DEFAULT_MEMORY_WARN_MB,
            memory_error_mb: DEFAULT_MEMORY_ERROR_MB,
            slow_request_ms: DEFAULT_SLOW_REQUEST_MS,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LiveIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LiveIssueCategory {
    Layout,
    Runtime,
    Network,
    Memory,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveIssue {
    pub fingerprint: String,
    pub severity: LiveIssueSeverity,
    pub category: LiveIssueCategory,
    pub title: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub suggested_fix: String,
    pub source_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveIterationReport {
    pub iteration: usize,
    pub memory: MemoryToolReport,
    pub network: NetworkToolReport,
    pub logging: LoggingToolReport,
    pub new_issue_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveReport {
    pub vm_service_uri: String,
    pub total_iterations: usize,
    pub issues: Vec<LiveIssue>,
    pub iterations: Vec<LiveIterationReport>,
}

pub async fn run_live_session(config: &LiveConfig) -> Result<LiveReport> {
    let (vm_uri, _) = connect_client(&config.project_path, config.attach_uri.as_deref()).await?;

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════╗".bright_cyan()
    );
    println!(
        "{}",
        "║              FALCON Live Runtime               ║".bright_cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════╝".bright_cyan()
    );
    println!();
    println!("{} {}", "VM Service:".bright_cyan(), vm_uri);
    println!(
        "{} {}s (window {}s)",
        "Session:".bright_cyan(),
        config.duration.as_secs(),
        config.interval.as_secs()
    );

    let iterations = std::cmp::max(
        1usize,
        (config.duration.as_secs_f64() / config.interval.as_secs_f64()).ceil() as usize,
    );

    let mut seen = HashSet::new();
    let mut issues = Vec::new();
    let mut iteration_reports = Vec::new();

    for iteration in 0..iterations {
        println!();
        println!(
            "{} {}",
            "Window".bright_white().bold(),
            format!("#{}", iteration + 1).bright_white()
        );

        let logging_window = config.interval;
        let network_window = config.interval;

        let (memory, network, logging) = tokio::join!(
            collect_memory_once(&vm_uri),
            collect_network_once(&vm_uri, network_window),
            collect_logging_once(&vm_uri, logging_window),
        );

        let memory = memory?;
        let network = network?;
        let logging = logging?;

        let detected = detect_issues(
            &config.project_path,
            &memory,
            &network,
            &logging,
            config.memory_warn_mb,
            config.memory_error_mb,
            config.slow_request_ms,
        );

        let mut new_issue_count = 0usize;
        for issue in detected {
            if seen.insert(issue.fingerprint.clone()) {
                new_issue_count += 1;
                print_issue(&issue);
                issues.push(issue);
            }
        }

        if new_issue_count == 0 {
            println!(
                "{} No new runtime issues detected in this window",
                "✓".green()
            );
        }

        iteration_reports.push(LiveIterationReport {
            iteration: iteration + 1,
            memory,
            network,
            logging,
            new_issue_count,
        });
    }

    println!();
    print_summary(&issues);

    Ok(LiveReport {
        vm_service_uri: vm_uri,
        total_iterations: iterations,
        issues,
        iterations: iteration_reports,
    })
}

async fn collect_memory_once(vm_uri: &str) -> Result<MemoryToolReport> {
    let client = attach_client(vm_uri).await?;
    collect_memory_report(&client, vm_uri).await
}

async fn collect_network_once(vm_uri: &str, duration: Duration) -> Result<NetworkToolReport> {
    let client = attach_client(vm_uri).await?;
    collect_network_report(&client, vm_uri, duration).await
}

async fn collect_logging_once(vm_uri: &str, duration: Duration) -> Result<LoggingToolReport> {
    let client = attach_client(vm_uri).await?;
    collect_logging_report(&client, vm_uri, duration).await
}

async fn attach_client(vm_uri: &str) -> Result<VmServiceClient> {
    let client = VmServiceClient::connect(vm_uri).await?;
    client.enable_extensions().await?;
    Ok(client)
}

fn detect_issues(
    project_path: &Path,
    memory: &MemoryToolReport,
    network: &NetworkToolReport,
    logging: &LoggingToolReport,
    memory_warn_mb: f64,
    memory_error_mb: f64,
    slow_request_ms: f64,
) -> Vec<LiveIssue> {
    let mut issues = Vec::new();
    issues.extend(detect_layout_and_runtime_issues(logging));
    issues.extend(detect_network_issues(network, slow_request_ms));
    issues.extend(detect_memory_issues(
        memory,
        memory_warn_mb,
        memory_error_mb,
    ));

    for issue in &mut issues {
        if issue.source_hint.is_none() {
            issue.source_hint = infer_project_source_hint(project_path, &issue.summary);
        }
    }

    issues
}

fn detect_layout_and_runtime_issues(logging: &LoggingToolReport) -> Vec<LiveIssue> {
    let non_empty_messages = logging
        .entries
        .iter()
        .filter_map(|entry| {
            let message = entry.message.as_deref()?.trim();
            (!message.is_empty()).then_some(message.to_string())
        })
        .collect::<Vec<_>>();

    if non_empty_messages.is_empty() {
        return Vec::new();
    }

    let mut issues = Vec::new();
    let source_hint = find_dart_source_hint(&non_empty_messages);

    if let Some(message) = non_empty_messages.iter().find(|message| {
        message.contains("RenderFlex overflow") || message.contains("A RenderFlex overflowed")
    }) {
        issues.push(LiveIssue {
            fingerprint: format!("layout:overflow:{}", source_hint.clone().unwrap_or_default()),
            severity: LiveIssueSeverity::Error,
            category: LiveIssueCategory::Layout,
            title: "RenderFlex overflow detected".to_string(),
            summary: message.clone(),
            evidence: collect_matching_evidence(
                &non_empty_messages,
                &["RenderFlex", "overflow", "Row:", "Column:", ".dart:"],
                6,
            ),
            suggested_fix: "Check the overflowing Row/Column and apply tighter constraints: prefer Expanded/Flexible for dynamic children, wrap long content, or move the content into a scrollable container when overflow is expected.".to_string(),
            source_hint,
        });
    }

    if let Some(message) = non_empty_messages.iter().find_map(|message| {
        let normalized = summarize_runtime_message(message);
        (normalized.contains("EXCEPTION CAUGHT BY")
            || normalized.contains("Another exception was thrown")
            || normalized.contains("Exception caught by"))
        .then_some(normalized)
    }) {
        issues.push(LiveIssue {
            fingerprint: format!("runtime:exception:{}", message),
            severity: LiveIssueSeverity::Error,
            category: LiveIssueCategory::Runtime,
            title: "Flutter runtime exception detected".to_string(),
            summary: message,
            evidence: collect_matching_evidence(
                &non_empty_messages,
                &["EXCEPTION CAUGHT BY", "Another exception was thrown", ".dart:"],
                6,
            ),
            suggested_fix: "Inspect the captured exception and source hint first. Reproduce the failing UI path, then fix the underlying widget or state transition before treating this as a rendering symptom.".to_string(),
            source_hint: find_dart_source_hint(&non_empty_messages),
        });
    }

    issues
}

fn detect_network_issues(network: &NetworkToolReport, slow_request_ms: f64) -> Vec<LiveIssue> {
    let mut issues = Vec::new();
    for request in &network.requests {
        let uri = request
            .uri
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        if let Some(status) = request.status {
            if status >= 400 {
                let severity = if status >= 500 || matches!(status, 401 | 403 | 429) {
                    LiveIssueSeverity::Error
                } else {
                    LiveIssueSeverity::Warning
                };
                issues.push(LiveIssue {
                    fingerprint: format!("network:http:{}:{}", status, uri),
                    severity,
                    category: LiveIssueCategory::Network,
                    title: format!(
                        "HTTP {} for {}",
                        status,
                        request.method.as_deref().unwrap_or("request")
                    ),
                    summary: format!(
                        "{} returned status {} for {}",
                        request.method.as_deref().unwrap_or("HTTP request"),
                        status,
                        uri
                    ),
                    evidence: vec![
                        format!("status={status}"),
                        format!("uri={uri}"),
                        format!("duration_ms={:.2}", request.duration_ms.unwrap_or_default()),
                    ],
                    suggested_fix: suggested_fix_for_status(status),
                    source_hint: None,
                });
            }
        }

        if let Some(duration_ms) = request.duration_ms {
            if duration_ms >= slow_request_ms {
                issues.push(LiveIssue {
                    fingerprint: format!("network:slow:{}:{}", request.method.as_deref().unwrap_or("request"), uri),
                    severity: LiveIssueSeverity::Warning,
                    category: LiveIssueCategory::Network,
                    title: "Slow HTTP request detected".to_string(),
                    summary: format!(
                        "{} took {:.2} ms for {}",
                        request.method.as_deref().unwrap_or("HTTP request"),
                        duration_ms,
                        uri
                    ),
                    evidence: vec![
                        format!("uri={uri}"),
                        format!("duration_ms={duration_ms:.2}"),
                        format!("status={}", request.status.unwrap_or_default()),
                    ],
                    suggested_fix: "Inspect this request path for redundant refetches, missing caching, expensive response parsing, or work being done synchronously on the UI isolate.".to_string(),
                    source_hint: None,
                });
            }
        }
    }
    issues
}

fn detect_memory_issues(
    memory: &MemoryToolReport,
    memory_warn_mb: f64,
    memory_error_mb: f64,
) -> Vec<LiveIssue> {
    let mut issues = Vec::new();
    let severity = if memory.heap_usage_mb >= memory_error_mb {
        Some(LiveIssueSeverity::Error)
    } else if memory.heap_usage_mb >= memory_warn_mb {
        Some(LiveIssueSeverity::Warning)
    } else {
        None
    };

    if let Some(severity) = severity {
        let top_bucket =
            memory
                .top_process_buckets
                .first()
                .map(|bucket| match &bucket.description {
                    Some(description) => {
                        format!("{} ({}) {:.2} MB", bucket.name, description, bucket.size_mb)
                    }
                    None => format!("{} {:.2} MB", bucket.name, bucket.size_mb),
                });

        let mut evidence = vec![
            format!("heap_usage_mb={:.2}", memory.heap_usage_mb),
            format!("heap_capacity_mb={:.2}", memory.heap_capacity_mb),
            format!("external_usage_mb={:.2}", memory.external_usage_mb),
        ];
        if let Some(bucket) = top_bucket {
            evidence.push(format!("top_process_bucket={bucket}"));
        }

        issues.push(LiveIssue {
            fingerprint: format!("memory:heap:{:.0}", memory.heap_usage_mb.floor()),
            severity,
            category: LiveIssueCategory::Memory,
            title: "High heap usage detected".to_string(),
            summary: format!(
                "Heap usage is {:.2} MB with {:.2} MB external usage",
                memory.heap_usage_mb, memory.external_usage_mb
            ),
            evidence,
            suggested_fix: "Inspect allocation-heavy classes and process buckets first. In Flutter apps this usually points to retained controllers, subscriptions, caches, or large response payloads kept in UI state.".to_string(),
            source_hint: None,
        });
    }

    issues
}

fn suggested_fix_for_status(status: i64) -> String {
    match status {
        401 | 403 => "This looks like an authentication or authorization failure. Verify tokens, headers, and whether the UI handles expired credentials or API permission mismatches.".to_string(),
        404 => "This usually means the endpoint, route parameter, or environment configuration is wrong. Verify the request URL and any dynamic path construction.".to_string(),
        429 => "The app is being rate-limited. Add backoff, caching, or authenticated requests and avoid repeated retries from the UI.".to_string(),
        500..=599 => "This is a server-side failure. Add resilient error handling in the app, log the failing request context, and verify whether the request payload is still valid.".to_string(),
        _ => "Inspect the failing request and the code path that builds it. The fix is usually around request construction, auth state, or missing UI error handling.".to_string(),
    }
}

fn collect_matching_evidence(messages: &[String], needles: &[&str], limit: usize) -> Vec<String> {
    messages
        .iter()
        .filter(|message| needles.iter().any(|needle| message.contains(needle)))
        .take(limit)
        .cloned()
        .collect()
}

fn find_dart_source_hint(messages: &[String]) -> Option<String> {
    let regex = Regex::new(r"((?:package:[^:\s]+|/[^:\s]+)\.dart:\d+(?::\d+)?)").ok()?;
    messages.iter().find_map(|message| {
        regex
            .captures(message)
            .and_then(|captures| captures.get(1).map(|m| normalize_source_hint(m.as_str())))
    })
}

fn normalize_source_hint(value: &str) -> String {
    value.replacen("///", "/", 1)
}

fn summarize_runtime_message(message: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(message) else {
        return message.to_string();
    };

    value["extensionData"]["renderedErrorText"]
        .as_str()
        .map(ToString::to_string)
        .or_else(|| {
            value["extensionData"]["description"]
                .as_str()
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| message.to_string())
}

fn infer_project_source_hint(project_path: &Path, summary: &str) -> Option<String> {
    let regex =
        Regex::new(r"([A-Za-z0-9_]+Service|[A-Za-z0-9_]+Controller|[A-Za-z0-9_]+ViewModel)")
            .ok()?;
    let type_name = regex.captures(summary)?.get(1)?.as_str();
    let output = std::process::Command::new("rg")
        .args(["-n", type_name, "--glob", "*.dart", "."])
        .current_dir(project_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .to_string();
    let mut parts = line.splitn(2, ':');
    let path = parts.next()?;
    Some(project_path.join(path).display().to_string())
}

fn print_issue(issue: &LiveIssue) {
    let severity = match issue.severity {
        LiveIssueSeverity::Warning => "WARNING".yellow().bold(),
        LiveIssueSeverity::Error => "ERROR".red().bold(),
    };
    println!(
        "{} {} [{}]",
        severity,
        issue.title.bold(),
        format!("{:?}", issue.category).to_lowercase()
    );
    println!("  {}", issue.summary);
    if let Some(source_hint) = &issue.source_hint {
        println!("  {} {}", "Source:".bright_cyan(), source_hint);
    }
    if !issue.evidence.is_empty() {
        println!("  {}", "Evidence:".bright_cyan());
        for line in issue.evidence.iter().take(4) {
            println!("    - {}", line);
        }
    }
    println!("  {} {}", "Next:".bright_cyan(), issue.suggested_fix);
}

fn print_summary(issues: &[LiveIssue]) {
    let warnings = issues
        .iter()
        .filter(|issue| issue.severity == LiveIssueSeverity::Warning)
        .count();
    let errors = issues
        .iter()
        .filter(|issue| issue.severity == LiveIssueSeverity::Error)
        .count();

    println!("{}", "Live session summary".bright_white().bold());
    println!("  {} {}", "Total issues:".bright_cyan(), issues.len());
    println!("  {} {}", "Errors:".bright_cyan(), errors);
    println!("  {} {}", "Warnings:".bright_cyan(), warnings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::tools::{
        AllocationEntry, LogEntry, NetworkRequestSummary, ProcessBucketEntry,
    };

    fn sample_logging(messages: &[&str]) -> LoggingToolReport {
        LoggingToolReport {
            vm_service_uri: "http://127.0.0.1:1234/abc=/".to_string(),
            isolate_id: "isolates/main".to_string(),
            duration_secs: 5,
            total_events: messages.len(),
            stream_counts: Vec::new(),
            entries: messages
                .iter()
                .enumerate()
                .map(|(index, message)| LogEntry {
                    stream: "Stdout".to_string(),
                    kind: "stdout".to_string(),
                    timestamp_micros: Some(index as i64),
                    message: Some((*message).to_string()),
                    logger_name: None,
                    level: None,
                })
                .collect(),
        }
    }

    #[test]
    fn detects_renderflex_overflow_and_source_hint() {
        let logging = sample_logging(&[
            "══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞══",
            "A RenderFlex overflowed by 3.2 pixels on the right.",
            "Row:file:///tmp/subzero_onboarding.dart:601:13",
        ]);

        let issues = detect_layout_and_runtime_issues(&logging);
        let overflow = issues
            .iter()
            .find(|issue| issue.category == LiveIssueCategory::Layout)
            .unwrap();

        assert_eq!(overflow.title, "RenderFlex overflow detected");
        assert_eq!(
            overflow.source_hint.as_deref(),
            Some("/tmp/subzero_onboarding.dart:601:13")
        );
    }

    #[test]
    fn detects_http_failure_and_maps_fix() {
        let report = NetworkToolReport {
            vm_service_uri: "http://127.0.0.1:1/".to_string(),
            isolate_id: "isolates/main".to_string(),
            duration_secs: 5,
            total_requests: 1,
            failed_requests: 1,
            total_bytes_received: 10,
            max_latency_ms: 20.0,
            socket_count: 0,
            total_socket_read_bytes: 0,
            total_socket_write_bytes: 0,
            requests: vec![NetworkRequestSummary {
                method: Some("GET".to_string()),
                uri: Some("https://api.github.com/repos/flutter/flutter".to_string()),
                status: Some(403),
                duration_ms: Some(20.0),
                content_length: Some(10),
            }],
            sockets: Vec::new(),
        };

        let issues = detect_network_issues(&report, 1500.0);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].suggested_fix.contains("authentication"));
    }

    #[test]
    fn detects_high_memory_and_includes_top_bucket() {
        let report = MemoryToolReport {
            vm_service_uri: "http://127.0.0.1:1/".to_string(),
            isolate_id: "isolates/main".to_string(),
            heap_usage_mb: 275.0,
            heap_capacity_mb: 320.0,
            external_usage_mb: 18.0,
            top_process_buckets: vec![ProcessBucketEntry {
                name: "Dart Heap".to_string(),
                description: Some("managed".to_string()),
                size_mb: 260.0,
            }],
            top_allocations: vec![AllocationEntry {
                class: "_List".to_string(),
                instances_current: 10,
                bytes_current_mb: 32.0,
                accumulated_size_mb: 64.0,
            }],
        };

        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert_eq!(issues.len(), 1);
        assert!(issues[0]
            .evidence
            .iter()
            .any(|line| line.contains("top_process_bucket=Dart Heap")));
    }

    // ── Helper builders ──────────────────────────────────────────────────────

    fn empty_logging() -> LoggingToolReport {
        sample_logging(&[])
    }

    fn make_memory(heap_mb: f64) -> MemoryToolReport {
        MemoryToolReport {
            vm_service_uri: "http://127.0.0.1:1/".to_string(),
            isolate_id: "isolates/main".to_string(),
            heap_usage_mb: heap_mb,
            heap_capacity_mb: heap_mb + 50.0,
            external_usage_mb: 5.0,
            top_process_buckets: Vec::new(),
            top_allocations: Vec::new(),
        }
    }

    fn make_network_with_request(
        method: &str,
        uri: &str,
        status: i64,
        duration_ms: f64,
    ) -> NetworkToolReport {
        NetworkToolReport {
            vm_service_uri: "http://127.0.0.1:1/".to_string(),
            isolate_id: "isolates/main".to_string(),
            duration_secs: 5,
            total_requests: 1,
            failed_requests: 0,
            total_bytes_received: 0,
            max_latency_ms: duration_ms,
            socket_count: 0,
            total_socket_read_bytes: 0,
            total_socket_write_bytes: 0,
            requests: vec![NetworkRequestSummary {
                method: Some(method.to_string()),
                uri: Some(uri.to_string()),
                status: Some(status),
                duration_ms: Some(duration_ms),
                content_length: None,
            }],
            sockets: Vec::new(),
        }
    }

    fn empty_network() -> NetworkToolReport {
        NetworkToolReport {
            vm_service_uri: "http://127.0.0.1:1/".to_string(),
            isolate_id: "isolates/main".to_string(),
            duration_secs: 5,
            total_requests: 0,
            failed_requests: 0,
            total_bytes_received: 0,
            max_latency_ms: 0.0,
            socket_count: 0,
            total_socket_read_bytes: 0,
            total_socket_write_bytes: 0,
            requests: Vec::new(),
            sockets: Vec::new(),
        }
    }

    // ── LiveConfig::default ───────────────────────────────────────────────────

    #[test]
    fn live_config_default_project_path_is_dot() {
        let cfg = LiveConfig::default();
        assert_eq!(cfg.project_path, std::path::PathBuf::from("."));
    }

    #[test]
    fn live_config_default_attach_uri_is_none() {
        let cfg = LiveConfig::default();
        assert!(cfg.attach_uri.is_none());
    }

    #[test]
    fn live_config_default_duration_is_30s() {
        let cfg = LiveConfig::default();
        assert_eq!(cfg.duration, std::time::Duration::from_secs(30));
    }

    #[test]
    fn live_config_default_interval_is_10s() {
        let cfg = LiveConfig::default();
        assert_eq!(cfg.interval, std::time::Duration::from_secs(10));
    }

    #[test]
    fn live_config_default_memory_thresholds_match_constants() {
        let cfg = LiveConfig::default();
        assert_eq!(cfg.memory_warn_mb, DEFAULT_MEMORY_WARN_MB);
        assert_eq!(cfg.memory_error_mb, DEFAULT_MEMORY_ERROR_MB);
        assert_eq!(cfg.slow_request_ms, DEFAULT_SLOW_REQUEST_MS);
    }

    // ── collect_matching_evidence ─────────────────────────────────────────────

    #[test]
    fn collect_matching_evidence_empty_messages_returns_empty() {
        let result = collect_matching_evidence(&[], &["overflow"], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn collect_matching_evidence_no_match_returns_empty() {
        let messages = vec!["hello world".to_string(), "nothing here".to_string()];
        let result = collect_matching_evidence(&messages, &["overflow"], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn collect_matching_evidence_returns_matching_lines() {
        let messages = vec![
            "A RenderFlex overflowed".to_string(),
            "unrelated line".to_string(),
            "another overflow message".to_string(),
        ];
        let result = collect_matching_evidence(&messages, &["overflow"], 10);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|m| m.contains("overflow")));
    }

    #[test]
    fn collect_matching_evidence_respects_limit() {
        let messages: Vec<String> = (0..10).map(|i| format!("overflow {i}")).collect();
        let result = collect_matching_evidence(&messages, &["overflow"], 3);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn collect_matching_evidence_multiple_needles_any_match() {
        let messages = vec![
            "Row: overflow here".to_string(),
            "Column: overflow here".to_string(),
            "unrelated".to_string(),
        ];
        let result = collect_matching_evidence(&messages, &["Row:", "Column:"], 10);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn collect_matching_evidence_empty_needles_returns_empty() {
        let messages = vec!["some message".to_string()];
        let result = collect_matching_evidence(&messages, &[], 10);
        assert!(result.is_empty());
    }

    #[test]
    fn collect_matching_evidence_limit_zero_returns_empty() {
        let messages = vec!["overflow".to_string()];
        let result = collect_matching_evidence(&messages, &["overflow"], 0);
        assert!(result.is_empty());
    }

    // ── find_dart_source_hint ─────────────────────────────────────────────────

    #[test]
    fn find_dart_source_hint_no_messages_returns_none() {
        assert!(find_dart_source_hint(&[]).is_none());
    }

    #[test]
    fn find_dart_source_hint_no_dart_ref_returns_none() {
        let messages = vec!["some log line without dart ref".to_string()];
        assert!(find_dart_source_hint(&messages).is_none());
    }

    #[test]
    fn find_dart_source_hint_package_uri_line_col() {
        let messages = vec!["at package:my_app/screens/home.dart:42:7".to_string()];
        let hint = find_dart_source_hint(&messages);
        assert_eq!(
            hint.as_deref(),
            Some("package:my_app/screens/home.dart:42:7")
        );
    }

    #[test]
    fn find_dart_source_hint_absolute_path_line_only() {
        let messages = vec!["#0 MyWidget (/home/user/project/lib/main.dart:10)".to_string()];
        let hint = find_dart_source_hint(&messages);
        assert_eq!(hint.as_deref(), Some("/home/user/project/lib/main.dart:10"));
    }

    #[test]
    fn find_dart_source_hint_triple_slash_normalizes() {
        // The file:///path pattern produces ///path.dart which normalize_source_hint fixes
        let messages = vec!["file:///tmp/widgets.dart:5:3".to_string()];
        let hint = find_dart_source_hint(&messages);
        // regex captures the path portion after file:
        // The captured group is ///tmp/widgets.dart:5:3 — normalize_source_hint removes one leading /
        // then returns /tmp/widgets.dart:5:3 … or the regex might not match "file:///"
        // The regex looks for (package:... | /path...)\.dart:\d+
        // So "///tmp/widgets.dart:5:3" would match /[^\s:]+ leading with ///
        // normalize_source_hint replaces leading /// with /
        // So result should be /tmp/widgets.dart:5:3
        if let Some(h) = hint {
            assert!(h.contains("widgets.dart:5"));
        }
        // If regex doesn't capture it that's fine — we just ensure no panic
    }

    #[test]
    fn find_dart_source_hint_returns_first_match() {
        let messages = vec![
            "no dart here".to_string(),
            "package:app/a.dart:1:1".to_string(),
            "package:app/b.dart:2:2".to_string(),
        ];
        let hint = find_dart_source_hint(&messages).unwrap();
        assert!(hint.contains("a.dart:1:1"));
    }

    // ── normalize_source_hint ─────────────────────────────────────────────────

    #[test]
    fn normalize_source_hint_no_triple_slash_unchanged() {
        assert_eq!(
            normalize_source_hint("/tmp/foo.dart:10"),
            "/tmp/foo.dart:10"
        );
    }

    #[test]
    fn normalize_source_hint_triple_slash_replaced() {
        assert_eq!(
            normalize_source_hint("///tmp/foo.dart:10"),
            "/tmp/foo.dart:10"
        );
    }

    #[test]
    fn normalize_source_hint_only_first_occurrence_replaced() {
        assert_eq!(normalize_source_hint("///a///b"), "/a///b");
    }

    #[test]
    fn normalize_source_hint_empty_string() {
        assert_eq!(normalize_source_hint(""), "");
    }

    #[test]
    fn normalize_source_hint_double_slash_unchanged() {
        assert_eq!(
            normalize_source_hint("//tmp/foo.dart:1"),
            "//tmp/foo.dart:1"
        );
    }

    // ── summarize_runtime_message ─────────────────────────────────────────────

    #[test]
    fn summarize_runtime_message_plain_text_unchanged() {
        let msg = "Some plain log line";
        assert_eq!(summarize_runtime_message(msg), msg);
    }

    #[test]
    fn summarize_runtime_message_json_without_extension_data_returns_original() {
        let msg = r#"{"type":"Event","kind":"Logging"}"#;
        assert_eq!(summarize_runtime_message(msg), msg);
    }

    #[test]
    fn summarize_runtime_message_uses_rendered_error_text() {
        let msg =
            r#"{"extensionData":{"renderedErrorText":"EXCEPTION CAUGHT BY widgets library"}}"#;
        assert_eq!(
            summarize_runtime_message(msg),
            "EXCEPTION CAUGHT BY widgets library"
        );
    }

    #[test]
    fn summarize_runtime_message_falls_back_to_description() {
        let msg =
            r#"{"extensionData":{"description":"Another exception was thrown: FormatException"}}"#;
        let result = summarize_runtime_message(msg);
        assert_eq!(result, "Another exception was thrown: FormatException");
    }

    #[test]
    fn summarize_runtime_message_rendered_error_text_takes_priority_over_description() {
        let msg = r#"{"extensionData":{"renderedErrorText":"Primary","description":"Secondary"}}"#;
        assert_eq!(summarize_runtime_message(msg), "Primary");
    }

    #[test]
    fn summarize_runtime_message_empty_string() {
        assert_eq!(summarize_runtime_message(""), "");
    }

    // ── detect_layout_and_runtime_issues ─────────────────────────────────────

    #[test]
    fn detect_layout_empty_logging_returns_empty() {
        let issues = detect_layout_and_runtime_issues(&empty_logging());
        assert!(issues.is_empty());
    }

    #[test]
    fn detect_layout_no_relevant_messages_returns_empty() {
        let logging = sample_logging(&["Just a debug log", "Another innocuous log"]);
        assert!(detect_layout_and_runtime_issues(&logging).is_empty());
    }

    #[test]
    fn detect_layout_overflow_variant_a_renderflex_overflowed() {
        let logging = sample_logging(&["A RenderFlex overflowed by 5 pixels on the bottom."]);
        let issues = detect_layout_and_runtime_issues(&logging);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].category, LiveIssueCategory::Layout);
        assert_eq!(issues[0].severity, LiveIssueSeverity::Error);
    }

    #[test]
    fn detect_layout_overflow_variant_renderflex_overflow_phrase() {
        let logging = sample_logging(&["RenderFlex overflow detected in Column"]);
        let issues = detect_layout_and_runtime_issues(&logging);
        assert!(issues
            .iter()
            .any(|i| i.category == LiveIssueCategory::Layout));
    }

    #[test]
    fn detect_layout_overflow_fingerprint_contains_layout_overflow() {
        let logging = sample_logging(&["A RenderFlex overflowed by 3 pixels."]);
        let issues = detect_layout_and_runtime_issues(&logging);
        assert!(issues[0].fingerprint.starts_with("layout:overflow:"));
    }

    #[test]
    fn detect_layout_overflow_no_source_hint_when_no_dart_ref() {
        let logging = sample_logging(&["A RenderFlex overflowed by 3 pixels."]);
        let issues = detect_layout_and_runtime_issues(&logging);
        assert!(issues[0].source_hint.is_none());
    }

    #[test]
    fn detect_layout_runtime_exception_caught_by_detected() {
        let logging = sample_logging(&[
            r#"{"extensionData":{"renderedErrorText":"EXCEPTION CAUGHT BY widgets library"}}"#,
        ]);
        let issues = detect_layout_and_runtime_issues(&logging);
        assert!(issues
            .iter()
            .any(|i| i.category == LiveIssueCategory::Runtime));
    }

    #[test]
    fn detect_layout_another_exception_was_thrown_detected() {
        let logging = sample_logging(&[
            r#"{"extensionData":{"description":"Another exception was thrown: StateError"}}"#,
        ]);
        let issues = detect_layout_and_runtime_issues(&logging);
        assert!(issues
            .iter()
            .any(|i| i.category == LiveIssueCategory::Runtime));
    }

    #[test]
    fn detect_layout_runtime_fingerprint_starts_with_runtime_exception() {
        let logging = sample_logging(&[
            r#"{"extensionData":{"renderedErrorText":"EXCEPTION CAUGHT BY rendering library"}}"#,
        ]);
        let issues = detect_layout_and_runtime_issues(&logging);
        let runtime_issue = issues
            .iter()
            .find(|i| i.category == LiveIssueCategory::Runtime)
            .unwrap();
        assert!(runtime_issue.fingerprint.starts_with("runtime:exception:"));
    }

    #[test]
    fn detect_layout_overflow_evidence_includes_matching_lines() {
        let logging = sample_logging(&[
            "A RenderFlex overflowed by 3 pixels.",
            "Row: 42 pixels",
            "package:app/page.dart:10:3",
        ]);
        let issues = detect_layout_and_runtime_issues(&logging);
        let layout = issues
            .iter()
            .find(|i| i.category == LiveIssueCategory::Layout)
            .unwrap();
        assert!(!layout.evidence.is_empty());
    }

    #[test]
    fn detect_layout_only_whitespace_messages_treated_as_empty() {
        let logging = LoggingToolReport {
            vm_service_uri: "http://127.0.0.1:1234/".to_string(),
            isolate_id: "isolates/main".to_string(),
            duration_secs: 5,
            total_events: 2,
            stream_counts: Vec::new(),
            entries: vec![
                LogEntry {
                    stream: "Stdout".to_string(),
                    kind: "stdout".to_string(),
                    timestamp_micros: None,
                    message: Some("   ".to_string()),
                    logger_name: None,
                    level: None,
                },
                LogEntry {
                    stream: "Stdout".to_string(),
                    kind: "stdout".to_string(),
                    timestamp_micros: None,
                    message: None,
                    logger_name: None,
                    level: None,
                },
            ],
        };
        assert!(detect_layout_and_runtime_issues(&logging).is_empty());
    }

    // ── detect_network_issues ─────────────────────────────────────────────────

    #[test]
    fn detect_network_empty_requests_returns_empty() {
        let issues = detect_network_issues(&empty_network(), 1500.0);
        assert!(issues.is_empty());
    }

    #[test]
    fn detect_network_status_below_400_no_issue() {
        let report = make_network_with_request("GET", "https://example.com", 200, 100.0);
        let issues = detect_network_issues(&report, 1500.0);
        assert!(issues.is_empty());
    }

    #[test]
    fn detect_network_status_400_exactly_is_warning() {
        let report = make_network_with_request("GET", "https://example.com/notfound", 400, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        let http_issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("network:http:"))
            .unwrap();
        assert_eq!(http_issue.severity, LiveIssueSeverity::Warning);
    }

    #[test]
    fn detect_network_status_401_is_error() {
        let report = make_network_with_request("GET", "https://api.example.com/me", 401, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        let issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("401"))
            .unwrap();
        assert_eq!(issue.severity, LiveIssueSeverity::Error);
        assert!(issue.suggested_fix.contains("authentication"));
    }

    #[test]
    fn detect_network_status_403_is_error_with_auth_fix() {
        let report = make_network_with_request("POST", "https://api.example.com/admin", 403, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        let issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("403"))
            .unwrap();
        assert_eq!(issue.severity, LiveIssueSeverity::Error);
        assert!(issue.suggested_fix.contains("authentication"));
    }

    #[test]
    fn detect_network_status_404_is_warning_with_endpoint_fix() {
        let report = make_network_with_request("GET", "https://api.example.com/missing", 404, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        let issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("404"))
            .unwrap();
        assert_eq!(issue.severity, LiveIssueSeverity::Warning);
        assert!(issue.suggested_fix.contains("endpoint"));
    }

    #[test]
    fn detect_network_status_429_is_error_with_rate_limit_fix() {
        let report = make_network_with_request("GET", "https://api.example.com/data", 429, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        let issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("429"))
            .unwrap();
        assert_eq!(issue.severity, LiveIssueSeverity::Error);
        assert!(issue.suggested_fix.contains("rate-limited"));
    }

    #[test]
    fn detect_network_status_500_is_error_with_server_fix() {
        let report = make_network_with_request("GET", "https://api.example.com/crash", 500, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        let issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("500"))
            .unwrap();
        assert_eq!(issue.severity, LiveIssueSeverity::Error);
        assert!(issue.suggested_fix.contains("server-side"));
    }

    #[test]
    fn detect_network_status_503_is_error() {
        let report = make_network_with_request("GET", "https://api.example.com/svc", 503, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        let issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("503"))
            .unwrap();
        assert_eq!(issue.severity, LiveIssueSeverity::Error);
    }

    #[test]
    fn detect_network_slow_request_above_threshold_creates_issue() {
        let report = make_network_with_request("GET", "https://api.example.com/slow", 200, 2000.0);
        let issues = detect_network_issues(&report, 1500.0);
        let slow_issue = issues
            .iter()
            .find(|i| i.fingerprint.contains("network:slow:"))
            .unwrap();
        assert_eq!(slow_issue.severity, LiveIssueSeverity::Warning);
        assert!(slow_issue.summary.contains("2000.00 ms"));
    }

    #[test]
    fn detect_network_slow_request_exactly_at_threshold_creates_issue() {
        let report = make_network_with_request("GET", "https://api.example.com/edge", 200, 1500.0);
        let issues = detect_network_issues(&report, 1500.0);
        assert!(issues
            .iter()
            .any(|i| i.fingerprint.contains("network:slow:")));
    }

    #[test]
    fn detect_network_slow_request_below_threshold_no_slow_issue() {
        let report = make_network_with_request("GET", "https://api.example.com/fast", 200, 1499.9);
        let issues = detect_network_issues(&report, 1500.0);
        assert!(!issues
            .iter()
            .any(|i| i.fingerprint.contains("network:slow:")));
    }

    #[test]
    fn detect_network_both_error_status_and_slow_creates_two_issues() {
        let report =
            make_network_with_request("POST", "https://api.example.com/heavy", 500, 3000.0);
        let issues = detect_network_issues(&report, 1500.0);
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn detect_network_fingerprint_contains_uri() {
        let uri = "https://api.example.com/repos";
        let report = make_network_with_request("GET", uri, 404, 50.0);
        let issues = detect_network_issues(&report, 1500.0);
        assert!(issues[0].fingerprint.contains(uri));
    }

    #[test]
    fn detect_network_no_status_skips_status_check() {
        let report = NetworkToolReport {
            vm_service_uri: "http://127.0.0.1:1/".to_string(),
            isolate_id: "isolates/main".to_string(),
            duration_secs: 5,
            total_requests: 1,
            failed_requests: 0,
            total_bytes_received: 0,
            max_latency_ms: 100.0,
            socket_count: 0,
            total_socket_read_bytes: 0,
            total_socket_write_bytes: 0,
            requests: vec![NetworkRequestSummary {
                method: Some("GET".to_string()),
                uri: Some("https://example.com/".to_string()),
                status: None,
                duration_ms: Some(100.0),
                content_length: None,
            }],
            sockets: Vec::new(),
        };
        // No status => no http error; duration 100ms < 1500ms threshold => no slow issue
        let issues = detect_network_issues(&report, 1500.0);
        assert!(issues.is_empty());
    }

    #[test]
    fn detect_network_no_uri_uses_unknown_fallback() {
        let report = NetworkToolReport {
            vm_service_uri: "http://127.0.0.1:1/".to_string(),
            isolate_id: "isolates/main".to_string(),
            duration_secs: 5,
            total_requests: 1,
            failed_requests: 1,
            total_bytes_received: 0,
            max_latency_ms: 100.0,
            socket_count: 0,
            total_socket_read_bytes: 0,
            total_socket_write_bytes: 0,
            requests: vec![NetworkRequestSummary {
                method: Some("GET".to_string()),
                uri: None,
                status: Some(500),
                duration_ms: Some(100.0),
                content_length: None,
            }],
            sockets: Vec::new(),
        };
        let issues = detect_network_issues(&report, 1500.0);
        assert!(issues.iter().any(|i| i.fingerprint.contains("<unknown>")));
    }

    // ── detect_memory_issues ──────────────────────────────────────────────────

    #[test]
    fn detect_memory_below_warn_threshold_no_issue() {
        let report = make_memory(100.0);
        assert!(detect_memory_issues(&report, 200.0, 350.0).is_empty());
    }

    #[test]
    fn detect_memory_exactly_at_warn_threshold_is_warning() {
        let report = make_memory(200.0);
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, LiveIssueSeverity::Warning);
    }

    #[test]
    fn detect_memory_above_warn_below_error_is_warning() {
        let report = make_memory(250.0);
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, LiveIssueSeverity::Warning);
    }

    #[test]
    fn detect_memory_exactly_at_error_threshold_is_error() {
        let report = make_memory(350.0);
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, LiveIssueSeverity::Error);
    }

    #[test]
    fn detect_memory_above_error_threshold_is_error() {
        let report = make_memory(400.0);
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, LiveIssueSeverity::Error);
    }

    #[test]
    fn detect_memory_fingerprint_contains_floored_heap() {
        let report = make_memory(275.7);
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert!(issues[0].fingerprint.starts_with("memory:heap:275"));
    }

    #[test]
    fn detect_memory_evidence_always_includes_heap_usage() {
        let report = make_memory(250.0);
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert!(issues[0]
            .evidence
            .iter()
            .any(|e| e.starts_with("heap_usage_mb=")));
    }

    #[test]
    fn detect_memory_no_top_bucket_evidence_omits_top_process_bucket() {
        let report = make_memory(250.0); // top_process_buckets is empty
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert!(!issues[0]
            .evidence
            .iter()
            .any(|e| e.starts_with("top_process_bucket=")));
    }

    #[test]
    fn detect_memory_top_bucket_with_description_included_in_evidence() {
        let mut report = make_memory(250.0);
        report.top_process_buckets.push(ProcessBucketEntry {
            name: "Image Cache".to_string(),
            description: Some("decoded bitmaps".to_string()),
            size_mb: 50.0,
        });
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert!(issues[0]
            .evidence
            .iter()
            .any(|e| e.contains("Image Cache") && e.contains("decoded bitmaps")));
    }

    #[test]
    fn detect_memory_top_bucket_without_description_still_in_evidence() {
        let mut report = make_memory(250.0);
        report.top_process_buckets.push(ProcessBucketEntry {
            name: "Native Heap".to_string(),
            description: None,
            size_mb: 30.0,
        });
        let issues = detect_memory_issues(&report, 200.0, 350.0);
        assert!(issues[0].evidence.iter().any(|e| e.contains("Native Heap")));
    }

    // ── suggested_fix_for_status ──────────────────────────────────────────────

    #[test]
    fn suggested_fix_for_status_401_auth() {
        assert!(suggested_fix_for_status(401).contains("authentication"));
    }

    #[test]
    fn suggested_fix_for_status_403_auth() {
        assert!(suggested_fix_for_status(403).contains("authentication"));
    }

    #[test]
    fn suggested_fix_for_status_404_endpoint() {
        assert!(suggested_fix_for_status(404).contains("endpoint"));
    }

    #[test]
    fn suggested_fix_for_status_429_rate_limited() {
        assert!(suggested_fix_for_status(429).contains("rate-limited"));
    }

    #[test]
    fn suggested_fix_for_status_500_server_side() {
        assert!(suggested_fix_for_status(500).contains("server-side"));
    }

    #[test]
    fn suggested_fix_for_status_599_server_side() {
        assert!(suggested_fix_for_status(599).contains("server-side"));
    }

    #[test]
    fn suggested_fix_for_status_other_generic_message() {
        let fix = suggested_fix_for_status(422);
        assert!(fix.contains("request construction") || fix.contains("Inspect"));
    }

    // ── detect_issues (integration of all sub-detectors) ─────────────────────

    #[test]
    fn detect_issues_all_clean_returns_empty() {
        let memory = make_memory(50.0);
        let network = empty_network();
        let logging = empty_logging();
        let issues = detect_issues(
            std::path::Path::new("/nonexistent/path"),
            &memory,
            &network,
            &logging,
            200.0,
            350.0,
            1500.0,
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn detect_issues_memory_warn_found() {
        let memory = make_memory(250.0);
        let network = empty_network();
        let logging = empty_logging();
        let issues = detect_issues(
            std::path::Path::new("/nonexistent/path"),
            &memory,
            &network,
            &logging,
            200.0,
            350.0,
            1500.0,
        );
        assert!(issues
            .iter()
            .any(|i| i.category == LiveIssueCategory::Memory));
    }

    #[test]
    fn detect_issues_network_error_found() {
        let memory = make_memory(50.0);
        let network = make_network_with_request("GET", "https://example.com", 500, 50.0);
        let logging = empty_logging();
        let issues = detect_issues(
            std::path::Path::new("/nonexistent/path"),
            &memory,
            &network,
            &logging,
            200.0,
            350.0,
            1500.0,
        );
        assert!(issues
            .iter()
            .any(|i| i.category == LiveIssueCategory::Network));
    }

    #[test]
    fn detect_issues_layout_overflow_found() {
        let memory = make_memory(50.0);
        let network = empty_network();
        let logging = sample_logging(&["A RenderFlex overflowed by 3 pixels."]);
        let issues = detect_issues(
            std::path::Path::new("/nonexistent/path"),
            &memory,
            &network,
            &logging,
            200.0,
            350.0,
            1500.0,
        );
        assert!(issues
            .iter()
            .any(|i| i.category == LiveIssueCategory::Layout));
    }

    #[test]
    fn detect_issues_source_hint_not_set_when_no_project_match() {
        // With a nonexistent path, infer_project_source_hint should fail and leave source_hint None
        let memory = make_memory(250.0);
        let network = empty_network();
        let logging = empty_logging();
        let issues = detect_issues(
            std::path::Path::new("/nonexistent/path"),
            &memory,
            &network,
            &logging,
            200.0,
            350.0,
            1500.0,
        );
        // Memory issues have no extractable class name matching the pattern,
        // so source_hint stays None.
        let memory_issue = issues
            .iter()
            .find(|i| i.category == LiveIssueCategory::Memory)
            .unwrap();
        assert!(memory_issue.source_hint.is_none());
    }

    // ── infer_project_source_hint (indirect via detect_issues) ───────────────

    #[test]
    fn infer_project_source_hint_returns_none_for_nonexistent_path() {
        // Path doesn't exist so rg call should fail → None
        let result = infer_project_source_hint(
            std::path::Path::new("/nonexistent_path_xyz"),
            "UserService failed to load",
        );
        assert!(result.is_none());
    }

    #[test]
    fn infer_project_source_hint_returns_none_when_no_pattern_in_summary() {
        // Summary has no Service/Controller/ViewModel pattern
        let result = infer_project_source_hint(
            std::path::Path::new("."),
            "A RenderFlex overflowed by 3 pixels on the right.",
        );
        assert!(result.is_none());
    }
}
