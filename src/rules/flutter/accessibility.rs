use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Rule: Ensure interactive widgets have Semantics labels.
pub struct EnsureSemanticsLabel;

impl Rule for EnsureSemanticsLabel {
    fn name(&self) -> &'static str {
        "ensure-semantics-label"
    }
    fn description(&self) -> &'static str {
        "Interactive widgets should have a Semantics label for accessibility"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        find_unlabeled_widgets(root, source, file)
    }
}

fn find_unlabeled_widgets(_root: Node, source: &str, file: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    let interactive_widgets = [
        "GestureDetector(",
        "InkWell(",
        "InkResponse(",
        "IconButton(",
    ];

    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        for widget in &interactive_widgets {
            if trimmed.contains(widget) {
                let block: String = source
                    .lines()
                    .skip(i)
                    .take(10)
                    .collect::<Vec<_>>()
                    .join(" ");

                let has_semantics = block.contains("Semantics(")
                    || block.contains("semanticsLabel")
                    || block.contains("tooltip:");

                if !has_semantics {
                    issues.push(Issue {
                        rule: "ensure-semantics-label".to_string(),
                        message: format!(
                            "{} should have a Semantics wrapper, semanticsLabel, or tooltip for screen reader accessibility",
                            widget.trim_end_matches('(')
                        ),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    });
                }
                break;
            }
        }
    }

    issues
}

/// Rule: Ensure images have semantic descriptions.
pub struct EnsureImageSemantics;

impl Rule for EnsureImageSemantics {
    fn name(&self) -> &'static str {
        "ensure-image-semantics"
    }
    fn description(&self) -> &'static str {
        "Images should have semanticLabel for screen reader accessibility"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, _root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            let image_constructors = [
                "Image.asset(",
                "Image.network(",
                "Image.file(",
                "Image.memory(",
            ];

            for constructor in &image_constructors {
                if trimmed.contains(constructor) {
                    let block: String =
                        source.lines().skip(i).take(8).collect::<Vec<_>>().join(" ");

                    if !block.contains("semanticLabel") && !block.contains("Semantics(") {
                        issues.push(Issue {
                            rule: "ensure-image-semantics".to_string(),
                            message: format!(
                                "{} should include semanticLabel for screen reader accessibility",
                                constructor.trim_end_matches('(')
                            ),
                            severity: Severity::Warning,
                            file: file.to_path_buf(),
                            line: i + 1,
                            column: 1,
                        });
                    }
                    break;
                }
            }
        }

        issues
    }
}

/// Rule: Ensure sufficient touch target size.
pub struct EnsureTouchTargetSize;

impl Rule for EnsureTouchTargetSize {
    fn name(&self) -> &'static str {
        "ensure-touch-target-size"
    }
    fn description(&self) -> &'static str {
        "Interactive elements should meet minimum 48x48 touch target size (WCAG 2.5.5)"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, _root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("SizedBox(") || trimmed.contains("Container(") {
                let block: String = source.lines().skip(i).take(5).collect::<Vec<_>>().join(" ");

                if (block.contains("onTap")
                    || block.contains("onPressed")
                    || block.contains("GestureDetector"))
                    && (block.contains("width:") || block.contains("height:"))
                {
                    let small = extract_dimension(&block, "width:").map_or(false, |d| d < 48.0)
                        || extract_dimension(&block, "height:").map_or(false, |d| d < 48.0);

                    if small {
                        issues.push(Issue {
                            rule: "ensure-touch-target-size".to_string(),
                            message: "Interactive element may be smaller than 48x48dp minimum touch target (WCAG 2.5.5)".to_string(),
                            severity: Severity::Info,
                            file: file.to_path_buf(),
                            line: i + 1,
                            column: 1,
                        });
                    }
                }
            }
        }

        issues
    }
}

fn extract_dimension(text: &str, prefix: &str) -> Option<f64> {
    let idx = text.find(prefix)? + prefix.len();
    let rest = text[idx..].trim();
    let num_str: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num_str.parse().ok()
}
