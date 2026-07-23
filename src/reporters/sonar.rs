use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use serde_json::{json, Value};
use std::path::PathBuf;

/// SonarQube Generic Issue Data format.
pub struct SonarReporter {
    pub output_path: Option<PathBuf>,
}

impl Reporter for SonarReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        output_sonar(&build_sonar(&report.issues), &self.output_path);
    }

    fn report_metrics(&self, _metrics: &[(PathBuf, MetricsResults)]) {
        output_sonar(&build_sonar(&[]), &self.output_path);
    }

    fn report_issues(&self, issues: &[Issue]) {
        output_sonar(&build_sonar(issues), &self.output_path);
    }
}

fn build_sonar(issues: &[Issue]) -> Value {
    let sonar_issues: Vec<Value> = issues
        .iter()
        .map(|issue| {
            json!({
                "engineId": "falcon",
                "ruleId": issue.rule,
                "severity": sonar_severity(&issue.severity),
                "type": sonar_type(&issue.rule),
                "primaryLocation": {
                    "message": issue.message,
                    "filePath": issue.file.to_string_lossy(),
                    "textRange": {
                        "startLine": issue.line,
                        "startColumn": issue.column.saturating_sub(1),
                    }
                }
            })
        })
        .collect();

    json!({ "issues": sonar_issues })
}

fn sonar_severity(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "CRITICAL",
        Severity::Warning => "MAJOR",
        Severity::Info => "MINOR",
    }
}

fn sonar_type(_rule: &str) -> &'static str {
    // All Falcon findings currently map to the SonarQube CODE_SMELL type.
    "CODE_SMELL"
}

fn output_sonar(sonar: &Value, output_path: &Option<PathBuf>) {
    let json_str = serde_json::to_string_pretty(sonar).unwrap_or_default();
    if let Some(path) = output_path {
        match std::fs::write(path, &json_str) {
            Ok(_) => eprintln!("SonarQube report written to: {}", path.display()),
            Err(e) => eprintln!("Failed to write SonarQube report: {}", e),
        }
    } else {
        println!("{}", json_str);
    }
}
