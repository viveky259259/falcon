use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

const CONST_ELIGIBLE_WIDGETS: &[&str] = &[
    "Text",
    "Icon",
    "SizedBox",
    "Spacer",
    "Divider",
    "EdgeInsets",
    "BorderRadius",
    "Duration",
    "Offset",
    "Size",
    "Radius",
    "Color",
];

pub struct PreferConstConstructors;

impl Rule for PreferConstConstructors {
    fn name(&self) -> &'static str {
        "prefer-const-constructors"
    }

    fn description(&self) -> &'static str {
        "Prefer using const constructors when all arguments are const-evaluable."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "identifier" || node.kind() == "type_identifier" {
                let text = &source[node.byte_range()];

                if !CONST_ELIGIBLE_WIDGETS.contains(&text) {
                    return;
                }

                if let Some(parent) = node.parent() {
                    let parent_text = &source[parent.byte_range()];
                    if parent_text.trim_start().starts_with("const") {
                        return;
                    }

                    if let Some(grandparent) = parent.parent() {
                        let gp_text = &source[grandparent.byte_range()];
                        if gp_text.trim_start().starts_with("const") {
                            return;
                        }
                    }

                    if has_only_literal_args(parent, source) {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: format!(
                                "Consider using 'const {}(...)' for better performance.",
                                text
                            ),
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

fn has_only_literal_args(node: Node, source: &str) -> bool {
    let text = &source[node.byte_range()];
    !text.contains("widget.") && !text.contains("context.") && !text.contains("this.")
}
