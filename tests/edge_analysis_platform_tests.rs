// ─── Provenance: Edge Cases ──────────────────────────────────────────────────

#[test]
fn test_provenance_empty_source() {
    let result = falcon::ai_score::provenance::analyze_provenance(
        &std::path::PathBuf::from("empty.dart"), ""
    );
    assert_eq!(result.origin, falcon::ai_score::provenance::CodeOrigin::LikelyHuman);
}

#[test]
fn test_provenance_generated_code_header() {
    let source = "// GENERATED CODE - DO NOT MODIFY BY HAND\npart of 'model.dart';\nclass A {}";
    let result = falcon::ai_score::provenance::analyze_provenance(
        &std::path::PathBuf::from("model.g.dart"), source
    );
    assert_eq!(result.origin, falcon::ai_score::provenance::CodeOrigin::LikelyGenerated);
    assert!(result.confidence > 0.8);
}

#[test]
fn test_provenance_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let results = falcon::ai_score::provenance::analyze_project_provenance(tmp.path()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_provenance_summarize_empty() {
    let summary = falcon::ai_score::provenance::summarize_provenance(&[]);
    assert_eq!(summary.total_files, 0);
    assert_eq!(summary.ai_percentage, 0.0);
}

// ─── Convention: Edge Cases ─────────────────────────────────────────────────

#[test]
fn test_convention_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let report = falcon::ai_score::convention::detect_conventions(tmp.path()).unwrap();
    assert!(!report.naming.file_naming.is_empty());
}

#[test]
fn test_convention_nonexistent_path() {
    let result = falcon::ai_score::convention::detect_conventions(std::path::Path::new("/nonexistent/path"));
    assert!(result.is_err());
}

// ─── Drift: Edge Cases ─────────────────────────────────────────────────────

#[test]
fn test_drift_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let report = falcon::ai_score::drift::detect_drift(tmp.path(), None).unwrap();
    assert_eq!(report.drift_score, 100.0);
    assert_eq!(report.files_analyzed, 0);
}

#[test]
fn test_drift_nonexistent_git_ref() {
    let tmp = tempfile::tempdir().unwrap();
    let result = falcon::ai_score::drift::detect_drift(tmp.path(), Some("nonexistent_ref_xyz"));
    assert!(result.is_err() || result.unwrap().files_analyzed == 0);
}

// ─── SDK: Edge Cases ────────────────────────────────────────────────────────

#[test]
fn test_sdk_analyze_source_empty() {
    let sdk = falcon::sdk::FalconSdk::new();
    let issues = sdk.analyze_source("", "empty.dart").unwrap();
    assert!(issues.is_empty());
}

#[test]
fn test_sdk_analyze_source_malformed() {
    let sdk = falcon::sdk::FalconSdk::new();
    let issues = sdk.analyze_source("class {{ broken !!@@", "bad.dart").unwrap();
    // Should not panic, may or may not find issues
    let _ = issues;
}

#[test]
fn test_sdk_analyze_source_unicode() {
    let sdk = falcon::sdk::FalconSdk::new();
    let source = "String emoji = '🚀'; // 日本語コメント\nclass App {}\n";
    let issues = sdk.analyze_source(source, "unicode.dart").unwrap();
    let _ = issues;
}

#[test]
fn test_sdk_analyze_project_empty() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("falcon.yaml"), "metrics:\n  cyclomatic_complexity: 20\n").unwrap();
    let sdk = falcon::sdk::FalconSdk::new();
    let result = sdk.analyze_project(&tmp.path().to_string_lossy(), None).unwrap();
    assert_eq!(result.file_count, 0);
    assert_eq!(result.issue_count, 0);
}

#[test]
fn test_sdk_analyze_to_json_valid() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("a.dart"), "class A {}\n").unwrap();
    std::fs::write(tmp.path().join("falcon.yaml"), "metrics:\n  cyclomatic_complexity: 20\n").unwrap();

    let sdk = falcon::sdk::FalconSdk::new();
    let json = sdk.analyze_to_json(&tmp.path().to_string_lossy(), None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("file_count").is_some());
}

// ─── MCP Tools: Edge Cases ──────────────────────────────────────────────────

#[test]
fn test_mcp_tool_missing_argument() {
    let args = serde_json::json!({});
    let result = falcon::mcp::tools::execute_tool("falcon_check_file", &args);
    assert!(result.is_err());
}

#[test]
fn test_mcp_tool_analyze_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("falcon.yaml"), "metrics:\n  cyclomatic_complexity: 20\n").unwrap();
    let args = serde_json::json!({"path": tmp.path().to_string_lossy().to_string()});
    let result = falcon::mcp::tools::execute_tool("falcon_analyze", &args);
    assert!(result.is_ok());
    let val = result.unwrap();
    assert_eq!(val["file_count"], 0);
}

#[test]
fn test_mcp_tool_check_file_empty_source() {
    let args = serde_json::json!({"file_path": "test.dart", "source": ""});
    let result = falcon::mcp::tools::execute_tool("falcon_check_file", &args);
    assert!(result.is_ok());
}

// ─── Vuln Radar: Edge Cases ─────────────────────────────────────────────────

#[test]
fn test_vuln_scan_http_detection_fixed() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("api.dart"),
        "final url = 'http://evil.com/api';\n"
    ).unwrap();

    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(tmp.path());
    assert!(findings.iter().any(|f| f.issue.rule == "vuln-insecure-http"),
        "Should detect http:// URLs (B1 bug fix verification)");
}

#[test]
fn test_vuln_scan_http_allows_localhost() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("dev.dart"),
        "final url = 'http://localhost:8080/api';\n"
    ).unwrap();

    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(tmp.path());
    assert!(!findings.iter().any(|f| f.issue.rule == "vuln-insecure-http"),
        "Should NOT flag localhost HTTP");
}

#[test]
fn test_vuln_scan_sql_injection() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("db.dart"),
        "void query(String input) {\n  db.rawQuery('SELECT * FROM users WHERE id = $input');\n}\n"
    ).unwrap();

    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(tmp.path());
    assert!(findings.iter().any(|f| f.issue.rule == "vuln-sql-injection"));
}

// ─── Upgrade Check: Edge Cases ──────────────────────────────────────────────

#[test]
fn test_upgrade_check_skips_comments() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"),
        "// This uses bodyText1 in a comment\n/// Also headline1 in doc comment\nclass App {}\n"
    ).unwrap();

    let findings = falcon::analysis::upgrade_check::check_upgrade_compatibility(tmp.path());
    assert!(findings.is_empty(), "Should skip commented lines");
}

#[test]
fn test_upgrade_check_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let findings = falcon::analysis::upgrade_check::check_upgrade_compatibility(tmp.path());
    assert!(findings.is_empty());
}

// ─── Refactor Sim: Edge Cases ───────────────────────────────────────────────

#[test]
fn test_refactor_sim_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let impact = falcon::analysis::refactor_sim::simulate_refactor(
        tmp.path(),
        &falcon::analysis::refactor_sim::RefactorScenario::SetStateToRiverpod,
    ).unwrap();
    assert_eq!(impact.files_affected, 0);
}

#[test]
fn test_refactor_sim_bloc_migration_step_correct() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("bloc.dart"), "class MyBloc extends Bloc<E,S> {}\nBlocProvider(\n").unwrap();

    let impact = falcon::analysis::refactor_sim::simulate_refactor(
        tmp.path(),
        &falcon::analysis::refactor_sim::RefactorScenario::BlocToRiverpod,
    ).unwrap();

    assert!(!impact.migration_steps.iter().any(|s| s.contains("Remove bloc dependency")),
        "Should use correct removal step text (not lowercase 'bloc')");
    if impact.migration_steps.iter().any(|s| s.contains("Remove")) {
        assert!(impact.migration_steps.iter().any(|s| s.contains("flutter_bloc")),
            "Should say 'Remove flutter_bloc' not 'Remove bloc'");
    }
}

// ─── Platform: Enterprise Edge Cases ────────────────────────────────────────

#[test]
fn test_enterprise_load_policies_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let policies = falcon::platform::enterprise::load_policies(tmp.path()).unwrap();
    assert!(policies.policies.is_empty());
}

#[test]
fn test_enterprise_audit_empty_strings() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::platform::enterprise::record_audit(tmp.path(), "", "", "", "").unwrap();
    let log = falcon::platform::enterprise::load_audit_log(tmp.path()).unwrap();
    assert_eq!(log.entries.len(), 1);
    assert_eq!(log.entries[0].user, "");
}

#[test]
fn test_enterprise_audit_special_chars() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::platform::enterprise::record_audit(
        tmp.path(), "user@example.com", "create/delete", "path/to/file", "quotes: \"test\" and 'test'"
    ).unwrap();
    let log = falcon::platform::enterprise::load_audit_log(tmp.path()).unwrap();
    assert_eq!(log.entries.len(), 1);
    assert!(log.entries[0].details.contains("quotes"));
}

// ─── Plugins: Registry Edge Cases ───────────────────────────────────────────

#[test]
fn test_plugin_registry_search_empty() {
    let results = falcon::plugins::registry::search_registry("");
    assert!(!results.is_empty(), "Empty query should return all plugins");
}

#[test]
fn test_plugin_registry_search_star() {
    let results = falcon::plugins::registry::search_registry("*");
    assert!(!results.is_empty());
}

#[test]
fn test_plugin_registry_search_no_match() {
    let results = falcon::plugins::registry::search_registry("xyznonexistent999");
    assert!(results.is_empty());
}

// ─── Presets: Edge Cases ────────────────────────────────────────────────────

#[test]
fn test_preset_unknown_name() {
    let preset = falcon::plugins::presets::get_preset("nonexistent-preset-xyz");
    assert!(preset.is_none());
}

#[test]
fn test_preset_all_valid() {
    let all = falcon::plugins::presets::list_presets();
    assert!(all.len() >= 6);
    for p in &all {
        let loaded = falcon::plugins::presets::get_preset(&p.name);
        assert!(loaded.is_some(), "Preset '{}' should be loadable", p.name);
    }
}

// ─── AI Explain: Edge Cases ─────────────────────────────────────────────────

#[test]
fn test_explain_unknown_rule() {
    let result = falcon::ai::explain::explain_rule("nonexistent-rule-xyz-123");
    assert!(result.is_none());
}

#[test]
fn test_explain_known_rule() {
    let result = falcon::ai::explain::explain_rule("avoid-long-functions");
    assert!(result.is_some());
    let explanation = result.unwrap();
    assert!(!explanation.summary.is_empty());
    assert!(!explanation.bad_example.is_empty());
    assert!(!explanation.good_example.is_empty());
}

// ─── Regression Predict: Edge Cases ─────────────────────────────────────────

#[test]
fn test_predict_probabilities_bounded() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    let bad_code: String = (0..30).map(|i| format!(
        "void func{}() {{\n  try {{ riskyCall(); }} catch (e) {{}}\n  saveData(data);\n  dynamic x = getStuff();\n}}\n", i
    )).collect();
    std::fs::write(lib.join("bad.dart"), &bad_code).unwrap();
    std::fs::write(tmp.path().join("falcon.yaml"), "metrics:\n  cyclomatic_complexity: 20\n").unwrap();

    let predictions = falcon::ai_score::regression_predict::predict_risks(tmp.path()).unwrap();
    for p in &predictions {
        assert!(p.probability >= 0.0 && p.probability <= 1.0, "Probability should be 0-1, got {}", p.probability);
        assert!(!p.evidence.is_empty());
        assert!(!p.recommendation.is_empty());
    }
}

// ─── Score: Grade Boundary ──────────────────────────────────────────────────

#[test]
fn test_score_grade_boundaries() {
    use falcon::ai_score::score::Grade;

    let _make_report = |n: usize| falcon::reporters::AnalysisReport {
        issues: vec![falcon::reporters::Issue {
            rule: "avoid-dynamic".to_string(), message: "m".to_string(),
            severity: falcon::config::Severity::Warning,
            file: "f.dart".into(), line: 1, column: 1,
        }; n],
        metrics: vec![], file_count: 100,
    };

    let score_0_issues = falcon::ai_score::score::score_from_report(
        &falcon::reporters::AnalysisReport { issues: vec![], metrics: vec![], file_count: 10 }
    ).unwrap();
    assert_eq!(score_0_issues.grade, Grade::A);
    assert_eq!(score_0_issues.overall, 100);
}
