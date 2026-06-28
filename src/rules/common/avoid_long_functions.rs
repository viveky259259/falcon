use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidLongFunctions {
    max_lines: u32,
}

impl Default for AvoidLongFunctions {
    fn default() -> Self {
        Self { max_lines: 50 }
    }
}

impl Rule for AvoidLongFunctions {
    fn name(&self) -> &'static str {
        "avoid-long-functions"
    }

    fn description(&self) -> &'static str {
        "Functions should not exceed the configured maximum number of lines."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn configure(&mut self, options: &HashMap<String, serde_yaml::Value>) {
        if let Some(val) = options.get("max-lines") {
            if let Some(n) = val.as_u64() {
                self.max_lines = n as u32;
            }
        }
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let functions = find_descendants_by_kind(root, "function_signature");
        let methods = find_descendants_by_kind(root, "method_signature");

        for node in functions.into_iter().chain(methods) {
            if let Some(parent) = node.parent() {
                let text = &source[parent.byte_range()];
                let line_count = text.lines().count() as u32;
                if line_count > self.max_lines {
                    let name =
                        dart_ast::get_declaration_name(node, source).unwrap_or("<anonymous>");
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Function '{}' has {} lines (max: {})",
                            name, line_count, self.max_lines
                        ),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(node),
                        column: 1,
                    });
                }
            }
        }

        issues
    }
}
