use crate::config::Severity;
use crate::parser::DartParser;
use crate::reporters::Issue;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Detect dead code paths: unreachable code after return/throw/break/continue,
/// and always-true/always-false conditions.
pub fn detect_dead_code(root: &Path, exclude: &[glob::Pattern]) -> Vec<Issue> {
    let mut issues = Vec::new();

    let dart_files: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    for file in &dart_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut parser = match DartParser::new() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let tree = match parser.parse(&source) {
            Some(t) => t,
            None => continue,
        };

        detect_unreachable_code(tree.root_node(), &source, file, &mut issues);
        detect_trivial_conditions(tree.root_node(), &source, file, &mut issues);
    }

    issues
}

fn detect_unreachable_code(
    node: tree_sitter::Node,
    source: &str,
    file: &Path,
    issues: &mut Vec<Issue>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "block" {
            check_block_for_unreachable(child, source, file, issues);
        }
        detect_unreachable_code(child, source, file, issues);
    }
}

fn check_block_for_unreachable(
    block: tree_sitter::Node,
    _source: &str,
    file: &Path,
    issues: &mut Vec<Issue>,
) {
    let mut cursor = block.walk();
    let children: Vec<tree_sitter::Node> = block.children(&mut cursor).collect();

    let terminal_kinds = [
        "return_statement",
        "throw_statement",
        "break_statement",
        "continue_statement",
        "rethrow_expression",
    ];

    for (i, child) in children.iter().enumerate() {
        let kind = child.kind();
        if terminal_kinds.contains(&kind) {
            for unreachable in &children[i + 1..] {
                let uk = unreachable.kind();
                if uk == "}" || uk == "comment" || uk == "{" {
                    continue;
                }
                issues.push(Issue {
                    rule: "dead-code-path".to_string(),
                    message: format!(
                        "Unreachable code after {} statement.",
                        kind.replace("_statement", "").replace("_expression", "")
                    ),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: unreachable.start_position().row + 1,
                    column: unreachable.start_position().column + 1,
                });
                break;
            }
        }
    }
}

fn detect_trivial_conditions(
    node: tree_sitter::Node,
    source: &str,
    file: &Path,
    issues: &mut Vec<Issue>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "if_statement" {
            let full_text = child.utf8_text(source.as_bytes()).unwrap_or("");

            let cond_text = extract_if_condition(full_text);

            if !cond_text.is_empty() {
                let line = child.start_position().row + 1;
                let col = child.start_position().column + 1;

                if cond_text == "true" {
                    issues.push(Issue {
                        rule: "dead-code-path".to_string(),
                        message: "Condition is always true — else branch is unreachable."
                            .to_string(),
                        severity: Severity::Info,
                        file: file.to_path_buf(),
                        line,
                        column: col,
                    });
                } else if cond_text == "false" {
                    issues.push(Issue {
                        rule: "dead-code-path".to_string(),
                        message: "Condition is always false — if branch is unreachable."
                            .to_string(),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line,
                        column: col,
                    });
                }
            }
        }

        detect_trivial_conditions(child, source, file, issues);
    }
}

fn extract_if_condition(if_text: &str) -> String {
    let trimmed = if_text.trim();
    if !trimmed.starts_with("if") {
        return String::new();
    }
    if let Some(paren_start) = trimmed.find('(') {
        let rest = &trimmed[paren_start + 1..];
        let mut depth = 1;
        let mut end = 0;
        for (i, ch) in rest.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end > 0 {
            return rest[..end].trim().to_string();
        }
    }
    String::new()
}
