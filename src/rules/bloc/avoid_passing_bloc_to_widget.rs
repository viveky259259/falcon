use crate::config::Severity;
use crate::parser::{find_descendants_by_kind, node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns when a BLoC instance is passed as a widget constructor parameter.
/// Widgets should use BlocProvider/context.read to access BLoCs.
pub struct AvoidPassingBlocToWidget;

impl Rule for AvoidPassingBlocToWidget {
    fn name(&self) -> &'static str {
        "avoid-passing-bloc-to-widget"
    }

    fn description(&self) -> &'static str {
        "Avoid passing BLoC instances as widget parameters. Use BlocProvider instead."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let classes = find_descendants_by_kind(root, "class_declaration");
        for class in classes {
            let class_text = &source[class.byte_range()];
            let is_widget =
                class_text.contains("StatelessWidget") || class_text.contains("StatefulWidget");

            if !is_widget {
                continue;
            }

            walk_tree(class, &mut |node| {
                if node.kind() == "formal_parameter" {
                    let text = &source[node.byte_range()];
                    if text.contains("Bloc ")
                        || text.contains("Bloc?")
                        || text.contains("Cubit ")
                        || text.contains("Cubit?")
                    {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: "Avoid passing BLoC/Cubit as widget parameter. Use BlocProvider/context.read.".to_string(),
                            severity: self.default_severity(),
                            file: file.to_path_buf(),
                            line: node_start_line(node),
                            column: node.start_position().column + 1,
                        });
                    }
                }
            });
        }

        issues
    }
}
