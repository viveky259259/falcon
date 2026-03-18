use crate::config::Severity;
use crate::parser::node_start_line;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidGlobalState;

impl Rule for AvoidGlobalState {
    fn name(&self) -> &'static str {
        "avoid-global-state"
    }

    fn description(&self) -> &'static str {
        "Avoid mutable global state. Top-level mutable variables make code harder to test and reason about."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, _source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let mut cursor = root.walk();
        let children: Vec<Node> = root.children(&mut cursor).collect();

        let mut i = 0;
        while i < children.len() {
            let child = children[i];

            if child.kind() == "var" {
                if let Some(next) = children.get(i + 1) {
                    if next.kind() == "initialized_identifier_list" {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: "Avoid mutable top-level variables. Use 'final' or 'const'."
                                .to_string(),
                            severity: self.default_severity(),
                            file: file.to_path_buf(),
                            line: node_start_line(child),
                            column: 1,
                        });
                    }
                }
            }

            i += 1;
        }

        issues
    }
}
