use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidUnusedParameters;

impl Rule for AvoidUnusedParameters {
    fn name(&self) -> &'static str {
        "avoid-unused-parameters"
    }

    fn description(&self) -> &'static str {
        "Function parameters should be used in the function body."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let signatures = find_descendants_by_kind(root, "function_signature");
        for sig in signatures {
            let params = dart_ast::get_function_parameters(sig);
            if params.is_empty() {
                continue;
            }

            let body = dart_ast::get_function_body(sig);
            let body = match body {
                Some(b) => b,
                None => continue,
            };

            let mut body_identifiers: HashSet<String> = HashSet::new();
            walk_tree(body, &mut |node| {
                if node.kind() == "identifier" {
                    body_identifiers.insert(source[node.byte_range()].to_string());
                }
            });

            for param in params {
                let mut param_name = None;
                walk_tree(param, &mut |node| {
                    if node.kind() == "identifier" && param_name.is_none() {
                        let text = &source[node.byte_range()];
                        if text != "required" {
                            param_name = Some(text.to_string());
                        }
                    }
                });

                if let Some(name) = param_name {
                    if name.starts_with('_') {
                        continue;
                    }
                    if !body_identifiers.contains(&name) {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: format!(
                                "Parameter '{}' is not used. Prefix with '_' if intentional.",
                                name
                            ),
                            severity: self.default_severity(),
                            file: file.to_path_buf(),
                            line: node_start_line(param),
                            column: param.start_position().column + 1,
                        });
                    }
                }
            }
        }

        issues
    }
}
