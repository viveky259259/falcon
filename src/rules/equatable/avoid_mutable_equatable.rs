use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Classes extending Equatable should be immutable (all fields final).
pub struct AvoidMutableEquatable;

impl Rule for AvoidMutableEquatable {
    fn name(&self) -> &'static str {
        "avoid-mutable-equatable"
    }

    fn description(&self) -> &'static str {
        "Classes extending Equatable should be immutable."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let classes = find_descendants_by_kind(root, "class_declaration");
        for class in classes {
            let class_text = &source[class.byte_range()];
            if !class_text.contains("Equatable") {
                continue;
            }

            let name = dart_ast::get_declaration_name(class, source).unwrap_or("<unknown>");

            let mut cursor = class.walk();
            for child in class.children(&mut cursor) {
                if child.kind() != "class_body" {
                    continue;
                }

                let mut body_cursor = child.walk();
                for member in child.children(&mut body_cursor) {
                    if member.kind() != "class_member" {
                        continue;
                    }

                    let mut mc = member.walk();
                    for mc_child in member.children(&mut mc) {
                        if mc_child.kind() == "declaration" {
                            let decl = &source[mc_child.byte_range()];
                            let trimmed = decl.trim();
                            if !trimmed.starts_with("final")
                                && !trimmed.starts_with("static")
                                && !trimmed.starts_with("const")
                            {
                                if !dart_ast::is_function_like(mc_child) {
                                    issues.push(Issue {
                                        rule: self.name().to_string(),
                                        message: format!("Equatable class '{}' has mutable field. Make it final.", name),
                                        severity: self.default_severity(),
                                        file: file.to_path_buf(),
                                        line: node_start_line(mc_child),
                                        column: mc_child.start_position().column + 1,
                                    });
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        issues
    }
}
