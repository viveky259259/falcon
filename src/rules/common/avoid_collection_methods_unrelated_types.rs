use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns when using collection lookup methods (contains, remove, indexOf)
/// with literal arguments that appear to be a type mismatch
/// (e.g. passing an int literal to a String list's contains).
pub struct AvoidCollectionMethodsUnrelatedTypes;

impl Rule for AvoidCollectionMethodsUnrelatedTypes {
    fn name(&self) -> &'static str {
        "avoid-collection-methods-with-unrelated-types"
    }

    fn description(&self) -> &'static str {
        "Avoid using collection methods like contains/remove with arguments of unrelated types."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "identifier" {
                return;
            }
            let text = &source[node.byte_range()];
            if !matches!(text, "contains" | "remove" | "indexOf" | "lookup") {
                return;
            }

            if let Some(parent) = node.parent() {
                if parent.kind() == "unconditional_assignable_selector"
                    || parent.kind() == "selector"
                {
                    let parent_text = &source[parent.byte_range()];
                    if has_mismatched_literal_arg(parent_text) {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: format!(
                                "Method '{}' may be called with an argument of unrelated type.",
                                text
                            ),
                            severity: self.default_severity(),
                            file: file.to_path_buf(),
                            line: node_start_line(node),
                            column: node.start_position().column + 1,
                        });
                    }
                }
            }
        });

        issues
    }
}

fn has_mismatched_literal_arg(text: &str) -> bool {
    if let Some(start) = text.find('(') {
        if let Some(end) = text.rfind(')') {
            let arg = text[start + 1..end].trim();
            if arg.is_empty() {
                return false;
            }
            let is_num = arg.parse::<f64>().is_ok();
            let is_string = arg.starts_with('\'') || arg.starts_with('"');
            let is_bool = arg == "true" || arg == "false";
            return (is_num && text.contains("String"))
                || (is_string && text.contains("int"))
                || (is_bool && (text.contains("String") || text.contains("int")));
        }
    }
    false
}
