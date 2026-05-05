use crate::config::Severity;
use crate::parser::node_start_line;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidUnawaitedFutures;

impl Rule for AvoidUnawaitedFutures {
    fn name(&self) -> &'static str {
        "avoid-unawaited-futures"
    }

    fn description(&self) -> &'static str {
        "Future-returning functions should be awaited. Fire-and-forget calls cause silent failures."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        find_unawaited(root, source, file, &mut issues, false);
        issues
    }
}

fn find_unawaited(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>, in_await: bool) {
    if node.kind() == "await_expression" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            find_unawaited(child, source, file, issues, true);
        }
        return;
    }

    if node.kind() == "expression_statement" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("").trim();

        if !in_await && !text.starts_with("await ") && !text.starts_with("unawaited(") {
            let is_future_call = is_likely_future_call(text);
            if is_future_call {
                issues.push(Issue {
                    rule: "avoid-unawaited-futures".to_string(),
                    message: "Future not awaited — may cause silent failures. Add `await` or wrap in `unawaited()`.".to_string(),
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
        find_unawaited(child, source, file, issues, in_await);
    }
}

fn is_likely_future_call(text: &str) -> bool {
    let trimmed = text.trim_end_matches(';').trim();

    let future_patterns = [
        "Future.delayed",
        "Future.wait",
        "Future.value",
        ".then(",
        ".whenComplete(",
        ".catchError(",
    ];
    for pattern in &future_patterns {
        if trimmed.contains(pattern) {
            return true;
        }
    }

    let async_method_prefixes = [
        "fetch",
        "load",
        "save",
        "send",
        "post",
        "get",
        "put",
        "delete",
        "upload",
        "download",
        "submit",
        "request",
        "init",
        "connect",
        "disconnect",
        "authenticate",
        "signIn",
        "signOut",
        "navigate",
    ];

    let call_name = trimmed.split('(').next().unwrap_or("");
    let method_name = call_name.split('.').last().unwrap_or(call_name);

    for prefix in &async_method_prefixes {
        if method_name.starts_with(prefix) && trimmed.contains('(') && !trimmed.contains("=>") {
            return true;
        }
    }

    false
}
