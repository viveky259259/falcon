//! `set-state-after-dispose` — detect `setState` calls reachable after a
//! widget's State has been disposed (e.g. after an `await` with no `mounted`
//! check).
//!
//! Full precision requires richer control-flow and alias analysis. This first
//! resolver-backed slice stays conservative: it only checks methods declared
//! inside known `State` subclasses and flags `setState(...)` calls that appear
//! after an `await` without an intervening mounted guard.

use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind};
use crate::reporters::Issue;
use crate::rules::{Rule, RuleContext};
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

            for body in method_bodies(class_node) {
                scan_method_body(body, source, file, self, &mut issues);
            }
        }

        issues
    }
}

fn method_bodies(class_node: Node) -> Vec<Node> {
    find_descendants_by_kind(class_node, "function_body")
        .into_iter()
        .filter(|body| {
            body.parent()
                .map(|parent| parent.kind() == "class_member" || parent.kind() == "declaration")
                .unwrap_or(false)
        })
        .collect()
}

fn scan_method_body(
    body: Node,
    source: &str,
    file: &Path,
    rule: &SetStateAfterDispose,
    issues: &mut Vec<Issue>,
) {
    let text = body.utf8_text(source.as_bytes()).unwrap_or("");
    let mut search_from = 0;

    while let Some(relative_set_state) = text[search_from..].find("setState(") {
        let set_state_idx = search_from + relative_set_state;
        search_from = set_state_idx + "setState(".len();

        let before_call = &text[..set_state_idx];
        let Some(await_idx) = before_call.rfind("await ") else {
            continue;
        };

        let guard_region = &before_call[await_idx..];
        if has_mounted_guard_after_await(guard_region) {
            continue;
        }

        issues.push(Issue {
            rule: rule.name().to_string(),
            message: "`setState` after an async gap must re-check `mounted` first; the State may be disposed before the continuation runs.".to_string(),
            severity: rule.default_severity(),
            file: file.to_path_buf(),
            line: line_for_byte(source, body.start_byte() + set_state_idx),
            column: column_for_byte(source, body.start_byte() + set_state_idx),
        });
    }
}

fn has_mounted_guard_after_await(region: &str) -> bool {
    let compact = compact_ws(region);
    [
        "if (!mounted) return",
        "if (!this.mounted) return",
        "if (!mounted) { return",
        "if (!this.mounted) { return",
        "if (mounted)",
        "if (this.mounted)",
    ]
    .iter()
    .any(|guard| compact.contains(guard))
}

fn compact_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn line_for_byte(source: &str, byte: usize) -> usize {
    source[..byte.min(source.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

fn column_for_byte(source: &str, byte: usize) -> usize {
    let clamped = byte.min(source.len());
    let line_start = source[..clamped]
        .rfind('\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    clamped.saturating_sub(line_start) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use crate::resolver::{ResolvedClass, ResolverIndex};
    use std::path::PathBuf;

    #[test]
    fn rule_metadata_is_present() {
        let rule = SetStateAfterDispose;
        assert_eq!(rule.name(), "set-state-after-dispose");
        assert!(!rule.description().is_empty());
        assert!(matches!(rule.default_severity(), Severity::Error));
    }

    fn state_class(name: &str, file: &str, superclass: Option<&str>) -> ResolvedClass {
        ResolvedClass {
            name: name.to_string(),
            file: PathBuf::from(file),
            line: 1,
            superclass: superclass.map(ToString::to_string),
            mixins: Vec::new(),
            interfaces: Vec::new(),
            methods: Vec::new(),
        }
    }

    #[test]
    fn plain_check_returns_empty_without_resolver_context() {
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
        assert!(
            issues.is_empty(),
            "plain per-file hook has no resolver context"
        );
    }

    fn run_with_index(source: &str, file: PathBuf, index: &ResolverIndex) -> Vec<Issue> {
        let rule = SetStateAfterDispose;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let resolver = index.resolver_for_file(&file, source);
        let context = RuleContext {
            resolver_index: Some(index),
            resolver: Some(&resolver),
        };
        rule.check_with_context(tree.root_node(), source, &file, &context)
    }

    #[test]
    fn context_flags_set_state_after_await_without_mounted_guard() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![state_class(
            "_ScreenState",
            "lib/screen.dart",
            Some("State"),
        )]);
        let issues = run_with_index(
            r#"
class _ScreenState extends State<W> {
  Future<void> load() async {
    await api.fetch();
    setState(() {});
  }
}
"#,
            file,
            &index,
        );

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "set-state-after-dispose");
        assert_eq!(issues[0].line, 5);
    }

    #[test]
    fn context_allows_post_await_mounted_return_guard() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![state_class(
            "_ScreenState",
            "lib/screen.dart",
            Some("State"),
        )]);
        let issues = run_with_index(
            r#"
class _ScreenState extends State<W> {
  Future<void> load() async {
    await api.fetch();
    if (!mounted) return;
    setState(() {});
  }
}
"#,
            file,
            &index,
        );

        assert!(issues.is_empty());
    }

    #[test]
    fn context_allows_set_state_inside_mounted_block() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![state_class(
            "_ScreenState",
            "lib/screen.dart",
            Some("State"),
        )]);
        let issues = run_with_index(
            r#"
class _ScreenState extends State<W> {
  Future<void> load() async {
    await api.fetch();
    if (mounted) {
      setState(() {});
    }
  }
}
"#,
            file,
            &index,
        );

        assert!(issues.is_empty());
    }

    #[test]
    fn context_ignores_non_state_class() {
        let file = PathBuf::from("lib/service.dart");
        let index = ResolverIndex::new(vec![state_class("Service", "lib/service.dart", None)]);
        let issues = run_with_index(
            r#"
class Service {
  Future<void> load() async {
    await api.fetch();
    setState(() {});
  }
}
"#,
            file,
            &index,
        );

        assert!(issues.is_empty());
    }

    #[test]
    fn context_flags_transitive_state_subclass() {
        let file = PathBuf::from("lib/screen.dart");
        let index = ResolverIndex::new(vec![
            state_class("BaseState", "lib/base.dart", Some("State")),
            state_class("_ScreenState", "lib/screen.dart", Some("BaseState")),
        ]);
        let issues = run_with_index(
            r#"
class _ScreenState extends BaseState {
  Future<void> load() async {
    await api.fetch();
    setState(() {});
  }
}
"#,
            file,
            &index,
        );

        assert_eq!(issues.len(), 1);
    }
}
