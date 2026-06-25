use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// SARIF (Static Analysis Results Interchange Format) v2.1.0
/// Used by GitHub Code Scanning, Azure DevOps, and other platforms.
pub struct SarifReporter {
    pub output_path: Option<PathBuf>,
}

impl Reporter for SarifReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        let sarif = build_sarif(&report.issues, report.project_path.as_deref());
        output_sarif(&sarif, &self.output_path);
    }

    fn report_metrics(&self, _metrics: &[(PathBuf, MetricsResults)]) {
        let sarif = build_sarif(&[], None);
        output_sarif(&sarif, &self.output_path);
    }

    fn report_issues(&self, issues: &[Issue]) {
        let sarif = build_sarif(issues, None);
        output_sarif(&sarif, &self.output_path);
    }
}

fn build_sarif(issues: &[Issue], project_root: Option<&Path>) -> Value {
    let (rules, rule_indices) = collect_unique_rules(issues);

    let results: Vec<Value> = issues
        .iter()
        .map(|issue| {
            let rule_index = rule_indices.get(&issue.rule).copied().unwrap_or_default();
            json!({
                "ruleId": issue.rule,
                "ruleIndex": rule_index,
                "kind": "fail",
                "level": sarif_level(&issue.severity),
                "message": { "text": issue.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": artifact_uri(&issue.file, project_root)
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
                    "semanticVersion": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/viveky259259/falcon",
                    "rules": rules
                }
            },
            "columnKind": "utf16CodeUnits",
            "invocations": [{
                "executionSuccessful": true
            }],
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

fn collect_unique_rules(issues: &[Issue]) -> (Vec<Value>, HashMap<String, usize>) {
    let mut rule_indices = HashMap::new();
    let mut rules = Vec::new();

    for issue in issues {
        if !rule_indices.contains_key(&issue.rule) {
            let index = rules.len();
            rule_indices.insert(issue.rule.clone(), index);
            rules.push(json!({
                "id": issue.rule,
                "name": issue.rule,
                "shortDescription": { "text": issue.rule },
                "fullDescription": { "text": issue.message },
                "defaultConfiguration": {
                    "level": sarif_level(&issue.severity)
                }
            }));
        }
    }

    (rules, rule_indices)
}

fn artifact_uri(file: &Path, project_root: Option<&Path>) -> String {
    let path = project_root
        .and_then(|root| file.strip_prefix(root).ok())
        .unwrap_or(file);
    path.to_string_lossy().replace('\\', "/")
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
