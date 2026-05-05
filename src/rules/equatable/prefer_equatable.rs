use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Suggests extending Equatable when a class manually implements == and hashCode.
pub struct PreferEquatable;

impl Rule for PreferEquatable {
    fn name(&self) -> &'static str {
        "prefer-equatable"
    }

    fn description(&self) -> &'static str {
        "Consider extending Equatable instead of manually overriding == and hashCode."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let classes = find_descendants_by_kind(root, "class_declaration");
        for class in classes {
            let class_text = &source[class.byte_range()];
            if class_text.contains("Equatable") {
                continue;
            }

            let has_equals = class_text.contains("operator ==");
            let has_hashcode = class_text.contains("get hashCode");

            if has_equals && has_hashcode {
                let name = dart_ast::get_declaration_name(class, source).unwrap_or("<unknown>");
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!(
                        "Class '{}' manually overrides == and hashCode. Consider using Equatable.",
                        name
                    ),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(class),
                    column: class.start_position().column + 1,
                });
            }
        }

        issues
    }
}
