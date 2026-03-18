use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidDoubleNegation;

impl Rule for AvoidDoubleNegation {
    fn name(&self) -> &'static str {
        "avoid-double-negation"
    }

    fn description(&self) -> &'static str {
        "Avoid double negation (!!). It reduces readability."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "prefix_expression" {
                let text = &source[node.byte_range()].trim_start();
                if text.starts_with('!') {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "prefix_expression" {
                            let inner = &source[child.byte_range()].trim_start();
                            if inner.starts_with('!') {
                                issues.push(Issue {
                                    rule: self.name().to_string(),
                                    message: "Avoid double negation. Simplify the expression."
                                        .to_string(),
                                    severity: self.default_severity(),
                                    file: file.to_path_buf(),
                                    line: node_start_line(node),
                                    column: node.start_position().column + 1,
                                });
                            }
                        }
                    }
                }
            }
        });

        issues
    }
}
