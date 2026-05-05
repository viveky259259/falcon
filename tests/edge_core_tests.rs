// ─── lib.rs: init_config ─────────────────────────────────────────────────────

#[test]
fn test_init_config_creates_file() {
    let tmp = tempfile::tempdir().unwrap();
    falcon::init_config(tmp.path()).unwrap();
    assert!(tmp.path().join("falcon.yaml").exists());
}

#[test]
fn test_init_config_fails_if_exists() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("falcon.yaml"), "existing: true\n").unwrap();
    let result = falcon::init_config(tmp.path());
    assert!(result.is_err());
}

// ─── lib.rs: Falcon::analyze_files ──────────────────────────────────────────

#[test]
fn test_analyze_files_empty_list() {
    let config = falcon::config::FalconConfig::default();
    let falcon_inst = falcon::Falcon::new(config).unwrap();
    let report = falcon_inst.analyze_files(&[]).unwrap();
    assert_eq!(report.file_count, 0);
    assert!(report.issues.is_empty());
}

#[test]
fn test_analyze_files_nonexistent_files() {
    let config = falcon::config::FalconConfig::default();
    let falcon_inst = falcon::Falcon::new(config).unwrap();
    let fake_files = vec![
        std::path::PathBuf::from("/nonexistent/path/to/file.dart"),
        std::path::PathBuf::from("/another/fake/file.dart"),
    ];
    let report = falcon_inst.analyze_files(&fake_files).unwrap();
    assert_eq!(report.file_count, 2);
    assert!(report.issues.is_empty());
}

// ─── lib.rs: Falcon::analyze ────────────────────────────────────────────────

#[test]
fn test_analyze_empty_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();
    let config = falcon::config::FalconConfig::load(tmp.path()).unwrap();
    let falcon_inst = falcon::Falcon::new(config).unwrap();
    let report = falcon_inst.analyze(tmp.path()).unwrap();
    assert_eq!(report.file_count, 0);
}

#[test]
fn test_analyze_malformed_dart_file() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("broken.dart"),
        "class {{ {{ invalid syntax !@#$%\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let config = falcon::config::FalconConfig::load(tmp.path()).unwrap();
    let falcon_inst = falcon::Falcon::new(config).unwrap();
    let report = falcon_inst.analyze(tmp.path()).unwrap();
    assert_eq!(report.file_count, 1);
}

// ─── Config: Load ───────────────────────────────────────────────────────────

#[test]
fn test_config_load_missing_file_uses_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let config = falcon::config::FalconConfig::load(tmp.path()).unwrap();
    assert!(!config.rules.is_empty());
}

#[test]
fn test_config_load_empty_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("falcon.yaml"), "").unwrap();
    let result = falcon::config::FalconConfig::load(tmp.path());
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_config_load_invalid_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("falcon.yaml"), "{{{{invalid yaml!!!!").unwrap();
    let result = falcon::config::FalconConfig::load(tmp.path());
    assert!(result.is_err());
}

#[test]
fn test_config_default_excludes() {
    let excludes = falcon::config::default_excludes();
    assert!(!excludes.is_empty());
    assert!(excludes.iter().any(|e| e.contains(".g.dart")));
}

// ─── Config: RuleConfig ─────────────────────────────────────────────────────

#[test]
fn test_rule_config_simple_name() {
    let rc = falcon::config::RuleConfig::Simple("avoid-dynamic".to_string());
    assert_eq!(rc.name(), "avoid-dynamic");
}

#[test]
fn test_rule_config_simple_severity() {
    let rc = falcon::config::RuleConfig::Simple("test-rule".to_string());
    assert_eq!(rc.severity(), falcon::config::Severity::Warning);
}

// ─── Parser: DartParser ─────────────────────────────────────────────────────

#[test]
fn test_parser_creation() {
    let parser = falcon::parser::DartParser::new();
    assert!(parser.is_ok());
}

#[test]
fn test_parser_empty_source() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse("");
    assert!(tree.is_some());
}

#[test]
fn test_parser_valid_dart() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse("class App { void main() {} }");
    assert!(tree.is_some());
}

#[test]
fn test_parser_unicode_source() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse("String emoji = '🚀🎯✅'; // Unicode test\n");
    assert!(tree.is_some());
}

#[test]
fn test_parser_very_long_source() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let long_source: String = (0..1000)
        .map(|i| format!("int x{} = {};\n", i, i))
        .collect();
    let tree = parser.parse(&long_source);
    assert!(tree.is_some());
}

// ─── Parser: Utilities ──────────────────────────────────────────────────────

#[test]
fn test_node_text_basic() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse("class Foo {}").unwrap();
    let text = falcon::parser::node_text(tree.root_node(), "class Foo {}");
    assert_eq!(text, "class Foo {}");
}

#[test]
fn test_find_children_by_kind_returns_empty_for_unknown() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse("class Foo {}").unwrap();
    let nodes = falcon::parser::find_children_by_kind(tree.root_node(), "nonexistent_xyz");
    assert!(nodes.is_empty());
}

#[test]
fn test_find_children_unknown_kind() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse("class Foo {}").unwrap();
    let nodes = falcon::parser::find_children_by_kind(tree.root_node(), "nonexistent_kind_xyz");
    assert!(nodes.is_empty());
}

#[test]
fn test_find_descendants_by_kind() {
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let source = "class Foo { void bar() { int x = 1; } }";
    let tree = parser.parse(source).unwrap();
    let decls = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_body");
    assert!(!decls.is_empty());
}

// ─── Metrics: Line Counting ────────────────────────────────────────────────

#[test]
fn test_count_lines_empty() {
    let (total, code) = falcon::metrics::lines::count_lines("");
    assert_eq!(total, 0);
    assert_eq!(code, 0);
}

#[test]
fn test_count_lines_only_comments() {
    let (total, code) =
        falcon::metrics::lines::count_lines("// comment 1\n// comment 2\n// comment 3\n");
    assert_eq!(total, 3);
    assert_eq!(code, 0);
}

#[test]
fn test_count_lines_only_blanks() {
    let (total, code) = falcon::metrics::lines::count_lines("\n\n\n\n\n");
    assert_eq!(total, 5);
    assert_eq!(code, 0);
}

#[test]
fn test_count_lines_mixed() {
    let source = "class Foo {\n  // comment\n  int x = 1;\n\n}\n";
    let (total, code) = falcon::metrics::lines::count_lines(source);
    assert_eq!(total, 5);
    assert!(code >= 2);
}

// ─── Score: score_from_report ───────────────────────────────────────────────

#[test]
fn test_score_from_empty_report() {
    let report = falcon::reporters::AnalysisReport {
        issues: vec![],
        metrics: vec![],
        file_count: 0,
        project_path: None,
    };
    let score = falcon::ai_score::score::score_from_report(&report).unwrap();
    assert_eq!(score.overall, 100);
    assert_eq!(score.total_issues, 0);
}

#[test]
fn test_score_from_report_with_errors() {
    let issues = vec![
        falcon::reporters::Issue {
            rule: "avoid-empty-catch".to_string(),
            message: "empty catch".to_string(),
            severity: falcon::config::Severity::Error,
            file: std::path::PathBuf::from("test.dart"),
            line: 1,
            column: 1,
        };
        50
    ];
    let report = falcon::reporters::AnalysisReport {
        issues,
        metrics: vec![],
        file_count: 10,
        project_path: None,
    };
    let score = falcon::ai_score::score::score_from_report(&report).unwrap();
    assert!(score.overall < 100);
    assert!(score.error_handling.score < 100);
}

// ─── AnalysisReport methods ─────────────────────────────────────────────────

#[test]
fn test_analysis_report_counts_empty() {
    let report = falcon::reporters::AnalysisReport {
        issues: vec![],
        metrics: vec![],
        file_count: 0,
        project_path: None,
    };
    assert_eq!(report.error_count(), 0);
    assert_eq!(report.warning_count(), 0);
    assert_eq!(report.info_count(), 0);
    assert!(!report.has_errors());
}

#[test]
fn test_analysis_report_counts_mixed() {
    let report = falcon::reporters::AnalysisReport {
        issues: vec![
            falcon::reporters::Issue {
                rule: "a".into(),
                message: "m".into(),
                severity: falcon::config::Severity::Error,
                file: "f.dart".into(),
                line: 1,
                column: 1,
            },
            falcon::reporters::Issue {
                rule: "b".into(),
                message: "m".into(),
                severity: falcon::config::Severity::Warning,
                file: "f.dart".into(),
                line: 2,
                column: 1,
            },
            falcon::reporters::Issue {
                rule: "c".into(),
                message: "m".into(),
                severity: falcon::config::Severity::Warning,
                file: "f.dart".into(),
                line: 3,
                column: 1,
            },
            falcon::reporters::Issue {
                rule: "d".into(),
                message: "m".into(),
                severity: falcon::config::Severity::Info,
                file: "f.dart".into(),
                line: 4,
                column: 1,
            },
        ],
        metrics: vec![],
        file_count: 1,
        project_path: None,
    };
    assert_eq!(report.error_count(), 1);
    assert_eq!(report.warning_count(), 2);
    assert_eq!(report.info_count(), 1);
    assert!(report.has_errors());
}
