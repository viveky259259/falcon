use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns against throwing a new exception inside a catch block,
/// which discards the original stack trace.
pub struct AvoidThrowInCatch;

impl Rule for AvoidThrowInCatch {
    fn name(&self) -> &'static str {
        "avoid-throw-in-catch-block"
    }

    fn description(&self) -> &'static str {
        "Avoid throwing new exceptions in catch blocks. Use rethrow instead."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "try_statement" {
                return;
            }

            // In tree-sitter-dart, try_statement children are:
            // try, block(try), catch_clause, block(catch), [finally, block(finally)]
            // The catch block is the block that follows a catch_clause.
            let mut found_catch = false;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "catch_clause" {
                    found_catch = true;
                    continue;
                }
                if found_catch && child.kind() == "block" {
                    found_catch = false;
                    walk_tree(child, &mut |inner| {
                        if inner.kind() == "throw_expression" {
                            let text = &source[inner.byte_range()];
                            if !text.trim().starts_with("rethrow") {
                                issues.push(Issue {
                                    rule: self.name().to_string(),
                                    message: "Avoid throwing new exceptions in catch blocks. Use 'rethrow' to preserve the stack trace.".to_string(),
                                    severity: self.default_severity(),
                                    file: file.to_path_buf(),
                                    line: node_start_line(inner),
                                    column: inner.start_position().column + 1,
                                });
                            }
                        }
                    });
                }
            }
        });

        issues
    }
}
