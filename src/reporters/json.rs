use super::{AnalysisReport, Issue, Reporter};
use crate::metrics::MetricsResults;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct JsonReporter;

fn analysis_to_json(report: &AnalysisReport) -> Value {
    json!({
        "summary": {
            "files_analyzed": report.file_count,
            "errors": report.error_count(),
            "warnings": report.warning_count(),
            "info": report.info_count(),
        },
        "issues": issues_to_json(&report.issues),
        "metrics": metrics_to_json(&report.metrics),
    })
}

fn metrics_doc(metrics: &[(PathBuf, MetricsResults)]) -> Value {
    json!({ "metrics": metrics_to_json(metrics) })
}

fn issues_doc(issues: &[Issue]) -> Value {
    json!({ "issues": issues_to_json(issues) })
}

impl Reporter for JsonReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        println!(
            "{}",
            serde_json::to_string_pretty(&analysis_to_json(report)).unwrap_or_default()
        );
    }

    fn report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]) {
        println!(
            "{}",
            serde_json::to_string_pretty(&metrics_doc(metrics)).unwrap_or_default()
        );
    }

    fn report_issues(&self, issues: &[Issue]) {
        println!(
            "{}",
            serde_json::to_string_pretty(&issues_doc(issues)).unwrap_or_default()
        );
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
                        "halstead_volume": f.halstead_volume,
                        "halstead_difficulty": f.halstead_difficulty,
                        "widgets_nesting_level": f.widgets_nesting_level,
                        "number_of_used_widgets": f.number_of_used_widgets,
                    })
                }).collect::<Vec<_>>(),
                "classes": result.classes.iter().map(|c| {
                    json!({
                        "name": c.name,
                        "line": c.line,
                        "number_of_methods": c.number_of_methods,
                        "lines_of_code": c.lines_of_code,
                        "coupling_between_objects": c.coupling_between_objects,
                        "depth_of_inheritance": c.depth_of_inheritance,
                        "number_of_added_methods": c.number_of_added_methods,
                        "number_of_interfaces": c.number_of_interfaces,
                        "number_of_overridden_methods": c.number_of_overridden_methods,
                        "response_for_class": c.response_for_class,
                        "tight_class_cohesion": c.tight_class_cohesion,
                        "weight_of_class": c.weight_of_class,
                        "weighted_methods_per_class": c.weighted_methods_per_class,
                        "lack_of_cohesion": c.lack_of_cohesion,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use crate::metrics::{ClassMetrics, FunctionMetrics, MetricsResults};
    use crate::reporters::{AnalysisReport, Issue};
    use std::path::PathBuf;

    fn sample_issue(rule: &str, severity: Severity) -> Issue {
        Issue {
            rule: rule.to_string(),
            message: format!("test message for {}", rule),
            severity,
            file: PathBuf::from("a.dart"),
            line: 10,
            column: 5,
        }
    }

    fn sample_function_metrics() -> FunctionMetrics {
        FunctionMetrics {
            name: "myFunc".to_string(),
            line: 5,
            cyclomatic_complexity: 3,
            lines_of_code: 20,
            source_lines_of_code: 15,
            maintainability_index: 75.0,
            max_nesting_level: 2,
            number_of_parameters: 1,
            halstead_volume: 50.0,
            halstead_difficulty: 5.0,
            widgets_nesting_level: 0,
            number_of_used_widgets: 0,
        }
    }

    fn sample_class_metrics() -> ClassMetrics {
        ClassMetrics {
            name: "MyClass".to_string(),
            line: 1,
            number_of_methods: 3,
            lines_of_code: 40,
            coupling_between_objects: 2,
            depth_of_inheritance: 1,
            number_of_added_methods: 0,
            number_of_interfaces: 1,
            number_of_overridden_methods: 0,
            response_for_class: 5,
            tight_class_cohesion: 0.5,
            weight_of_class: 0.8,
            weighted_methods_per_class: 3,
            lack_of_cohesion: 0,
        }
    }

    fn sample_metrics_results() -> MetricsResults {
        MetricsResults {
            file_lines_of_code: 100,
            file_source_lines_of_code: 80,
            functions: vec![sample_function_metrics()],
            classes: vec![sample_class_metrics()],
        }
    }

    #[test]
    fn issues_to_json_empty_returns_empty_array() {
        let result = issues_to_json(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn issues_to_json_serializes_all_fields() {
        let issue = Issue {
            rule: "no-print".to_string(),
            message: "remove print".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("a.dart"),
            line: 12,
            column: 3,
        };
        let result = issues_to_json(&[issue]);
        assert_eq!(result.len(), 1);
        let v = &result[0];
        assert_eq!(v["rule"], "no-print");
        assert_eq!(v["message"], "remove print");
        assert_eq!(v["severity"], "warning");
        assert_eq!(v["file"], "a.dart");
        assert_eq!(v["line"], 12);
        assert_eq!(v["column"], 3);
    }

    #[test]
    fn metrics_to_json_empty_returns_empty_array() {
        let result = metrics_to_json(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn metrics_to_json_serializes_function_and_class_fields() {
        let metrics = vec![(PathBuf::from("a.dart"), sample_metrics_results())];
        let result = metrics_to_json(&metrics);
        assert_eq!(result.len(), 1);
        let entry = &result[0];
        assert_eq!(entry["file"], "a.dart");
        assert_eq!(entry["lines_of_code"], 100);
        assert_eq!(entry["source_lines_of_code"], 80);

        let functions = entry["functions"].as_array().unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0]["name"], "myFunc");
        assert_eq!(functions[0]["cyclomatic_complexity"], 3);

        let classes = entry["classes"].as_array().unwrap();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0]["name"], "MyClass");
        assert_eq!(classes[0]["number_of_methods"], 3);
    }

    #[test]
    fn analysis_to_json_includes_summary_issues_and_metrics_keys() {
        let report = AnalysisReport {
            issues: vec![sample_issue("rule-a", Severity::Warning)],
            metrics: vec![(PathBuf::from("a.dart"), sample_metrics_results())],
            file_count: 1,
            project_path: None,
        };
        let v = analysis_to_json(&report);
        assert!(v["summary"].is_object());
        let issues_arr = v["issues"].as_array().unwrap();
        assert_eq!(issues_arr.len(), 1);
        let metrics_arr = v["metrics"].as_array().unwrap();
        assert_eq!(metrics_arr.len(), 1);
    }

    #[test]
    fn analysis_to_json_summary_counts_severities_correctly() {
        let report = AnalysisReport {
            issues: vec![
                sample_issue("r1", Severity::Error),
                sample_issue("r2", Severity::Error),
                sample_issue("r3", Severity::Warning),
                sample_issue("r4", Severity::Info),
            ],
            metrics: vec![],
            file_count: 5,
            project_path: None,
        };
        let v = analysis_to_json(&report);
        assert_eq!(v["summary"]["errors"], 2);
        assert_eq!(v["summary"]["warnings"], 1);
        assert_eq!(v["summary"]["info"], 1);
        assert_eq!(v["summary"]["files_analyzed"], 5);
    }

    #[test]
    fn metrics_doc_wraps_metrics_under_key() {
        let v = metrics_doc(&[]);
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert!(obj.contains_key("metrics"));
        assert_eq!(v["metrics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn issues_doc_wraps_issues_under_key() {
        let v = issues_doc(&[]);
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert!(obj.contains_key("issues"));
        assert_eq!(v["issues"].as_array().unwrap().len(), 0);
    }
}
