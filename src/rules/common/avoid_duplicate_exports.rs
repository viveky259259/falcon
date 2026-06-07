use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidDuplicateExports;

impl Rule for AvoidDuplicateExports {
    fn name(&self) -> &'static str {
        "avoid-duplicate-exports"
    }

    fn description(&self) -> &'static str {
        "Avoid exporting the same library or file multiple times."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut seen_exports: HashSet<String> = HashSet::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "import_or_export" {
                return;
            }

            let text = &source[node.byte_range()];
            if !text.trim_start().starts_with("export") {
                return;
            }

            let mut uri = String::new();
            walk_tree(node, &mut |inner| {
                if (inner.kind() == "string_literal"
                    || inner.kind() == "string_literal_single_quotes"
                    || inner.kind() == "string_literal_double_quotes")
                    && uri.is_empty()
                {
                    uri = source[inner.byte_range()]
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string();
                }
            });

            if !uri.is_empty() && !seen_exports.insert(uri.clone()) {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Duplicate export: '{}'.", uri),
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
