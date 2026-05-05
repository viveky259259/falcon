use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidLateKeyword {
    skip_test_files: bool,
    skip_framework_patterns: bool,
}

impl Default for AvoidLateKeyword {
    fn default() -> Self {
        Self {
            skip_test_files: true,
            skip_framework_patterns: true,
        }
    }
}

impl Rule for AvoidLateKeyword {
    fn name(&self) -> &'static str {
        "avoid-late-keyword"
    }

    fn description(&self) -> &'static str {
        "Avoid using 'late' keyword. Prefer nullable types or factory constructors."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn configure(&mut self, options: &HashMap<String, serde_yaml::Value>) {
        if let Some(val) = options.get("skip_test_files") {
            if let Some(b) = val.as_bool() {
                self.skip_test_files = b;
            }
        }
        if let Some(val) = options.get("skip_framework_patterns") {
            if let Some(b) = val.as_bool() {
                self.skip_framework_patterns = b;
            }
        }
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let file_str = file.to_string_lossy();
        if self.skip_test_files && (file_str.ends_with("_test.dart") || file_str.contains("/test/"))
        {
            return issues;
        }

        walk_tree(root, &mut |node| {
            if node.kind() == "late"
                || (node.kind() == "identifier" && &source[node.byte_range()] == "late")
            {
                if let Some(parent) = node.parent() {
                    if parent.kind() == "identifier" {
                        return;
                    }

                    if self.skip_framework_patterns {
                        let parent_text = parent.utf8_text(source.as_bytes()).unwrap_or("");

                        if is_lazy_late_final(parent_text) {
                            return;
                        }

                        if is_framework_required_late(parent_text) {
                            return;
                        }
                    }

                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: "Avoid using 'late'. Prefer nullable types or initialization in constructors."
                            .to_string(),
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

fn is_lazy_late_final(declaration: &str) -> bool {
    let trimmed = declaration.trim();
    trimmed.starts_with("late final") && trimmed.contains('=')
}

fn is_framework_required_late(declaration: &str) -> bool {
    let framework_types = [
        "AnimationController",
        "TabController",
        "ScrollController",
        "TextEditingController",
        "FocusNode",
        "PageController",
        "DraggableScrollableController",
    ];
    framework_types.iter().any(|t| declaration.contains(t))
}
