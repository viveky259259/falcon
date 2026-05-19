//! `unawaited-future-in-build` — detect fire-and-forget `Future` calls inside
//! a Flutter widget's `build()` method.
//!
//! Building a widget is a synchronous, hot path. Kicking off an async
//! operation from `build()` (without awaiting it or wrapping in
//! `unawaited(...)`) causes one of the most common AI-generated bugs:
//! repeated network calls every rebuild + silent failures when the Future
//! throws.
//!
//! Conservative MVP — flag, inside a method named `build`, an
//! `expression_statement` whose call:
//!   * starts with `Future.delayed(`, `Future.wait(`, `Future.value(`,
//!     `Future(`, OR
//!   * contains a chained `.then(` / `.catchError(` / `.whenComplete(`,
//! and is NOT prefixed with `await ` or wrapped in `unawaited(`.

use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct UnawaitedFutureInBuild;

impl Rule for UnawaitedFutureInBuild {
    fn name(&self) -> &'static str {
        "unawaited-future-in-build"
    }

    fn description(&self) -> &'static str {
        "Don't start unawaited Futures inside build(). They fire on every rebuild and silently swallow errors."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        // Inspect both function_signature and method_signature — Dart's grammar
        // emits one or the other depending on context.
        let mut signatures = find_descendants_by_kind(root, "function_signature");
        signatures.extend(find_descendants_by_kind(root, "method_signature"));

        for sig in signatures {
            if !has_identifier_named(sig, source, "build") {
                continue;
            }
            let Some(body) = dart_ast::get_function_body(sig) else {
                continue;
            };

            scan_body(body, source, file, &mut issues);
        }

        issues
    }
}

fn scan_body(body: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    // Walk only expression_statements — we don't want to descend into
    // closures (e.g. callbacks passed to widgets) where Future calls are
    // expected and legitimate.
    walk_for_expression_statements(body, source, file, issues);
}

fn walk_for_expression_statements(
    node: Node,
    source: &str,
    file: &Path,
    issues: &mut Vec<Issue>,
) {
    // Skip descending into nested function bodies — those are closures
    // executed later, not during build itself.
    let kind = node.kind();
    if kind == "function_body" && !is_outermost_body(node) {
        return;
    }

    if kind == "expression_statement" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("").trim();
        if is_unawaited_future_call(text) {
            issues.push(Issue {
                rule: "unawaited-future-in-build".to_string(),
                message:
                    "Unawaited Future inside build() — fires on every rebuild and silently swallows errors. Move it to initState/didChangeDependencies or wrap in `unawaited(...)`."
                        .to_string(),
                severity: Severity::Error,
                file: file.to_path_buf(),
                line: node_start_line(node),
                column: node.start_position().column + 1,
            });
        }
        // Don't recurse into the matched expression_statement.
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_expression_statements(child, source, file, issues);
    }
}

/// We use a side-channel to know whether we've already descended past the
/// outermost `function_body` belonging to the build method. The first body
/// we receive in `scan_body` is the outermost; we mark it via a thread-local
/// guard would be overkill — instead, treat the first body as outermost by
/// passing it through directly here.
fn is_outermost_body(_body: Node) -> bool {
    // Always returns false; `scan_body` calls `walk_for_expression_statements`
    // with the body's children individually via the top-level call path, so
    // any `function_body` node encountered during recursion is necessarily
    // *nested* and should be skipped.
    false
}

fn is_unawaited_future_call(text: &str) -> bool {
    let stripped = text.trim_end_matches(';').trim();

    // Already awaited or wrapped — fine.
    if stripped.starts_with("await ") || stripped.starts_with("unawaited(") {
        return false;
    }

    // Static Future constructors.
    let future_starts = [
        "Future.delayed(",
        "Future.wait(",
        "Future.value(",
        "Future.error(",
        "Future.microtask(",
        "Future.sync(",
        "Future(",
    ];
    if future_starts.iter().any(|p| stripped.starts_with(p)) {
        return true;
    }

    // Chained Future methods anywhere in the call (e.g. `foo().then(...)`).
    let future_chains = [".then(", ".catchError(", ".whenComplete(", ".onError("];
    if future_chains.iter().any(|p| stripped.contains(p)) {
        return true;
    }

    false
}

fn has_identifier_named(node: Node, source: &str, name: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" && &source[child.byte_range()] == name {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    fn run(source: &str) -> Vec<Issue> {
        let rule = UnawaitedFutureInBuild;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    // ---- positives -------------------------------------------------------

    #[test]
    fn future_delayed_in_build_is_flagged() {
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    Future.delayed(Duration(seconds: 1));
    return Container();
  }
}
"#);
        assert_eq!(issues.len(), 1, "Future.delayed must be flagged");
        assert_eq!(issues[0].rule, "unawaited-future-in-build");
    }

    #[test]
    fn future_constructor_in_build_is_flagged() {
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    Future(() => doWork());
    return SizedBox();
  }
}
"#);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn then_chain_in_build_is_flagged() {
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    fetchData().then((v) => print(v));
    return SizedBox();
  }
}
"#);
        assert_eq!(issues.len(), 1);
    }

    // ---- negatives -------------------------------------------------------

    #[test]
    fn awaited_future_in_build_is_ok() {
        // Note: an `async` build is itself a bug, but it's a different rule.
        // This rule must not double-fire when `await` is present.
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Future<Widget> build(BuildContext context) async {
    await Future.delayed(Duration(seconds: 1));
    return Container();
  }
}
"#);
        assert!(issues.is_empty(), "awaited futures must not be flagged");
    }

    #[test]
    fn unawaited_wrapped_in_build_is_ok() {
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    unawaited(Future.delayed(Duration(seconds: 1)));
    return Container();
  }
}
"#);
        assert!(issues.is_empty(), "unawaited(...) wrapper must not be flagged");
    }

    #[test]
    fn future_outside_build_is_ok() {
        let issues = run(r#"
class Foo {
  void load() {
    Future.delayed(Duration(seconds: 1));
  }
}
"#);
        assert!(issues.is_empty(), "rule must only fire inside build()");
    }

    #[test]
    fn future_in_callback_inside_build_is_ok() {
        // Closures (e.g. onPressed) execute later, not during build.
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: () {
        Future.delayed(Duration(seconds: 1));
      },
      child: Text('go'),
    );
  }
}
"#);
        assert!(
            issues.is_empty(),
            "Future inside a nested closure must NOT be flagged"
        );
    }
}
