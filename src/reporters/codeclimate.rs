use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use serde_json::{json, Value};
use std::path::PathBuf;

/// CodeClimate / GitLab Code Quality format.
pub struct CodeClimateReporter {
    pub output_path: Option<PathBuf>,
}

impl Reporter for CodeClimateReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        output_cc(&build_cc(&report.issues), &self.output_path);
    }

    fn report_metrics(&self, _metrics: &[(PathBuf, MetricsResults)]) {
        output_cc(&build_cc(&[]), &self.output_path);
    }

    fn report_issues(&self, issues: &[Issue]) {
        output_cc(&build_cc(issues), &self.output_path);
    }
}

fn build_cc(issues: &[Issue]) -> Vec<Value> {
    issues
        .iter()
        .map(|issue| {
            let fingerprint = format!(
                "{:x}",
                md5_hash(&format!("{}:{}:{}:{}", issue.rule, issue.file.display(), issue.line, issue.column))
            );

            json!({
                "type": "issue",
                "check_name": issue.rule,
                "description": issue.message,
                "categories": ["Style"],
                "severity": cc_severity(&issue.severity),
                "fingerprint": fingerprint,
                "location": {
                    "path": issue.file.to_string_lossy(),
                    "lines": {
                        "begin": issue.line,
                        "end": issue.line
                    }
                }
            })
        })
        .collect()
}

fn cc_severity(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "critical",
        Severity::Warning => "major",
        Severity::Info => "minor",
    }
}

fn md5_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn output_cc(entries: &[Value], output_path: &Option<PathBuf>) {
    let json_str = serde_json::to_string_pretty(entries).unwrap_or_default();
    if let Some(path) = output_path {
        match std::fs::write(path, &json_str) {
            Ok(_) => eprintln!("CodeClimate report written to: {}", path.display()),
            Err(e) => eprintln!("Failed to write CodeClimate report: {}", e),
        }
    } else {
        println!("{}", json_str);
    }
}
