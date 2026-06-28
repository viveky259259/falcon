use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns about unnecessary type assertions like `x is String` when x is already typed as String,
/// or `x is Object` / `x is dynamic` which are always true.
pub struct AvoidUnnecessaryTypeAssertions;

impl Rule for AvoidUnnecessaryTypeAssertions {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-type-assertions"
    }

    fn description(&self) -> &'static str {
        "Avoid unnecessary type assertions that are always true or always false."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            let text = &source[node.byte_range()];

            if (node.kind() == "is_operator"
                || (node.kind() == "binary_expression" && text.contains(" is ")))
                && (text.ends_with("Object") || text.ends_with("dynamic"))
            {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: "Unnecessary type check. Every non-null value is an Object."
                        .to_string(),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(node),
                    column: node.start_position().column + 1,
                });
            }

            if node.kind() == "as_expression" && text.ends_with("dynamic") {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: "Unnecessary cast to dynamic.".to_string(),
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
