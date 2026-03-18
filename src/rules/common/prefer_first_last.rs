use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests using `.first` / `.last` instead of `[0]` / `[list.length - 1]`.
pub struct PreferFirstLast;

impl Rule for PreferFirstLast {
    fn name(&self) -> &'static str {
        "prefer-first-last"
    }

    fn description(&self) -> &'static str {
        "Prefer using .first and .last instead of index-based access."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "index_expression" || node.kind() == "selector" {
                let text = &source[node.byte_range()];

                if text.ends_with("[0]") {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: "Use '.first' instead of '[0]'.".to_string(),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(node),
                        column: node.start_position().column + 1,
                    });
                }

                if text.contains(".length - 1]") {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: "Use '.last' instead of '[list.length - 1]'.".to_string(),
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
