use crate::config::Severity;
use crate::metrics::parameters;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidLongParameterList {
    max_params: u32,
}

impl Default for AvoidLongParameterList {
    fn default() -> Self {
        Self { max_params: 4 }
    }
}

impl Rule for AvoidLongParameterList {
    fn name(&self) -> &'static str {
        "avoid-long-parameter-list"
    }

    fn description(&self) -> &'static str {
        "Functions should not have too many parameters. Consider using a parameter object."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn configure(&mut self, options: &HashMap<String, serde_yaml::Value>) {
        if let Some(val) = options.get("max-params") {
            if let Some(n) = val.as_u64() {
                self.max_params = n as u32;
            }
        }
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let functions = find_descendants_by_kind(root, "function_signature");
        let methods = find_descendants_by_kind(root, "method_signature");

        for node in functions.into_iter().chain(methods.into_iter()) {
            let count = parameters::count(node);
            if count > self.max_params {
                let name = dart_ast::get_declaration_name(node, source).unwrap_or("<anonymous>");
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!(
                        "Function '{}' has {} parameters (max: {})",
                        name, count, self.max_params
                    ),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(node),
                    column: 1,
                });
            }
        }

        issues
    }
}
