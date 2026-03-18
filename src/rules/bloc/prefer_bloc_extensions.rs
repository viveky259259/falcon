use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests using BLoC extension methods (context.read, context.watch, context.select)
/// instead of BlocProvider.of(context).
pub struct PreferBlocExtensions;

impl Rule for PreferBlocExtensions {
    fn name(&self) -> &'static str {
        "prefer-bloc-extensions"
    }

    fn description(&self) -> &'static str {
        "Prefer context.read/watch/select over BlocProvider.of(context)."
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
                let pt = &source[parent.byte_range()];
                if pt.contains("BlocProvider.of") {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: "Use 'context.read<T>()' instead of 'BlocProvider.of<T>(context)'.".to_string(),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(node),
                        column: node.start_position().column + 1,
                    });
                }
            }
        });

        issues
    }
}
