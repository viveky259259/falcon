pub mod dart_ast;
pub mod visitor;

use anyhow::Result;
use tree_sitter::{Node, Parser, Tree};

pub struct DartParser {
    parser: Parser,
}

impl DartParser {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_dart::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| anyhow::anyhow!("Failed to set Dart language: {}", e))?;
        Ok(Self { parser })
    }

    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }
}

pub fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

pub fn node_start_line(node: Node) -> usize {
    node.start_position().row + 1
}

pub fn node_end_line(node: Node) -> usize {
    node.end_position().row + 1
}

pub fn find_children_by_kind<'a>(node: Node<'a>, kind: &str) -> Vec<Node<'a>> {
    let mut results = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            results.push(child);
        }
    }
    results
}

pub fn find_descendants_by_kind<'a>(node: Node<'a>, kind: &str) -> Vec<Node<'a>> {
    let mut results = Vec::new();
    collect_descendants(node, kind, &mut results);
    results
}

fn collect_descendants<'a>(node: Node<'a>, kind: &str, results: &mut Vec<Node<'a>>) {
    if node.kind() == kind {
        results.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_descendants(child, kind, results);
    }
}

pub fn walk_tree<F>(node: Node, callback: &mut F)
where
    F: FnMut(Node),
{
    callback(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree(child, callback);
    }
}

#[cfg(test)]
mod tests {
    //! Foundation parser coverage — pins down round-trip behaviour, Dart 3.x
    //! grammar acceptance, error recovery, and helper correctness.
    //! See council roadmap EPIC 3.1 for the (future) name/import resolver.
    use super::*;

    fn parse(src: &str) -> Tree {
        let mut p = DartParser::new().expect("DartParser::new");
        p.parse(src).expect("parser returned a tree")
    }

    fn has_kind(root: Node, kind: &str) -> bool {
        let mut found = false;
        walk_tree(root, &mut |n| {
            if n.kind() == kind {
                found = true;
            }
        });
        found
    }

    #[test]
    fn parses_small_valid_snippets_clean() {
        let corpus = [
            "void main() {}",
            "class A {}",
            "class A { int x = 0; }",
            "int add(int a, int b) => a + b;",
            "enum Color { red, green, blue }",
            "mixin M {}",
            "extension StringX on String { String shout() => toUpperCase(); }",
            "abstract class Animal { void speak(); }",
        ];
        for src in corpus.iter() {
            let mut p = DartParser::new().unwrap();
            let tree = p.parse(src).expect("parse returned None for valid input");
            let root = tree.root_node();
            assert!(
                !root.has_error(),
                "expected clean parse for `{}` but got error tree",
                src
            );
            assert_eq!(root.kind(), "source_file");
        }
    }

    #[test]
    fn parses_dart3_records_cleanly() {
        let src = "void main() { var r = (1, 'a'); print(r); }";
        let tree = parse(src);
        let root = tree.root_node();
        assert!(!root.has_error(), "record literal should parse cleanly");
        assert!(
            has_kind(root, "record_literal"),
            "expected `record_literal` node in tree"
        );
    }

    #[test]
    fn parses_dart3_switch_with_patterns_cleanly() {
        let src = r#"
            void f(int x) {
              switch (x) {
                case 1: print('one');
                case 2: print('two');
                default: print('other');
              }
            }
        "#;
        let tree = parse(src);
        let root = tree.root_node();
        assert!(!root.has_error(), "switch should parse cleanly");
        assert!(has_kind(root, "switch_statement"));
        assert!(has_kind(root, "switch_statement_case"));
    }

    #[test]
    fn parses_sealed_class_cleanly() {
        let src = "sealed class Shape {} class Square extends Shape {}";
        let tree = parse(src);
        let root = tree.root_node();
        assert!(!root.has_error(), "sealed class should parse cleanly");
        // The `sealed` modifier surfaces as a leaf token of that kind.
        assert!(has_kind(root, "sealed"));
        assert!(has_kind(root, "class_declaration"));
    }

    #[test]
    fn recovers_from_broken_input_without_panic() {
        // The parser must NEVER panic on malformed Dart; it should produce a tree
        // with `has_error()` == true and at least one ERROR node.
        let broken = "void main() { print('hello' ";
        let mut p = DartParser::new().unwrap();
        let tree = p.parse(broken).expect("parser should still produce a tree");
        let root = tree.root_node();
        assert!(root.has_error(), "broken input should set has_error()");
        assert!(
            has_kind(root, "ERROR"),
            "expected at least one ERROR node in recovered tree"
        );
    }

    #[test]
    fn find_helpers_have_shallow_vs_recursive_semantics() {
        // `find_children_by_kind` is shallow; `find_descendants_by_kind` recurses.
        let src = "class Widget {}\nclass State {}\n";
        let tree = parse(src);
        let root = tree.root_node();

        // Two classes at the top level.
        let top_classes = find_children_by_kind(root, "class_declaration");
        assert_eq!(top_classes.len(), 2);
        let all_classes = find_descendants_by_kind(root, "class_declaration");
        assert_eq!(all_classes.len(), 2);

        // Identifiers are nested, NOT direct children of source_file.
        assert!(find_children_by_kind(root, "identifier").is_empty());
        assert!(!find_descendants_by_kind(root, "identifier").is_empty());

        // Negative case: missing kind returns empty Vec, not panic.
        assert!(find_descendants_by_kind(root, "nonexistent_kind").is_empty());

        // node_text + line helpers agree with the source.
        let first = all_classes[0];
        assert_eq!(node_start_line(first), 1);
        assert_eq!(node_start_line(all_classes[1]), 2);
        assert!(node_end_line(all_classes[1]) >= 2);
        assert!(node_text(first, src).starts_with("class Widget"));
    }

    #[test]
    #[ignore = "performance smoke; run with `cargo test -- --ignored`"]
    fn parses_500_line_synthetic_file_under_100ms() {
        use std::time::Instant;

        // Build ~500 lines of trivially valid Dart.
        let mut src = String::with_capacity(20_000);
        src.push_str("// generated\n");
        for i in 0..500 {
            src.push_str(&format!("int v{} = {};\n", i, i));
        }

        let mut p = DartParser::new().unwrap();
        let start = Instant::now();
        let tree = p.parse(&src).expect("should parse");
        let elapsed = start.elapsed();

        assert!(!tree.root_node().has_error());
        assert!(
            elapsed.as_millis() < 100,
            "parsing 500 lines took {:?} (expected < 100ms)",
            elapsed
        );
    }
}
