use crate::config::Severity;
use crate::parser::node_start_line;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct PreferNamedBoolParams;

impl Rule for PreferNamedBoolParams {
    fn name(&self) -> &'static str {
        "prefer-named-boolean-parameters"
    }

    fn description(&self) -> &'static str {
        "Boolean parameters should use named parameters for readability."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        find_positional_bools(root, source, file, &mut issues);
        issues
    }
}

fn find_positional_bools(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    if node.kind() == "formal_parameter_list" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");

        if text.contains('{') {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                find_positional_bools(child, source, file, issues);
            }
            return;
        }

        let bool_count = text
            .split(',')
            .filter(|p| {
                let t = p.trim();
                t.starts_with("bool ") || t.contains("bool ")
            })
            .count();

        if bool_count >= 2 {
            issues.push(Issue {
                rule: "prefer-named-boolean-parameters".to_string(),
                message: format!(
                    "{} positional boolean parameters — use named parameters for readability.",
                    bool_count
                ),
                severity: Severity::Info,
                file: file.to_path_buf(),
                line: node_start_line(node),
                column: node.start_position().column + 1,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_positional_bools(child, source, file, issues);
    }
}
