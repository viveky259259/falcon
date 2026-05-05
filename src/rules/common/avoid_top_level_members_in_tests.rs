use crate::config::Severity;
use crate::parser::{dart_ast, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns about top-level classes/functions in test files (except main).
pub struct AvoidTopLevelMembersInTests;

impl Rule for AvoidTopLevelMembersInTests {
    fn name(&self) -> &'static str {
        "avoid-top-level-members-in-tests"
    }

    fn description(&self) -> &'static str {
        "Avoid declaring top-level classes/functions in test files."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let file_str = file.to_string_lossy();
        if !file_str.contains("_test.dart") && !file_str.contains("/test/") {
            return issues;
        }

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if dart_ast::is_class_like(child) {
                let name = dart_ast::get_declaration_name(child, source).unwrap_or("<unknown>");
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Avoid declaring top-level class '{}' in test files. Move to a helper file.", name),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(child),
                    column: child.start_position().column + 1,
                });
            }

            if child.kind() == "function_signature" || child.kind() == "method_signature" {
                let name = dart_ast::get_declaration_name(child, source).unwrap_or("");
                if name != "main" {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Avoid declaring top-level function '{}' in test files.",
                            name
                        ),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(child),
                        column: child.start_position().column + 1,
                    });
                }
            }
        }

        issues
    }
}
