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
//! `expression_statement` whose call starts with `Future.delayed(`,
//! `Future.wait(`, `Future.value(`, `Future(`, or contains a chained `.then(` /
//! `.catchError(` / `.whenComplete(`. The call must not be prefixed with
//! `await ` or wrapped in `unawaited(`.

use crate::config::Severity;
use crate::parser::node_start_line;
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
        walk_build_bodies(root, source, file, &mut issues);
        issues
    }
}

/// Walk the tree looking for class members that define a `build()` method,
/// then scan each matching method body for unawaited Futures.
///
/// tree-sitter-dart 0.1.x places `method_signature` and `function_body` as
/// sibling children of `class_member`, so we must locate them together.
fn walk_build_bodies(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    if node.kind() == "class_member" || node.kind() == "declaration" {
        if let Some(body) = extract_build_body(node, source) {
            scan_body(body, source, file, issues);
            return; // Don't recurse further into the matched member.
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_build_bodies(child, source, file, issues);
    }
}

/// Given a `class_member` (or `declaration`) node, return its `function_body`
/// if the node defines a method named `build`.
fn extract_build_body<'t>(member: Node<'t>, source: &str) -> Option<Node<'t>> {
    let mut sig_has_build = false;
    let mut body: Option<Node> = None;
    let mut cursor = member.walk();
    for child in member.children(&mut cursor) {
        match child.kind() {
            "method_signature" | "function_signature"
                if identifier_in_subtree(child, source, "build") =>
            {
                sig_has_build = true;
            }
            "function_body" => {
                body = Some(child);
            }
            _ => {}
        }
    }
    if sig_has_build {
        body
    } else {
        None
    }
}

/// Recursively checks whether any `identifier` node in `node`'s subtree has
/// the given text.
fn identifier_in_subtree(node: Node, source: &str, name: &str) -> bool {
    if node.kind() == "identifier" && &source[node.byte_range()] == name {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if identifier_in_subtree(child, source, name) {
            return true;
        }
    }
    false
}

fn scan_body(body: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    // `body` is a `function_body` node.  Iterate its direct children so the
    // top-level `function_body` itself does not trigger the nested-closure
    // guard inside `walk_for_expression_statements`.
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        walk_for_expression_statements(child, source, file, issues);
    }
}

fn walk_for_expression_statements(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    let kind = node.kind();
    // Skip nested closure bodies — their contents execute asynchronously,
    // not during the build call itself.
    if kind == "function_body" || kind == "function_expression_body" {
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
        assert!(
            issues.is_empty(),
            "unawaited(...) wrapper must not be flagged"
        );
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

    #[test]
    fn future_wait_in_build_is_flagged() {
        // Future.wait is a common pattern AI emits when "loading multiple things".
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    Future.wait([fetchA(), fetchB()]);
    return SizedBox();
  }
}
"#);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "unawaited-future-in-build");
    }

    #[test]
    fn future_in_initstate_is_ok() {
        // initState is the canonical correct location for kicking off async work.
        let issues = run(r#"
class _S extends State<W> {
  @override
  void initState() {
    super.initState();
    Future.delayed(Duration(seconds: 1));
  }

  @override
  Widget build(BuildContext context) => Container();
}
"#);
        assert!(
            issues.is_empty(),
            "rule must only fire inside build(), not initState()"
        );
    }

    #[test]
    fn catch_error_chain_in_build_is_flagged() {
        let issues = run(r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    fetchData().catchError((e) => print(e));
    return SizedBox();
  }
}
"#);
        assert_eq!(issues.len(), 1);
    }
}
