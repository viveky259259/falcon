use falcon::config::MetricsConfig;
use falcon::metrics::{self, halstead, widget_metrics};
use falcon::parser::DartParser;

fn parse_source(source: &str) -> tree_sitter::Tree {
    let mut parser = DartParser::new().unwrap();
    parser.parse(source).unwrap()
}

#[test]
fn test_halstead_basic() {
    let source = r#"
int add(int a, int b) {
  return a + b;
}
"#;
    let tree = parse_source(source);
    let funcs = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_signature");
    assert!(!funcs.is_empty());

    if let Some(body) = falcon::parser::dart_ast::get_function_body(funcs[0]) {
        let h = halstead::calculate(body, source);
        assert!(h.vocabulary > 0, "Should have non-zero vocabulary");
        assert!(
            h.volume > 0.0,
            "Should have non-zero volume, got {}",
            h.volume
        );
        assert!(h.length > 0, "Should have non-zero length");
    }
}

#[test]
fn test_halstead_complex_function() {
    let simple = r#"
void simple() {
  print('hello');
}
"#;
    let complex = r#"
int calculate(int a, int b, int c) {
  if (a > b) {
    return a * c + b;
  } else if (b > c) {
    return b - a;
  } else {
    for (var i = 0; i < c; i++) {
      a += i;
    }
    return a;
  }
}
"#;
    let tree_s = parse_source(simple);
    let tree_c = parse_source(complex);

    let funcs_s =
        falcon::parser::find_descendants_by_kind(tree_s.root_node(), "function_signature");
    let funcs_c =
        falcon::parser::find_descendants_by_kind(tree_c.root_node(), "function_signature");

    let vol_s = funcs_s
        .first()
        .and_then(|f| falcon::parser::dart_ast::get_function_body(*f))
        .map(|b| halstead::calculate(b, simple).volume)
        .unwrap_or(0.0);

    let vol_c = funcs_c
        .first()
        .and_then(|f| falcon::parser::dart_ast::get_function_body(*f))
        .map(|b| halstead::calculate(b, complex).volume)
        .unwrap_or(0.0);

    assert!(
        vol_c > vol_s,
        "Complex function should have higher Halstead volume ({} vs {})",
        vol_c,
        vol_s
    );
}

#[test]
fn test_widget_nesting_level() {
    let source = r#"
Widget build(BuildContext context) {
  return Scaffold(
    body: Center(
      child: Column(
        children: [
          Text('hello'),
          Icon(Icons.star),
        ],
      ),
    ),
  );
}
"#;
    let tree = parse_source(source);
    let funcs = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_signature");

    if let Some(body) = funcs
        .first()
        .and_then(|f| falcon::parser::dart_ast::get_function_body(*f))
    {
        let wnl = widget_metrics::widgets_nesting_level(body, source);
        assert!(
            wnl >= 3,
            "Widget nesting should be >= 3 (Scaffold > Center > Column), got {}",
            wnl
        );
    }
}

#[test]
fn test_count_used_widgets() {
    let source = r#"
Widget build(BuildContext context) {
  return Column(
    children: [
      Text('hello'),
      Icon(Icons.star),
      SizedBox(height: 8),
      ElevatedButton(child: Text('tap')),
    ],
  );
}
"#;
    let tree = parse_source(source);
    let funcs = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_signature");

    if let Some(body) = funcs
        .first()
        .and_then(|f| falcon::parser::dart_ast::get_function_body(*f))
    {
        let count = widget_metrics::count_used_widgets(body, source);
        assert!(
            count >= 4,
            "Should count >= 4 widget types (Column, Text, Icon, SizedBox, ElevatedButton), got {}",
            count
        );
    }
}

#[test]
fn test_coupling_between_objects() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let service_locator = results.classes.iter().find(|c| c.name == "ServiceLocator");
    assert!(
        service_locator.is_some(),
        "Should find ServiceLocator class"
    );
    let sl = service_locator.unwrap();
    assert!(sl.coupling_between_objects >= 4, "ServiceLocator should have CBO >= 4 (references DatabaseService, ApiClient, Logger, CacheManager), got {}", sl.coupling_between_objects);
}

#[test]
fn test_depth_of_inheritance() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let dog = results.classes.iter().find(|c| c.name == "Dog");
    assert!(dog.is_some());
    assert_eq!(
        dog.unwrap().depth_of_inheritance,
        1,
        "Dog extends Animal, DIT should be 1"
    );

    let animal = results.classes.iter().find(|c| c.name == "Animal");
    assert!(animal.is_some());
    assert_eq!(
        animal.unwrap().depth_of_inheritance,
        0,
        "Animal has no superclass, DIT should be 0"
    );
}

#[test]
fn test_number_of_interfaces() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let dog = results.classes.iter().find(|c| c.name == "Dog");
    assert!(dog.is_some());
    assert!(
        dog.unwrap().number_of_interfaces >= 1,
        "Dog implements Comparable, should have >= 1 interface, got {}",
        dog.unwrap().number_of_interfaces
    );
}

#[test]
fn test_overridden_methods() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let dog = results.classes.iter().find(|c| c.name == "Dog");
    assert!(dog.is_some());
    let d = dog.unwrap();
    assert!(
        d.number_of_overridden_methods >= 2,
        "Dog should have >= 2 overridden methods (speak, compareTo), got {}",
        d.number_of_overridden_methods
    );
    assert!(
        d.number_of_added_methods >= 2,
        "Dog should have >= 2 added methods (fetch, rollOver), got {}",
        d.number_of_added_methods
    );
}

#[test]
fn test_weighted_methods_per_class() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let animal = results.classes.iter().find(|c| c.name == "Animal");
    assert!(animal.is_some());
    let a = animal.unwrap();
    assert!(
        a.weighted_methods_per_class >= a.number_of_methods,
        "WMC should be >= number of methods (each method has CC >= 1)"
    );
}

#[test]
fn test_weight_of_class() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let dog = results.classes.iter().find(|c| c.name == "Dog");
    assert!(dog.is_some());
    let d = dog.unwrap();
    assert!(
        d.weight_of_class > 0.0,
        "Dog should have WOC > 0 (has public methods)"
    );
    assert!(d.weight_of_class <= 1.0, "WOC should be <= 1.0");
}

#[test]
fn test_tight_class_cohesion() {
    let source = r#"
class Cohesive {
  int x = 0;
  int y = 0;

  void useX() { print(x); }
  void useY() { print(y); }
  void useBoth() { print(x + y); }
}
"#;
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let cohesive = results.classes.iter().find(|c| c.name == "Cohesive");
    assert!(cohesive.is_some());
    let c = cohesive.unwrap();
    assert!(
        c.tight_class_cohesion > 0.0,
        "Cohesive class should have TCC > 0, got {}",
        c.tight_class_cohesion
    );
}

#[test]
fn test_lack_of_cohesion() {
    let source = r#"
class NotCohesive {
  int a = 0;
  int b = 0;
  int c = 0;

  void methodA() { print(a); }
  void methodB() { print(b); }
  void methodC() { print(c); }
}
"#;
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let nc = results.classes.iter().find(|c| c.name == "NotCohesive");
    assert!(nc.is_some());
    let n = nc.unwrap();
    assert!(
        n.lack_of_cohesion >= 2,
        "NotCohesive should have LCOM >= 2 (3 disjoint pairs - 0 shared = 3), got {}",
        n.lack_of_cohesion
    );
}

#[test]
fn test_response_for_class() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let sl = results.classes.iter().find(|c| c.name == "ServiceLocator");
    assert!(sl.is_some());
    let s = sl.unwrap();
    assert!(
        s.response_for_class >= s.number_of_methods,
        "RFC ({}) should be >= number of methods ({})",
        s.response_for_class,
        s.number_of_methods
    );
}

#[test]
fn test_threshold_levels() {
    use falcon::metrics::{threshold_level, threshold_level_inverted, ThresholdLevel};

    assert_eq!(threshold_level(5, 10, 20, 30), ThresholdLevel::Ok);
    assert_eq!(threshold_level(15, 10, 20, 30), ThresholdLevel::Noted);
    assert_eq!(threshold_level(25, 10, 20, 30), ThresholdLevel::Warning);
    assert_eq!(threshold_level(35, 10, 20, 30), ThresholdLevel::Alarm);

    assert_eq!(
        threshold_level_inverted(80.0, 20.0, 40.0, 60.0),
        ThresholdLevel::Ok
    );
    assert_eq!(
        threshold_level_inverted(50.0, 20.0, 40.0, 60.0),
        ThresholdLevel::Noted
    );
    assert_eq!(
        threshold_level_inverted(30.0, 20.0, 40.0, 60.0),
        ThresholdLevel::Warning
    );
    assert_eq!(
        threshold_level_inverted(10.0, 20.0, 40.0, 60.0),
        ThresholdLevel::Alarm
    );
}

#[test]
fn test_flutter_widget_metrics_in_fixture() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let build_funcs: Vec<_> = results
        .functions
        .iter()
        .filter(|f| f.name == "build" || f.number_of_used_widgets > 0)
        .collect();

    assert!(
        !build_funcs.is_empty(),
        "Should find build method with widgets"
    );

    let has_widget_metrics = build_funcs.iter().any(|f| f.number_of_used_widgets > 0);
    assert!(has_widget_metrics, "Build method should use widgets");
}

#[test]
fn test_all_new_class_metrics_present() {
    let source = include_str!("fixtures/class_hierarchy.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    assert!(!results.classes.is_empty(), "Should have classes");

    for class in &results.classes {
        let _ = class.coupling_between_objects;
        let _ = class.depth_of_inheritance;
        let _ = class.number_of_added_methods;
        let _ = class.number_of_interfaces;
        let _ = class.number_of_overridden_methods;
        let _ = class.response_for_class;
        let _ = class.tight_class_cohesion;
        let _ = class.weight_of_class;
        let _ = class.weighted_methods_per_class;
        let _ = class.lack_of_cohesion;
    }
}
