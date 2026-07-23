//! End-to-end parser integration coverage.
//!
//! Confirms the public `DartParser` facade works on realistic Flutter code and
//! survives a battery of malformed inputs. Helper-level coverage lives inline
//! beside the implementation in `src/parser/*.rs`.

use falcon::parser::{dart_ast, find_descendants_by_kind, walk_tree, DartParser};

#[test]
fn parses_realistic_widget_file_cleanly() {
    let src = r#"
        import 'package:flutter/material.dart';

        class HelloWidget extends StatelessWidget {
          const HelloWidget({super.key, required this.name});
          final String name;

          @override
          Widget build(BuildContext context) {
            return Text('Hello, $name');
          }
        }
    "#;
    let mut p = DartParser::new().unwrap();
    let tree = p.parse(src).expect("tree");
    let root = tree.root_node();
    assert!(!root.has_error(), "realistic widget should parse cleanly");

    let classes = find_descendants_by_kind(root, "class_declaration");
    assert_eq!(classes.len(), 1);
    assert_eq!(
        dart_ast::get_declaration_name(classes[0], src),
        Some("HelloWidget")
    );
}

#[test]
fn parser_is_robust_to_many_malformed_inputs() {
    // Battery of breakage: parser must not panic and must report has_error()=true
    // for each input. Walks must terminate.
    for input in [
        "class { }",
        "void foo(",
        "import 'unterminated",
        "@@@",
        "}}}{{{",
    ] {
        let mut p = DartParser::new().unwrap();
        let tree = p.parse(input).expect("tree");
        let root = tree.root_node();
        assert!(root.has_error(), "expected error for {:?}", input);
        let mut count = 0usize;
        walk_tree(root, &mut |_| count += 1);
        assert!(count > 0);
    }
}
