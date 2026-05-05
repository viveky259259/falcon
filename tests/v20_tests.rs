// ─── MCP Tools ──────────────────────────────────────────────────────────────

#[test]
fn test_mcp_tool_list() {
    let tools = falcon::mcp::tools::list_tools();
    assert!(
        tools.len() >= 7,
        "Should have at least 7 MCP tools, got {}",
        tools.len()
    );

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"falcon_analyze"));
    assert!(names.contains(&"falcon_ai_score"));
    assert!(names.contains(&"falcon_check_file"));
    assert!(names.contains(&"falcon_explain_rule"));
    assert!(names.contains(&"falcon_fix"));
    assert!(names.contains(&"falcon_conventions"));
    assert!(names.contains(&"falcon_provenance"));
}

#[test]
fn test_mcp_tool_schemas_valid() {
    let tools = falcon::mcp::tools::list_tools();
    for tool in &tools {
        assert!(!tool.name.is_empty(), "Tool name should not be empty");
        assert!(
            !tool.description.is_empty(),
            "Tool description should not be empty"
        );
        assert!(
            tool.input_schema.is_object(),
            "Input schema should be an object"
        );
        assert!(
            tool.input_schema.get("properties").is_some(),
            "Input schema for {} should have properties",
            tool.name
        );
    }
}

#[test]
fn test_mcp_check_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.dart");
    std::fs::write(
        &file,
        "class MyWidget {\n  void build() {\n    try { doStuff(); } catch (e) {}\n  }\n}\n",
    )
    .unwrap();

    let args = serde_json::json!({ "file_path": file.to_string_lossy() });
    let result = falcon::mcp::tools::execute_tool("falcon_check_file", &args);
    assert!(result.is_ok(), "check_file should succeed");

    let val = result.unwrap();
    assert!(val.get("issue_count").is_some());
    assert!(val.get("issues").is_some());
}

#[test]
fn test_mcp_check_file_with_source() {
    let args = serde_json::json!({
        "file_path": "virtual.dart",
        "source": "class Hello {\n  String greet() => 'hello';\n}\n"
    });
    let result = falcon::mcp::tools::execute_tool("falcon_check_file", &args);
    assert!(result.is_ok());
}

#[test]
fn test_mcp_explain_rule() {
    let args = serde_json::json!({ "rule": "avoid-long-functions" });
    let result = falcon::mcp::tools::execute_tool("falcon_explain_rule", &args);
    assert!(
        result.is_ok(),
        "explain_rule should succeed for known rules"
    );

    let val = result.unwrap();
    assert!(val.get("rule").is_some());
    assert!(val.get("summary").is_some());
}

#[test]
fn test_mcp_explain_unknown_rule() {
    let args = serde_json::json!({ "rule": "nonexistent-rule-xyz" });
    let result = falcon::mcp::tools::execute_tool("falcon_explain_rule", &args);
    assert!(result.is_err());
}

#[test]
fn test_mcp_unknown_tool() {
    let args = serde_json::json!({});
    let result = falcon::mcp::tools::execute_tool("nonexistent_tool", &args);
    assert!(result.is_err());
}

#[test]
fn test_mcp_analyze_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "class App {}\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let args = serde_json::json!({ "path": tmp.path().to_string_lossy() });
    let result = falcon::mcp::tools::execute_tool("falcon_analyze", &args);
    assert!(result.is_ok());

    let val = result.unwrap();
    assert!(val.get("file_count").is_some());
    assert!(val.get("issue_count").is_some());
}

#[test]
fn test_mcp_conventions() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class AppWidget {}\n").unwrap();

    let args = serde_json::json!({ "path": tmp.path().to_string_lossy() });
    let result = falcon::mcp::tools::execute_tool("falcon_conventions", &args);
    assert!(result.is_ok());
}

// ─── Drift Detector ─────────────────────────────────────────────────────────

#[test]
fn test_drift_clean_arch_no_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(lib.join("domain/entities")).unwrap();
    std::fs::create_dir_all(lib.join("data/repositories")).unwrap();
    std::fs::create_dir_all(lib.join("presentation/pages")).unwrap();

    std::fs::write(lib.join("domain/entities/user.dart"), "class User {}\n").unwrap();
    std::fs::write(
        lib.join("data/repositories/repo.dart"),
        "class UserRepo {}\n",
    )
    .unwrap();

    let report = falcon::ai_score::drift::detect_drift(tmp.path(), None).unwrap();
    assert!(report.drift_score > 0.0);
    assert!(report.files_analyzed >= 2);
}

#[test]
fn test_drift_report_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();

    let report = falcon::ai_score::drift::detect_drift(tmp.path(), None).unwrap();
    assert!(report.drift_score >= 0.0 && report.drift_score <= 100.0);
    assert!(report.files_analyzed >= 1);
    assert!(!report.conventions.naming.file_naming.is_empty());
}

#[test]
fn test_drift_print_no_panic() {
    let report = falcon::ai_score::drift::DriftReport {
        conventions: falcon::ai_score::convention::ConventionReport {
            naming: falcon::ai_score::convention::NamingConventions {
                file_naming: "snake_case".to_string(),
                class_naming: "PascalCase".to_string(),
                private_prefix: true,
                uses_underscore_params: false,
            },
            architecture: falcon::ai_score::convention::ArchitectureConventions {
                pattern: "Clean Architecture".to_string(),
                layers_detected: vec!["domain".to_string(), "data".to_string()],
                has_separate_models: false,
                has_separate_services: false,
            },
            error_handling: falcon::ai_score::convention::ErrorHandlingConventions {
                uses_result_type: false,
                uses_either: false,
                uses_try_catch: true,
                uses_custom_exceptions: false,
            },
            state_management: Some("BLoC".to_string()),
            consistency_score: 80.0,
        },
        findings: vec![],
        drift_score: 100.0,
        files_analyzed: 10,
    };
    falcon::ai_score::drift::print_drift_report(&report);
}

// ─── Self-Tuning Rules ──────────────────────────────────────────────────────

#[test]
fn test_self_tune_load_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let history = falcon::ai_score::self_tune::load_tune_history(tmp.path()).unwrap();
    assert!(history.records.is_empty());
    assert_eq!(history.snapshots, 0);
}

#[test]
fn test_self_tune_save_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    let mut history = falcon::ai_score::self_tune::TuneHistory::default();
    history.snapshots = 3;
    history.records.insert(
        "avoid-dynamic".to_string(),
        falcon::ai_score::self_tune::RuleTuneRecord {
            rule: "avoid-dynamic".to_string(),
            trigger_count: 100,
            suppress_count: 80,
            fix_accepted: 5,
            fix_rejected: 2,
            signal_ratio: 0.2,
        },
    );

    falcon::ai_score::self_tune::save_tune_history(tmp.path(), &history).unwrap();
    let loaded = falcon::ai_score::self_tune::load_tune_history(tmp.path()).unwrap();
    assert_eq!(loaded.snapshots, 3);
    assert!(loaded.records.contains_key("avoid-dynamic"));
}

#[test]
fn test_self_tune_recommendations_disable() {
    let mut history = falcon::ai_score::self_tune::TuneHistory::default();
    history.records.insert(
        "noisy-rule".to_string(),
        falcon::ai_score::self_tune::RuleTuneRecord {
            rule: "noisy-rule".to_string(),
            trigger_count: 100,
            suppress_count: 80,
            fix_accepted: 0,
            fix_rejected: 0,
            signal_ratio: 0.2,
        },
    );

    let recs = falcon::ai_score::self_tune::generate_recommendations(&history);
    assert!(!recs.is_empty());
    assert_eq!(
        recs[0].action,
        falcon::ai_score::self_tune::TuneAction::Disable
    );
}

#[test]
fn test_self_tune_recommendations_upgrade() {
    let mut history = falcon::ai_score::self_tune::TuneHistory::default();
    history.records.insert(
        "high-value-rule".to_string(),
        falcon::ai_score::self_tune::RuleTuneRecord {
            rule: "high-value-rule".to_string(),
            trigger_count: 50,
            suppress_count: 1,
            fix_accepted: 40,
            fix_rejected: 0,
            signal_ratio: 0.98,
        },
    );

    let recs = falcon::ai_score::self_tune::generate_recommendations(&history);
    assert!(!recs.is_empty());
    assert_eq!(
        recs[0].action,
        falcon::ai_score::self_tune::TuneAction::Upgrade
    );
}

#[test]
fn test_self_tune_print_no_panic() {
    let history = falcon::ai_score::self_tune::TuneHistory::default();
    let recs = falcon::ai_score::self_tune::generate_recommendations(&history);
    falcon::ai_score::self_tune::print_tune_recommendations(&recs, &history);
}

// ─── Score Trends ───────────────────────────────────────────────────────────

#[test]
fn test_score_trends_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let history = falcon::ai_score::score_trends::load_score_history(tmp.path()).unwrap();
    assert!(history.snapshots.is_empty());
}

#[test]
fn test_score_trends_save_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    let mut history = falcon::ai_score::score_trends::ScoreHistory::default();
    history
        .snapshots
        .push(falcon::ai_score::score_trends::ScoreSnapshot {
            timestamp: "2026-03-18T10:00:00".to_string(),
            overall: 75,
            resource_safety: 80,
            error_handling: 60,
            type_safety: 90,
            security: 100,
            convention_match: 70,
            complexity: 65,
            file_count: 50,
            total_issues: 120,
            grade: "C".to_string(),
            git_commit: Some("abc1234".to_string()),
        });

    falcon::ai_score::score_trends::save_score_history(tmp.path(), &history).unwrap();
    let loaded = falcon::ai_score::score_trends::load_score_history(tmp.path()).unwrap();
    assert_eq!(loaded.snapshots.len(), 1);
    assert_eq!(loaded.snapshots[0].overall, 75);
}

#[test]
fn test_score_trends_compare() {
    let old = falcon::ai_score::score_trends::ScoreSnapshot {
        timestamp: "t1".to_string(),
        overall: 60,
        resource_safety: 50,
        error_handling: 40,
        type_safety: 80,
        security: 90,
        convention_match: 60,
        complexity: 50,
        file_count: 50,
        total_issues: 200,
        grade: "D".to_string(),
        git_commit: None,
    };
    let new = falcon::ai_score::score_trends::ScoreSnapshot {
        timestamp: "t2".to_string(),
        overall: 75,
        resource_safety: 70,
        error_handling: 60,
        type_safety: 85,
        security: 95,
        convention_match: 65,
        complexity: 70,
        file_count: 55,
        total_issues: 150,
        grade: "C".to_string(),
        git_commit: Some("def5678".to_string()),
    };

    let delta = falcon::ai_score::score_trends::compare_scores(&old, &new);
    assert_eq!(delta.overall, 15);
    assert_eq!(delta.error_handling, 20);
    assert_eq!(delta.issue_delta, -50);
}

#[test]
fn test_score_trends_print_no_panic() {
    let mut history = falcon::ai_score::score_trends::ScoreHistory::default();
    history
        .snapshots
        .push(falcon::ai_score::score_trends::ScoreSnapshot {
            timestamp: "2026-03-18T10:00:00".to_string(),
            overall: 75,
            resource_safety: 80,
            error_handling: 60,
            type_safety: 90,
            security: 100,
            convention_match: 70,
            complexity: 65,
            file_count: 50,
            total_issues: 120,
            grade: "C".to_string(),
            git_commit: Some("abc1234".to_string()),
        });
    falcon::ai_score::score_trends::print_score_history(&history, 10);
}
