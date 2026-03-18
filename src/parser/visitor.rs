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
