use tree_sitter::Node;

/// Calculates cyclomatic complexity for a function body.
/// CC = 1 + number of decision points (if, for, while, do, case, catch, &&, ||, ?:)
pub fn calculate(node: Node, source: &str) -> u32 {
    let mut complexity = 1u32;
    count_decisions(node, source, &mut complexity);
    complexity
}

fn count_decisions(node: Node, source: &str, complexity: &mut u32) {
    match node.kind() {
        "if_statement" | "for_statement" | "while_statement" | "do_statement" => {
            *complexity += 1;
        }
        "switch_expression_case" => {
            *complexity += 1;
        }
        "catch_clause" => {
            *complexity += 1;
        }
        "conditional_expression" => {
            *complexity += 1;
        }
        "binary_expression" => {
            let op_text = &source[node.byte_range()];
            if op_text.contains("&&") || op_text.contains("||") {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "&&" || child.kind() == "||" {
                        *complexity += 1;
                    }
                }
                if !node.children(&mut node.walk()).any(|c| c.kind() == "&&" || c.kind() == "||") {
                    if op_text.contains("&&") {
                        *complexity += 1;
                    } else if op_text.contains("||") {
                        *complexity += 1;
                    }
                }
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count_decisions(child, source, complexity);
    }
}
