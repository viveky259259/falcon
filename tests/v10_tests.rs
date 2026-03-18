use std::path::PathBuf;

// ============================================================
// New AI-Critical Rule Tests
// ============================================================

#[test]
fn test_avoid_empty_catch() {
    let source = r#"
void main() {
  try {
    doSomething();
  } catch (e) {}
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("test.dart"));

    let empty_catch: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-empty-catch")
        .collect();
    assert!(!empty_catch.is_empty(), "Should detect empty catch block");
}

#[test]
fn test_avoid_empty_catch_with_print() {
    let source = r#"
void main() {
  try {
    doSomething();
  } catch (e) {
    print(e);
  }
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("test.dart"));

    let catch_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-empty-catch")
        .collect();
    assert!(
        !catch_issues.is_empty(),
        "Should detect catch that only prints"
    );
}

#[test]
fn test_avoid_print_in_production() {
    let source = r#"
void main() {
  print('hello world');
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/main.dart"));

    let print_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-print-in-production")
        .collect();
    assert!(!print_issues.is_empty(), "Should detect print() in production");
}

#[test]
fn test_print_allowed_in_tests() {
    let source = r#"
void main() {
  print('test output');
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("test/my_test.dart"));

    let print_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-print-in-production")
        .collect();
    assert!(
        print_issues.is_empty(),
        "Should allow print() in test files"
    );
}

#[test]
fn test_avoid_hardcoded_credentials() {
    let source = r#"
const apiKey = 'sk_live_abc123def456';
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/config.dart"));

    let cred_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-hardcoded-credentials")
        .collect();
    assert!(
        !cred_issues.is_empty(),
        "Should detect hardcoded API key"
    );
}

#[test]
fn test_hardcoded_creds_skip_empty() {
    let source = r#"
String apiKey = '';
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);
    let issues = registry.check(tree.root_node(), source, &PathBuf::from("lib/config.dart"));

    let cred_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-hardcoded-credentials")
        .collect();
    assert!(
        cred_issues.is_empty(),
        "Should skip empty string values"
    );
}

#[test]
fn test_ensure_dispose_lifecycle() {
    let source = r#"
class _MyState extends State<MyWidget> {
  late TextEditingController controller;
  late FocusNode focusNode;

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

    let dispose_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "ensure-dispose-lifecycle")
        .collect();
    assert!(
        !dispose_issues.is_empty(),
        "Should detect missing dispose() for controllers"
    );
}

#[test]
fn test_dispose_lifecycle_ok_with_dispose() {
    let source = r#"
class _MyState extends State<MyWidget> {
  late TextEditingController controller;

  @override
  void dispose() {
    controller.dispose();
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

    let dispose_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "ensure-dispose-lifecycle")
        .collect();
    assert!(
        dispose_issues.is_empty(),
        "Should not flag when dispose() exists"
    );
}

// ============================================================
// DCM Migration Tests
// ============================================================

#[test]
fn test_dcm_rule_mapping() {
    let mapping = falcon::migration::dcm::rule_mapping();
    assert!(mapping.len() > 30);
    assert_eq!(mapping.get("avoid-dynamic"), Some(&"avoid-dynamic"));
    assert_eq!(
        mapping.get("avoid-nested-conditional-expressions"),
        Some(&"avoid-nested-conditionals")
    );
}

#[test]
fn test_dcm_unmapped_rules() {
    let unmapped = falcon::migration::dcm::unmapped_dcm_rules();
    assert!(unmapped.len() > 50);
    assert!(unmapped.contains(&"avoid-shrink-wrap-in-lists"));
}

#[test]
fn test_dcm_migration_from_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let dcm_yaml = r#"
dart_code_metrics:
  rules:
    - avoid-dynamic
    - avoid-late-keyword
    - some-unknown-rule
  metrics:
    cyclomatic-complexity: 20
analyzer:
  exclude:
    - "**/*.g.dart"
"#;
    let config_path = dir.path().join("analysis_options.yaml");
    std::fs::write(&config_path, dcm_yaml).unwrap();

    let result = falcon::migration::dcm::migrate_from_dcm(&config_path).unwrap();
    assert_eq!(result.mapped_rules.len(), 2);
    assert_eq!(result.unmapped_rules.len(), 1);
    assert!(result.unmapped_rules.contains(&"some-unknown-rule".to_string()));
    assert!(!result.falcon_yaml_content.is_empty());
}

#[test]
fn test_feature_gap_report() {
    let report = falcon::migration::dcm::feature_gap_report();
    assert!(report.contains("Mapped rules:"));
    assert!(report.contains("Unmapped rules:"));
}

// ============================================================
// Benchmark Tests
// ============================================================

#[test]
fn test_benchmark_on_small_project() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(
        lib.join("main.dart"),
        "void main() { print('hello'); }\n",
    )
    .unwrap();

    let result = falcon::benchmark::run_benchmark(dir.path()).unwrap();
    assert_eq!(result.file_count, 1);
    assert!(result.total_lines > 0);
    assert!(result.total_time_ms < 5000);
    assert!(result.files_per_second > 0.0);
}

// ============================================================
// Rule Docs Tests
// ============================================================

#[test]
fn test_generate_rule_docs() {
    let docs = falcon::docs::rule_docs::generate_rule_docs();
    assert!(docs.len() >= 48, "Should have 48+ rules, got {}", docs.len());

    let names: Vec<&str> = docs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"avoid-empty-catch"));
    assert!(names.contains(&"avoid-print-in-production"));
    assert!(names.contains(&"avoid-hardcoded-credentials"));
    assert!(names.contains(&"ensure-dispose-lifecycle"));
}

#[test]
fn test_generate_markdown_docs() {
    let docs = falcon::docs::rule_docs::generate_rule_docs();
    let md = falcon::docs::rule_docs::generate_markdown_docs(&docs);

    assert!(md.contains("# Falcon Rule Reference"));
    assert!(md.contains("Dart"));
    assert!(md.contains("Flutter"));
    assert!(md.contains("avoid-dynamic"));
}

#[test]
fn test_rule_categories() {
    let docs = falcon::docs::rule_docs::generate_rule_docs();

    let categories: Vec<&str> = docs.iter().map(|d| d.category.as_str()).collect();
    assert!(categories.contains(&"Dart"));
    assert!(categories.contains(&"Flutter"));
    assert!(categories.contains(&"Riverpod"));
    assert!(categories.contains(&"BLoC"));
    assert!(categories.contains(&"Equatable"));
}

// ============================================================
// Rule Count Verification
// ============================================================

#[test]
fn test_total_rule_count() {
    let config = falcon::config::FalconConfig::default();
    let mut registry = falcon::rules::RuleRegistry::new();
    registry.register_defaults(&config);

    let count = registry.rules().len();
    assert!(
        count >= 48,
        "Should have at least 48 rules, got {}",
        count
    );
}
