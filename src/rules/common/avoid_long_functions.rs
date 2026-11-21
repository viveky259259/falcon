use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidLongFunctions {
    max_lines: u32,
}

impl Default for AvoidLongFunctions {
    fn default() -> Self {
        Self { max_lines: 50 }
    }
}

impl Rule for AvoidLongFunctions {
    fn name(&self) -> &'static str {
        "avoid-long-functions"
    }

    fn description(&self) -> &'static str {
        "Functions should not exceed the configured maximum number of lines."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn configure(&mut self, options: &HashMap<String, serde_yaml::Value>) {
        if let Some(val) = options.get("max-lines") {
            if let Some(n) = val.as_u64() {
                self.max_lines = n as u32;
            }
        }
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let functions = find_descendants_by_kind(root, "function_signature");
        let methods = find_descendants_by_kind(root, "method_signature");

        for node in functions.into_iter().chain(methods) {
            if let Some(span) = function_span(node) {
                let text = &source[span];
                let line_count = text.lines().count() as u32;
                if line_count > self.max_lines {
                    let name =
                        dart_ast::get_declaration_name(node, source).unwrap_or("<anonymous>");
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Function '{}' has {} lines (max: {})",
                            name, line_count, self.max_lines
                        ),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(node),
                        column: 1,
                    });
                }
            }
        }

        issues
    }
}

/// Byte range of a function from its signature to the end of its body.
///
/// The body is the signature's next sibling (`function_body`). For a
/// top-level function both hang off `source_file`, so the parent can't be used:
/// it spans the whole file. A `function_signature` nested in a
/// `method_signature` is skipped; the method signature is measured instead.
fn function_span(signature: Node) -> Option<std::ops::Range<usize>> {
    let parent = signature.parent()?;
    if parent.kind() == "method_signature" {
        return None;
    }
    let mut next = signature.next_sibling();
    while let Some(n) = next {
        if n.kind() == "function_body" {
            return Some(signature.start_byte()..n.end_byte());
        }
        if n.is_named() && n.kind() != "comment" {
            break;
        }
        next = n.next_sibling();
    }
    if parent.kind() == "source_file" {
        return Some(signature.byte_range());
    }
    Some(parent.byte_range())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    fn run(source: &str, max: u32) -> Vec<Issue> {
        let rule = AvoidLongFunctions { max_lines: max };
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }

    fn filler(n: usize) -> String {
        (0..n).map(|i| format!("final v{i} = {i};\n")).collect()
    }

    #[test]
    fn short_top_level_function_is_not_measured_to_end_of_file() {
        let src = format!("void main() => runApp(App());\n{}", filler(60));
        assert!(run(&src, 50).is_empty());
    }

    #[test]
    fn long_top_level_function_is_reported_with_its_own_length() {
        let body: String = (0..55).map(|i| format!("  print({i});\n")).collect();
        let src = format!("void big() {{\n{body}}}\n{}", filler(10));
        let issues = run(&src, 50);
        assert_eq!(issues.len(), 1);
        assert!(
            issues[0].message.contains("'big' has 57 lines"),
            "{}",
            issues[0].message
        );
    }

    #[test]
    fn long_method_is_reported_once() {
        let body: String = (0..55).map(|i| format!("    print({i});\n")).collect();
        let src = format!("class A {{\n  void m() {{\n{body}  }}\n  void n() {{}}\n}}\n");
        let issues = run(&src, 50);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].message.contains("'m'"));
    }
}
