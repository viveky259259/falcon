use falcon::config::FalconConfig;
use falcon::parser::DartParser;
use falcon::rules::RuleRegistry;
use std::path::PathBuf;

fn parse_and_check(source: &str, filename: &str) -> Vec<falcon::reporters::Issue> {
    let mut parser = DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();
    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);
    registry.check(tree.root_node(), source, &PathBuf::from(filename))
}

fn has_rule(issues: &[falcon::reporters::Issue], rule: &str) -> bool {
    issues.iter().any(|i| i.rule == rule)
}

#[test]
fn test_all_43_rules_registered() {
    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);
    let source = "";
    let tree = DartParser::new().unwrap().parse(source).unwrap();
    let _ = registry.check(tree.root_node(), source, &PathBuf::from("test.dart"));
    assert_eq!(config.rules.len(), 43, "Should have 43 default rules configured");
}

#[test]
fn test_avoid_throw_in_catch() {
    let source = r#"
void test() {
  try {
    riskyOp();
  } catch (e) {
    throw Exception('oops');
  }
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "avoid-throw-in-catch-block"),
        "Should detect throw in catch block");
}

#[test]
fn test_prefer_first_last() {
    let source = r#"
void test() {
  final list = [1, 2, 3];
  final x = list[0];
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "prefer-first-last"),
        "Should detect [0] usage, issues: {:?}", issues.iter().map(|i| &i.rule).collect::<Vec<_>>());
}

#[test]
fn test_double_literal_format() {
    let source = r#"
void test() {
  final a = .5;
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "double-literal-format"),
        "Should detect leading dot in double literal");
}

#[test]
fn test_binary_expression_operand_order() {
    let source = r#"
void test(int x) {
  if (0 == x) {
    print('yoda');
  }
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "binary-expression-operand-order"),
        "Should detect Yoda condition");
}

#[test]
fn test_avoid_unused_parameters() {
    let source = r#"
void test(int used, int unused) {
  print(used);
}
"#;
    let issues = parse_and_check(source, "test.dart");
    let unused = issues.iter().filter(|i| i.rule == "avoid-unused-parameters").collect::<Vec<_>>();
    assert!(!unused.is_empty(), "Should detect unused parameter 'unused'");
}

#[test]
fn test_avoid_non_ascii_symbols() {
    let source = "void testFuncti\u{00f6}n() {}\n";
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "avoid-non-ascii-symbols"),
        "Should detect non-ASCII in identifier");
}

#[test]
fn test_avoid_duplicate_exports() {
    let source = r#"
export 'package:foo/foo.dart';
export 'package:foo/foo.dart';
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "avoid-duplicate-exports"),
        "Should detect duplicate export");
}

#[test]
fn test_always_override_equals_and_hashcode() {
    let source_only_equals = r#"
class Bad {
  bool operator ==(Object other) => false;
}
"#;
    let issues = parse_and_check(source_only_equals, "test.dart");
    assert!(has_rule(&issues, "always-override-equals-and-hashcode"),
        "Should detect == without hashCode");
}

#[test]
fn test_prefer_equatable() {
    let source = r#"
class MyVal {
  final int x;
  MyVal(this.x);

  bool operator ==(Object other) => other is MyVal && other.x == x;
  int get hashCode => x.hashCode;
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "prefer-equatable"),
        "Should suggest Equatable when both == and hashCode are overridden");
}

#[test]
fn test_avoid_mutable_equatable() {
    let source = r#"
class Bad extends Equatable {
  String name;

  Bad(this.name);

  List<Object> get props => [name];
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "avoid-mutable-equatable"),
        "Should detect mutable field in Equatable class");
}

#[test]
fn test_avoid_ref_read_inside_build() {
    let source = r#"
Widget build(BuildContext context) {
  final value = ref.read(myProvider);
  return Text(value);
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "avoid-ref-read-inside-build"),
        "Should detect ref.read inside build");
}

#[test]
fn test_avoid_watch_outside_build() {
    let source = r#"
void onTap() {
  final value = ref.watch(myProvider);
  doSomething(value);
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "avoid-watch-outside-build"),
        "Should detect ref.watch outside build");
}

#[test]
fn test_prefer_bloc_extensions() {
    let source = r#"
void test(BuildContext context) {
  final bloc = BlocProvider.of<MyBloc>(context);
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "prefer-bloc-extensions"),
        "Should suggest context.read over BlocProvider.of");
}

#[test]
fn test_newline_before_return() {
    let source = r#"
int test(int x) {
  final y = x * 2;
  return y;
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(has_rule(&issues, "newline-before-return"),
        "Should detect missing newline before return");
}

#[test]
fn test_avoid_top_level_members_in_tests() {
    let source = r#"
class TestHelper {
  void help() {}
}

void main() {}
"#;
    let issues = parse_and_check(source, "widget_test.dart");
    assert!(has_rule(&issues, "avoid-top-level-members-in-tests"),
        "Should detect top-level class in test file");
}

#[test]
fn test_suppression_per_rule() {
    let source = r#"
// ignore_for_file: avoid-dynamic
void test(dynamic x) {
  print(x);
}
"#;
    let issues = parse_and_check(source, "test.dart");
    assert!(!has_rule(&issues, "avoid-dynamic"),
        "avoid-dynamic should be suppressed by ignore_for_file");
}
