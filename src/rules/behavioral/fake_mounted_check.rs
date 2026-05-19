//! `fake-mounted-check` — detect `if (mounted) { ... }` blocks that contain
//! an `await` *without* a second `mounted` check after the await.
//!
//! The async gap caused by `await` means the State may be disposed by the
//! time the continuation runs, so the original `mounted` check is stale.
//! This is one of the canonical AI-failure modes in Flutter (LLMs emit the
//! check at the top of the method and forget the post-await re-check).
//!
//! Conservative MVP: walk every `if_statement` whose condition is exactly
//! the identifier `mounted`. If its block body contains an `await_expression`
//! and that block does NOT also contain a later `if (mounted)` test, flag
//! the outer `if`.

use crate::config::Severity;
use crate::parser::{node_start_line, node_text};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct FakeMountedCheck;

impl Rule for FakeMountedCheck {
    fn name(&self) -> &'static str {
        "fake-mounted-check"
    }

    fn description(&self) -> &'static str {
        "An `if (mounted)` block followed by `await` must re-check `mounted` afterward — the State may be disposed during the async gap."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        walk(root, source, file, &mut issues);
        issues
    }
}

fn walk(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    if node.kind() == "if_statement" && condition_is_mounted(node, source) {
        if let Some(body) = if_then_block(node) {
            let (has_await, first_await_byte) = first_await_position(body);
            if has_await && !has_post_await_mounted_check(body, source, first_await_byte) {
                issues.push(Issue {
                    rule: "fake-mounted-check".to_string(),
                    message:
                        "`if (mounted)` followed by `await` without a re-check — State may be disposed during the async gap."
                            .to_string(),
                    severity: Severity::Error,
                    file: file.to_path_buf(),
                    line: node_start_line(node),
                    column: node.start_position().column + 1,
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, file, issues);
    }
}

/// True iff the `if`'s condition is the bare identifier `mounted`.
fn condition_is_mounted(if_node: Node, source: &str) -> bool {
    let mut cursor = if_node.walk();
    for child in if_node.children(&mut cursor) {
        if child.kind() == "parenthesized_expression" {
            // Strip outer parens, look at the inner expression text.
            let text = node_text(child, source).trim();
            let inner = text
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or("")
                .trim();
            return inner == "mounted" || inner == "this.mounted";
        }
    }
    false
}

/// Returns the `then` block of an if-statement (the first `block` child
/// after the parenthesized condition).
fn if_then_block(if_node: Node) -> Option<Node> {
    let mut cursor = if_node.walk();
    let mut saw_paren = false;
    for child in if_node.children(&mut cursor) {
        if child.kind() == "parenthesized_expression" {
            saw_paren = true;
            continue;
        }
        if saw_paren && child.kind() == "block" {
            return Some(child);
        }
    }
    None
}

/// First `await_expression` descendant of `node`, returning its start byte.
fn first_await_position(node: Node) -> (bool, usize) {
    let mut found: Option<usize> = None;
    let mut cursor = node.walk();
    let mut stack: Vec<Node> = node.children(&mut cursor).collect();
    while let Some(n) = stack.pop() {
        if n.kind() == "await_expression" {
            let b = n.start_byte();
            found = Some(match found {
                Some(prev) => prev.min(b),
                None => b,
            });
        }
        // Don't descend into nested function bodies — their awaits are not
        // part of *this* control flow.
        if n.kind() == "function_body" {
            continue;
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    match found {
        Some(b) => (true, b),
        None => (false, 0),
    }
}

/// Is there an `if (mounted)` (or `if (!mounted)` early-return guard) at or
/// after `after_byte` inside `body`?
fn has_post_await_mounted_check(body: Node, source: &str, after_byte: usize) -> bool {
    let mut cursor = body.walk();
    let mut stack: Vec<Node> = body.children(&mut cursor).collect();
    while let Some(n) = stack.pop() {
        if n.start_byte() > after_byte && n.kind() == "if_statement" {
            // Look at the parenthesized condition's inner text.
            let mut c2 = n.walk();
            for child in n.children(&mut c2) {
                if child.kind() == "parenthesized_expression" {
                    let text = node_text(child, source).trim();
                    let inner = text
                        .strip_prefix('(')
                        .and_then(|s| s.strip_suffix(')'))
                        .unwrap_or("")
                        .trim();
                    if inner == "mounted"
                        || inner == "this.mounted"
                        || inner == "!mounted"
                        || inner == "!this.mounted"
                    {
                        return true;
                    }
                }
            }
        }
        if n.kind() == "function_body" {
            continue;
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
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
        let rule = FakeMountedCheck;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    // ---- positives -------------------------------------------------------

    #[test]
    fn mounted_then_await_without_recheck_is_flagged() {
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    if (mounted) {
      await fetchData();
      setState(() {});
    }
  }
}
"#);
        assert_eq!(issues.len(), 1, "should flag stale mounted check");
        assert_eq!(issues[0].rule, "fake-mounted-check");
    }

    #[test]
    fn mounted_then_await_then_unrelated_if_is_flagged() {
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    if (mounted) {
      await fetchData();
      if (loading) {
        setState(() {});
      }
    }
  }
}
"#);
        assert_eq!(
            issues.len(),
            1,
            "unrelated if after await does not satisfy re-check"
        );
    }

    // ---- negatives -------------------------------------------------------

    #[test]
    fn mounted_recheck_after_await_is_ok() {
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    if (mounted) {
      await fetchData();
      if (mounted) {
        setState(() {});
      }
    }
  }
}
"#);
        assert!(
            issues.is_empty(),
            "re-checking mounted after await is the correct pattern"
        );
    }

    #[test]
    fn mounted_without_await_is_ok() {
        let issues = run(r#"
class _S extends State<W> {
  void update() {
    if (mounted) {
      setState(() {});
    }
  }
}
"#);
        assert!(issues.is_empty(), "no await means no async gap to worry about");
    }
}
