use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// BLoC classes should not expose public methods.
/// Use events (add) to communicate with a BLoC.
pub struct AvoidBlocPublicMethods;

impl Rule for AvoidBlocPublicMethods {
    fn name(&self) -> &'static str {
        "avoid-bloc-public-methods"
    }

    fn description(&self) -> &'static str {
        "Avoid public methods in BLoC classes. Use events instead."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let classes = find_descendants_by_kind(root, "class_declaration");
        for class in classes {
            let class_text = &source[class.byte_range()];
            if !class_text.contains("Bloc<") && !class_text.contains("extends Bloc") {
                continue;
            }

            let methods = dart_ast::get_class_methods(class);
            for method in methods {
                let name = dart_ast::get_declaration_name(method, source).unwrap_or("");
                if name.starts_with('_')
                    || name == "build"
                    || name == "close"
                    || name == "onEvent"
                    || name == "onTransition"
                    || name == "onError"
                    || name == "onChange"
                {
                    continue;
                }

                let member_text = &source[method.byte_range()];
                if member_text.contains("@override") || member_text.contains("@visibleForTesting") {
                    continue;
                }

                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Public method '{}' in BLoC class. Use events (add) instead.", name),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(method),
                    column: method.start_position().column + 1,
                });
            }
        }

        issues
    }
}
