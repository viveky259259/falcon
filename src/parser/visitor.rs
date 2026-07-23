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
    //! Behavioural test for the AstVisitor dispatch — confirms each typed
    //! hook fires exactly when its corresponding node kind is encountered.
    use super::*;
    use crate::parser::DartParser;

    #[derive(Default)]
    struct CountingVisitor {
        nodes: usize,
        classes: usize,
        functions: usize,
        methods: usize,
        imports: usize,
        variables: usize,
    }

    impl AstVisitor for CountingVisitor {
        fn visit_node(&mut self, _n: Node, _s: &str) {
            self.nodes += 1;
        }
        fn visit_class(&mut self, _n: Node, _s: &str) {
            self.classes += 1;
        }
        fn visit_function(&mut self, _n: Node, _s: &str) {
            self.functions += 1;
        }
        fn visit_method(&mut self, _n: Node, _s: &str) {
            self.methods += 1;
        }
        fn visit_import(&mut self, _n: Node, _s: &str) {
            self.imports += 1;
        }
        fn visit_variable(&mut self, _n: Node, _s: &str) {
            self.variables += 1;
        }
    }

    #[test]
    fn visitor_dispatches_typed_hooks() {
        // We use a function-local variable for the `visit_variable` hook because
        // the grammar surfaces `initialized_variable_definition` for locals
        // (top-level vars use a different node kind).
        let src = r#"
            import 'dart:async';
            class A { void m() {} }
            int top() {
              int x = 0;
              return x;
            }
        "#;
        let mut p = DartParser::new().unwrap();
        let tree = p.parse(src).unwrap();
        let root = tree.root_node();

        let mut v = CountingVisitor::default();
        walk_and_visit(root, src, &mut v);

        assert!(v.nodes > 0, "visit_node should fire many times");
        assert_eq!(v.classes, 1, "expected one class hook");
        assert!(v.imports >= 1, "expected at least one import hook");
        // `top()` produces a function_signature at top level; `m()` produces a
        // method_signature inside the class.
        assert!(v.functions >= 1, "expected at least one function hook");
        assert!(v.methods >= 1, "expected at least one method hook");
        assert!(
            v.variables >= 1,
            "expected at least one variable hook (local `int x = 0`)"
        );
    }

    #[test]
    fn default_visitor_impl_is_inert_and_safe() {
        // Default AstVisitor impl must be safe to use without overriding hooks.
        struct Inert;
        impl AstVisitor for Inert {}
        let mut p = DartParser::new().unwrap();
        let tree = p.parse("class A {}").unwrap();
        walk_and_visit(tree.root_node(), "class A {}", &mut Inert);
    }
}
