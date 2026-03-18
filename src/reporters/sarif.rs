use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use serde_json::{json, Value};
use std::path::PathBuf;

/// SARIF (Static Analysis Results Interchange Format) v2.1.0
/// Used by GitHub Code Scanning, Azure DevOps, and other platforms.
pub struct SarifReporter {
    pub output_path: Option<PathBuf>,
}

impl Reporter for SarifReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        let sarif = build_sarif(&report.issues);
        output_sarif(&sarif, &self.output_path);
    }

    fn report_metrics(&self, _metrics: &[(PathBuf, MetricsResults)]) {
        let sarif = build_sarif(&[]);
        output_sarif(&sarif, &self.output_path);
    }

    fn report_issues(&self, issues: &[Issue]) {
        let sarif = build_sarif(issues);
        output_sarif(&sarif, &self.output_path);
    }
}

fn build_sarif(issues: &[Issue]) -> Value {
    let rules: Vec<Value> = collect_unique_rules(issues);

    let results: Vec<Value> = issues
        .iter()
        .map(|issue| {
            json!({
                "ruleId": issue.rule,
                "level": sarif_level(&issue.severity),
                "message": { "text": issue.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": issue.file.to_string_lossy(),
                            "uriBaseId": "%SRCROOT%"
                        },
                        "region": {
                            "startLine": issue.line,
                            "startColumn": issue.column
                        }
                    }
                }]
            })
        })
        .collect();

    json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "Falcon",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/viveky259259/falcon",
                    "rules": rules
                }
            },
            "results": results
        }]
    })
}

fn sarif_level(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

fn collect_unique_rules(issues: &[Issue]) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut rules = Vec::new();

    for issue in issues {
        if seen.insert(issue.rule.clone()) {
            rules.push(json!({
                "id": issue.rule,
                "shortDescription": { "text": issue.rule },
                "defaultConfiguration": {
                    "level": sarif_level(&issue.severity)
                }
            }));
        }
    }

    rules
}

fn output_sarif(sarif: &Value, output_path: &Option<PathBuf>) {
    let json_str = serde_json::to_string_pretty(sarif).unwrap_or_default();
    if let Some(path) = output_path {
        match std::fs::write(path, &json_str) {
            Ok(_) => eprintln!("SARIF report written to: {}", path.display()),
            Err(e) => eprintln!("Failed to write SARIF report: {}", e),
        }
    } else {
        println!("{}", json_str);
    }
}
