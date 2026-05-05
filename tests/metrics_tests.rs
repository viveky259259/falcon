use falcon::config::MetricsConfig;
use falcon::metrics::{self, cyclomatic, lines, maintainability, nesting, parameters};
use falcon::parser::DartParser;

fn parse_source(source: &str) -> tree_sitter::Tree {
    let mut parser = DartParser::new().unwrap();
    parser.parse(source).unwrap()
}

#[test]
fn test_lines_of_code() {
    let source = r#"
// This is a comment
class Foo {
  void bar() {
    print('hello');
  }

  // another comment
  void baz() {}
}
"#;
    let (loc, sloc) = lines::count_lines(source);
    assert!(loc >= 10, "LOC should be >= 10, got {}", loc);
    assert!(sloc >= 6, "SLOC should be >= 6, got {}", sloc);
    assert!(
        sloc < loc,
        "SLOC ({}) should be less than LOC ({})",
        sloc,
        loc
    );
}

#[test]
fn test_lines_block_comment() {
    let source = r#"
/* This is
   a block comment */
class Foo {}
"#;
    let (_loc, sloc) = lines::count_lines(source);
    assert!(sloc >= 1, "SLOC should be >= 1, got {}", sloc);
}

#[test]
fn test_cyclomatic_simple_function() {
    let source = r#"
void simple() {
  print('hello');
}
"#;
    let tree = parse_source(source);
    let funcs = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_signature");
    assert!(!funcs.is_empty(), "Should find at least one function");

    if let Some(body) = falcon::parser::dart_ast::get_function_body(funcs[0]) {
        let cc = cyclomatic::calculate(body, source);
        assert_eq!(cc, 1, "Simple function should have CC=1");
    }
}

#[test]
fn test_cyclomatic_with_branches() {
    let source = r#"
int calculate(int x) {
  if (x > 0) {
    return x;
  } else if (x < 0) {
    return -x;
  } else {
    for (var i = 0; i < 10; i++) {
      print(i);
    }
    return 0;
  }
}
"#;
    let tree = parse_source(source);
    let funcs = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_signature");
    assert!(!funcs.is_empty());

    if let Some(body) = falcon::parser::dart_ast::get_function_body(funcs[0]) {
        let cc = cyclomatic::calculate(body, source);
        assert!(
            cc >= 3,
            "Function with branches should have CC >= 3, got {}",
            cc
        );
    }
}

#[test]
fn test_nesting_depth() {
    let source = r#"
void deep() {
  if (true) {
    if (true) {
      if (true) {
        print('deep');
      }
    }
  }
}
"#;
    let tree = parse_source(source);
    let funcs = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_signature");
    assert!(!funcs.is_empty());

    if let Some(body) = falcon::parser::dart_ast::get_function_body(funcs[0]) {
        let depth = nesting::calculate(body);
        assert!(depth >= 3, "Should have nesting depth >= 3, got {}", depth);
    }
}

#[test]
fn test_parameter_count() {
    let source = r#"
void noParams() {}
void twoParams(int a, String b) {}
void manyParams(int a, String b, double c, bool d, List e) {}
"#;
    let tree = parse_source(source);
    let funcs = falcon::parser::find_descendants_by_kind(tree.root_node(), "function_signature");
    assert!(funcs.len() >= 3, "Should find 3 functions");

    let counts: Vec<u32> = funcs.iter().map(|f| parameters::count(*f)).collect();
    assert_eq!(counts[0], 0, "noParams should have 0 parameters");
    assert_eq!(counts[1], 2, "twoParams should have 2 parameters");
    assert_eq!(counts[2], 5, "manyParams should have 5 parameters");
}

#[test]
fn test_maintainability_index() {
    let mi_simple = maintainability::calculate(1, 5, 0.0);
    assert!(
        mi_simple > 50.0,
        "Simple function should have MI > 50, got {}",
        mi_simple
    );

    let mi_complex = maintainability::calculate(30, 200, 0.0);
    assert!(
        mi_complex < mi_simple,
        "Complex function should have lower MI"
    );
}

#[test]
fn test_file_metrics() {
    let source = include_str!("fixtures/complex_function.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    assert!(results.file_lines_of_code > 0);
    assert!(results.file_source_lines_of_code > 0);
    assert!(!results.functions.is_empty(), "Should have functions");
    assert!(!results.classes.is_empty(), "Should have classes");

    let process_data = results.functions.iter().find(|f| f.name == "processData");
    assert!(process_data.is_some(), "Should find processData function");
    let pd = process_data.unwrap();
    assert!(
        pd.cyclomatic_complexity > 1,
        "processData should have CC > 1"
    );
    assert!(
        pd.number_of_parameters >= 6,
        "processData should have >= 6 params"
    );
    assert!(
        pd.max_nesting_level >= 5,
        "processData should have deep nesting"
    );
}

#[test]
fn test_class_methods_count() {
    let source = include_str!("fixtures/complex_function.dart");
    let tree = parse_source(source);
    let config = MetricsConfig::default();
    let results = metrics::calculate_file_metrics(tree.root_node(), source, &config);

    let dp_class = results.classes.iter().find(|c| c.name == "DataProcessor");
    assert!(dp_class.is_some(), "Should find DataProcessor class");
    let dp = dp_class.unwrap();
    assert!(
        dp.number_of_methods >= 10,
        "DataProcessor should have >= 10 methods, got {}",
        dp.number_of_methods
    );
}
