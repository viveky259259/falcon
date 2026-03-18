use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct PreferCorrectIdentifierLength {
    min_length: usize,
    max_length: usize,
    exceptions: Vec<String>,
}

impl Default for PreferCorrectIdentifierLength {
    fn default() -> Self {
        Self {
            min_length: 3,
            max_length: 40,
            exceptions: vec![
                "i".into(), "j".into(), "k".into(), "x".into(), "y".into(),
                "z".into(), "e".into(), "v".into(), "a".into(), "b".into(),
                "id".into(), "db".into(), "io".into(), "ui".into(), "ok".into(),
            ],
        }
    }
}

impl Rule for PreferCorrectIdentifierLength {
    fn name(&self) -> &'static str {
        "prefer-correct-identifier-length"
    }

    fn description(&self) -> &'static str {
        "Identifiers should have a reasonable length (not too short or too long)."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn configure(&mut self, options: &HashMap<String, serde_yaml::Value>) {
        if let Some(val) = options.get("min-length") {
            if let Some(n) = val.as_u64() {
                self.min_length = n as usize;
            }
        }
        if let Some(val) = options.get("max-length") {
            if let Some(n) = val.as_u64() {
                self.max_length = n as usize;
            }
        }
        if let Some(val) = options.get("exceptions") {
            if let Some(seq) = val.as_sequence() {
                self.exceptions = seq.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            }
        }
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "identifier" {
                return;
            }

            if let Some(parent) = node.parent() {
                if !matches!(parent.kind(),
                    "function_signature" | "initialized_identifier" | "formal_parameter"
                    | "class_declaration" | "enum_declaration" | "mixin_declaration"
                    | "initialized_variable_definition"
                ) {
                    return;
                }
            }

            let text = &source[node.byte_range()];

            if text.starts_with('_') {
                return;
            }

            if self.exceptions.iter().any(|e| e == text) {
                return;
            }

            if text.len() < self.min_length {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Identifier '{}' is too short ({} chars, min: {}).", text, text.len(), self.min_length),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(node),
                    column: node.start_position().column + 1,
                });
            } else if text.len() > self.max_length {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Identifier '{}' is too long ({} chars, max: {}).", text, text.len(), self.max_length),
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
