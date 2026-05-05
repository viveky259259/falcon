use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct NoMagicNumbers {
    allowed: Vec<String>,
    context_aware: bool,
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
            context_aware: true,
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
        if let Some(val) = options.get("context_aware") {
            if let Some(b) = val.as_bool() {
                self.context_aware = b;
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
            if self.allowed.iter().any(|a| a == text) {
                return;
            }

            if self.context_aware {
                if let Ok(num) = text.parse::<i64>() {
                    if is_http_status_code(num) && is_in_http_context(node, source) {
                        return;
                    }
                    if is_well_known_constant(num) {
                        return;
                    }
                }
                if is_in_duration_context(node, source) || is_in_edge_insets_context(node, source) {
                    return;
                }
            }

            issues.push(Issue {
                rule: self.name().to_string(),
                message: format!(
                    "Avoid magic number '{}'. Extract to a named constant.",
                    text
                ),
                severity: self.default_severity(),
                file: file.to_path_buf(),
                line: node_start_line(node),
                column: node.start_position().column + 1,
            });
        });

        issues
    }
}

fn is_http_status_code(num: i64) -> bool {
    (100..=599).contains(&num)
}

fn is_well_known_constant(num: i64) -> bool {
    matches!(
        num,
        8 | 16
            | 32
            | 64
            | 128
            | 256
            | 512
            | 1024
            | 2048
            | 4096
            | 10
            | 100
            | 1000
            | 1000000
            | 24
            | 60
            | 360
            | 365
            | 255
    )
}

fn is_in_http_context(node: Node, source: &str) -> bool {
    let mut current = node.parent();
    for _ in 0..6 {
        match current {
            Some(n) => {
                let text = n.utf8_text(source.as_bytes()).unwrap_or("");
                let lower = text.to_lowercase();
                if lower.contains("status")
                    || lower.contains("response")
                    || lower.contains("http")
                    || lower.contains("statuscode")
                {
                    return true;
                }
                current = n.parent();
            }
            None => break,
        }
    }
    false
}

fn is_in_duration_context(node: Node, source: &str) -> bool {
    let mut current = node.parent();
    for _ in 0..4 {
        match current {
            Some(n) => {
                let text = n.utf8_text(source.as_bytes()).unwrap_or("");
                if text.contains("Duration(") || text.contains("Duration.") {
                    return true;
                }
                current = n.parent();
            }
            None => break,
        }
    }
    false
}

fn is_in_edge_insets_context(node: Node, source: &str) -> bool {
    let mut current = node.parent();
    for _ in 0..4 {
        match current {
            Some(n) => {
                let text = n.utf8_text(source.as_bytes()).unwrap_or("");
                if text.contains("EdgeInsets")
                    || text.contains("BorderRadius")
                    || text.contains("Radius")
                {
                    return true;
                }
                current = n.parent();
            }
            None => break,
        }
    }
    false
}
