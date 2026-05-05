use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidExpandedAsSpacer;

impl Rule for AvoidExpandedAsSpacer {
    fn name(&self) -> &'static str {
        "avoid-expanded-as-spacer"
    }

    fn description(&self) -> &'static str {
        "Use Spacer widget instead of Expanded with an empty Container/SizedBox."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "identifier" {
                let text = &source[node.byte_range()];
                if text == "Expanded" {
                    if let Some(parent) = node.parent() {
                        let parent_text = &source[parent.byte_range()];
                        if (parent_text.contains("Container()")
                            || parent_text.contains("SizedBox()"))
                            && !parent_text.contains("width:")
                            && !parent_text.contains("height:")
                            && !parent_text.contains("child:")
                        {
                            issues.push(Issue {
                                rule: self.name().to_string(),
                                message:
                                    "Use 'Spacer()' instead of 'Expanded' with an empty child."
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
        });

        issues
    }
}
