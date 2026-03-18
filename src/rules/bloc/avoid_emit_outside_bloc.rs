use crate::config::Severity;
use crate::parser::{find_descendants_by_kind, node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// `emit()` should only be called inside BLoC/Cubit classes,
/// not from external code.
pub struct AvoidEmitOutsideBloc;

impl Rule for AvoidEmitOutsideBloc {
    fn name(&self) -> &'static str {
        "avoid-emit-outside-bloc"
    }

    fn description(&self) -> &'static str {
        "Avoid calling emit() outside of BLoC/Cubit classes."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let classes = find_descendants_by_kind(root, "class_declaration");
        let mut bloc_class_ranges = Vec::new();

        for class in &classes {
            let class_text = &source[class.byte_range()];
            if class_text.contains("Bloc<")
                || class_text.contains("Cubit<")
                || class_text.contains("extends Bloc")
                || class_text.contains("extends Cubit")
            {
                bloc_class_ranges.push(class.byte_range());
            }
        }

        walk_tree(root, &mut |node| {
            if node.kind() != "identifier" {
                return;
            }
            let text = &source[node.byte_range()];
            if text != "emit" {
                return;
            }

            if let Some(parent) = node.parent() {
                let pt = &source[parent.byte_range()];
                if !pt.contains("emit(") {
                    return;
                }
            }

            let pos = node.byte_range().start;
            let inside_bloc = bloc_class_ranges.iter().any(|r| r.contains(&pos));
            if !inside_bloc {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: "Avoid calling emit() outside a BLoC or Cubit class.".to_string(),
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
