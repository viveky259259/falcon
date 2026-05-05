use crate::config::Severity;
use crate::parser::find_descendants_by_kind;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct EnsureStreamSubscriptionCancel;

impl Rule for EnsureStreamSubscriptionCancel {
    fn name(&self) -> &'static str {
        "ensure-stream-subscription-cancel"
    }

    fn description(&self) -> &'static str {
        "StreamSubscription must be cancelled in dispose() to prevent memory leaks and unexpected behavior."
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

            let has_subscription = class_text.contains("StreamSubscription");
            let has_listen = class_text.contains(".listen(");

            if !has_subscription && !has_listen {
                continue;
            }

            let has_cancel = class_text.contains(".cancel()");
            let has_dispose =
                class_text.contains("void dispose()") || class_text.contains("void dispose(");

            if (has_subscription || has_listen) && (!has_cancel || !has_dispose) {
                let class_start = class.start_position().row;

                for (line_num, line) in source.lines().enumerate() {
                    if line_num >= class_start && line_num <= class.end_position().row {
                        let trimmed = line.trim();
                        if trimmed.contains("StreamSubscription") || trimmed.contains(".listen(") {
                            let msg = if !has_dispose {
                                "Stream subscription found but no dispose() method — subscription will leak."
                            } else {
                                "Stream subscription found but .cancel() not called in dispose()."
                            };
                            issues.push(Issue {
                                rule: "ensure-stream-subscription-cancel".to_string(),
                                message: msg.to_string(),
                                severity: Severity::Error,
                                file: file.to_path_buf(),
                                line: line_num + 1,
                                column: 1,
                            });
                            break;
                        }
                    }
                }
            }
        }

        issues
    }
}
