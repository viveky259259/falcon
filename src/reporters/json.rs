use super::{AnalysisReport, Issue, Reporter};
use crate::metrics::MetricsResults;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct JsonReporter;

impl Reporter for JsonReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        let output = json!({
            "summary": {
                "files_analyzed": report.file_count,
                "errors": report.error_count(),
                "warnings": report.warning_count(),
                "info": report.info_count(),
            },
            "issues": issues_to_json(&report.issues),
            "metrics": metrics_to_json(&report.metrics),
        });

        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    }

    fn report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]) {
        let output = json!({
            "metrics": metrics_to_json(metrics),
        });

        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    }

    fn report_issues(&self, issues: &[Issue]) {
        let output = json!({
            "issues": issues_to_json(issues),
        });

        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    }
}

fn issues_to_json(issues: &[Issue]) -> Vec<Value> {
    issues
        .iter()
        .map(|issue| {
            json!({
                "rule": issue.rule,
                "message": issue.message,
                "severity": issue.severity.to_string(),
                "file": issue.file.to_string_lossy(),
                "line": issue.line,
                "column": issue.column,
            })
        })
        .collect()
}

fn metrics_to_json(metrics: &[(PathBuf, MetricsResults)]) -> Vec<Value> {
    metrics
        .iter()
        .map(|(file, result)| {
            json!({
                "file": file.to_string_lossy(),
                "lines_of_code": result.file_lines_of_code,
                "source_lines_of_code": result.file_source_lines_of_code,
                "functions": result.functions.iter().map(|f| {
                    json!({
                        "name": f.name,
                        "line": f.line,
                        "cyclomatic_complexity": f.cyclomatic_complexity,
                        "lines_of_code": f.lines_of_code,
                        "source_lines_of_code": f.source_lines_of_code,
                        "maintainability_index": f.maintainability_index,
                        "max_nesting_level": f.max_nesting_level,
                        "number_of_parameters": f.number_of_parameters,
                    })
                }).collect::<Vec<_>>(),
                "classes": result.classes.iter().map(|c| {
                    json!({
                        "name": c.name,
                        "line": c.line,
                        "number_of_methods": c.number_of_methods,
                        "lines_of_code": c.lines_of_code,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect()
}
