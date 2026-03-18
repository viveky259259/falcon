use falcon::config::{FalconConfig, Severity};
use falcon::parser::DartParser;
use falcon::rules::RuleRegistry;
use std::path::PathBuf;

fn parse_and_check(source: &str, config: &FalconConfig) -> Vec<falcon::reporters::Issue> {
    let mut parser = DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();
    let file = PathBuf::from("test.dart");

    let mut registry = RuleRegistry::new();
    registry.register_defaults(config);
    registry.check(tree.root_node(), source, &file)
}

#[test]
fn test_avoid_long_parameter_list() {
    let source = r#"
void tooMany(int a, String b, double c, bool d, List e) {
  print('many params');
}
"#;
    let config = FalconConfig::default();
    let issues = parse_and_check(source, &config);

    let param_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-long-parameter-list")
        .collect();

    assert!(!param_issues.is_empty(), "Should flag function with > 4 params");
}

#[test]
fn test_avoid_dynamic() {
    let source = r#"
dynamic getValue() {
  return null;
}
"#;
    let config = FalconConfig::default();
    let issues = parse_and_check(source, &config);

    let dynamic_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-dynamic")
        .collect();

    assert!(!dynamic_issues.is_empty(), "Should flag 'dynamic' usage");
}

#[test]
fn test_avoid_global_state() {
    let source = r#"
var globalCounter = 0;
final safeGlobal = 'ok';
const alsoSafe = 42;
"#;
    let config = FalconConfig::default();
    let issues = parse_and_check(source, &config);

    let global_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-global-state")
        .collect();

    assert!(
        !global_issues.is_empty(),
        "Should flag mutable global variable"
    );
}

#[test]
fn test_no_issues_for_clean_code() {
    let source = r#"
void hello() {
  print('hello');
}
"#;
    let config = FalconConfig::default();
    let issues = parse_and_check(source, &config);

    let major_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == Severity::Error || i.severity == Severity::Warning)
        .filter(|i| {
            i.rule != "prefer-match-file-name"
                && i.rule != "no-magic-numbers"
                && i.rule != "avoid-print-in-production"
        })
        .collect();

    assert!(
        major_issues.is_empty(),
        "Clean code should have no major issues, but found: {:?}",
        major_issues.iter().map(|i| &i.rule).collect::<Vec<_>>()
    );
}

#[test]
fn test_suppression_ignore_for_file() {
    let source = r#"// ignore_for_file: avoid-dynamic

dynamic getValue() {
  return null;
}
"#;
    let config = FalconConfig::default();
    let issues = parse_and_check(source, &config);

    let dynamic_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "avoid-dynamic")
        .collect();

    assert!(
        dynamic_issues.is_empty(),
        "Should suppress avoid-dynamic via ignore_for_file"
    );
}

#[test]
fn test_all_rules_registered() {
    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    let source = "void x() {}";
    let mut parser = DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();
    let file = PathBuf::from("test.dart");

    let _ = registry.check(tree.root_node(), source, &file);
}

#[test]
fn test_complex_fixture_rules() {
    let source = include_str!("fixtures/complex_function.dart");
    let config = FalconConfig::default();
    let issues = parse_and_check(source, &config);

    assert!(!issues.is_empty(), "Complex fixture should trigger rules");

    let rule_names: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
    assert!(
        rule_names.contains(&"avoid-long-parameter-list"),
        "Should flag processData's long parameter list"
    );
}

#[test]
fn test_flutter_fixture_rules() {
    let source = include_str!("fixtures/flutter_patterns.dart");
    let config = FalconConfig::default();
    let issues = parse_and_check(source, &config);

    assert!(!issues.is_empty(), "Flutter fixture should trigger rules");
}
