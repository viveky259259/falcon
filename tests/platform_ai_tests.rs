// ─── Cross-Project Learning ─────────────────────────────────────────────────

#[test]
fn test_cross_project_empty_db() {
    let tmp = tempfile::tempdir().unwrap();
    let db = falcon::ai_score::cross_project::load_learning_db(tmp.path()).unwrap();
    assert!(db.profiles.is_empty());
}

#[test]
fn test_cross_project_record_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let lib = project.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();
    std::fs::write(
        project.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let profile =
        falcon::ai_score::cross_project::record_project(tmp.path(), project.path()).unwrap();
    assert!(!profile.project_id.is_empty());
    assert!(profile.file_count >= 1);

    let db = falcon::ai_score::cross_project::load_learning_db(tmp.path()).unwrap();
    assert_eq!(db.profiles.len(), 1);
}

#[test]
fn test_cross_project_deduplication() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let lib = project.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();
    std::fs::write(
        project.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    falcon::ai_score::cross_project::record_project(tmp.path(), project.path()).unwrap();
    falcon::ai_score::cross_project::record_project(tmp.path(), project.path()).unwrap();

    let db = falcon::ai_score::cross_project::load_learning_db(tmp.path()).unwrap();
    assert_eq!(
        db.profiles.len(),
        1,
        "Duplicate recordings should replace, not append"
    );
}

#[test]
fn test_cross_project_insights() {
    let db = falcon::ai_score::cross_project::LearningDatabase {
        profiles: vec![falcon::ai_score::cross_project::ProjectProfile {
            project_id: "p1".to_string(),
            timestamp: "2026-01-01".to_string(),
            file_count: 50,
            architecture: "Clean Architecture".to_string(),
            state_management: Some("BLoC".to_string()),
            naming_convention: "snake_case".to_string(),
            error_handling_style: "Result".to_string(),
            ai_score: Some(72),
            rule_violations: [("avoid-dynamic".to_string(), 10)].into_iter().collect(),
            top_patterns: vec!["avoid-dynamic".to_string()],
        }],
        convention_frequencies: [("snake_case".to_string(), 1)].into_iter().collect(),
        rule_violation_totals: [("avoid-dynamic".to_string(), 10)].into_iter().collect(),
        architecture_distribution: [("Clean Architecture".to_string(), 1)]
            .into_iter()
            .collect(),
        state_mgmt_distribution: [("BLoC".to_string(), 1)].into_iter().collect(),
    };

    let insights = falcon::ai_score::cross_project::derive_insights(&db);
    assert_eq!(insights.total_projects, 1);
    assert_eq!(insights.most_common_architecture, "Clean Architecture");
    assert_eq!(insights.avg_score as u32, 72);
}

// ─── Regression Prediction ──────────────────────────────────────────────────

#[test]
fn test_predict_clean_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("app.dart"),
        "class App {\n  String greet() => 'hi';\n}\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let predictions = falcon::ai_score::regression_predict::predict_risks(tmp.path()).unwrap();
    assert!(predictions.is_empty() || predictions.iter().all(|p| p.probability < 0.5));
}

#[test]
fn test_predict_no_division_by_zero() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("lib")).unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let result = falcon::ai_score::regression_predict::predict_risks(tmp.path());
    assert!(result.is_ok());
}

#[test]
fn test_predict_print_no_panic() {
    let predictions = vec![falcon::ai_score::regression_predict::RiskPrediction {
        category: falcon::ai_score::regression_predict::RiskCategory::MemoryLeak,
        probability: 0.5,
        impact: "test impact".to_string(),
        evidence: vec!["evidence".to_string()],
        recommendation: "fix it".to_string(),
        timeframe: "1 week".to_string(),
    }];
    falcon::ai_score::regression_predict::print_risk_predictions(&predictions);
}

// ─── AI Tool Profiling ──────────────────────────────────────────────────────

#[test]
fn test_ai_profiling_empty_db() {
    let db = falcon::ai_score::benchmark_db::BenchmarkDatabase::default();
    let profiles = falcon::ai_score::ai_profiling::build_tool_profiles(&db);
    assert!(profiles.is_empty());
}

#[test]
fn test_ai_profiling_with_data() {
    let mut db = falcon::ai_score::benchmark_db::BenchmarkDatabase::default();
    db.entries
        .push(falcon::ai_score::benchmark_db::ProjectBenchmark {
            project_name: "app1".to_string(),
            ai_tool: "cursor".to_string(),
            score: 65,
            grade: "D".to_string(),
            error_count: 5,
            warning_count: 30,
            total_issues: 35,
            file_count: 20,
            timestamp: "2026-01-01".to_string(),
        });

    let profiles = falcon::ai_score::ai_profiling::build_tool_profiles(&db);
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].tool, "cursor");
    assert_eq!(profiles[0].projects, 1);
}

// ─── Auto Rule Discovery ───────────────────────────────────────────────────

#[test]
fn test_discover_rules_clean() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("app.dart"),
        "class App {\n  String greet() => 'hi';\n}\n",
    )
    .unwrap();

    let rules = falcon::ai_score::auto_rules::discover_patterns(tmp.path());
    assert!(rules.is_empty() || rules.iter().all(|r| r.occurrences < 3));
}

#[test]
fn test_discover_rules_finds_bang_operator() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    let code: String = (0..10)
        .map(|i| format!("final x{} = obj!.value;\n", i))
        .collect();
    std::fs::write(lib.join("bangs.dart"), &code).unwrap();

    let rules = falcon::ai_score::auto_rules::discover_patterns(tmp.path());
    let bang = rules.iter().find(|r| r.name == "avoid-bang-operator");
    assert!(bang.is_some(), "Should detect bang operator pattern");
}

// ─── Fix Tracking ───────────────────────────────────────────────────────────

#[test]
fn test_fix_tracking_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let history = falcon::ai_score::fix_tracking::load_fix_history(tmp.path()).unwrap();
    assert!(history.records.is_empty());
}

#[test]
fn test_fix_tracking_record_and_load() {
    let tmp = tempfile::tempdir().unwrap();

    falcon::ai_score::fix_tracking::record_fix(
        tmp.path(),
        "avoid-dynamic",
        "lib/main.dart",
        falcon::ai_score::fix_tracking::FixOutcome::Accepted,
    )
    .unwrap();

    falcon::ai_score::fix_tracking::record_fix(
        tmp.path(),
        "avoid-dynamic",
        "lib/app.dart",
        falcon::ai_score::fix_tracking::FixOutcome::Rejected,
    )
    .unwrap();

    let history = falcon::ai_score::fix_tracking::load_fix_history(tmp.path()).unwrap();
    assert_eq!(history.records.len(), 2);

    let eff = falcon::ai_score::fix_tracking::compute_effectiveness(&history);
    assert_eq!(eff.len(), 1);
    assert_eq!(eff[0].accepted, 1);
    assert_eq!(eff[0].rejected, 1);
    assert!((eff[0].acceptance_rate - 50.0).abs() < 0.1);
}

// ─── Platform: Cloud ────────────────────────────────────────────────────────

#[test]
fn test_cloud_init_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::platform::cloud::init_cloud(tmp.path(), "Test Team").unwrap();

    let config = falcon::platform::cloud::load_cloud_config(tmp.path()).unwrap();
    assert_eq!(config.team_name, "Test Team");
    assert!(!config.alerts.is_empty());
}

#[test]
fn test_cloud_register_project() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::platform::cloud::init_cloud(tmp.path(), "Team").unwrap();
    falcon::platform::cloud::register_project(tmp.path(), "my-app", "/path/to/app").unwrap();

    let config = falcon::platform::cloud::load_cloud_config(tmp.path()).unwrap();
    assert_eq!(config.projects.len(), 1);
    assert_eq!(config.projects[0].name, "my-app");
}

#[test]
fn test_cloud_duplicate_project_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::platform::cloud::init_cloud(tmp.path(), "Team").unwrap();
    falcon::platform::cloud::register_project(tmp.path(), "app", "/path").unwrap();

    let result = falcon::platform::cloud::register_project(tmp.path(), "app", "/other");
    assert!(result.is_err());
}

// ─── Platform: Enterprise ───────────────────────────────────────────────────

#[test]
fn test_enterprise_default_policies() {
    let policies = falcon::platform::enterprise::default_policies();
    assert!(policies.policies.len() >= 4);
    assert!(policies
        .policies
        .iter()
        .any(|p| p.name == "production-readiness"));
}

#[test]
fn test_enterprise_audit_log() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::platform::enterprise::record_audit(
        tmp.path(),
        "admin",
        "init",
        "policies",
        "created defaults",
    )
    .unwrap();

    let log = falcon::platform::enterprise::load_audit_log(tmp.path()).unwrap();
    assert_eq!(log.entries.len(), 1);
    assert_eq!(log.entries[0].user, "admin");
    assert_eq!(log.entries[0].action, "init");
}

// ─── Platform: Marketplace ──────────────────────────────────────────────────

#[test]
fn test_marketplace_browse_all() {
    let listings = falcon::platform::marketplace::browse_marketplace(None);
    assert!(listings.len() >= 5);
}

#[test]
fn test_marketplace_search() {
    let listings = falcon::platform::marketplace::browse_marketplace(Some("security"));
    assert!(!listings.is_empty());
    assert!(listings
        .iter()
        .any(|l| l.name.contains("security") || l.description.to_lowercase().contains("security")));
}

#[test]
fn test_marketplace_search_no_results() {
    let listings = falcon::platform::marketplace::browse_marketplace(Some("xyznonexistent999"));
    assert!(listings.is_empty());
}

// ─── Platform: Certification ────────────────────────────────────────────────

#[test]
fn test_certification_evaluation() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let result = falcon::platform::certification::evaluate_certification(tmp.path()).unwrap();
    assert!(!result.criteria.is_empty());
    assert!(!result.badge_markdown.is_empty());
    assert!(result.badge_markdown.contains("Falcon"));
}

// ─── Platform: Partner ──────────────────────────────────────────────────────

#[test]
fn test_partner_list() {
    let partners = falcon::platform::partner::list_partners();
    assert!(partners.len() >= 4);
    assert!(partners.iter().any(|p| p.name == "Cursor"));
    assert!(partners
        .iter()
        .any(|p| p.status == falcon::platform::partner::PartnerStatus::Certified));
}
