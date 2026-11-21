use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use std::path::Path;
use tree_sitter::Node;

/// Detect unnecessary widget rebuilds and overly complex build methods.
pub fn detect_widget_issues(root: Node, source: &str, file: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();
    detect_rebuild_triggers(root, source, file, &mut issues);
    detect_complex_build(root, source, file, &mut issues);
    issues
}

fn detect_rebuild_triggers(root: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    let class_decls = find_descendants_by_kind(root, "class_declaration");

    for class in &class_decls {
        let class_text = class.utf8_text(source.as_bytes()).unwrap_or("");
        if !class_text.contains("State<") && !class_text.contains("Widget") {
            continue;
        }

        let method_sigs = find_descendants_by_kind(*class, "method_signature");
        for sig in &method_sigs {
            if dart_ast::get_declaration_name(*sig, source) != Some("build") {
                continue;
            }
            let Some(body) = method_body(*sig) else {
                continue;
            };
            {
                let body_text = body.utf8_text(source.as_bytes()).unwrap_or("");

                if let Some(call) = direct_set_state_call(body, source) {
                    issues.push(Issue {
                        rule: "widget-rebuild".to_string(),
                        message:
                            "Calling setState inside build method causes infinite rebuild loop."
                                .to_string(),
                        severity: Severity::Error,
                        file: file.to_path_buf(),
                        line: node_start_line(call),
                        column: call.start_position().column + 1,
                    });
                }

                let media_count = body_text.matches("MediaQuery.of(").count()
                    + body_text.matches("Theme.of(").count();
                if media_count > 2 {
                    issues.push(Issue {
                        rule: "widget-rebuild".to_string(),
                        message: format!(
                            "Multiple MediaQuery.of/Theme.of calls ({}) in build method cause rebuilds on every media/theme change. Extract to variables.",
                            media_count
                        ),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: node_start_line(*sig),
                        column: sig.start_position().column + 1,
                    });
                }
            }
        }
    }
}

/// The `function_body` that follows a method signature.
fn method_body(sig: Node) -> Option<Node> {
    let mut next = sig.next_sibling();
    while let Some(n) = next {
        if n.kind() == "function_body" {
            return Some(n);
        }
        next = n.next_sibling();
    }
    None
}

/// A `setState` identifier that runs while `build` runs, i.e. not inside a
/// closure (`onTap: () => setState(...)`) or a local function, which only
/// run later.
fn direct_set_state_call<'a>(body: Node<'a>, source: &str) -> Option<Node<'a>> {
    find_descendants_by_kind(body, "identifier")
        .into_iter()
        .filter(|id| &source[id.byte_range()] == "setState")
        .find(|id| {
            let mut node = id.parent();
            while let Some(n) = node {
                if n.id() == body.id() {
                    return true;
                }
                if matches!(
                    n.kind(),
                    "function_expression" | "local_function_declaration"
                ) {
                    return false;
                }
                node = n.parent();
            }
            true
        })
}

fn detect_complex_build(root: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    let class_decls = find_descendants_by_kind(root, "class_declaration");

    for class in &class_decls {
        let class_text = class.utf8_text(source.as_bytes()).unwrap_or("");

        if !class_text.contains("Widget") && !class_text.contains("State<") {
            continue;
        }

        let methods = find_descendants_by_kind(*class, "method_signature");
        for method_sig in &methods {
            let name = method_sig.utf8_text(source.as_bytes()).unwrap_or("");
            if !name.contains("build") {
                continue;
            }

            if let Some(parent) = method_sig.parent() {
                let body = find_descendants_by_kind(parent, "function_body");
                if let Some(body_node) = body.first() {
                    let body_text = body_node.utf8_text(source.as_bytes()).unwrap_or("");
                    let line_count = body_text.lines().count();

                    if line_count > 80 {
                        issues.push(Issue {
                            rule: "build-method-complexity".to_string(),
                            message: format!(
                                "Build method is {} lines long. Extract widget subtrees into separate widgets for better rebuild performance.",
                                line_count
                            ),
                            severity: Severity::Warning,
                            file: file.to_path_buf(),
                            line: node_start_line(*method_sig),
                            column: method_sig.start_position().column + 1,
                        });
                    }

                    let nesting = max_widget_nesting(*body_node, source);
                    if nesting > 8 {
                        issues.push(Issue {
                            rule: "build-method-complexity".to_string(),
                            message: format!(
                                "Widget nesting depth is {} in build method. Extract deeply nested subtrees to improve readability.",
                                nesting
                            ),
                            severity: Severity::Warning,
                            file: file.to_path_buf(),
                            line: node_start_line(*method_sig),
                            column: method_sig.start_position().column + 1,
                        });
                    }
                }
            }
        }
    }
}

fn max_widget_nesting(node: Node, source: &str) -> usize {
    let mut max_depth = 0;
    count_nesting(node, source, 0, &mut max_depth);
    max_depth
}

fn count_nesting(node: Node, source: &str, depth: usize, max: &mut usize) {
    let kind = node.kind();
    let is_widget_call = kind == "selector" || kind == "identifier";
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");

    let new_depth = if is_widget_call
        && text.len() > 1
        && text.chars().next().is_some_and(|c| c.is_uppercase())
        && (text.contains('(') || node.next_sibling().is_some_and(|n| n.kind() == "selector"))
    {
        depth + 1
    } else {
        depth
    };

    if new_depth > *max {
        *max = new_depth;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count_nesting(child, source, new_depth, max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    fn set_state_issues(src: &str) -> Vec<Issue> {
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(src).unwrap();
        detect_widget_issues(tree.root_node(), src, &PathBuf::from("lib/a.dart"))
            .into_iter()
            .filter(|i| i.message.contains("setState"))
            .collect()
    }

    #[test]
    fn set_state_in_callbacks_is_not_reported() {
        let src = "class S extends State<W> { Widget build(BuildContext c) { \
                   void later() { setState(() {}); } \
                   return X(onTap: () => setState(() {}), onB: () { setState(() {}); }); } }";
        assert!(set_state_issues(src).is_empty());
    }

    #[test]
    fn direct_set_state_in_build_is_reported() {
        let src = "class S extends State<W> {\n  Widget build(BuildContext c) {\n    setState(() {});\n    return X();\n  }\n}";
        let issues = set_state_issues(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 3);
    }

    #[test]
    fn other_methods_named_like_build_are_ignored() {
        let src =
            "class S extends State<W> { Widget _buildRow() { setState(() {}); return X(); } }";
        assert!(set_state_issues(src).is_empty());
    }
}
