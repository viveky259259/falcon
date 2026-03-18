use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidCascadeAfterIfNull;

impl Rule for AvoidCascadeAfterIfNull {
    fn name(&self) -> &'static str {
        "avoid-cascade-after-if-null"
    }

    fn description(&self) -> &'static str {
        "Avoid using cascade operator after if-null (??) operator."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "cascade_section" {
                if let Some(parent) = node.parent() {
                    let parent_text = &source[parent.byte_range()];
                    if parent_text.contains("??") {
                        let cascade_start = node.byte_range().start;
                        let qq_pos = parent_text.find("??");
                        if let Some(pos) = qq_pos {
                            let abs_pos = parent.byte_range().start + pos;
                            if cascade_start > abs_pos {
                                issues.push(Issue {
                                    rule: self.name().to_string(),
                                    message: "Avoid cascade (..) after if-null (??) operator. The cascade applies to the right operand only.".to_string(),
                                    severity: self.default_severity(),
                                    file: file.to_path_buf(),
                                    line: node_start_line(node),
                                    column: node.start_position().column + 1,
                                });
                            }
                        }
                    }
                }
            }
        });

        issues
    }
}
