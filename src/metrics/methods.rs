use tree_sitter::Node;

/// Counts the number of methods in a class declaration node.
pub fn count(class_node: Node) -> usize {
    let methods = crate::parser::dart_ast::get_class_methods(class_node);
    methods.len()
}
