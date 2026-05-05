use std::collections::HashMap;

// ============================================================
// Snapshot Tests
// ============================================================

#[test]
fn test_snapshot_capture() {
    let report = create_test_report();
    let dir = tempfile::tempdir().unwrap();
    let snapshot = falcon::dashboard::snapshot::AnalysisSnapshot::capture(&report, dir.path());

    assert_eq!(snapshot.file_count, 1);
    assert!(snapshot.health_score > 0.0);
    assert_eq!(snapshot.issues.total, 2);
    assert_eq!(snapshot.issues.warnings, 2);
    assert!(!snapshot.timestamp.is_empty());
}

#[test]
fn test_snapshot_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let report = create_test_report();
    let snapshot = falcon::dashboard::snapshot::AnalysisSnapshot::capture(&report, dir.path());

    falcon::dashboard::snapshot::save_snapshot(dir.path(), &snapshot).unwrap();

    let history = falcon::dashboard::snapshot::load_history(dir.path()).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].file_count, 1);
    assert_eq!(history[0].issues.total, 2);
}

#[test]
fn test_snapshot_history_append() {
    let dir = tempfile::tempdir().unwrap();
    let report = create_test_report();

    for _ in 0..3 {
        let snapshot = falcon::dashboard::snapshot::AnalysisSnapshot::capture(&report, dir.path());
        falcon::dashboard::snapshot::save_snapshot(dir.path(), &snapshot).unwrap();
    }

    let history = falcon::dashboard::snapshot::load_history(dir.path()).unwrap();
    assert_eq!(history.len(), 3);
}

#[test]
fn test_snapshot_empty_history() {
    let dir = tempfile::tempdir().unwrap();
    let history = falcon::dashboard::snapshot::load_history(dir.path()).unwrap();
    assert!(history.is_empty());
}

// ============================================================
// Trends Tests
// ============================================================

#[test]
fn test_trends_need_minimum_snapshots() {
    let history = vec![create_test_snapshot(80.0, 10)];
    let report = falcon::dashboard::trends::analyze_trends(&history, 10);
    assert!(report.is_none());
}

#[test]
fn test_trends_improving() {
    let history = vec![
        create_test_snapshot(60.0, 50),
        create_test_snapshot(70.0, 40),
        create_test_snapshot(85.0, 20),
    ];
    let report = falcon::dashboard::trends::analyze_trends(&history, 10);
    assert!(report.is_some());
    let report = report.unwrap();
    assert_eq!(
        report.health_trend,
        falcon::dashboard::trends::TrendDirection::Improving
    );
}

#[test]
fn test_trends_declining() {
    let history = vec![
        create_test_snapshot(90.0, 5),
        create_test_snapshot(80.0, 15),
        create_test_snapshot(60.0, 50),
    ];
    let report = falcon::dashboard::trends::analyze_trends(&history, 10);
    assert!(report.is_some());
    let report = report.unwrap();
    assert_eq!(
        report.health_trend,
        falcon::dashboard::trends::TrendDirection::Declining
    );
}

#[test]
fn test_trends_stable() {
    let history = vec![
        create_test_snapshot(80.0, 10),
        create_test_snapshot(81.0, 9),
    ];
    let report = falcon::dashboard::trends::analyze_trends(&history, 10);
    assert!(report.is_some());
    let report = report.unwrap();
    assert_eq!(
        report.health_trend,
        falcon::dashboard::trends::TrendDirection::Stable
    );
}

// ============================================================
// Rule Impact Tests
// ============================================================

#[test]
fn test_rule_impact_measurement() {
    let mut history = Vec::new();
    for i in 0..5 {
        let mut snap = create_test_snapshot(80.0, 10);
        snap.rule_counts.insert("avoid-dynamic".to_string(), 10 + i);
        snap.rule_counts.insert("prefer-const".to_string(), 2);
        history.push(snap);
    }

    let impacts = falcon::dashboard::rule_impact::measure_rule_impact(&history);
    assert!(!impacts.is_empty());

    let dynamic_rule = impacts.iter().find(|i| i.rule == "avoid-dynamic");
    assert!(dynamic_rule.is_some());
    assert!(dynamic_rule.unwrap().total_triggers > 0);
}

#[test]
fn test_auto_tune_recommendations() {
    let impacts = vec![
        falcon::dashboard::rule_impact::RuleImpact {
            rule: "noisy-rule".to_string(),
            total_triggers: 500,
            trend: falcon::dashboard::rule_impact::ImpactTrend::Stable,
            signal_score: 20.0,
        },
        falcon::dashboard::rule_impact::RuleImpact {
            rule: "good-rule".to_string(),
            total_triggers: 2,
            trend: falcon::dashboard::rule_impact::ImpactTrend::Stable,
            signal_score: 95.0,
        },
    ];

    let recs = falcon::dashboard::rule_impact::auto_tune_recommendations(&impacts);
    assert!(!recs.is_empty());

    let noisy = recs.iter().find(|r| r.rule == "noisy-rule");
    assert!(noisy.is_some());
}

#[test]
fn test_rule_impact_empty_history() {
    let impacts = falcon::dashboard::rule_impact::measure_rule_impact(&[]);
    assert!(impacts.is_empty());
}

// ============================================================
// Exports Tests
// ============================================================

#[test]
fn test_export_prometheus() {
    let snapshot = create_test_snapshot(85.0, 15);
    let metrics = falcon::dashboard::exports::export_prometheus(&snapshot);

    assert!(metrics.contains("falcon_health_score 85.0"));
    assert!(metrics.contains("falcon_files_total"));
    assert!(metrics.contains("falcon_issues_total"));
    assert!(metrics.contains("falcon_complexity_avg"));
}

#[test]
fn test_export_json() {
    let snapshot = create_test_snapshot(85.0, 15);
    let json = falcon::dashboard::exports::export_json(&snapshot).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["health_score"], 85.0);
    assert_eq!(parsed["file_count"], 10);
}

#[test]
fn test_webhook_payload() {
    let snapshot = create_test_snapshot(45.0, 50);
    let payload = falcon::dashboard::exports::WebhookPayload::from_snapshot(&snapshot, "my-app");

    assert_eq!(payload.event, "health_critical");
    assert_eq!(payload.project, "my-app");
    assert_eq!(payload.health_score, 45.0);

    let json = payload.to_json().unwrap();
    assert!(json.contains("health_critical"));
}

#[test]
fn test_webhook_healthy_event() {
    let snapshot = create_test_snapshot(90.0, 5);
    let payload = falcon::dashboard::exports::WebhookPayload::from_snapshot(&snapshot, "app");
    assert_eq!(payload.event, "analysis_complete");
}

#[test]
fn test_prometheus_save_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let snapshot = create_test_snapshot(80.0, 10);
    let path = dir.path().join("metrics.prom");

    falcon::dashboard::exports::save_prometheus_metrics(&snapshot, &path).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("falcon_health_score"));
}

// ============================================================
// Dashboard Server Tests
// ============================================================

#[test]
fn test_dashboard_server_needs_history() {
    let dir = tempfile::tempdir().unwrap();
    let result = falcon::dashboard::server::start_dashboard(dir.path(), 0);
    assert!(result.is_err());
}

// ============================================================
// Helpers
// ============================================================

fn create_test_report() -> falcon::reporters::AnalysisReport {
    use falcon::config::Severity;
    use falcon::reporters::Issue;
    use std::path::PathBuf;

    let issues = vec![
        Issue {
            rule: "avoid-dynamic".to_string(),
            message: "Avoid dynamic".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("test.dart"),
            line: 1,
            column: 1,
        },
        Issue {
            rule: "avoid-dynamic".to_string(),
            message: "Avoid dynamic".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("test.dart"),
            line: 5,
            column: 1,
        },
    ];

    let mut parser = falcon::parser::DartParser::new().unwrap();
    let source = "void main() { dynamic x = 1; dynamic y = 2; }";
    let tree = parser.parse(source).unwrap();
    let config = falcon::config::FalconConfig::default();
    let metrics =
        falcon::metrics::calculate_file_metrics(tree.root_node(), source, &config.metrics);

    falcon::reporters::AnalysisReport {
        issues,
        metrics: vec![(PathBuf::from("test.dart"), metrics)],
        file_count: 1,
        project_path: None,
    }
}

fn create_test_snapshot(
    health: f64,
    total_issues: usize,
) -> falcon::dashboard::snapshot::AnalysisSnapshot {
    let mut rule_counts = HashMap::new();
    rule_counts.insert("avoid-dynamic".to_string(), total_issues / 2);
    rule_counts.insert("prefer-const".to_string(), total_issues / 2);

    falcon::dashboard::snapshot::AnalysisSnapshot {
        timestamp: "2026-03-18T10:00:00".to_string(),
        commit_hash: Some("abc1234".to_string()),
        commit_message: Some("test commit".to_string()),
        branch: Some("main".to_string()),
        file_count: 10,
        total_lines: 1000,
        health_score: health,
        issues: falcon::dashboard::snapshot::IssueSummary {
            errors: 0,
            warnings: total_issues,
            info: 0,
            total: total_issues,
        },
        metrics_summary: falcon::dashboard::snapshot::MetricsSummary {
            avg_cyclomatic: 5.0,
            max_cyclomatic: 15,
            avg_maintainability: 70.0,
            avg_lines_per_file: 100.0,
            god_file_count: 0,
        },
        rule_counts,
        per_package: Vec::new(),
    }
}
