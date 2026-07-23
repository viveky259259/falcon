use std::path::PathBuf;

// ============================================================
// New AI-Critical Rule Tests (v1.1)
// ============================================================

#[test]
fn test_avoid_unawaited_futures() {
    let source = r#"
void main() async {
  fetchData();
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/main.dart"));

    let unawaited: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-unawaited-futures")
        .collect();
    assert!(!unawaited.is_empty(), "Should detect unawaited future call");
}

#[test]
fn test_awaited_futures_ok() {
    let source = r#"
void main() async {
  await fetchData();
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/main.dart"));

    let unawaited: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-unawaited-futures")
        .collect();
    assert!(
        unawaited.is_empty(),
        "Awaited futures should not trigger rule"
    );
}

#[test]
fn test_prefer_specific_catch_type() {
    let source = r#"
void main() {
  try {
    doSomething();
  } catch (e) {
    handleError(e);
  }
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/main.dart"));

    let catch_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "prefer-specific-catch-type")
        .collect();
    assert!(!catch_issues.is_empty(), "Should detect generic catch (e)");
}

#[test]
fn test_ensure_stream_subscription_cancel() {
    let source = r#"
class _MyState extends State<MyWidget> {
  late StreamSubscription subscription;

  @override
  void initState() {
    subscription = stream.listen((data) {});
  }

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/widget.dart"));

    let sub_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "ensure-stream-subscription-cancel")
        .collect();
    assert!(
        !sub_issues.is_empty(),
        "Should detect uncancelled StreamSubscription"
    );
}

#[test]
fn test_stream_subscription_with_cancel_ok() {
    let source = r#"
class _MyState extends State<MyWidget> {
  late StreamSubscription subscription;

  @override
  void initState() {
    subscription = stream.listen((data) {});
  }

  @override
  void dispose() {
    subscription.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/widget.dart"));

    let sub_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "ensure-stream-subscription-cancel")
        .collect();
    assert!(
        sub_issues.is_empty(),
        "Should not flag when cancel() is called in dispose()"
    );
}

#[test]
fn test_avoid_excessive_widget_nesting() {
    let mut deep_widget = "return ".to_string();
    for _ in 0..15 {
        deep_widget.push_str("Container(child: ");
    }
    deep_widget.push_str("Text('deep')");
    for _ in 0..15 {
        deep_widget.push(')');
    }
    deep_widget.push(';');

    let source = format!(
        r#"
class MyWidget extends StatelessWidget {{
  @override
  Widget build(BuildContext context) {{
    {}
  }}
}}"#,
        deep_widget
    );

    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(&source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), &source, &PathBuf::from("lib/widget.dart"));

    let nesting_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-excessive-widget-nesting")
        .collect();
    assert!(
        !nesting_issues.is_empty(),
        "Should detect excessive widget nesting"
    );
}

// ============================================================
// Preset Tests
// ============================================================

#[test]
fn test_ai_generated_preset_exists() {
    let preset = falcon::plugins::presets::get_preset("ai-generated");
    assert!(preset.is_some(), "ai-generated preset should exist");
    let p = preset.unwrap();
    assert!(
        p.rules.len() >= 15,
        "ai-generated preset should have 15+ rules, got {}",
        p.rules.len()
    );
    assert!(p.description.contains("AI"));
}

#[test]
fn test_presets_now_include_ai_generated() {
    let presets = falcon::plugins::presets::list_presets();
    let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"ai-generated"));
    assert_eq!(presets.len(), 7);
}

// ============================================================
// Rule Count
// ============================================================

#[test]
fn test_total_rule_count_v11() {
    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);

    let count = registry.rules().len();
    assert!(
        count >= 52,
        "Should have at least 52 rules after v1.1, got {}",
        count
    );
}

// ============================================================
// Showcase Tests
// ============================================================

#[test]
fn test_showcase_analyze_project() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();

    let result = falcon::showcase::analyze_local_project(dir.path(), "test-project").unwrap();
    assert_eq!(result.name, "test-project");
    assert_eq!(result.file_count, 1);
    assert!(result.health_score > 0.0);
}

#[test]
fn test_showcase_report_generation() {
    let repos = vec![
        falcon::showcase::RepoAnalysis {
            name: "app-a".to_string(),
            file_count: 50,
            issue_count: 100,
            top_rules: vec![("avoid-dynamic".to_string(), 30)],
            health_score: 90.0,
            analysis_time_ms: 200,
        },
        falcon::showcase::RepoAnalysis {
            name: "app-b".to_string(),
            file_count: 30,
            issue_count: 60,
            top_rules: vec![("avoid-dynamic".to_string(), 20)],
            health_score: 80.0,
            analysis_time_ms: 150,
        },
    ];

    let report = falcon::showcase::generate_showcase_report(repos);
    assert_eq!(report.total_files, 80);
    assert_eq!(report.total_issues, 160);
    assert!((report.avg_health_score - 85.0).abs() < 0.1);
    assert!(!report.most_common_rules.is_empty());
}

#[test]
fn test_showcase_markdown_report() {
    let repos = vec![falcon::showcase::RepoAnalysis {
        name: "test-app".to_string(),
        file_count: 10,
        issue_count: 5,
        top_rules: vec![("avoid-dynamic".to_string(), 3)],
        health_score: 97.5,
        analysis_time_ms: 50,
    }];

    let report = falcon::showcase::generate_showcase_report(repos);
    let md = falcon::showcase::generate_markdown_report(&report);
    assert!(md.contains("Falcon Showcase"));
    assert!(md.contains("test-app"));
}

// ============================================================
// Compare Tests
// ============================================================

#[test]
fn test_compare_on_small_project() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();

    let result = falcon::benchmark_compare::compare_with_dart_analyze(dir.path()).unwrap();
    assert!(result.falcon_time_ms < 5000);
    assert!(!result.falcon_unique.is_empty());
}
