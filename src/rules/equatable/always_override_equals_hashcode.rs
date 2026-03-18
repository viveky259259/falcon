use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// If a class overrides `==`, it should also override `hashCode`, and vice versa.
pub struct AlwaysOverrideEqualsHashCode;

impl Rule for AlwaysOverrideEqualsHashCode {
    fn name(&self) -> &'static str {
        "always-override-equals-and-hashcode"
    }

    fn description(&self) -> &'static str {
        "Always override both == and hashCode together."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let classes = find_descendants_by_kind(root, "class_declaration");
        for class in classes {
            let class_text = &source[class.byte_range()];
            let has_equals = class_text.contains("operator ==");
            let has_hashcode = class_text.contains("get hashCode");

            let name = dart_ast::get_declaration_name(class, source).unwrap_or("<unknown>");

            if has_equals && !has_hashcode {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Class '{}' overrides == but not hashCode.", name),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(class),
                    column: class.start_position().column + 1,
                });
            }

            if has_hashcode && !has_equals {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!("Class '{}' overrides hashCode but not ==.", name),
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
