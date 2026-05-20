use crate::config::Severity;
use crate::parser::node_start_line;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidEmptyCatch;

impl Rule for AvoidEmptyCatch {
    fn name(&self) -> &'static str {
        "avoid-empty-catch"
    }

    fn description(&self) -> &'static str {
        "Empty catch blocks silently swallow errors. Handle the exception or rethrow."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();
        find_empty_catches(root, source, file, &mut issues);
        issues
    }
}

fn find_empty_catches(node: Node, source: &str, file: &Path, issues: &mut Vec<Issue>) {
    // In tree-sitter-dart, try_statement has children:
    //   try, block (try body), catch_clause, block (catch body)
    // The catch body is a sibling block following catch_clause.
    if node.kind() == "try_statement" {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        for i in 0..children.len() {
            if children[i].kind() == "catch_clause" {
                if let Some(body_node) = children.get(i + 1) {
                    if body_node.kind() == "block" {
                        let body_text = body_node.utf8_text(source.as_bytes()).unwrap_or("").trim();
                        let inner = body_text
                            .strip_prefix('{')
                            .and_then(|s| s.strip_suffix('}'))
                            .unwrap_or("")
                            .trim();

                        if inner.is_empty() {
                            issues.push(Issue {
                                rule: "avoid-empty-catch".to_string(),
                                message: "Empty catch block — exceptions are silently swallowed."
                                    .to_string(),
                                severity: Severity::Error,
                                file: file.to_path_buf(),
                                line: node_start_line(children[i]),
                                column: children[i].start_position().column + 1,
                            });
                        } else if is_print_only(inner) {
                            issues.push(Issue {
                                rule: "avoid-empty-catch".to_string(),
                                message:
                                    "Catch block only prints the error — use a logger or handle properly."
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
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_empty_catches(child, source, file, issues);
    }
}

fn is_print_only(inner: &str) -> bool {
    let trimmed = inner.trim().trim_end_matches(';').trim();
    trimmed.starts_with("print(") && !trimmed.contains('\n')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    fn run(source: &str) -> Vec<Issue> {
        let rule = AvoidEmptyCatch;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    #[test]
    fn empty_catch_is_flagged() {
        let issues = run(r#"
void main() {
  try { doIt(); } catch (e) {}
}
"#);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "avoid-empty-catch");
    }

    #[test]
    fn print_only_catch_is_flagged() {
        let issues = run(r#"
void main() {
  try { doIt(); } catch (e) { print(e); }
}
"#);
        assert_eq!(issues.len(), 1, "print-only catch should be flagged");
        assert!(issues[0].message.contains("only prints"));
    }

    #[test]
    fn catch_with_logger_is_ok() {
        let issues = run(r#"
void main() {
  try {
    doIt();
  } catch (e) {
    logger.error('boom', e);
  }
}
"#);
        assert!(
            issues.is_empty(),
            "non-print, non-empty catch must not be flagged"
        );
    }
}
