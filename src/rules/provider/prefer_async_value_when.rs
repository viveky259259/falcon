use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests using `asyncValue.when()` pattern instead of manual state checks
/// when handling AsyncValue in Riverpod.
pub struct PreferAsyncValueWhen;

impl Rule for PreferAsyncValueWhen {
    fn name(&self) -> &'static str {
        "prefer-async-value-when"
    }

    fn description(&self) -> &'static str {
        "Prefer using .when() for handling AsyncValue states."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "if_statement" {
                return;
            }

            let text = &source[node.byte_range()];
            let has_loading = text.contains(".isLoading") || text.contains("is AsyncLoading");
            let has_error = text.contains(".hasError") || text.contains("is AsyncError");
            let has_data = text.contains(".hasValue") || text.contains("is AsyncData");

            if (has_data || has_error) && has_loading || (has_error && has_data) {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: "Consider using '.when(data:, error:, loading:)' instead of manual if-checks for AsyncValue.".to_string(),
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
