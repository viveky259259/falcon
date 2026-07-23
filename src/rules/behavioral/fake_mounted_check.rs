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

/// Extract the text of an if-statement's condition, stripping the surrounding
/// parentheses.  Handles both grammar styles:
///   - tree-sitter-dart 0.1.x: condition is bare `(` … `)` children
///   - tree-sitter-dart ≥ 0.2.x: condition is a `parenthesized_expression`
fn extract_if_condition(if_node: Node, source: &str) -> Option<String> {
    let mut cursor = if_node.walk();
    let mut open_end: Option<usize> = None;
    for child in if_node.children(&mut cursor) {
        if child.kind() == "parenthesized_expression" {
            let text = node_text(child, source);
            let trimmed = text.trim();
            return Some(
                trimmed
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or(trimmed)
                    .trim()
                    .to_string(),
            );
        }
        if child.kind() == "(" {
            open_end = Some(child.end_byte());
        }
        if child.kind() == ")" {
            if let Some(start) = open_end {
                return Some(source[start..child.start_byte()].trim().to_string());
            }
        }
    }
    None
}

/// True iff the `if`'s condition is the bare identifier `mounted`.
fn condition_is_mounted(if_node: Node, source: &str) -> bool {
    matches!(
        extract_if_condition(if_node, source).as_deref(),
        Some("mounted") | Some("this.mounted")
    )
}

/// Returns the `then` block of an if-statement (the first `block` child
/// after the condition ends).  Works for both grammar styles.
fn if_then_block(if_node: Node) -> Option<Node> {
    let mut cursor = if_node.walk();
    let mut past_condition = false;
    for child in if_node.children(&mut cursor) {
        match child.kind() {
            // New grammar: whole condition is one parenthesized_expression node.
            "parenthesized_expression" => {
                past_condition = true;
            }
            // Old grammar (0.1.x): closing paren is the last piece of the
            // bare `( … )` condition.
            ")" => {
                past_condition = true;
            }
            "block" if past_condition => {
                return Some(child);
            }
            _ => {}
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
            if let Some(condition) = extract_if_condition(n, source) {
                if condition == "mounted" || condition == "this.mounted" {
                    return true;
                }
                // Negated guard only satisfies the re-check when the branch
                // is proven to exit early (return/throw) — otherwise a bare
                // `if (!mounted) { … }` can continue execution and still
                // operate on a disposed State.
                if (condition == "!mounted" || condition == "!this.mounted")
                    && if_consequent_exits(n)
                {
                    return true;
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

/// True iff the consequent (then-branch) of an if-statement contains an
/// early-exit statement (`return` or `throw`) as a direct child.
fn if_consequent_exits(if_node: Node) -> bool {
    let mut cursor = if_node.walk();
    let mut past_condition = false;
    for child in if_node.children(&mut cursor) {
        match child.kind() {
            "parenthesized_expression" | ")" => {
                past_condition = true;
            }
            k if past_condition && k != "else" => {
                return block_has_exit(child);
            }
            _ => {}
        }
    }
    false
}

/// True iff `node` is, or directly contains as a child, a return or throw.
///
/// In Dart's grammar `throw X;` is an `expression_statement` wrapping a
/// `throw_expression`, so we unwrap that layer before comparing kinds.
fn block_has_exit(node: Node) -> bool {
    if node.kind() == "return_statement"
        || node.kind() == "throw_statement"
        || node.kind() == "throw_expression"
    {
        return true;
    }
    // `throw X;` as a lone statement arrives as expression_statement(throw_expression).
    if node.kind() == "expression_statement" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "throw_expression" || child.kind() == "throw_statement" {
                return true;
            }
        }
    }
    if node.kind() == "block" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "return_statement"
                || child.kind() == "throw_statement"
                || child.kind() == "throw_expression"
            {
                return true;
            }
            // Unwrap expression_statement(throw_expression) inside a block too.
            if child.kind() == "expression_statement" {
                let mut c2 = child.walk();
                for grandchild in child.children(&mut c2) {
                    if grandchild.kind() == "throw_expression"
                        || grandchild.kind() == "throw_statement"
                    {
                        return true;
                    }
                }
            }
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
        assert!(
            issues.is_empty(),
            "no await means no async gap to worry about"
        );
    }

    #[test]
    fn negated_mounted_with_early_return_is_ok() {
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    if (mounted) {
      await fetchData();
      if (!mounted) return;
      setState(() {});
    }
  }
}
"#);
        assert!(
            issues.is_empty(),
            "if (!mounted) return; is a valid early-exit guard"
        );
    }

    #[test]
    fn negated_mounted_with_early_throw_is_ok() {
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    if (mounted) {
      await fetchData();
      if (!mounted) throw StateError('disposed');
      setState(() {});
    }
  }
}
"#);
        assert!(
            issues.is_empty(),
            "if (!mounted) throw ...; is a valid early-exit guard"
        );
    }

    #[test]
    fn negated_mounted_without_early_exit_is_flagged() {
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    if (mounted) {
      await fetchData();
      if (!mounted) {
        log('not mounted');
      }
      setState(() {});
    }
  }
}
"#);
        assert_eq!(
            issues.len(),
            1,
            "if (!mounted) without early exit does not satisfy the re-check"
        );
    }

    #[test]
    fn nested_mounted_recheck_inside_re_guarded_block_is_ok() {
        // Canonical pattern: outer if(mounted) → await → inner if(mounted) { setState }.
        // This is the EXACT shape the rule was designed to bless.
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
            "outer mounted + post-await mounted re-check is correct"
        );
    }

    #[test]
    fn early_negated_mounted_return_before_setstate_is_ok() {
        // `if (!mounted) return;` as a guard immediately after await is the
        // most idiomatic correct pattern; it must never be flagged.
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    if (mounted) {
      await fetchData();
      if (!mounted) return;
      setState(() {});
    }
  }
}
"#);
        assert!(
            issues.is_empty(),
            "early-out `if (!mounted) return;` is the correct shape"
        );
    }

    #[test]
    fn no_mounted_check_at_all_is_not_flagged() {
        // The rule narrowly targets the *fake* mounted check.  Methods that
        // never wrap in if (mounted) at all are out of scope here (other rules
        // catch them).
        let issues = run(r#"
class _S extends State<W> {
  Future<void> load() async {
    await fetchData();
    setState(() {});
  }
}
"#);
        assert!(
            issues.is_empty(),
            "method with no outer if(mounted) should not be flagged by this rule"
        );
    }
}
