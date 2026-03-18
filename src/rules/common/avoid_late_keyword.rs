use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidLateKeyword;

impl Rule for AvoidLateKeyword {
    fn name(&self) -> &'static str {
        "avoid-late-keyword"
    }

    fn description(&self) -> &'static str {
        "Avoid using 'late' keyword. Prefer nullable types or factory constructors."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "late" || (node.kind() == "identifier" && &source[node.byte_range()] == "late") {
                if let Some(parent) = node.parent() {
                    if parent.kind() != "identifier" {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: "Avoid using 'late'. Prefer nullable types or initialization in constructors.".to_string(),
                            severity: self.default_severity(),
                            file: file.to_path_buf(),
                            line: node_start_line(node),
                            column: node.start_position().column + 1,
                        });
                    }
                }
            }
        });

        issues
    }
}
