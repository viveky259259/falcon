use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns when `ref.watch` is used outside a `build` method.
pub struct AvoidWatchOutsideBuild;

impl Rule for AvoidWatchOutsideBuild {
    fn name(&self) -> &'static str {
        "avoid-watch-outside-build"
    }

    fn description(&self) -> &'static str {
        "Avoid using ref.watch outside build methods. Use ref.read instead."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let sigs = find_descendants_by_kind(root, "function_signature");
        for sig in sigs {
            if has_identifier_child(sig, source, "build") {
                continue;
            }

            if let Some(body) = dart_ast::get_function_body(sig) {
                let body_text = &source[body.byte_range()];
                if !body_text.contains("ref.watch") && !body_text.contains("ref?.watch") {
                    continue;
                }

                walk_tree(body, &mut |node| {
                    if node.kind() == "identifier" && &source[node.byte_range()] == "watch" {
                        if let Some(parent) = node.parent() {
                            if parent.kind() == "unconditional_assignable_selector" {
                                if let Some(gp) = parent.parent() {
                                    if let Some(prev_sib) = gp.prev_sibling() {
                                        if &source[prev_sib.byte_range()] == "ref" {
                                            issues.push(Issue {
                                                rule: self.name().to_string(),
                                                message: "Avoid 'ref.watch' outside build(). Use 'ref.read' for one-time reads.".to_string(),
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
                    }
                });
            }
        }

        issues
    }
}

fn has_identifier_child(node: Node, source: &str, name: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" && &source[child.byte_range()] == name {
            return true;
        }
    }
    false
}
