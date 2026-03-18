use crate::config::Severity;
use crate::parser::node_start_line;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidPrintInProduction;

impl Rule for AvoidPrintInProduction {
    fn name(&self) -> &'static str {
        "avoid-print-in-production"
    }

    fn description(&self) -> &'static str {
        "Avoid using print() in production code. Use a logging framework instead."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let file_str = file.to_string_lossy();
        if file_str.contains("_test.dart") || file_str.contains("/test/") {
            return Vec::new();
        }

        let mut issues = Vec::new();
        find_prints(root, source, file, &mut issues);
        issues
    }
}

fn find_prints(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    if node.kind() == "expression_statement" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("").trim();
        if text.starts_with("print(") || text.starts_with("debugPrint(") {
            issues.push(Issue {
                rule: "avoid-print-in-production".to_string(),
                message: "print()/debugPrint() found in production code — use a logger."
                    .to_string(),
                severity: Severity::Warning,
                file: file.to_path_buf(),
                line: node_start_line(node),
                column: node.start_position().column + 1,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_prints(child, source, file, issues);
    }
}
