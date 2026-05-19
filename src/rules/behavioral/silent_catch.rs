//! `silent-catch` — detect `catch` clauses whose body is empty or contains
//! only comments / trivia. These swallow exceptions silently and are a
//! classic AI-failure mode (LLMs emit `catch (_) {}` to make tests pass).
//!
//! This is intentionally a *behavioral* sibling of `avoid-empty-catch`:
//! it focuses on the runtime consequence (silent failure) and flags
//! comment-only bodies that the syntactic rule misses.

use crate::config::Severity;
use crate::parser::node_start_line;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct SilentCatch;

impl Rule for SilentCatch {
    fn name(&self) -> &'static str {
        "silent-catch"
    }

    fn description(&self) -> &'static str {
        "Catch blocks that are empty or comment-only swallow exceptions silently. Log, rethrow, or handle the error."
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
    if node.kind() == "try_statement" {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        for i in 0..children.len() {
            if children[i].kind() == "catch_clause" {
                // The catch body is the immediately following `block` sibling.
                if let Some(body) = children.get(i + 1) {
                    if body.kind() == "block" && is_effectively_empty(*body, source) {
                        issues.push(Issue {
                            rule: "silent-catch".to_string(),
                            message:
                                "Catch block is empty or comment-only — exceptions are silently swallowed."
                                    .to_string(),
                            severity: Severity::Error,
                            file: file.to_path_buf(),
                            line: node_start_line(children[i]),
                            column: children[i].start_position().column + 1,
                        });
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, file, issues);
    }
}

/// A block is "effectively empty" when stripping `{`, `}`, whitespace, and
/// comment trivia leaves nothing executable.
fn is_effectively_empty(block: Node, source: &str) -> bool {
    // Fast path: inspect raw text first.
    let text = block.utf8_text(source.as_bytes()).unwrap_or("").trim();
    let inner = text
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or("")
        .trim();
    if inner.is_empty() {
        return true;
    }

    // Walk children: a non-empty block with only `comment` named children is
    // still effectively empty.
    let mut cursor = block.walk();
    for child in block.named_children(&mut cursor) {
        if child.kind() != "comment" {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    fn run(source: &str) -> Vec<Issue> {
        let rule = SilentCatch;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    // ---- positives -------------------------------------------------------

    #[test]
    fn empty_catch_body_is_flagged() {
        let issues = run(r#"
void main() {
  try { doIt(); } catch (e) {}
}
"#);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "silent-catch");
    }

    #[test]
    fn comment_only_catch_body_is_flagged() {
        let issues = run(r#"
void main() {
  try {
    doIt();
  } catch (e) {
    // TODO: handle this later
  }
}
"#);
        assert_eq!(issues.len(), 1, "comment-only body should be flagged");
    }

    #[test]
    fn typed_on_clause_empty_body_is_flagged() {
        let issues = run(r#"
void main() {
  try {
    doIt();
  } on FormatException catch (e) {
  }
}
"#);
        assert_eq!(issues.len(), 1, "empty body after typed on-clause should fire");
    }

    // ---- negatives -------------------------------------------------------

    #[test]
    fn catch_with_logging_is_ok() {
        let issues = run(r#"
void main() {
  try { doIt(); } catch (e) { log(e.toString()); }
}
"#);
        assert!(issues.is_empty(), "non-empty catch must not be flagged");
    }

    #[test]
    fn catch_with_rethrow_is_ok() {
        let issues = run(r#"
void main() {
  try {
    doIt();
  } catch (e) {
    rethrow;
  }
}
"#);
        assert!(issues.is_empty());
    }

    #[test]
    fn no_try_statement_no_issues() {
        let issues = run(r#"
void main() { print('hi'); }
"#);
        assert!(issues.is_empty());
    }
}
