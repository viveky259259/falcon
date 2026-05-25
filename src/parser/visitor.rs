use tree_sitter::Node;

pub trait AstVisitor {
    fn visit_node(&mut self, node: Node, source: &str) {
        let _ = (node, source);
    }

    fn visit_class(&mut self, node: Node, source: &str) {
        let _ = (node, source);
    }

    fn visit_function(&mut self, node: Node, source: &str) {
        let _ = (node, source);
    }

    fn visit_method(&mut self, node: Node, source: &str) {
        let _ = (node, source);
    }

    fn visit_import(&mut self, node: Node, source: &str) {
        let _ = (node, source);
    }

    fn visit_variable(&mut self, node: Node, source: &str) {
        let _ = (node, source);
    }
}

pub fn walk_and_visit<V: AstVisitor>(node: Node, source: &str, visitor: &mut V) {
    visit_recursive(node, source, visitor);
}

fn visit_recursive<V: AstVisitor>(node: Node, source: &str, visitor: &mut V) {
    visitor.visit_node(node, source);

    match node.kind() {
        "class_declaration" => visitor.visit_class(node, source),
        "function_signature" => visitor.visit_function(node, source),
        "method_signature" => visitor.visit_method(node, source),
        "import_or_export" => visitor.visit_import(node, source),
        "initialized_variable_definition" => visitor.visit_variable(node, source),
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_recursive(child, source, visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::{walk_and_visit, AstVisitor};
    use crate::parser::DartParser;

    #[derive(Default)]
    struct RecordingVisitor {
        nodes: usize,
        classes: usize,
        functions: usize,
        methods: usize,
        imports: usize,
        variables: usize,
        kinds_visited: Vec<String>,
    }

    impl AstVisitor for RecordingVisitor {
        fn visit_node(&mut self, node: tree_sitter::Node, _source: &str) {
            self.nodes += 1;
            self.kinds_visited.push(node.kind().to_string());
        }

        fn visit_class(&mut self, _node: tree_sitter::Node, _source: &str) {
            self.classes += 1;
        }

        fn visit_function(&mut self, _node: tree_sitter::Node, _source: &str) {
            self.functions += 1;
        }

        fn visit_method(&mut self, _node: tree_sitter::Node, _source: &str) {
            self.methods += 1;
        }

        fn visit_import(&mut self, _node: tree_sitter::Node, _source: &str) {
            self.imports += 1;
        }

        fn visit_variable(&mut self, _node: tree_sitter::Node, _source: &str) {
            self.variables += 1;
        }
    }

    fn parse_dart(source: &str) -> tree_sitter::Tree {
        let mut parser = DartParser::new().expect("DartParser::new failed");
        parser.parse(source).expect("failed to parse Dart source")
    }

    // Debug helper — used ad-hoc during development; keeps tests self-contained.
    fn collect_all_kinds(node: tree_sitter::Node) -> Vec<String> {
        let mut kinds = Vec::new();
        collect_kinds_rec(node, &mut kinds);
        kinds
    }

    fn collect_kinds_rec(node: tree_sitter::Node, out: &mut Vec<String>) {
        out.push(node.kind().to_string());
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_kinds_rec(child, out);
        }
    }

    #[test]
    fn walk_and_visit_calls_visit_node_for_every_node() {
        let source = "class A {}";
        let tree = parse_dart(source);
        let mut visitor = RecordingVisitor::default();
        walk_and_visit(tree.root_node(), source, &mut visitor);
        assert!(
            visitor.nodes > 0,
            "visit_node should be called at least once; got 0"
        );
    }

    #[test]
    fn walk_and_visit_dispatches_class_declaration() {
        let source = "class Foo {}";
        let tree = parse_dart(source);
        let mut visitor = RecordingVisitor::default();
        walk_and_visit(tree.root_node(), source, &mut visitor);
        assert!(
            visitor.classes >= 1,
            "visit_class should be called at least once for 'class Foo {{}}'; kinds: {:?}",
            visitor.kinds_visited
        );
    }

    #[test]
    fn walk_and_visit_dispatches_function_signature() {
        let source = "void foo() {}";
        let tree = parse_dart(source);
        let mut visitor = RecordingVisitor::default();
        walk_and_visit(tree.root_node(), source, &mut visitor);
        assert!(
            visitor.functions >= 1,
            "visit_function should be called at least once for 'void foo() {{}}'; kinds: {:?}",
            visitor.kinds_visited
        );
    }

    #[test]
    fn walk_and_visit_dispatches_method_signature() {
        let source = "class A { void m() {} }";
        let tree = parse_dart(source);
        let mut visitor = RecordingVisitor::default();
        walk_and_visit(tree.root_node(), source, &mut visitor);
        assert!(
            visitor.methods >= 1,
            "visit_method should be called at least once for 'class A {{ void m() {{}} }}'; kinds: {:?}",
            visitor.kinds_visited
        );
    }

    #[test]
    fn walk_and_visit_dispatches_import() {
        let source = "import 'dart:async';";
        let tree = parse_dart(source);
        let mut visitor = RecordingVisitor::default();
        walk_and_visit(tree.root_node(), source, &mut visitor);
        assert!(
            visitor.imports >= 1,
            "visit_import should be called at least once for import directive; kinds: {:?}",
            visitor.kinds_visited
        );
    }

    #[test]
    fn walk_and_visit_dispatches_variable() {
        // "initialized_variable_definition" appears in local_variable_declaration
        // inside a function body, not at the top level (which uses initialized_identifier_list).
        let source = "void foo() { var x = 1; }";
        let tree = parse_dart(source);
        let mut visitor = RecordingVisitor::default();
        walk_and_visit(tree.root_node(), source, &mut visitor);
        assert!(
            visitor.variables >= 1,
            "visit_variable should be called for local 'var x = 1;' inside a function; kinds: {:?}",
            visitor.kinds_visited
        );
    }

    #[test]
    fn walk_and_visit_default_impls_are_noop() {
        struct DefaultVisitor;
        impl AstVisitor for DefaultVisitor {}

        let source = "class A {}";
        let tree = parse_dart(source);
        let mut visitor = DefaultVisitor;
        // Should not panic — default trait methods are no-ops.
        walk_and_visit(tree.root_node(), source, &mut visitor);
    }

    #[test]
    fn walk_and_visit_unmatched_kind_ignored() {
        // The root node ("source_file") and many intermediate nodes do not match any
        // of the specific arms, so the `_ => {}` fallback is exercised. We verify
        // that visit_node is still called (recursion works) while none of the
        // specific dispatch counters are incremented for a snippet that contains
        // only a top-level variable declaration (no class/function/method/import).
        let source = "var _val = 42;";
        let tree = parse_dart(source);
        let all_kinds = collect_all_kinds(tree.root_node());
        // Confirm that "source_file" (root) is not a matched arm — it hits `_ => {}`.
        assert!(
            all_kinds.contains(&"source_file".to_string()),
            "expected a 'source_file' root node; got: {:?}",
            all_kinds
        );

        let mut visitor = RecordingVisitor::default();
        walk_and_visit(tree.root_node(), source, &mut visitor);
        assert!(visitor.nodes > 0, "visit_node counter should be > 0; got 0");
        // "source_file", "var", identifiers, etc. are not matched arms, so fallback is hit.
        assert!(
            visitor.classes == 0 && visitor.functions == 0 && visitor.methods == 0,
            "no class/function/method expected for 'var _val = 42;'"
        );
    }
}
