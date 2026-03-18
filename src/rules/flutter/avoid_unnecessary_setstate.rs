use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidUnnecessarySetState;

impl Rule for AvoidUnnecessarySetState {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-setstate"
    }

    fn description(&self) -> &'static str {
        "Avoid calling setState in initState, didChangeDependencies, or dispose."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut in_lifecycle_method = false;
        let mut lifecycle_method_name = String::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "method_signature" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = &source[child.byte_range()];
                        if matches!(name, "initState" | "didChangeDependencies" | "dispose" | "deactivate") {
                            in_lifecycle_method = true;
                            lifecycle_method_name = name.to_string();
                        }
                    }
                }
            }

            if in_lifecycle_method && node.kind() == "identifier" {
                let text = &source[node.byte_range()];
                if text == "setState" {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Avoid calling 'setState' in '{}'. This may cause unexpected rebuilds.",
                            lifecycle_method_name
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
