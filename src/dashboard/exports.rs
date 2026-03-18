use super::snapshot::AnalysisSnapshot;
use serde::Serialize;
use std::path::Path;

/// Export snapshot as Prometheus metrics format.
pub fn export_prometheus(snapshot: &AnalysisSnapshot) -> String {
    let mut output = String::new();

    output.push_str("# HELP falcon_health_score Code health score 0-100\n");
    output.push_str("# TYPE falcon_health_score gauge\n");
    output.push_str(&format!("falcon_health_score {:.1}\n", snapshot.health_score));

    output.push_str("# HELP falcon_issues_total Total number of issues\n");
    output.push_str("# TYPE falcon_issues_total gauge\n");
    output.push_str(&format!(
        "falcon_issues_total{{severity=\"error\"}} {}\n",
        snapshot.issues.errors
    ));
    output.push_str(&format!(
        "falcon_issues_total{{severity=\"warning\"}} {}\n",
        snapshot.issues.warnings
    ));
    output.push_str(&format!(
        "falcon_issues_total{{severity=\"info\"}} {}\n",
        snapshot.issues.info
    ));

    output.push_str("# HELP falcon_files_total Total number of files analyzed\n");
    output.push_str("# TYPE falcon_files_total gauge\n");
    output.push_str(&format!("falcon_files_total {}\n", snapshot.file_count));

    output.push_str("# HELP falcon_lines_total Total lines of code\n");
    output.push_str("# TYPE falcon_lines_total gauge\n");
    output.push_str(&format!("falcon_lines_total {}\n", snapshot.total_lines));

    output.push_str("# HELP falcon_complexity_avg Average cyclomatic complexity\n");
    output.push_str("# TYPE falcon_complexity_avg gauge\n");
    output.push_str(&format!(
        "falcon_complexity_avg {:.2}\n",
        snapshot.metrics_summary.avg_cyclomatic
    ));

    output.push_str("# HELP falcon_maintainability_avg Average maintainability index\n");
    output.push_str("# TYPE falcon_maintainability_avg gauge\n");
    output.push_str(&format!(
        "falcon_maintainability_avg {:.2}\n",
        snapshot.metrics_summary.avg_maintainability
    ));

    output.push_str("# HELP falcon_god_files Number of god files (>500 lines)\n");
    output.push_str("# TYPE falcon_god_files gauge\n");
    output.push_str(&format!(
        "falcon_god_files {}\n",
        snapshot.metrics_summary.god_file_count
    ));

    output.push_str("# HELP falcon_rule_violations Violations per rule\n");
    output.push_str("# TYPE falcon_rule_violations gauge\n");
    for (rule, count) in &snapshot.rule_counts {
        output.push_str(&format!(
            "falcon_rule_violations{{rule=\"{}\"}} {}\n",
            rule, count
        ));
    }

    output
}

/// Export snapshot as JSON for Grafana/Datadog.
pub fn export_json(snapshot: &AnalysisSnapshot) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(snapshot)?)
}

/// Webhook payload for quality alerts.
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub event: String,
    pub timestamp: String,
    pub project: String,
    pub health_score: f64,
    pub issues: WebhookIssues,
    pub details: String,
}

#[derive(Debug, Serialize)]
pub struct WebhookIssues {
    pub errors: usize,
    pub warnings: usize,
    pub total: usize,
}

impl WebhookPayload {
    pub fn from_snapshot(snapshot: &AnalysisSnapshot, project: &str) -> Self {
        let event = if snapshot.health_score < 50.0 {
            "health_critical"
        } else if snapshot.health_score < 70.0 {
            "health_warning"
        } else {
            "analysis_complete"
        };

        WebhookPayload {
            event: event.to_string(),
            timestamp: snapshot.timestamp.clone(),
            project: project.to_string(),
            health_score: snapshot.health_score,
            issues: WebhookIssues {
                errors: snapshot.issues.errors,
                warnings: snapshot.issues.warnings,
                total: snapshot.issues.total,
            },
            details: format!(
                "{} files analyzed, health {:.0}/100, {} issues",
                snapshot.file_count, snapshot.health_score, snapshot.issues.total
            ),
        }
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

/// Send a webhook notification.
pub fn send_webhook(url: &str, payload: &WebhookPayload) -> anyhow::Result<()> {
    let json = payload.to_json()?;

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
        anyhow::bail!("Webhook failed: {}", stderr);
    }

    Ok(())
}

/// Save Prometheus metrics to a file for scraping.
pub fn save_prometheus_metrics(snapshot: &AnalysisSnapshot, path: &Path) -> anyhow::Result<()> {
    let metrics = export_prometheus(snapshot);
    std::fs::write(path, metrics)?;
    Ok(())
}
