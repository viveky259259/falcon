use tree_sitter::Node;

/// Calculate cognitive complexity for a function/method node.
/// Unlike cyclomatic complexity, this weights nested structures
/// more heavily and ignores boolean short-circuits.
///
/// Increments (+1 for each):
///   - if, else if, else
///   - for, while, do-while, for-in
///   - catch
///   - switch
///   - break/continue to label
///   - ternary operator
///   - logical sequence change (&&/|| alternation)
///
/// Nesting increment (+1 per nesting level):
///   Applied to if, for, while, do, catch, switch, ternary, lambda
pub fn calculate_cognitive_complexity(node: Node, source: &str) -> u32 {
    let mut complexity = 0;
    let nesting = 0;
    walk_cognitive(node, source, nesting, &mut complexity);
    complexity
}

fn walk_cognitive(
    node: Node,
    source: &str,
    nesting: u32,
    complexity: &mut u32,
) {
    let kind = node.kind();

    let (increment, increases_nesting) = match kind {
        "if_statement" => {
            if is_else_if(node) {
                (1, false)
            } else {
                (1 + nesting, true)
            }
        }
        "for_statement" | "for_in_statement" | "while_statement" | "do_statement" => {
            (1 + nesting, true)
        }
        "catch_clause" => (1 + nesting, true),
        "switch_statement" => (1 + nesting, true),
        "conditional_expression" => (1 + nesting, true),
        "break_statement" | "continue_statement" => {
            let has_label = node.child_count() > 1;
            if has_label { (1, false) } else { (0, false) }
        }
        "binary_expression" => {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("");
            if text.contains("&&") || text.contains("||") {
                (count_logical_sequence_changes(node, source), false)
            } else {
                (0, false)
            }
        }
        _ => (0, false),
    };

    *complexity += increment;

    let next_nesting = if increases_nesting {
        nesting + 1
    } else {
        nesting
    };

    let child_nesting = if kind == "function_body"
        || kind == "function_expression_body"
        || kind == "function_expression"
    {
        nesting + 1
    } else {
        next_nesting
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "else" {
            *complexity += 1;
            walk_cognitive(child, source, next_nesting, complexity);
        } else {
            walk_cognitive(child, source, child_nesting, complexity);
        }
    }
}

fn is_else_if(node: Node) -> bool {
    if let Some(parent) = node.parent() {
        if parent.kind() == "else" {
            return true;
        }
    }
    false
}

fn count_logical_sequence_changes(node: Node, source: &str) -> u32 {
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");

    let mut changes = 0u32;
    let mut last_op: Option<&str> = None;

    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len().saturating_sub(1) {
        let pair = &text[i..i + 2];
        if pair == "&&" || pair == "||" {
            match last_op {
                None => changes += 1,
                Some(prev) if prev != pair => changes += 1,
                _ => {}
            }
            last_op = Some(if pair == "&&" { "&&" } else { "||" });
            i += 2;
        } else {
            i += 1;
        }
    }

    changes
}

/// Calculate cognitive complexity for all functions in a file.
pub fn file_cognitive_complexity(root: Node, source: &str) -> Vec<(String, u32, usize)> {
    let mut results = Vec::new();
    collect_function_complexity(root, source, &mut results);
    results
}

fn collect_function_complexity(
    node: Node,
    source: &str,
    results: &mut Vec<(String, u32, usize)>,
) {
    let kind = node.kind();

    if kind == "function_signature"
        || kind == "method_signature"
    {
        if let Some(parent) = node.parent() {
            let func_name = extract_func_name(node, source);
            let body = find_function_body(parent);
            if let Some(body_node) = body {
                let complexity = calculate_cognitive_complexity(body_node, source);
                let line = node.start_position().row + 1;
                results.push((func_name, complexity, line));
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_function_complexity(child, source, results);
    }
}

fn extract_func_name(sig_node: Node, source: &str) -> String {
    let mut cursor = sig_node.walk();
    for child in sig_node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child.utf8_text(source.as_bytes()).unwrap_or("unknown").to_string();
        }
    }
    "anonymous".to_string()
}

fn find_function_body(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_body" || child.kind() == "block" {
            return Some(child);
        }
    }
    None
}
