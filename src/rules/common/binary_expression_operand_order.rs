use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests putting literals on the right side of binary expressions
/// for better readability (Yoda conditions).
pub struct BinaryExpressionOperandOrder;

impl Rule for BinaryExpressionOperandOrder {
    fn name(&self) -> &'static str {
        "binary-expression-operand-order"
    }

    fn description(&self) -> &'static str {
        "Place literals on the right side of binary comparisons for readability."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            // tree-sitter-dart uses equality_expression for == and !=
            if node.kind() != "equality_expression" && node.kind() != "binary_expression" {
                return;
            }

            if node.child_count() < 3 {
                return;
            }

            let operator = node.child(1);
            let operator_text = operator.map(|o| &source[o.byte_range()]).unwrap_or("");

            if !matches!(operator_text, "==" | "!=") {
                return;
            }

            if let Some(left) = node.child(0) {
                if is_literal(left) {
                    if let Some(right) = node.child(2) {
                        if !is_literal(right) {
                            issues.push(Issue {
                                rule: self.name().to_string(),
                                message: "Place the literal on the right side of the comparison.".to_string(),
                                severity: self.default_severity(),
                                file: file.to_path_buf(),
                                line: node_start_line(node),
                                column: node.start_position().column + 1,
                            });
                        }
                    }
                }
            }
        });

        issues
    }
}

fn is_literal(node: Node) -> bool {
    matches!(
        node.kind(),
        "decimal_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_integer_literal"
            | "string_literal"
            | "null_literal"
            | "true"
            | "false"
    )
}
