// ============================================================
// Stability Contract Tests
// ============================================================

#[test]
fn test_stability_contract_defaults() {
    let contract = falcon::stability::contract::StabilityContract::default();
    assert_eq!(contract.version, "1.2");
    assert!(contract.guarantees.len() >= 5);
    assert_eq!(contract.deprecation_policy.notice_period_months, 6);
    assert!(contract.migration_policy.auto_migration);
}

#[test]
fn test_stability_guarantees_cover_key_areas() {
    let contract = falcon::stability::contract::StabilityContract::default();
    let areas: Vec<&str> = contract.guarantees.iter().map(|g| g.area.as_str()).collect();
    assert!(areas.contains(&"Configuration"));
    assert!(areas.contains(&"Rule naming"));
    assert!(areas.contains(&"Exit codes"));
    assert!(areas.contains(&"Performance"));
}

#[test]
fn test_deprecation_stages() {
    let contract = falcon::stability::contract::StabilityContract::default();
    assert_eq!(contract.deprecation_policy.deprecation_stages.len(), 4);
}

// ============================================================
// Deprecation Tests
// ============================================================

#[test]
fn test_no_deprecated_rules_initially() {
    let deprecated = falcon::stability::deprecation::deprecated_rules();
    assert!(deprecated.is_empty(), "No rules should be deprecated yet");
}

#[test]
fn test_is_deprecated_returns_none() {
    assert!(falcon::stability::deprecation::is_deprecated("avoid-dynamic").is_none());
}

#[test]
fn test_check_deprecated_in_config() {
    let config = falcon::config::FalconConfig::default();
    let found = falcon::stability::deprecation::check_deprecated_in_config(&config);
    assert!(found.is_empty());
}

// ============================================================
// Performance Tracking Tests
// ============================================================

#[test]
fn test_perf_snapshot_capture() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();

    let snapshot = falcon::stability::perf_track::capture_perf_snapshot(dir.path()).unwrap();
    assert_eq!(snapshot.file_count, 1);
    assert!(snapshot.total_lines > 0);
    assert!(!snapshot.falcon_version.is_empty());
}

#[test]
fn test_perf_history_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();

    let snapshot = falcon::stability::perf_track::capture_perf_snapshot(dir.path()).unwrap();
    falcon::stability::perf_track::save_perf_snapshot(dir.path(), &snapshot).unwrap();

    let history = falcon::stability::perf_track::load_perf_history(dir.path()).unwrap();
    assert_eq!(history.snapshots.len(), 1);
}

#[test]
fn test_perf_regression_detection() {
    let history = falcon::stability::perf_track::PerfHistory {
        snapshots: vec![
            falcon::stability::perf_track::PerfSnapshot {
                timestamp: "2026-01-01".to_string(),
                falcon_version: "0.1.0".to_string(),
                file_count: 100,
                total_lines: 10000,
                analysis_time_ms: 500,
                files_per_second: 200.0,
                lines_per_second: 20000.0,
                issue_count: 50,
                git_commit: None,
            },
            falcon::stability::perf_track::PerfSnapshot {
                timestamp: "2026-02-01".to_string(),
                falcon_version: "0.1.0".to_string(),
                file_count: 100,
                total_lines: 10000,
                analysis_time_ms: 800,
                files_per_second: 125.0,
                lines_per_second: 12500.0,
                issue_count: 50,
                git_commit: None,
            },
        ],
    };

    let regression = falcon::stability::perf_track::check_regression(&history);
    assert!(regression.is_some());
    let r = regression.unwrap();
    assert!(r.time_change_pct > 50.0);
    assert!(r.is_regression);
}

#[test]
fn test_no_regression_when_stable() {
    let history = falcon::stability::perf_track::PerfHistory {
        snapshots: vec![
            falcon::stability::perf_track::PerfSnapshot {
                timestamp: "2026-01-01".to_string(),
                falcon_version: "0.1.0".to_string(),
                file_count: 100,
                total_lines: 10000,
                analysis_time_ms: 500,
                files_per_second: 200.0,
                lines_per_second: 20000.0,
                issue_count: 50,
                git_commit: None,
            },
            falcon::stability::perf_track::PerfSnapshot {
                timestamp: "2026-02-01".to_string(),
                falcon_version: "0.1.0".to_string(),
                file_count: 100,
                total_lines: 10000,
                analysis_time_ms: 510,
                files_per_second: 196.0,
                lines_per_second: 19600.0,
                issue_count: 50,
                git_commit: None,
            },
        ],
    };

    let regression = falcon::stability::perf_track::check_regression(&history);
    assert!(regression.is_some());
    assert!(!regression.unwrap().is_regression);
}

// ============================================================
// Suppression Database Tests
// ============================================================

#[test]
fn test_suppression_add_and_load() {
    let dir = tempfile::tempdir().unwrap();

    falcon::stability::suppression::add_suppression(
        dir.path(),
        &falcon::stability::suppression::SuppressionRequest {
            rule: "avoid-dynamic",
            file: "lib/main.dart",
            line: Some(42),
            reason: "Legacy API requires dynamic",
            category: falcon::stability::suppression::SuppressionCategory::WontFix,
        },
    )
    .unwrap();

    let db = falcon::stability::suppression::load_suppressions(dir.path()).unwrap();
    assert_eq!(db.entries.len(), 1);
    assert_eq!(db.entries[0].rule, "avoid-dynamic");
    assert_eq!(db.entries[0].category, falcon::stability::suppression::SuppressionCategory::WontFix);
}

#[test]
fn test_suppression_stats() {
    let db = falcon::stability::suppression::SuppressionDatabase {
        entries: vec![
            falcon::stability::suppression::SuppressionEntry {
                rule: "avoid-dynamic".to_string(),
                file: "a.dart".to_string(),
                line: Some(1),
                reason: "test".to_string(),
                category: falcon::stability::suppression::SuppressionCategory::FalsePositive,
                created_at: "2026-01-01".to_string(),
                created_by: None,
            },
            falcon::stability::suppression::SuppressionEntry {
                rule: "avoid-dynamic".to_string(),
                file: "b.dart".to_string(),
                line: None,
                reason: "test".to_string(),
                category: falcon::stability::suppression::SuppressionCategory::WontFix,
                created_at: "2026-01-01".to_string(),
                created_by: None,
            },
            falcon::stability::suppression::SuppressionEntry {
                rule: "no-magic-numbers".to_string(),
                file: "c.dart".to_string(),
                line: None,
                reason: "test".to_string(),
                category: falcon::stability::suppression::SuppressionCategory::FalsePositive,
                created_at: "2026-01-01".to_string(),
                created_by: None,
            },
        ],
    };

    let stats = falcon::stability::suppression::suppression_stats(&db);
    assert_eq!(stats.total, 3);
    assert_eq!(stats.false_positives, 2);
    assert_eq!(stats.wont_fix, 1);
    assert!((stats.false_positive_rate - 66.67).abs() < 1.0);
    assert_eq!(stats.top_suppressed_rules[0].0, "avoid-dynamic");
}

#[test]
fn test_empty_suppression_db() {
    let dir = tempfile::tempdir().unwrap();
    let db = falcon::stability::suppression::load_suppressions(dir.path()).unwrap();
    assert!(db.entries.is_empty());

    let stats = falcon::stability::suppression::suppression_stats(&db);
    assert_eq!(stats.total, 0);
    assert_eq!(stats.false_positive_rate, 0.0);
}

// ============================================================
// Community Tests
// ============================================================

#[test]
fn test_submit_rule_request() {
    let dir = tempfile::tempdir().unwrap();

    let id = falcon::community::submit_rule_request(
        dir.path(),
        "prefer-early-return",
        "Prefer early return over deep nesting",
        "dart",
    )
    .unwrap();

    assert_eq!(id, "req-0001");

    let data = falcon::community::load_community(dir.path()).unwrap();
    assert_eq!(data.rule_requests.len(), 1);
    assert_eq!(data.rule_requests[0].name, "prefer-early-return");
    assert_eq!(data.rule_requests[0].votes, 1);
}

#[test]
fn test_vote_rule_request() {
    let dir = tempfile::tempdir().unwrap();

    falcon::community::submit_rule_request(
        dir.path(),
        "prefer-early-return",
        "desc",
        "dart",
    )
    .unwrap();

    let votes = falcon::community::vote_rule_request(dir.path(), "req-0001").unwrap();
    assert_eq!(votes, 2);

    let votes = falcon::community::vote_rule_request(dir.path(), "req-0001").unwrap();
    assert_eq!(votes, 3);
}

#[test]
fn test_vote_nonexistent_request() {
    let dir = tempfile::tempdir().unwrap();
    let result = falcon::community::vote_rule_request(dir.path(), "req-9999");
    assert!(result.is_err());
}

#[test]
fn test_sample_contributed_rules() {
    let rules = falcon::community::sample_contributed_rules();
    assert!(rules.len() >= 3);
    assert!(rules.iter().any(|r| r.name.contains("getx")));
}

#[test]
fn test_multiple_rule_requests() {
    let dir = tempfile::tempdir().unwrap();

    falcon::community::submit_rule_request(dir.path(), "rule-a", "desc-a", "dart").unwrap();
    falcon::community::submit_rule_request(dir.path(), "rule-b", "desc-b", "flutter").unwrap();
    falcon::community::submit_rule_request(dir.path(), "rule-c", "desc-c", "riverpod").unwrap();

    let data = falcon::community::load_community(dir.path()).unwrap();
    assert_eq!(data.rule_requests.len(), 3);
    assert_eq!(data.rule_requests[2].id, "req-0003");
}
