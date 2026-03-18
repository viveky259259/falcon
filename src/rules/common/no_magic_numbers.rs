use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct NoMagicNumbers {
    allowed: Vec<String>,
}

impl Default for NoMagicNumbers {
    fn default() -> Self {
        Self {
            allowed: vec![
                "0".to_string(),
                "1".to_string(),
                "-1".to_string(),
                "2".to_string(),
                "0.0".to_string(),
                "1.0".to_string(),
            ],
        }
    }
}

impl Rule for NoMagicNumbers {
    fn name(&self) -> &'static str {
        "no-magic-numbers"
    }

    fn description(&self) -> &'static str {
        "Avoid magic numbers in code. Extract them into named constants."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn configure(&mut self, options: &HashMap<String, serde_yaml::Value>) {
        if let Some(val) = options.get("allowed") {
            if let Some(seq) = val.as_sequence() {
                self.allowed = seq
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }
        }
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            let is_numeric = matches!(
                node.kind(),
                "decimal_integer_literal"
                    | "decimal_floating_point_literal"
                    | "hex_integer_literal"
            );

            if !is_numeric {
                return;
            }

            if let Some(parent) = node.parent() {
                if parent.kind() == "initialized_variable_definition" {
                    if let Some(grandparent) = parent.parent() {
                        let gp_text = &source[grandparent.byte_range()];
                        if gp_text.trim_start().starts_with("const")
                            || gp_text.trim_start().starts_with("final")
                        {
                            return;
                        }
                    }
                }
                if parent.kind() == "enum_constant" {
                    return;
                }
            }

            let text = &source[node.byte_range()];
            if !self.allowed.iter().any(|a| a == text) {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Avoid magic number '{}'. Extract to a named constant.", text),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(node),
                    column: node.start_position().column + 1,
                });
            }
        });

        issues
    }
}
