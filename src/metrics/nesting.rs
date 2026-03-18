use tree_sitter::Node;

/// Calculates the maximum nesting level within a function body.
pub fn calculate(node: Node) -> u32 {
    let mut max_depth = 0u32;
    walk_nesting(node, 0, &mut max_depth);
    max_depth
}

fn walk_nesting(node: Node, current_depth: u32, max_depth: &mut u32) {
    let new_depth = if is_nesting_node(node) {
        let d = current_depth + 1;
        if d > *max_depth {
            *max_depth = d;
        }
        d
    } else {
        current_depth
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_nesting(child, new_depth, max_depth);
    }
}

fn is_nesting_node(node: Node) -> bool {
    matches!(
        node.kind(),
        "if_statement"
            | "for_statement"
            | "while_statement"
            | "do_statement"
            | "switch_statement"
            | "try_statement"
            | "catch_clause"
            | "conditional_expression"
    )
}
