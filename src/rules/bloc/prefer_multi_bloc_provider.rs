use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests using MultiBlocProvider when multiple BlocProvider widgets are nested.
pub struct PreferMultiBlocProvider;

impl Rule for PreferMultiBlocProvider {
    fn name(&self) -> &'static str {
        "prefer-multi-bloc-provider"
    }

    fn description(&self) -> &'static str {
        "Use MultiBlocProvider instead of nesting multiple BlocProvider widgets."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() != "identifier" {
                return;
            }
            let text = &source[node.byte_range()];
            if text != "BlocProvider" {
                return;
            }

            if let Some(parent) = node.parent() {
                let parent_text = &source[parent.byte_range()];
                if parent_text.contains("BlocProvider") {
                    let count = parent_text.matches("BlocProvider(").count()
                        + parent_text.matches("BlocProvider<").count();
                    if count >= 2 {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: "Multiple nested BlocProviders detected. Use MultiBlocProvider instead.".to_string(),
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
