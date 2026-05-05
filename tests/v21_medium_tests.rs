// ─── Webhook Callbacks ──────────────────────────────────────────────────────

#[test]
fn test_webhook_preview() {
    let data = serde_json::json!({ "score": 85, "grade": "B" });
    let result = falcon::ci::webhook::preview_webhook(
        falcon::ci::webhook::WebhookEvent::ScoreChanged,
        "test-project",
        data,
    );
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("score.changed"));
    assert!(json.contains("test-project"));
}

#[test]
fn test_webhook_events_display() {
    assert_eq!(
        falcon::ci::webhook::WebhookEvent::AnalysisComplete.to_string(),
        "analysis.complete"
    );
    assert_eq!(
        falcon::ci::webhook::WebhookEvent::ScoreChanged.to_string(),
        "score.changed"
    );
    assert_eq!(
        falcon::ci::webhook::WebhookEvent::DriftDetected.to_string(),
        "drift.detected"
    );
    assert_eq!(
        falcon::ci::webhook::WebhookEvent::ThresholdExceeded.to_string(),
        "threshold.exceeded"
    );
}

// ─── Benchmark Database ─────────────────────────────────────────────────────

#[test]
fn test_benchmark_db_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let db = falcon::ai_score::benchmark_db::load_benchmark_db(tmp.path()).unwrap();
    assert!(db.entries.is_empty());
}

#[test]
fn test_benchmark_db_save_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    let mut db = falcon::ai_score::benchmark_db::BenchmarkDatabase::default();
    db.entries
        .push(falcon::ai_score::benchmark_db::ProjectBenchmark {
            project_name: "test-app".to_string(),
            ai_tool: "cursor".to_string(),
            score: 72,
            grade: "C".to_string(),
            error_count: 5,
            warning_count: 30,
            total_issues: 35,
            file_count: 20,
            timestamp: "2026-03-18".to_string(),
        });
    db.entries
        .push(falcon::ai_score::benchmark_db::ProjectBenchmark {
            project_name: "other-app".to_string(),
            ai_tool: "copilot".to_string(),
            score: 65,
            grade: "D".to_string(),
            error_count: 10,
            warning_count: 50,
            total_issues: 60,
            file_count: 25,
            timestamp: "2026-03-18".to_string(),
        });

    falcon::ai_score::benchmark_db::save_benchmark_db(tmp.path(), &db).unwrap();
    let loaded = falcon::ai_score::benchmark_db::load_benchmark_db(tmp.path()).unwrap();
    assert_eq!(loaded.entries.len(), 2);
}

#[test]
fn test_benchmark_db_tool_stats() {
    let mut db = falcon::ai_score::benchmark_db::BenchmarkDatabase::default();
    for score in [70, 75, 80] {
        db.entries
            .push(falcon::ai_score::benchmark_db::ProjectBenchmark {
                project_name: format!("app-{}", score),
                ai_tool: "cursor".to_string(),
                score,
                grade: "C".to_string(),
                error_count: 0,
                warning_count: 10,
                total_issues: 10,
                file_count: 20,
                timestamp: "2026-03-18".to_string(),
            });
    }
    db.entries
        .push(falcon::ai_score::benchmark_db::ProjectBenchmark {
            project_name: "human-app".to_string(),
            ai_tool: "human".to_string(),
            score: 90,
            grade: "A".to_string(),
            error_count: 0,
            warning_count: 2,
            total_issues: 2,
            file_count: 30,
            timestamp: "2026-03-18".to_string(),
        });

    let stats = falcon::ai_score::benchmark_db::compute_tool_stats(&db);
    assert_eq!(stats.len(), 2);

    let human_stats = stats.iter().find(|s| s.tool == "human").unwrap();
    assert_eq!(human_stats.avg_score as u32, 90);
    assert_eq!(human_stats.projects_analyzed, 1);

    let cursor_stats = stats.iter().find(|s| s.tool == "cursor").unwrap();
    assert_eq!(cursor_stats.projects_analyzed, 3);
    assert_eq!(cursor_stats.min_score, 70);
    assert_eq!(cursor_stats.max_score, 80);
}

#[test]
fn test_benchmark_db_print_no_panic() {
    let stats = vec![falcon::ai_score::benchmark_db::ToolStats {
        tool: "cursor".to_string(),
        projects_analyzed: 5,
        avg_score: 72.0,
        min_score: 60,
        max_score: 85,
        avg_issues_per_file: 3.2,
        common_issues: vec![],
    }];
    falcon::ai_score::benchmark_db::print_benchmark_summary(&stats);
}

// ─── LSP AI Extensions ─────────────────────────────────────────────────────

#[test]
fn test_lsp_ai_commands() {
    let commands = falcon::lsp::ai_extensions::ai_commands();
    assert!(commands.contains(&"falcon.aiScore".to_string()));
    assert!(commands.contains(&"falcon.provenance".to_string()));
    assert!(commands.contains(&"falcon.explainRule".to_string()));
    assert!(commands.contains(&"falcon.drift".to_string()));
}
