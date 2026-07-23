use crate::config::Severity;
use crate::parser::{find_descendants_by_kind, node_start_line};
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
            let sig_text = sig.utf8_text(source.as_bytes()).unwrap_or("");
            if !sig_text.contains("build") {
                continue;
            }

            if let Some(parent) = sig.parent() {
                let body_text = parent.utf8_text(source.as_bytes()).unwrap_or("");

                if body_text.contains("setState") {
                    issues.push(Issue {
                        rule: "widget-rebuild".to_string(),
                        message:
                            "Calling setState inside build method causes infinite rebuild loop."
                                .to_string(),
                        severity: Severity::Error,
                        file: file.to_path_buf(),
                        line: node_start_line(*sig),
                        column: sig.start_position().column + 1,
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
