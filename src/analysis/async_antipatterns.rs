use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use std::path::Path;
use tree_sitter::Node;

/// Detect common async/await anti-patterns.
pub fn detect_async_antipatterns(root: Node, source: &str, file: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();
    detect_unawaited_futures(root, source, file, &mut issues);
    detect_async_void(root, source, file, &mut issues);
    detect_sequential_awaits(root, source, file, &mut issues);
    detect_await_in_loop(root, source, file, &mut issues);
    issues
}

fn detect_unawaited_futures(root: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    walk_tree(root, &mut |node| {
        if node.kind() != "expression_statement" {
            return;
        }

        let text = node.utf8_text(source.as_bytes()).unwrap_or("").trim();

        let is_future_call = (text.contains(".then(")
            || text.contains(".whenComplete(")
            || text.contains(".catchError("))
            && !text.starts_with("await")
            && !text.starts_with("unawaited(");

        if is_future_call {
            issues.push(Issue {
                rule: "unawaited-future".to_string(),
                message: "Future chain without await — errors will be silently swallowed. Use await or unawaited().".to_string(),
                severity: Severity::Warning,
                file: file.to_path_buf(),
                line: node_start_line(node),
                column: node.start_position().column + 1,
            });
        }
    });
}

fn detect_async_void(root: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    walk_tree(root, &mut |node| {
        if node.kind() != "function_signature" && node.kind() != "method_signature" {
            return;
        }

        let text = node.utf8_text(source.as_bytes()).unwrap_or("");

        if !text.contains("async") {
            if let Some(parent) = node.parent() {
                let parent_text = parent.utf8_text(source.as_bytes()).unwrap_or("");
                if !parent_text.contains("async") {
                    return;
                }
            } else {
                return;
            }
        }

        let has_void_return = text.trim().starts_with("void ");
        let func_name = extract_name(node, source);

        if has_void_return && !is_lifecycle_method(&func_name) && !is_event_handler(&func_name) {
            issues.push(Issue {
                rule: "async-void".to_string(),
                message: format!(
                    "Async function '{}' returns void — exceptions will be unhandled. Return Future<void> instead.",
                    func_name
                ),
                severity: Severity::Warning,
                file: file.to_path_buf(),
                line: node_start_line(node),
                column: node.start_position().column + 1,
            });
        }
    });
}

fn detect_sequential_awaits(root: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    walk_tree(root, &mut |node| {
        if node.kind() != "block" {
            return;
        }

        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        let mut consecutive_awaits = 0;
        let mut first_await_line = 0;

        for child in &children {
            let text = child.utf8_text(source.as_bytes()).unwrap_or("");
            let trimmed = text.trim();

            if trimmed.starts_with("await ") || trimmed.contains("= await ") {
                if consecutive_awaits == 0 {
                    first_await_line = node_start_line(*child);
                }
                consecutive_awaits += 1;
            } else if child.kind() != "comment" && !trimmed.is_empty() {
                if consecutive_awaits >= 3 {
                    let independent =
                        check_independent_awaits(&children, source, consecutive_awaits);
                    if independent {
                        issues.push(Issue {
                            rule: "sequential-awaits".to_string(),
                            message: format!(
                                "{} sequential awaits that appear independent — consider using Future.wait() for parallel execution.",
                                consecutive_awaits
                            ),
                            severity: Severity::Info,
                            file: file.to_path_buf(),
                            line: first_await_line,
                            column: 1,
                        });
                    }
                }
                consecutive_awaits = 0;
            }
        }

        if consecutive_awaits >= 3 {
            let independent = check_independent_awaits(&children, source, consecutive_awaits);
            if independent {
                issues.push(Issue {
                    rule: "sequential-awaits".to_string(),
                    message: format!(
                        "{} sequential awaits that appear independent — consider using Future.wait() for parallel execution.",
                        consecutive_awaits
                    ),
                    severity: Severity::Info,
                    file: file.to_path_buf(),
                    line: first_await_line,
                    column: 1,
                });
            }
        }
    });
}

fn check_independent_awaits(children: &[Node], source: &str, _count: usize) -> bool {
    let mut assigned_vars = Vec::new();

    for child in children {
        let text = child.utf8_text(source.as_bytes()).unwrap_or("");
        let trimmed = text.trim();

        if trimmed.contains("= await ") {
            if let Some(var_part) = trimmed.split("= await").next() {
                let var = var_part.trim().split_whitespace().last().unwrap_or("");
                for prev_var in &assigned_vars {
                    if trimmed.contains(prev_var) {
                        return false;
                    }
                }
                assigned_vars.push(var.to_string());
            }
        }
    }

    true
}

fn detect_await_in_loop(root: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    walk_tree(root, &mut |node| {
        let is_loop = matches!(
            node.kind(),
            "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
        );

        if !is_loop {
            return;
        }

        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        let await_count = text.matches("await ").count();

        if await_count > 0 {
            issues.push(Issue {
                rule: "await-in-loop".to_string(),
                message: format!(
                    "{} await expression(s) inside a loop — consider batching with Future.wait() for better performance.",
                    await_count
                ),
                severity: Severity::Info,
                file: file.to_path_buf(),
                line: node_start_line(node),
                column: node.start_position().column + 1,
            });
        }
    });
}

fn extract_name(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child
                .utf8_text(source.as_bytes())
                .unwrap_or("unknown")
                .to_string();
        }
    }
    "anonymous".to_string()
}

fn is_lifecycle_method(name: &str) -> bool {
    matches!(
        name,
        "initState"
            | "dispose"
            | "didChangeDependencies"
            | "didUpdateWidget"
            | "deactivate"
            | "main"
    )
}

fn is_event_handler(name: &str) -> bool {
    name.starts_with("on")
        || name.starts_with("_on")
        || name.starts_with("handle")
        || name.starts_with("_handle")
}
