use crate::config::Severity;
use crate::parser::{node_start_line, walk_tree};
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

const CALLBACK_PARAMS: &[&str] = &[
    "onPressed",
    "onTap",
    "onChanged",
    "onSubmitted",
    "onSaved",
    "onLongPress",
    "onDoubleTap",
    "onDismissed",
    "onRefresh",
    "onSelected",
];

pub struct PreferExtractingCallbacks;

impl Rule for PreferExtractingCallbacks {
    fn name(&self) -> &'static str {
        "prefer-extracting-callbacks"
    }

    fn description(&self) -> &'static str {
        "Extract long inline callbacks into named methods for better readability."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        walk_tree(root, &mut |node| {
            if node.kind() == "named_argument" {
                let text = &source[node.byte_range()];
                for param in CALLBACK_PARAMS {
                    if text.starts_with(param) && contains_multiline_closure(node, source) {
                        issues.push(Issue {
                            rule: self.name().to_string(),
                            message: format!(
                                "Extract '{}' callback into a named method.",
                                param
                            ),
                            severity: self.default_severity(),
                            file: file.to_path_buf(),
                            line: node_start_line(node),
                            column: node.start_position().column + 1,
                        });
                        break;
                    }
                }
            }
        });

        issues
    }
}

fn contains_multiline_closure(node: Node, _source: &str) -> bool {
    let start = node.start_position().row;
    let end = node.end_position().row;
    end - start > 3
}
