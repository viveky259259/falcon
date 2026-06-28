use crate::config::Severity;
use crate::parser::{find_descendants_by_kind, node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Warns when a Notifier/StateNotifier class has public mutable properties.
/// State should only be modified through methods.
pub struct AvoidPublicNotifierProperties;

impl Rule for AvoidPublicNotifierProperties {
    fn name(&self) -> &'static str {
        "avoid-public-notifier-properties"
    }

    fn description(&self) -> &'static str {
        "Avoid public mutable properties in Notifier classes. Use methods to modify state."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let classes = find_descendants_by_kind(root, "class_declaration");
        for class in classes {
            let class_text = &source[class.byte_range()];
            let is_notifier = class_text.contains("Notifier")
                || class_text.contains("StateNotifier")
                || class_text.contains("ChangeNotifier");

            if !is_notifier {
                continue;
            }

            let mut cursor = class.walk();
            for child in class.children(&mut cursor) {
                if child.kind() == "class_body" {
                    let mut body_cursor = child.walk();
                    for member in child.children(&mut body_cursor) {
                        if member.kind() != "class_member" {
                            continue;
                        }
                        let member_text = &source[member.byte_range()].trim_start().to_string();
                        if !member_text.starts_with('_')
                            && !member_text.starts_with("final")
                            && !member_text.starts_with("static")
                            && !member_text.starts_with("@")
                        {
                            let mut mc = member.walk();
                            for mc_child in member.children(&mut mc) {
                                if mc_child.kind() == "declaration" {
                                    let decl_text = &source[mc_child.byte_range()];
                                    if !decl_text.trim_start().starts_with("final")
                                        && !decl_text.trim_start().starts_with("static")
                                    {
                                        walk_tree(mc_child, &mut |n| {
                                            if n.kind() == "identifier" {
                                                let id = &source[n.byte_range()];
                                                if !id.starts_with('_')
                                                    && id
                                                        .chars()
                                                        .next()
                                                        .is_some_and(|c| c.is_lowercase())
                                                {
                                                    issues.push(Issue {
                                                        rule: self.name().to_string(),
                                                        message: format!("Public mutable property '{}' in Notifier class. Make it private or final.", id),
                                                        severity: self.default_severity(),
                                                        file: file.to_path_buf(),
                                                        line: node_start_line(mc_child),
                                                        column: mc_child.start_position().column + 1,
                                                    });
                                                }
                                            }
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        issues
    }
}
