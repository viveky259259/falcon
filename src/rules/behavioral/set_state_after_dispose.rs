//! `set-state-after-dispose` — detect `setState` calls reachable after a
//! widget's State has been disposed (e.g. after an `await` with no `mounted`
//! check).
//!
//! Precise detection requires cross-method/data-flow analysis (knowing which
//! identifiers refer to a `State` subclass, tracking control-flow after
//! `await`). The CST alone produces too many false positives.
//!
//! TODO(resolver): see EPIC 3.1 — currently returns empty until resolver
//! lands. The companion runtime-correctness rule `fake-mounted-check` covers
//! the most common surface case (await inside `if (mounted)` without a
//! re-check) using CST-only heuristics.

use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct SetStateAfterDispose;

impl Rule for SetStateAfterDispose {
    fn name(&self) -> &'static str {
        "set-state-after-dispose"
    }

    fn description(&self) -> &'static str {
        "setState must not run after dispose(). Calling it on an unmounted State throws and is a top AI-failure mode."
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
        let rule = SetStateAfterDispose;
        assert_eq!(rule.name(), "set-state-after-dispose");
        assert!(!rule.description().is_empty());
        assert!(matches!(rule.default_severity(), Severity::Error));
    }

    #[test]
    fn stub_check_returns_empty_without_panic() {
        let rule = SetStateAfterDispose;
        let source = r#"
class MyWidget extends StatefulWidget {}
class _MyWidgetState extends State<MyWidget> {
  Future<void> load() async {
    await Future.delayed(Duration.zero);
    setState(() {});
  }
}
"#;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let issues = rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"));
        assert!(issues.is_empty(), "stub must not emit issues yet");
    }
}
