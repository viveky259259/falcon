use crate::config::Severity;
use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct PreferMatchFileName;

impl Rule for PreferMatchFileName {
    fn name(&self) -> &'static str {
        "prefer-match-file-name"
    }

    fn description(&self) -> &'static str {
        "The primary class/enum/mixin name should match the file name."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let file_stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        if file_stem.is_empty() {
            return issues;
        }

        let expected = to_pascal_case(file_stem);

        let classes = find_descendants_by_kind(root, "class_declaration");
        let enums = find_descendants_by_kind(root, "enum_declaration");

        let all_decls: Vec<Node> = classes.into_iter().chain(enums.into_iter()).collect();

        if all_decls.len() == 1 {
            let decl = all_decls[0];
            if let Some(name) = dart_ast::get_declaration_name(decl, source) {
                if name != expected {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!(
                            "Type '{}' does not match file name '{}' (expected '{}')",
                            name, file_stem, expected
                        ),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: node_start_line(decl),
                        column: 1,
                    });
                }
            }
        }

        issues
    }
}

fn to_pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}
