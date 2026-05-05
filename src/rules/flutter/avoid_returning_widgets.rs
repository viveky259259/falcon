use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line, node_text};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

const WIDGET_TYPES: &[&str] = &[
    "Widget",
    "StatelessWidget",
    "StatefulWidget",
    "Container",
    "Row",
    "Column",
    "Stack",
    "Scaffold",
    "Text",
    "Icon",
    "Image",
    "ListView",
    "GridView",
    "Card",
    "AppBar",
    "Padding",
    "Center",
    "Expanded",
    "Flexible",
    "SizedBox",
    "Align",
];

pub struct AvoidReturningWidgets;

impl Rule for AvoidReturningWidgets {
    fn name(&self) -> &'static str {
        "avoid-returning-widgets"
    }

    fn description(&self) -> &'static str {
        "Avoid returning widgets from methods. Extract into separate widget classes instead."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let methods = find_descendants_by_kind(root, "method_signature");

        for method in methods {
            if is_build_method(method, source) {
                continue;
            }

            let method_text = node_text(method, source);
            for widget in WIDGET_TYPES {
                if method_text.contains(widget) && has_widget_return_type(method, source) {
                    let name =
                        dart_ast::get_declaration_name(method, source).unwrap_or("<anonymous>");
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Method '{}' returns a Widget. Extract into a separate widget class.",
                            name
                        ),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(method),
                        column: 1,
                    });
                    break;
                }
            }
        }

        issues
    }
}

fn is_build_method(node: Node, source: &str) -> bool {
    dart_ast::get_declaration_name(node, source)
        .map(|name| name == "build")
        .unwrap_or(false)
}

fn has_widget_return_type(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            let type_name = &source[child.byte_range()];
            return WIDGET_TYPES.contains(&type_name);
        }
    }
    false
}
