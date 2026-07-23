//! `dispose-not-called` — detect `State` subclasses (and other types that
//! implement a `dispose()` contract) where `dispose()` is missing or never
//! invokes `super.dispose()`.
//!
//! Reliably knowing whether a class extends `State<T>` requires the
//! class-hierarchy resolver — the CST only sees the syntactic
//! `extends`/`with`/`implements` clauses at the local file, not transitive
//! parents declared in other files or generated code.
//!
//! The plain per-file rule hook intentionally remains conservative. The
//! project-aware hook uses the resolver index so it can detect direct and
//! transitive `State` subclasses.

use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind};
use crate::reporters::Issue;
use crate::rules::{Rule, RuleContext};
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct DisposeNotCalled;

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

impl Rule for DisposeNotCalled {
    fn name(&self) -> &'static str {
        "dispose-not-called"
    }

    fn description(&self) -> &'static str {
        "Subclasses of State (and other disposable types) must override dispose() and call super.dispose()."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, _root: Node, _source: &str, _file: &Path) -> Vec<Issue> {
        Vec::new()
    }

    fn check_with_context(
        &self,
        root: Node,
        source: &str,
        file: &Path,
        context: &RuleContext<'_>,
    ) -> Vec<Issue> {
        let Some(resolver) = context.resolver else {
            return self.check(root, source, file);
        };

        let mut issues = Vec::new();
        for class_node in find_descendants_by_kind(root, "class_declaration") {
            let Some(class_name) = dart_ast::get_declaration_name(class_node, source) else {
                continue;
            };

            let Some(class) = resolver.resolve_class_name(class_name) else {
                continue;
            };

            if !resolver.is_subtype_of(class, "State") {
                continue;
            }

            let class_text = class_node.utf8_text(source.as_bytes()).unwrap_or("");
            let disposables = disposable_fields(source, class_node);
            if disposables.is_empty() {
                continue;
            }

            if !class.has_method("dispose") {
                for disposable in disposables {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "{} field in State subclass '{}' requires a dispose() override.",
                            disposable.ty, class.name
                        ),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: disposable.line,
                        column: 1,
                    });
                }
                continue;
            }

            if !class_text.contains("super.dispose()") && !class_text.contains("super.dispose(") {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: format!(
                        "State subclass '{}' overrides dispose() but does not call super.dispose().",
                        class.name
                    ),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: dispose_line(source, class_node).unwrap_or(class.line),
                    column: 1,
                });
            }
        }

        issues
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DisposableField {
    ty: &'static str,
    line: usize,
}

fn disposable_fields(source: &str, class: Node) -> Vec<DisposableField> {
    let class_start = class.start_position().row;
    let class_end = class.end_position().row;
    let mut fields = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        if line_idx < class_start || line_idx > class_end {
            continue;
        }

        let trimmed = line.trim();
        if !looks_like_field(trimmed) {
            continue;
        }

        for ty in DISPOSABLE_TYPES {
            if trimmed.contains(ty) {
                fields.push(DisposableField {
                    ty,
                    line: line_idx + 1,
                });
                break;
            }
        }
    }

    fields
}

fn looks_like_field(line: &str) -> bool {
    if line.starts_with("//") || line.contains('(') && line.contains(')') && line.contains('{') {
        return false;
    }

    line.starts_with("final ")
        || line.starts_with("late ")
        || line.starts_with("var ")
        || DISPOSABLE_TYPES.iter().any(|ty| line.starts_with(ty))
}

fn dispose_line(source: &str, class: Node) -> Option<usize> {
    let class_start = class.start_position().row;
    let class_end = class.end_position().row;
    source
        .lines()
        .enumerate()
        .skip(class_start)
        .take(class_end.saturating_sub(class_start) + 1)
        .find_map(|(line_idx, line)| line.contains("dispose(").then_some(line_idx + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use crate::resolver::{ResolvedClass, ResolverIndex};
    use std::path::PathBuf;

    #[test]
    fn rule_metadata_is_present() {
        let rule = DisposeNotCalled;
        assert_eq!(rule.name(), "dispose-not-called");
        assert!(!rule.description().is_empty());
        assert!(matches!(rule.default_severity(), Severity::Error));
    }

    #[test]
    fn plain_check_returns_empty_without_resolver_context() {
        let rule = DisposeNotCalled;
        let source = r#"
class _MyWidgetState extends State<MyWidget> {
  final c = TextEditingController();
}
"#;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let issues = rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"));
        assert!(
            issues.is_empty(),
            "plain per-file hook must wait for resolver context"
        );
    }

    fn check_with_index(source: &str, file: PathBuf, index: &ResolverIndex) -> Vec<Issue> {
        let rule = DisposeNotCalled;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let resolver = index.resolver_for_file(&file, source);
        let context = RuleContext {
            resolver_index: Some(index),
            resolver: Some(&resolver),
        };
        rule.check_with_context(tree.root_node(), source, &file, &context)
    }

    fn class(name: &str, file: &str, superclass: Option<&str>, methods: &[&str]) -> ResolvedClass {
        ResolvedClass {
            name: name.to_string(),
            file: PathBuf::from(file),
            line: 1,
            superclass: superclass.map(ToString::to_string),
            mixins: Vec::new(),
            interfaces: Vec::new(),
            methods: methods.iter().map(|method| method.to_string()).collect(),
        }
    }

    #[test]
    fn context_flags_state_subclass_missing_dispose() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![class(
            "_ScreenState",
            "lib/screen.dart",
            Some("State"),
            &[],
        )]);
        let issues = check_with_index(
            r#"
class _ScreenState extends State<W> {
  final TextEditingController controller = TextEditingController();
}
"#,
            file,
            &index,
        );

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "dispose-not-called");
        assert!(issues[0].message.contains("requires a dispose() override"));
    }

    #[test]
    fn context_flags_transitive_state_subclass_missing_dispose() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![
            class("BaseState", "lib/base.dart", Some("State"), &[]),
            class("_ScreenState", "lib/screen.dart", Some("BaseState"), &[]),
        ]);
        let issues = check_with_index(
            r#"
class _ScreenState extends BaseState {
  final TextEditingController controller = TextEditingController();
}
"#,
            file,
            &index,
        );

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "dispose-not-called");
        assert!(issues[0].message.contains("requires a dispose() override"));
    }

    #[test]
    fn context_flags_dispose_without_super_dispose() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![class(
            "_ScreenState",
            "lib/screen.dart",
            Some("State"),
            &["dispose"],
        )]);
        let issues = check_with_index(
            r#"
class _ScreenState extends State<W> {
  late FocusNode focusNode;

  @override
  void dispose() {
    focusNode.dispose();
  }
}
"#,
            file,
            &index,
        );

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("does not call super.dispose()"));
    }

    #[test]
    fn context_allows_proper_dispose() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![class(
            "_ScreenState",
            "lib/screen.dart",
            Some("State"),
            &["dispose"],
        )]);
        let issues = check_with_index(
            r#"
class _ScreenState extends State<W> {
  late FocusNode focusNode;

  @override
  void dispose() {
    focusNode.dispose();
    super.dispose();
  }
}
"#,
            file,
            &index,
        );

        assert!(issues.is_empty());
    }

    fn check(source: &str) -> Vec<Issue> {
        let rule = DisposeNotCalled;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    #[test]
    fn properly_disposed_state_is_not_flagged() {
        // Plain per-file hooks do not have resolver context; the project-aware
        // path below verifies this stays unflagged once context is available.
        let issues = check(
            r#"
class _S extends State<W> {
  final c = TextEditingController();
  @override
  void dispose() {
    c.dispose();
    super.dispose();
  }
}
"#,
        );
        assert!(
            issues.is_empty(),
            "properly disposed State must not be flagged"
        );
    }

    #[test]
    fn state_with_no_disposable_fields_is_not_flagged() {
        // No fields that own resources → no dispose contract to violate.
        let issues = check(
            r#"
class _S extends State<W> {
  int counter = 0;
  String label = 'hi';
}
"#,
        );
        assert!(
            issues.is_empty(),
            "State with no disposable fields must not be flagged"
        );
    }

    #[test]
    fn non_state_class_with_controller_is_not_flagged() {
        // Plain class (not a Flutter State) — out of scope for this rule.
        let issues = check(
            r#"
class MyService {
  final c = TextEditingController();
}
"#,
        );
        assert!(
            issues.is_empty(),
            "non-State class is out of scope for dispose-not-called"
        );
    }

    #[test]
    fn context_ignores_non_state_service_with_disposable_field() {
        let file = PathBuf::from("lib/service.dart");
        let index = ResolverIndex::new(vec![class("MyService", "lib/service.dart", None, &[])]);
        let issues = check_with_index(
            r#"
class MyService {
  final TextEditingController controller = TextEditingController();
}
"#,
            file,
            &index,
        );

        assert!(issues.is_empty());
    }
}
