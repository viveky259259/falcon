use crate::config::Severity;
use crate::parser::{find_descendants_by_kind, missing_trailing_comma_offset, node_start_line};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct PreferTrailingComma;

impl Rule for PreferTrailingComma {
    fn name(&self) -> &'static str {
        "prefer-trailing-comma"
    }

    fn description(&self) -> &'static str {
        "Multi-line parameter/argument lists should end with a trailing comma."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, _source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        let param_lists = find_descendants_by_kind(root, "formal_parameter_list");
        let arg_lists = find_descendants_by_kind(root, "arguments");

        for list in param_lists.into_iter().chain(arg_lists) {
            let start_line = list.start_position().row;
            let end_line = list.end_position().row;
            if start_line == end_line {
                continue;
            }

            if missing_trailing_comma_offset(list).is_some() {
                issues.push(Issue {
                    rule: self.name().to_string(),
                    message: "Multi-line list should end with a trailing comma.".to_string(),
                    severity: self.default_severity(),
                    file: file.to_path_buf(),
                    line: node_start_line(list),
                    column: list.start_position().column + 1,
                });
            }
        }

        issues
    }
}
