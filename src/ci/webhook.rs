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
            "-s", "-X", "POST",
            "-H", "Content-Type: application/json",
            "-d", &json,
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
