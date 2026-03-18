use tree_sitter::Node;

/// Counts the number of formal parameters for a function/method signature node.
pub fn count(node: Node) -> u32 {
    let mut count = 0u32;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "formal_parameter_list" {
            count += count_params_in_list(child);
            break;
        }
    }
    count
}

fn count_params_in_list(node: Node) -> u32 {
    let mut count = 0u32;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "formal_parameter" => count += 1,
            "optional_formal_parameters" | "named_formal_parameters" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "formal_parameter" {
                        count += 1;
                    }
                }
            }
            _ => {}
        }
    }
    count
}
