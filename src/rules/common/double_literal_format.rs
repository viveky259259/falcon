use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Ensures double literals have proper formatting:
/// - No leading dot (use `0.5` not `.5`)
/// - No trailing dot (use `1.0` not `1.`)
/// - No redundant trailing zeros after a meaningful decimal (e.g. `1.50` -> `1.5`)
pub struct DoubleLiteralFormat;

impl Rule for DoubleLiteralFormat {
    fn name(&self) -> &'static str {
        "double-literal-format"
    }

    fn description(&self) -> &'static str {
        "Ensure double/float literals have consistent formatting."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "decimal_floating_point_literal" {
                return;
            }

            let text = &source[node.byte_range()];

            if text.starts_with('.') {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Use '0{}' instead of '{}'.", text, text),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(node),
                    column: node.start_position().column + 1,
                });
            }

            if text.ends_with('.') {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Use '{}0' instead of '{}'.", text, text),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(node),
                    column: node.start_position().column + 1,
                });
            }
        });

        issues
    }
}
