use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns about unnecessary type casts (e.g. `x as Object`, `x as dynamic`).
pub struct AvoidUnnecessaryTypeCasts;

impl Rule for AvoidUnnecessaryTypeCasts {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-type-casts"
    }

    fn description(&self) -> &'static str {
        "Avoid unnecessary type casts that have no effect."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "as_expression" {
                return;
            }

            let text = &source[node.byte_range()];
            let parts: Vec<&str> = text.splitn(2, " as ").collect();
            if parts.len() != 2 {
                return;
            }

            let target_type = parts[1].trim();
            if target_type == "Object" || target_type == "Object?" {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: "Unnecessary cast to Object.".to_string(),
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
