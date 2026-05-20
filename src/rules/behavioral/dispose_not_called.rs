//! `dispose-not-called` — detect `State` subclasses (and other types that
//! implement a `dispose()` contract) where `dispose()` is missing or never
//! invokes `super.dispose()`.
//!
//! Reliably knowing whether a class extends `State<T>` requires the
//! class-hierarchy resolver — the CST only sees the syntactic
//! `extends`/`with`/`implements` clauses at the local file, not transitive
//! parents declared in other files or generated code.
//!
//! TODO(resolver): see EPIC 3.1 — currently returns empty until resolver
//! lands. The narrower `ensure-dispose-lifecycle` rule already covers the
//! common case where a `State` subclass owns a disposable controller.

use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct DisposeNotCalled;

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
        // TODO(resolver): see EPIC 3.1 — currently returns empty until resolver lands.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    #[test]
    fn rule_metadata_is_present() {
        let rule = DisposeNotCalled;
        assert_eq!(rule.name(), "dispose-not-called");
        assert!(!rule.description().is_empty());
        assert!(matches!(rule.default_severity(), Severity::Error));
    }

    #[test]
    fn stub_check_returns_empty_without_panic() {
        let rule = DisposeNotCalled;
        let source = r#"
class _MyWidgetState extends State<MyWidget> {
  final c = TextEditingController();
}
"#;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let issues = rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"));
        assert!(issues.is_empty(), "stub must not emit issues yet");
    }

    fn check(source: &str) -> Vec<Issue> {
        let rule = DisposeNotCalled;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    #[test]
    fn properly_disposed_state_is_not_flagged() {
        // State with a disposable that overrides dispose() AND calls
        // super.dispose() — the correct shape.  Stub returns empty (and once
        // the resolver lands, this case should remain unflagged).
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
}
