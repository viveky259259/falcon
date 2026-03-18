use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests having a blank line before return statements when they are not
/// the only statement in a block.
pub struct NewlineBeforeReturn;

impl Rule for NewlineBeforeReturn {
    fn name(&self) -> &'static str {
        "newline-before-return"
    }

    fn description(&self) -> &'static str {
        "Add a blank line before return statements for readability."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        walk_tree(root, &mut |node| {
            if node.kind() != "return_statement" {
                return;
            }

            if let Some(parent) = node.parent() {
                if parent.kind() != "block" {
                    return;
                }

                let mut stmt_count = 0;
                let mut cursor = parent.walk();
                for child in parent.children(&mut cursor) {
                    if child.is_named() && child.kind() != "{" && child.kind() != "}" {
                        stmt_count += 1;
                    }
                }

                if stmt_count <= 1 {
                    return;
                }

                let return_line = node_start_line(node);
                if return_line < 2 {
                    return;
                }

                let prev_line_idx = return_line - 2;
                if prev_line_idx < lines.len() {
                    let prev_line = lines[prev_line_idx].trim();
                    if !prev_line.is_empty() && prev_line != "{" {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: "Add a blank line before the return statement.".to_string(),
                            severity: self.default_severity(),
                            file: file.to_path_buf(),
                            line: return_line,
                            column: node.start_position().column + 1,
                        });
                    }
                }
            }
        });

        issues
    }
}
