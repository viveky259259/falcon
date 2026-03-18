use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests using `ref.read` instead of `ref.watch` when calling methods
/// on a provider (not subscribing to state changes).
pub struct PreferRefReadForMethods;

impl Rule for PreferRefReadForMethods {
    fn name(&self) -> &'static str {
        "prefer-ref-read-for-methods"
    }

    fn description(&self) -> &'static str {
        "Use ref.read when calling provider methods, not ref.watch."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "selector" {
                return;
            }

            let text = &source[node.byte_range()];
            if !text.contains("ref.watch") {
                return;
            }

            if text.contains(").") {
                let after_paren = text.split(").").nth(1).unwrap_or("");
                let method_call = after_paren.split('(').next().unwrap_or("").trim();
                if !method_call.is_empty() && method_call != "state" && method_call != "notifier" {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Use 'ref.read' instead of 'ref.watch' when calling method '{}'.",
                            method_call
                        ),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(node),
                        column: node.start_position().column + 1,
                    });
                }
            }
        });

        issues
    }
}
