// ─── Incremental: Baseline ──────────────────────────────────────────────────

#[test]
fn test_baseline_create_empty_issues() {
    let tmp = tempfile::tempdir().unwrap();
    let path = falcon::incremental::baseline::Baseline::create(&[], tmp.path()).unwrap();
    assert!(path.exists());
}

#[test]
fn test_baseline_create_with_issues() {
    let tmp = tempfile::tempdir().unwrap();
    let issues = vec![
        falcon::reporters::Issue {
            rule: "test".to_string(), message: "msg".to_string(),
            severity: falcon::config::Severity::Warning,
            file: std::path::PathBuf::from("lib/a.dart"), line: 10, column: 1,
        },
    ];
    falcon::incremental::baseline::Baseline::create(&issues, tmp.path()).unwrap();

    let baseline = falcon::incremental::baseline::Baseline::load(tmp.path()).unwrap();
    let new_issues = vec![
        falcon::reporters::Issue {
            rule: "test".to_string(), message: "msg".to_string(),
            severity: falcon::config::Severity::Warning,
            file: std::path::PathBuf::from("lib/a.dart"), line: 10, column: 1,
        },
        falcon::reporters::Issue {
            rule: "new-rule".to_string(), message: "new".to_string(),
            severity: falcon::config::Severity::Error,
            file: std::path::PathBuf::from("lib/b.dart"), line: 5, column: 1,
        },
    ];

    let filtered = baseline.filter_new_issues(new_issues, tmp.path());
    assert!(filtered.len() >= 1);
}

#[test]
fn test_baseline_load_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let result = falcon::incremental::baseline::Baseline::load(tmp.path());
    assert!(result.is_err());
}

// ─── Incremental: Cache ─────────────────────────────────────────────────────

#[test]
fn test_cache_load_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = falcon::incremental::cache::AnalysisCache::load(tmp.path());
    assert!(cache.entries.is_empty());
}

#[test]
fn test_cache_save_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cache = falcon::incremental::cache::AnalysisCache::load(tmp.path());
    cache.update_entry(&std::path::PathBuf::from("lib/main.dart"), 5, false);
    cache.save(tmp.path()).unwrap();

    let loaded = falcon::incremental::cache::AnalysisCache::load(tmp.path());
    assert!(!loaded.entries.is_empty());
}

#[test]
fn test_cache_changed_files_empty() {
    let cache = falcon::incremental::cache::AnalysisCache::load(std::path::Path::new("/nonexistent"));
    let changed = cache.changed_files(&[]);
    assert!(changed.is_empty());
}

// ─── Incremental: Dep Graph ─────────────────────────────────────────────────

#[test]
fn test_dep_graph_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let graph = falcon::incremental::dep_graph::DependencyGraph::build(tmp.path(), &[]);
    assert!(graph.imports.is_empty());
}

#[test]
fn test_dep_graph_simple_imports() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("a.dart"), "import 'b.dart';\nclass A {}\n").unwrap();
    std::fs::write(lib.join("b.dart"), "class B {}\n").unwrap();

    let graph = falcon::incremental::dep_graph::DependencyGraph::build(tmp.path(), &[]);
    assert!(!graph.imports.is_empty());
}

#[test]
fn test_dep_graph_affected_files() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("a.dart"), "import 'b.dart';\n").unwrap();
    std::fs::write(lib.join("b.dart"), "class B {}\n").unwrap();

    let graph = falcon::incremental::dep_graph::DependencyGraph::build(tmp.path(), &[]);
    let changed = vec![lib.join("b.dart")];
    let affected = graph.affected_files(&changed);
    assert!(affected.len() >= 1);
}

// ─── Suppression: Persistence ───────────────────────────────────────────────

#[test]
fn test_suppression_load_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let db = falcon::stability::suppression::load_suppressions(tmp.path()).unwrap();
    assert!(db.entries.is_empty());
}

#[test]
fn test_suppression_save_load_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::stability::suppression::add_suppression(
        tmp.path(),
        &falcon::stability::suppression::SuppressionRequest {
            rule: "test-rule",
            file: "lib/test.dart",
            line: Some(42),
            reason: "testing",
            category: falcon::stability::suppression::SuppressionCategory::FalsePositive,
        },
    ).unwrap();

    let db = falcon::stability::suppression::load_suppressions(tmp.path()).unwrap();
    assert_eq!(db.entries.len(), 1);
    assert_eq!(db.entries[0].rule, "test-rule");
    assert_eq!(db.entries[0].line, Some(42));
}

// ─── Score Trends: Persistence ──────────────────────────────────────────────

#[test]
fn test_score_trends_load_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let history = falcon::ai_score::score_trends::load_score_history(tmp.path()).unwrap();
    assert!(history.snapshots.is_empty());
}

#[test]
fn test_score_trends_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let mut history = falcon::ai_score::score_trends::ScoreHistory::default();
    history.snapshots.push(falcon::ai_score::score_trends::ScoreSnapshot {
        timestamp: "2026-01-01".to_string(), overall: 80,
        resource_safety: 90, error_handling: 70, type_safety: 85,
        security: 100, convention_match: 75, complexity: 60,
        file_count: 50, total_issues: 100, grade: "B".to_string(),
        git_commit: None,
    });

    falcon::ai_score::score_trends::save_score_history(tmp.path(), &history).unwrap();
    let loaded = falcon::ai_score::score_trends::load_score_history(tmp.path()).unwrap();
    assert_eq!(loaded.snapshots.len(), 1);
    assert_eq!(loaded.snapshots[0].overall, 80);
}

// ─── Self-Tune: Persistence ─────────────────────────────────────────────────

#[test]
fn test_tune_history_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let mut history = falcon::ai_score::self_tune::TuneHistory::default();
    history.snapshots = 5;

    falcon::ai_score::self_tune::save_tune_history(tmp.path(), &history).unwrap();
    let loaded = falcon::ai_score::self_tune::load_tune_history(tmp.path()).unwrap();
    assert_eq!(loaded.snapshots, 5);
}

// ─── Community: Persistence ─────────────────────────────────────────────────

#[test]
fn test_community_load_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let data = falcon::community::load_community(tmp.path()).unwrap();
    assert!(data.rule_requests.is_empty());
}

#[test]
fn test_community_submit_and_vote() {
    let tmp = tempfile::tempdir().unwrap();
    let id = falcon::community::submit_rule_request(
        tmp.path(), "new-rule", "A great new rule", "dart"
    ).unwrap();
    assert!(!id.is_empty());

    let votes = falcon::community::vote_rule_request(tmp.path(), &id).unwrap();
    assert!(votes >= 2);
}

// ─── Benchmark DB: Persistence ──────────────────────────────────────────────

#[test]
fn test_benchmark_db_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let mut db = falcon::ai_score::benchmark_db::BenchmarkDatabase::default();
    db.entries.push(falcon::ai_score::benchmark_db::ProjectBenchmark {
        project_name: "test".to_string(), ai_tool: "cursor".to_string(),
        score: 72, grade: "C".to_string(), error_count: 3, warning_count: 20,
        total_issues: 23, file_count: 10, timestamp: "2026-01-01".to_string(),
    });

    falcon::ai_score::benchmark_db::save_benchmark_db(tmp.path(), &db).unwrap();
    let loaded = falcon::ai_score::benchmark_db::load_benchmark_db(tmp.path()).unwrap();
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].score, 72);
}

// ─── Fix Tracking: Edge cases ───────────────────────────────────────────────

#[test]
fn test_fix_tracking_effectiveness_empty() {
    let history = falcon::ai_score::fix_tracking::FixHistory { records: vec![] };
    let eff = falcon::ai_score::fix_tracking::compute_effectiveness(&history);
    assert!(eff.is_empty());
}

#[test]
fn test_fix_tracking_all_accepted() {
    let history = falcon::ai_score::fix_tracking::FixHistory {
        records: vec![
            falcon::ai_score::fix_tracking::FixRecord {
                rule: "rule-a".to_string(), file: "a.dart".to_string(),
                timestamp: "t".to_string(), outcome: falcon::ai_score::fix_tracking::FixOutcome::Accepted,
            },
            falcon::ai_score::fix_tracking::FixRecord {
                rule: "rule-a".to_string(), file: "b.dart".to_string(),
                timestamp: "t".to_string(), outcome: falcon::ai_score::fix_tracking::FixOutcome::Accepted,
            },
        ],
    };
    let eff = falcon::ai_score::fix_tracking::compute_effectiveness(&history);
    assert_eq!(eff[0].acceptance_rate, 100.0);
}
