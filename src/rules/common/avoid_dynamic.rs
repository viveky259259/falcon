use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn name(&self) -> &'static str {
        "avoid-dynamic"
    }

    fn description(&self) -> &'static str {
        "Avoid using 'dynamic' type. Prefer explicit types for better type safety."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "type_identifier" {
                let text = &source[node.byte_range()];
                if text == "dynamic" {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: "Avoid using 'dynamic' type. Prefer explicit types.".to_string(),
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
