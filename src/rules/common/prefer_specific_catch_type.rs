use crate::config::Severity;
use crate::parser::node_start_line;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct PreferSpecificCatchType;

impl Rule for PreferSpecificCatchType {
    fn name(&self) -> &'static str {
        "prefer-specific-catch-type"
    }

    fn description(&self) -> &'static str {
        "Prefer specific exception types in catch clauses instead of catching all exceptions."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        find_generic_catches(root, source, file, &mut issues);
        issues
    }
}

fn find_generic_catches(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    if node.kind() == "try_statement" {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        for child in &children {
            if child.kind() == "catch_clause" {
                let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                // `catch (e)` without `on SomeType` is a generic catch
                if text.starts_with("catch") && !text.contains("on ") {
                    let has_on_clause = children.iter().any(|sibling| {
                        sibling.kind() == "on_part"
                            && sibling.start_position().row <= child.start_position().row
                    });

                    if !has_on_clause {
                        issues.push(Issue {
                            rule: "prefer-specific-catch-type".to_string(),
                            message: "Generic `catch (e)` — prefer `on SpecificException catch (e)` for targeted error handling.".to_string(),
                            severity: Severity::Warning,
                            file: file.to_path_buf(),
                            line: node_start_line(*child),
                            column: child.start_position().column + 1,
                        });
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_generic_catches(child, source, file, issues);
    }
}
