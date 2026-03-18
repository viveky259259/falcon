use crate::config::Severity;
use crate::parser::find_descendants_by_kind;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidExcessiveWidgetNesting;

const MAX_NESTING: usize = 10;

impl Rule for AvoidExcessiveWidgetNesting {
    fn name(&self) -> &'static str {
        "avoid-excessive-widget-nesting"
    }

    fn description(&self) -> &'static str {
        "Widget trees should not be nested too deeply. Extract sub-widgets for readability and performance."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        let classes = find_descendants_by_kind(root, "class_declaration");

        for class in &classes {
            let class_text = class.utf8_text(source.as_bytes()).unwrap_or("");

            if !class_text.contains("Widget") && !class_text.contains("State<") {
                continue;
            }

            let class_start = class.start_position().row;
            let class_end = class.end_position().row;

            let mut in_build = false;
            let mut build_start = 0;
            let mut max_depth: usize = 0;
            let mut current_depth: usize = 0;

            for (line_num, line) in source.lines().enumerate() {
                if line_num < class_start || line_num > class_end {
                    continue;
                }

                let trimmed = line.trim();

                if trimmed.contains("Widget build(") {
                    in_build = true;
                    build_start = line_num;
                    current_depth = 0;
                    max_depth = 0;
                    continue;
                }

                if in_build {
                    for ch in line.chars() {
                        if ch == '(' {
                            current_depth += 1;
                            if current_depth > max_depth {
                                max_depth = current_depth;
                            }
                        } else if ch == ')' {
                            current_depth = current_depth.saturating_sub(1);
                        }
                    }

                    if trimmed == "}" && current_depth == 0 {
                        if max_depth > MAX_NESTING {
                            issues.push(Issue {
                                rule: "avoid-excessive-widget-nesting".to_string(),
                                message: format!(
                                    "Widget tree nested {} levels deep (max {}). Extract sub-widgets.",
                                    max_depth, MAX_NESTING
                                ),
                                severity: Severity::Warning,
                                file: file.to_path_buf(),
                                line: build_start + 1,
                                column: 1,
                            });
                        }
                        in_build = false;
                    }
                }
            }
        }

        issues
    }
}
