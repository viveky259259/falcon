use crate::config::Severity;
use crate::parser::find_descendants_by_kind;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct EnsureDisposeLifecycle;

const DISPOSABLE_TYPES: &[&str] = &[
    "TextEditingController",
    "ScrollController",
    "AnimationController",
    "TabController",
    "FocusNode",
    "PageController",
    "StreamSubscription",
    "Timer",
];

impl Rule for EnsureDisposeLifecycle {
    fn name(&self) -> &'static str {
        "ensure-dispose-lifecycle"
    }

    fn description(&self) -> &'static str {
        "Controllers, FocusNodes, and subscriptions must be disposed in dispose() to prevent memory leaks."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        let classes = find_descendants_by_kind(root, "class_declaration");

        for class in &classes {
            let class_text = class.utf8_text(source.as_bytes()).unwrap_or("");
            if !class_text.contains("State<") {
                continue;
            }

            let mut disposables_found: Vec<(&str, usize)> = Vec::new();
            let has_dispose = class_text.contains("void dispose()") || class_text.contains("void dispose(");

            for dtype in DISPOSABLE_TYPES {
                if class_text.contains(dtype) {
                    for (line_num, line) in source.lines().enumerate() {
                        let class_start = class.start_position().row;
                        let class_end = class.end_position().row;
                        if line_num >= class_start && line_num <= class_end {
                            if line.contains(dtype) && (line.contains("late") || line.contains("final") || line.trim().starts_with(dtype)) {
                                disposables_found.push((dtype, line_num + 1));
                            }
                        }
                    }
                }
            }

            if !disposables_found.is_empty() && !has_dispose {
                for (dtype, line) in &disposables_found {
                    issues.push(Issue {
                        rule: "ensure-dispose-lifecycle".to_string(),
                        message: format!(
                            "{} created but no dispose() method found — will cause memory leak.",
                            dtype
                        ),
                        severity: Severity::Error,
                        file: file.to_path_buf(),
                        line: *line,
                        column: 1,
                    });
                }
            }
        }

        issues
    }
}
