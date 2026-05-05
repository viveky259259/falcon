use crate::config::Severity;
use crate::metrics::nesting;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidNestedConditionals {
    max_depth: u32,
}

impl Default for AvoidNestedConditionals {
    fn default() -> Self {
        Self { max_depth: 5 }
    }
}

impl Rule for AvoidNestedConditionals {
    fn name(&self) -> &'static str {
        "avoid-nested-conditionals"
    }

    fn description(&self) -> &'static str {
        "Avoid deeply nested conditional statements. Consider extracting into separate functions."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn configure(&mut self, options: &HashMap<String, serde_yaml::Value>) {
        if let Some(val) = options.get("max-depth") {
            if let Some(n) = val.as_u64() {
                self.max_depth = n as u32;
            }
        }
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let functions = find_descendants_by_kind(root, "function_signature");
        let methods = find_descendants_by_kind(root, "method_signature");

        for node in functions.into_iter().chain(methods.into_iter()) {
            if let Some(body) = dart_ast::get_function_body(node) {
                let depth = nesting::calculate(body);
                if depth > self.max_depth {
                    let name =
                        dart_ast::get_declaration_name(node, source).unwrap_or("<anonymous>");
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Function '{}' has nesting depth of {} (max: {})",
                            name, depth, self.max_depth
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
