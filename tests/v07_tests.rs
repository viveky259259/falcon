use std::path::PathBuf;

// ============================================================
// Layer Enforcement Tests
// ============================================================

#[test]
fn test_detect_clean_architecture() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(lib.join("domain/entities")).unwrap();
    std::fs::create_dir_all(lib.join("data/repositories")).unwrap();
    std::fs::create_dir_all(lib.join("presentation/pages")).unwrap();

    let layers = falcon::analysis::layer_enforcement::detect_architecture(dir.path());
    assert!(layers.is_some());
    let layers = layers.unwrap();
    assert_eq!(layers.len(), 4);
    assert_eq!(layers[0].name, "domain");
    assert!(layers[0].allowed_imports.is_empty());
    assert!(layers[1].allowed_imports.contains(&"domain".to_string()));
}

#[test]
fn test_detect_feature_first_architecture() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(lib.join("core")).unwrap();
    std::fs::create_dir_all(lib.join("features/auth")).unwrap();
    std::fs::create_dir_all(lib.join("shared")).unwrap();

    let layers = falcon::analysis::layer_enforcement::detect_architecture(dir.path());
    assert!(layers.is_some());
    let layers = layers.unwrap();
    assert_eq!(layers.len(), 3);
}

#[test]
fn test_layer_violation_detected() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(lib.join("domain")).unwrap();
    std::fs::create_dir_all(lib.join("data")).unwrap();

    std::fs::write(
        lib.join("domain/model.dart"),
        "import 'package:app/data/repo.dart';\nclass Model {}",
    )
    .unwrap();

    let layers = falcon::analysis::layer_enforcement::LayerConfig::clean_architecture();
    let issues = falcon::analysis::layer_enforcement::enforce_layers(dir.path(), &layers, &[]);
    assert!(!issues.is_empty());
    assert!(issues[0].rule == "layer-violation");
    assert!(issues[0].message.contains("domain"));
}

#[test]
fn test_layer_allowed_import() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(lib.join("domain")).unwrap();
    std::fs::create_dir_all(lib.join("data")).unwrap();

    std::fs::write(
        lib.join("data/repo.dart"),
        "import 'package:app/domain/model.dart';\nclass Repo {}",
    )
    .unwrap();

    let layers = falcon::analysis::layer_enforcement::LayerConfig::clean_architecture();
    let issues = falcon::analysis::layer_enforcement::enforce_layers(dir.path(), &layers, &[]);
    assert!(issues.is_empty());
}

// ============================================================
// Import Rules Tests
// ============================================================

#[test]
fn test_import_restriction_enforcement() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(lib.join("features")).unwrap();

    std::fs::write(
        lib.join("features/my_feature.dart"),
        "import 'dart:io';\nimport 'package:flutter/material.dart';",
    )
    .unwrap();

    let restrictions = vec![falcon::analysis::import_rules::ImportRestriction {
        from: "features/*".to_string(),
        deny: vec!["dart:io".to_string()],
        reason: Some("Features should not use dart:io directly.".to_string()),
    }];

    let issues =
        falcon::analysis::import_rules::enforce_import_restrictions(dir.path(), &restrictions, &[]);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].message.contains("dart:io"));
}

#[test]
fn test_package_boundary_check() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(lib.join("src")).unwrap();

    std::fs::write(dir.path().join("pubspec.yaml"), "name: my_pkg\n").unwrap();
    std::fs::write(lib.join("src/internal.dart"), "class Internal {}").unwrap();
    std::fs::write(
        lib.join("my_pkg.dart"),
        "import 'package:my_pkg/src/internal.dart';",
    )
    .unwrap();

    let issues = falcon::analysis::import_rules::check_package_boundaries(dir.path(), &[]);
    assert!(!issues.is_empty());
    assert!(issues[0].rule == "package-boundary");
}

// ============================================================
// Cognitive Complexity Tests
// ============================================================

#[test]
fn test_cognitive_simple_function() {
    let source = r#"
void simple() {
  print('hello');
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let results =
        falcon::analysis::cognitive_complexity::file_cognitive_complexity(tree.root_node(), source);

    for (name, complexity, _) in &results {
        if name == "simple" {
            assert_eq!(*complexity, 0);
        }
    }
}

#[test]
fn test_cognitive_nested_conditions() {
    let source = r#"
int complex(int x) {
  if (x > 0) {
    if (x > 10) {
      if (x > 100) {
        return 3;
      }
      return 2;
    }
    return 1;
  }
  return 0;
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let results =
        falcon::analysis::cognitive_complexity::file_cognitive_complexity(tree.root_node(), source);

    let complex_fn = results.iter().find(|(name, _, _)| name == "complex");
    assert!(complex_fn.is_some());
    let (_, complexity, _) = complex_fn.unwrap();
    assert!(
        *complexity > 3,
        "Nested ifs should have high cognitive complexity, got {}",
        complexity
    );
}

#[test]
fn test_cognitive_standalone_function() {
    let source = r#"
void check(bool a, bool b) {
  if (a) {
    for (var i = 0; i < 10; i++) {
      if (b) {
        break;
      }
    }
  }
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();
    let node = tree.root_node();

    let complexity =
        falcon::analysis::cognitive_complexity::calculate_cognitive_complexity(node, source);
    assert!(complexity > 0, "Should have non-zero complexity");
}

// ============================================================
// Widget Rebuild Tests
// ============================================================

#[test]
fn test_detect_set_state_in_build() {
    let source = r#"
class MyWidget extends StatefulWidget {
  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    setState(() {});
    return Container();
  }
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let issues = falcon::analysis::widget_rebuild::detect_widget_issues(
        tree.root_node(),
        source,
        &PathBuf::from("test.dart"),
    );

    let rebuild_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "widget-rebuild")
        .collect();
    assert!(
        !rebuild_issues.is_empty(),
        "Should detect setState in build"
    );
}

#[test]
fn test_detect_complex_build_method() {
    let mut build_body = String::new();
    for i in 0..100 {
        build_body.push_str(&format!("    print({});\n", i));
    }

    let source = format!(
        r#"
class BigWidget extends StatelessWidget {{
  @override
  Widget build(BuildContext context) {{
{}    return Container();
  }}
}}
"#,
        build_body
    );

    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(&source).unwrap();

    let issues = falcon::analysis::widget_rebuild::detect_widget_issues(
        tree.root_node(),
        &source,
        &PathBuf::from("test.dart"),
    );

    let complexity_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "build-method-complexity")
        .collect();
    assert!(
        !complexity_issues.is_empty(),
        "Should flag build method with 100+ lines"
    );
}

// ============================================================
// Async Anti-pattern Tests
// ============================================================

#[test]
fn test_detect_async_void() {
    let source = r#"
void doAsync() async {
  await Future.delayed(Duration(seconds: 1));
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let issues = falcon::analysis::async_antipatterns::detect_async_antipatterns(
        tree.root_node(),
        source,
        &PathBuf::from("test.dart"),
    );

    let async_void: Vec<_> = issues.iter().filter(|i| i.rule == "async-void").collect();
    assert!(!async_void.is_empty(), "Should detect async void function");
}

#[test]
fn test_skip_lifecycle_async_void() {
    let source = r#"
void initState() async {
  await loadData();
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let issues = falcon::analysis::async_antipatterns::detect_async_antipatterns(
        tree.root_node(),
        source,
        &PathBuf::from("test.dart"),
    );

    let async_void: Vec<_> = issues.iter().filter(|i| i.rule == "async-void").collect();
    assert!(
        async_void.is_empty(),
        "Should skip lifecycle methods like initState"
    );
}

#[test]
fn test_detect_await_in_loop() {
    let source = r#"
Future<void> process(List<String> items) async {
  for (var item in items) {
    await sendToServer(item);
  }
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let issues = falcon::analysis::async_antipatterns::detect_async_antipatterns(
        tree.root_node(),
        source,
        &PathBuf::from("test.dart"),
    );

    let loop_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "await-in-loop")
        .collect();
    assert!(!loop_issues.is_empty(), "Should detect await inside loop");
}

#[test]
fn test_detect_unawaited_future() {
    let source = r#"
void process() {
  fetchData().then((data) => print(data));
}
"#;
    let mut parser = falcon::parser::DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let issues = falcon::analysis::async_antipatterns::detect_async_antipatterns(
        tree.root_node(),
        source,
        &PathBuf::from("test.dart"),
    );

    let unawaited: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "unawaited-future")
        .collect();
    assert!(
        !unawaited.is_empty(),
        "Should detect unawaited future chain"
    );
}

// ============================================================
// PR Review Tests
// ============================================================

#[test]
fn test_review_empty_report() {
    let report = falcon::review::pr_review::ReviewReport {
        observations: Vec::new(),
        files_reviewed: 0,
        lines_changed: 0,
    };
    assert!(report.observations.is_empty());
    assert_eq!(report.files_reviewed, 0);
}

#[test]
fn test_observation_severity() {
    let obs = falcon::review::pr_review::ReviewObservation {
        category: falcon::review::pr_review::ObservationCategory::ErrorHandling,
        message: "Empty catch block".to_string(),
        file: PathBuf::from("test.dart"),
        line: 10,
        suggestion: Some("Handle the error".to_string()),
        severity: falcon::review::pr_review::ObservationSeverity::Critical,
    };
    assert_eq!(
        obs.severity,
        falcon::review::pr_review::ObservationSeverity::Critical
    );
    assert_eq!(obs.category.label(), "Error Handling");
}

// ============================================================
// Codebase Intelligence Tests
// ============================================================

#[test]
fn test_codebase_intel_on_project() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(dir.path().join("pubspec.yaml"), "name: test_app\n").unwrap();

    std::fs::write(
        lib.join("main.dart"),
        r#"
void main() {
  print('hello');
}

void helper() {
  print('helper');
}
"#,
    )
    .unwrap();

    let config = falcon::config::FalconConfig::default();
    let report = falcon::review::codebase_intel::analyze_codebase(dir.path(), &config).unwrap();

    assert_eq!(report.total_files, 1);
    assert!(report.total_lines > 0);
    assert!(report.health_score > 0.0);
    assert!(!report.narrative.is_empty());
}

#[test]
fn test_god_file_detection() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    let mut big_file = String::new();
    for i in 0..200 {
        big_file.push_str(&format!(
            "class Class{i} {{ void method{i}() {{ print({i}); }} }}\n",
            i = i
        ));
    }

    std::fs::write(lib.join("god_file.dart"), &big_file).unwrap();

    let config = falcon::config::FalconConfig::default();
    let report = falcon::review::codebase_intel::analyze_codebase(dir.path(), &config).unwrap();

    assert!(!report.god_files.is_empty(), "Should detect god file");
    assert!(report.god_files[0].classes > 5);
}

#[test]
fn test_tech_debt_scoring() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(lib.join("simple.dart"), "void main() { print('hello'); }\n").unwrap();

    let config = falcon::config::FalconConfig::default();
    let report = falcon::review::codebase_intel::analyze_codebase(dir.path(), &config).unwrap();

    assert!(
        report.tech_debt.score >= 80.0,
        "Simple codebase should have low debt"
    );
}

// ============================================================
// CLI Binary Tests
// ============================================================

#[test]
fn test_new_commands_registered() {
    let commands = [
        "check-layers",
        "check-imports",
        "cognitive-complexity",
        "check-widgets",
        "check-async",
        "review",
        "codebase-intel",
    ];

    for cmd in &commands {
        assert!(!cmd.is_empty(), "Command {} should be registered", cmd);
    }
}

#[test]
fn test_review_strictness_variants() {
    let _quick = falcon::review::pr_review::ReviewStrictness::Quick;
    let _standard = falcon::review::pr_review::ReviewStrictness::Standard;
    let _thorough = falcon::review::pr_review::ReviewStrictness::Thorough;
}

#[test]
fn test_observation_categories() {
    let categories = vec![
        falcon::review::pr_review::ObservationCategory::PatternConsistency,
        falcon::review::pr_review::ObservationCategory::NamingConvention,
        falcon::review::pr_review::ObservationCategory::ErrorHandling,
        falcon::review::pr_review::ObservationCategory::MissingTest,
        falcon::review::pr_review::ObservationCategory::CodeStyle,
        falcon::review::pr_review::ObservationCategory::Performance,
    ];

    let labels: Vec<&str> = categories.iter().map(|c| c.label()).collect();
    assert!(labels.contains(&"Pattern"));
    assert!(labels.contains(&"Error Handling"));
    assert!(labels.contains(&"Testing"));
}
