use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns when a switch statement on an enum-like value doesn't have a default case
/// and might be missing enum constants.
pub struct AvoidMissingEnumConstantInMap;

impl Rule for AvoidMissingEnumConstantInMap {
    fn name(&self) -> &'static str {
        "avoid-missing-enum-constant-in-map"
    }

    fn description(&self) -> &'static str {
        "Ensure switch statements on enum values handle all cases or have a default."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "switch_statement" {
                return;
            }

            let text = &source[node.byte_range()];
            let has_default = text.contains("default:");

            if !has_default {
                let mut case_count = 0u32;
                let mut inner = node.walk();
                for child in node.children(&mut inner) {
                    if child.kind() == "switch_block" {
                        let mut block_cursor = child.walk();
                        for block_child in child.children(&mut block_cursor) {
                            if block_child.kind() == "switch_label" {
                                case_count += 1;
                            }
                        }
                    }
                }

                if case_count > 0 && case_count <= 10 {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Switch statement has {} cases but no default. Ensure all enum values are handled.",
                            case_count
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
