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
            let has_dispose =
                class_text.contains("void dispose()") || class_text.contains("void dispose(");

            for dtype in DISPOSABLE_TYPES {
                if class_text.contains(dtype) {
                    for (line_num, line) in source.lines().enumerate() {
                        let class_start = class.start_position().row;
                        let class_end = class.end_position().row;
                        if line_num >= class_start && line_num <= class_end {
                            if line.contains(dtype)
                                && (line.contains("late")
                                    || line.contains("final")
                                    || line.trim().starts_with(dtype))
                            {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    fn run(source: &str) -> Vec<Issue> {
        let rule = EnsureDisposeLifecycle;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    #[test]
    fn controller_without_dispose_is_flagged() {
        let issues = run(r#"
class _S extends State<W> {
  final TextEditingController controller = TextEditingController();

  @override
  Widget build(BuildContext context) => Container();
}
"#);
        assert!(
            !issues.is_empty(),
            "controller without dispose() should be flagged"
        );
        assert_eq!(issues[0].rule, "ensure-dispose-lifecycle");
        assert!(issues[0].message.contains("TextEditingController"));
    }

    #[test]
    fn controller_with_dispose_is_ok() {
        let issues = run(r#"
class _S extends State<W> {
  final TextEditingController controller = TextEditingController();

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Container();
}
"#);
        assert!(
            issues.is_empty(),
            "State that overrides dispose() must not be flagged"
        );
    }

    #[test]
    fn non_state_class_with_controller_is_not_flagged() {
        // Plain class — rule only applies to State<...> subclasses.
        let issues = run(r#"
class Service {
  final TextEditingController controller = TextEditingController();
}
"#);
        assert!(
            issues.is_empty(),
            "non-State class is out of scope for ensure-dispose-lifecycle"
        );
    }
}
