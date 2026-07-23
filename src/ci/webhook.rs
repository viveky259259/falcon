use serde::{Deserialize, Serialize};
/// Webhook event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookEvent {
    AnalysisComplete,
    ScoreChanged,
    DriftDetected,
    ThresholdExceeded,
}

impl std::fmt::Display for WebhookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookEvent::AnalysisComplete => write!(f, "analysis.complete"),
            WebhookEvent::ScoreChanged => write!(f, "score.changed"),
            WebhookEvent::DriftDetected => write!(f, "drift.detected"),
            WebhookEvent::ThresholdExceeded => write!(f, "threshold.exceeded"),
        }
    }
}

/// Webhook payload sent to registered endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub timestamp: String,
    pub project: String,
    pub data: serde_json::Value,
}

/// Configuration for webhook endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookConfig {
    pub endpoints: Vec<WebhookEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub url: String,
    pub events: Vec<String>,
    pub secret: Option<String>,
}

/// Send an analysis-complete webhook.
pub fn send_analysis_webhook(
    url: &str,
    project: &str,
    report: &crate::reporters::AnalysisReport,
) -> anyhow::Result<()> {
    let timestamp = get_timestamp();

    let payload = WebhookPayload {
        event: WebhookEvent::AnalysisComplete.to_string(),
        timestamp,
        project: project.to_string(),
        data: serde_json::json!({
            "file_count": report.file_count,
            "error_count": report.error_count(),
            "warning_count": report.warning_count(),
            "total_issues": report.issues.len(),
            "passed": report.error_count() == 0
        }),
    };

    send_payload(url, &payload)
}

/// Send a score-changed webhook.
pub fn send_score_webhook(
    url: &str,
    project: &str,
    score: &crate::ai_score::score::AiCodeScore,
    previous_score: Option<u32>,
) -> anyhow::Result<()> {
    let timestamp = get_timestamp();
    let delta = previous_score.map(|prev| score.overall as i32 - prev as i32);

    let payload = WebhookPayload {
        event: WebhookEvent::ScoreChanged.to_string(),
        timestamp,
        project: project.to_string(),
        data: serde_json::json!({
            "score": score.overall,
            "grade": score.grade.to_string(),
            "previous_score": previous_score,
            "delta": delta,
            "production_ready": score.overall >= 85
        }),
    };

    send_payload(url, &payload)
}

/// Send a drift-detected webhook.
pub fn send_drift_webhook(
    url: &str,
    project: &str,
    drift_report: &crate::ai_score::drift::DriftReport,
) -> anyhow::Result<()> {
    let timestamp = get_timestamp();

    let payload = WebhookPayload {
        event: WebhookEvent::DriftDetected.to_string(),
        timestamp,
        project: project.to_string(),
        data: serde_json::json!({
            "drift_score": drift_report.drift_score,
            "findings_count": drift_report.findings.len(),
            "files_analyzed": drift_report.files_analyzed,
            "architecture": drift_report.conventions.architecture.pattern
        }),
    };

    send_payload(url, &payload)
}

fn send_payload(url: &str, payload: &WebhookPayload) -> anyhow::Result<()> {
    let json = serde_json::to_string(payload)?;

    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            &json,
            url,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Webhook to {} failed: {}", url, stderr);
    }

    Ok(())
}

fn get_timestamp() -> String {
    std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        })
}

/// Generate a webhook payload as JSON (for testing/preview).
pub fn preview_webhook(
    event: WebhookEvent,
    project: &str,
    data: serde_json::Value,
) -> anyhow::Result<String> {
    let payload = WebhookPayload {
        event: event.to_string(),
        timestamp: get_timestamp(),
        project: project.to_string(),
        data,
    };
    Ok(serde_json::to_string_pretty(&payload)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- WebhookEvent::Display ----

    #[test]
    fn test_display_analysis_complete() {
        assert_eq!(
            WebhookEvent::AnalysisComplete.to_string(),
            "analysis.complete"
        );
    }

    #[test]
    fn test_display_score_changed() {
        assert_eq!(WebhookEvent::ScoreChanged.to_string(), "score.changed");
    }

    #[test]
    fn test_display_drift_detected() {
        assert_eq!(WebhookEvent::DriftDetected.to_string(), "drift.detected");
    }

    #[test]
    fn test_display_threshold_exceeded() {
        assert_eq!(
            WebhookEvent::ThresholdExceeded.to_string(),
            "threshold.exceeded"
        );
    }

    // ---- WebhookEvent serde round-trip ----

    #[test]
    fn test_webhook_event_serde_round_trip() {
        let events = vec![
            WebhookEvent::AnalysisComplete,
            WebhookEvent::ScoreChanged,
            WebhookEvent::DriftDetected,
            WebhookEvent::ThresholdExceeded,
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let decoded: WebhookEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event.to_string(), decoded.to_string());
        }
    }

    // ---- WebhookPayload serde round-trip ----

    #[test]
    fn test_webhook_payload_serde_round_trip() {
        let payload = WebhookPayload {
            event: "analysis.complete".to_string(),
            timestamp: "2024-01-01T00:00:00".to_string(),
            project: "my_project".to_string(),
            data: json!({"file_count": 10, "passed": true}),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let decoded: WebhookPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.event, "analysis.complete");
        assert_eq!(decoded.project, "my_project");
        assert_eq!(decoded.timestamp, "2024-01-01T00:00:00");
        assert_eq!(decoded.data["file_count"], 10);
        assert_eq!(decoded.data["passed"], true);
    }

    #[test]
    fn test_webhook_payload_clone() {
        let payload = WebhookPayload {
            event: "score.changed".to_string(),
            timestamp: "2024-01-01T00:00:00".to_string(),
            project: "proj".to_string(),
            data: json!({"score": 90}),
        };
        let cloned = payload.clone();
        assert_eq!(cloned.event, payload.event);
        assert_eq!(cloned.project, payload.project);
    }

    // ---- WebhookConfig serde round-trip ----

    #[test]
    fn test_webhook_config_default_is_empty() {
        let config = WebhookConfig::default();
        assert!(config.endpoints.is_empty());
    }

    #[test]
    fn test_webhook_config_serde_round_trip() {
        let config = WebhookConfig {
            endpoints: vec![
                WebhookEndpoint {
                    url: "https://example.com/hook".to_string(),
                    events: vec!["analysis.complete".to_string()],
                    secret: Some("s3cr3t".to_string()),
                },
                WebhookEndpoint {
                    url: "https://other.com/hook".to_string(),
                    events: vec!["score.changed".to_string(), "drift.detected".to_string()],
                    secret: None,
                },
            ],
        };
        let json = serde_json::to_string(&config).unwrap();
        let decoded: WebhookConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.endpoints.len(), 2);
        assert_eq!(decoded.endpoints[0].url, "https://example.com/hook");
        assert_eq!(decoded.endpoints[0].secret, Some("s3cr3t".to_string()));
        assert_eq!(decoded.endpoints[1].secret, None);
        assert_eq!(decoded.endpoints[1].events.len(), 2);
    }

    // ---- WebhookEndpoint serde round-trip ----

    #[test]
    fn test_webhook_endpoint_with_secret_round_trip() {
        let ep = WebhookEndpoint {
            url: "https://hooks.example.com/webhook".to_string(),
            events: vec!["analysis.complete".to_string()],
            secret: Some("mysecret".to_string()),
        };
        let json = serde_json::to_string(&ep).unwrap();
        let decoded: WebhookEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.url, ep.url);
        assert_eq!(decoded.events, ep.events);
        assert_eq!(decoded.secret, ep.secret);
    }

    #[test]
    fn test_webhook_endpoint_without_secret_round_trip() {
        let ep = WebhookEndpoint {
            url: "https://hooks.example.com/other".to_string(),
            events: vec!["drift.detected".to_string()],
            secret: None,
        };
        let json = serde_json::to_string(&ep).unwrap();
        let decoded: WebhookEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.url, ep.url);
        assert_eq!(decoded.secret, None);
    }

    #[test]
    fn test_webhook_endpoint_clone() {
        let ep = WebhookEndpoint {
            url: "https://example.com".to_string(),
            events: vec!["score.changed".to_string()],
            secret: Some("key".to_string()),
        };
        let cloned = ep.clone();
        assert_eq!(cloned.url, ep.url);
        assert_eq!(cloned.secret, ep.secret);
    }

    // ---- get_timestamp ----

    #[test]
    fn test_get_timestamp_returns_non_empty() {
        let ts = get_timestamp();
        // On any system with `date`, must be non-empty; if date fails, returns "unknown"
        assert!(!ts.is_empty());
    }

    #[test]
    fn test_get_timestamp_looks_like_date_or_unknown() {
        let ts = get_timestamp();
        // Either a datetime string (contains '-') or the fallback "unknown"
        assert!(ts.contains('-') || ts == "unknown");
    }

    // ---- preview_webhook ----

    #[test]
    fn test_preview_webhook_analysis_complete() {
        let data = json!({"file_count": 5, "passed": true});
        let result = preview_webhook(WebhookEvent::AnalysisComplete, "my_app", data);
        assert!(result.is_ok());
        let s = result.unwrap();
        assert!(s.contains("analysis.complete"));
        assert!(s.contains("my_app"));
        assert!(s.contains("file_count"));
    }

    #[test]
    fn test_preview_webhook_score_changed() {
        let data = json!({"score": 88, "grade": "B"});
        let result = preview_webhook(WebhookEvent::ScoreChanged, "proj_b", data);
        assert!(result.is_ok());
        let s = result.unwrap();
        assert!(s.contains("score.changed"));
        assert!(s.contains("proj_b"));
        assert!(s.contains("score"));
    }

    #[test]
    fn test_preview_webhook_drift_detected() {
        let data = json!({"drift_score": 0.25, "findings_count": 3});
        let result = preview_webhook(WebhookEvent::DriftDetected, "drift_proj", data);
        assert!(result.is_ok());
        let s = result.unwrap();
        assert!(s.contains("drift.detected"));
        assert!(s.contains("drift_proj"));
    }

    #[test]
    fn test_preview_webhook_threshold_exceeded() {
        let data = json!({"threshold": 80, "actual": 72});
        let result = preview_webhook(WebhookEvent::ThresholdExceeded, "t_proj", data);
        assert!(result.is_ok());
        let s = result.unwrap();
        assert!(s.contains("threshold.exceeded"));
        assert!(s.contains("t_proj"));
    }

    #[test]
    fn test_preview_webhook_is_valid_json() {
        let data = json!({"key": "value"});
        let result = preview_webhook(WebhookEvent::AnalysisComplete, "json_test", data);
        assert!(result.is_ok());
        let s = result.unwrap();
        // Must parse back as valid JSON object
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(parsed.is_object());
        assert_eq!(parsed["event"], "analysis.complete");
        assert_eq!(parsed["project"], "json_test");
        assert!(parsed["timestamp"].is_string());
        assert_eq!(parsed["data"]["key"], "value");
    }

    #[test]
    fn test_preview_webhook_empty_project_name() {
        let data = json!({});
        let result = preview_webhook(WebhookEvent::ScoreChanged, "", data);
        assert!(result.is_ok());
        let s = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["project"], "");
    }

    // ---- send_analysis_webhook with unreachable URL ----

    #[test]
    fn test_send_analysis_webhook_bad_url_returns_err() {
        use crate::config::Severity;
        use crate::reporters::{AnalysisReport, Issue};
        use std::path::PathBuf;

        let report = AnalysisReport {
            issues: vec![
                Issue {
                    rule: "test_rule".to_string(),
                    message: "test message".to_string(),
                    severity: Severity::Error,
                    file: PathBuf::from("lib/main.dart"),
                    line: 1,
                    column: 0,
                },
                Issue {
                    rule: "warn_rule".to_string(),
                    message: "warning message".to_string(),
                    severity: Severity::Warning,
                    file: PathBuf::from("lib/other.dart"),
                    line: 5,
                    column: 2,
                },
            ],
            metrics: vec![],
            file_count: 2,
            project_path: None,
        };

        // Port 1 is reserved/unreachable — curl will exit non-zero
        let result = send_analysis_webhook("http://127.0.0.1:1", "test_proj", &report);
        assert!(result.is_err());
    }

    // ---- send_score_webhook with unreachable URL ----

    #[test]
    fn test_send_score_webhook_bad_url_returns_err() {
        use crate::ai_score::score::{AiCodeScore, DimensionScore, Grade};

        let score = AiCodeScore {
            overall: 90,
            resource_safety: DimensionScore {
                score: 90,
                findings: vec![],
            },
            error_handling: DimensionScore {
                score: 85,
                findings: vec![],
            },
            type_safety: DimensionScore {
                score: 95,
                findings: vec![],
            },
            security: DimensionScore {
                score: 88,
                findings: vec![],
            },
            convention_match: DimensionScore {
                score: 80,
                findings: vec![],
            },
            complexity: DimensionScore {
                score: 92,
                findings: vec![],
            },
            file_count: 10,
            total_issues: 2,
            grade: Grade::A,
        };

        let result = send_score_webhook("http://127.0.0.1:1", "score_proj", &score, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_score_webhook_with_previous_score_bad_url() {
        use crate::ai_score::score::{AiCodeScore, DimensionScore, Grade};

        let score = AiCodeScore {
            overall: 75,
            resource_safety: DimensionScore {
                score: 70,
                findings: vec!["issue1".to_string()],
            },
            error_handling: DimensionScore {
                score: 80,
                findings: vec![],
            },
            type_safety: DimensionScore {
                score: 75,
                findings: vec![],
            },
            security: DimensionScore {
                score: 70,
                findings: vec![],
            },
            convention_match: DimensionScore {
                score: 78,
                findings: vec![],
            },
            complexity: DimensionScore {
                score: 77,
                findings: vec![],
            },
            file_count: 5,
            total_issues: 1,
            grade: Grade::C,
        };

        let result = send_score_webhook("http://127.0.0.1:1", "proj_c", &score, Some(80));
        assert!(result.is_err());
    }

    // ---- send_drift_webhook with unreachable URL ----

    #[test]
    fn test_send_drift_webhook_bad_url_returns_err() {
        use crate::ai_score::convention::{
            ArchitectureConventions, ConventionReport, ErrorHandlingConventions, NamingConventions,
        };
        use crate::ai_score::drift::DriftReport;

        let drift_report = DriftReport {
            conventions: ConventionReport {
                naming: NamingConventions {
                    file_naming: "snake_case".to_string(),
                    class_naming: "PascalCase".to_string(),
                    private_prefix: true,
                    uses_underscore_params: false,
                },
                architecture: ArchitectureConventions {
                    pattern: "BLoC".to_string(),
                    layers_detected: vec!["bloc".to_string(), "repository".to_string()],
                    has_separate_models: true,
                    has_separate_services: false,
                },
                error_handling: ErrorHandlingConventions {
                    uses_result_type: true,
                    uses_either: false,
                    uses_try_catch: true,
                    uses_custom_exceptions: false,
                },
                state_management: Some("bloc".to_string()),
                consistency_score: 0.85,
            },
            findings: vec![],
            drift_score: 0.15,
            files_analyzed: 8,
        };

        let result = send_drift_webhook("http://127.0.0.1:1", "drift_test", &drift_report);
        assert!(result.is_err());
    }
}
