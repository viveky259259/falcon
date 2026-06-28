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
}
