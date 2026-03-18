use crate::parser::DartParser;
use crate::reporters::Issue;
use crate::config::Severity;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Detect unused method/function parameters across the project.
pub fn detect_unused_params(root: &Path, exclude: &[glob::Pattern]) -> Vec<Issue> {
    let mut issues = Vec::new();

    let dart_files: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
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

        let root_node = tree.root_node();
        find_unused_params_in_node(root_node, &source, file, &mut issues);
    }

    issues
}

fn find_unused_params_in_node(
    node: tree_sitter::Node,
    source: &str,
    file: &Path,
    issues: &mut Vec<Issue>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "function_signature" || kind == "method_signature" {
            check_function_params(child, node, source, file, issues);
        }
        find_unused_params_in_node(child, source, file, issues);
    }
}

fn check_function_params(
    sig_node: tree_sitter::Node,
    parent: tree_sitter::Node,
    source: &str,
    file: &Path,
    issues: &mut Vec<Issue>,
) {
    let func_name = get_func_name(sig_node, source);

    let skip_names: HashSet<&str> = [
        "main", "build", "createState", "initState", "dispose",
        "didChangeDependencies", "didUpdateWidget", "deactivate",
        "reassemble", "toString", "hashCode", "noSuchMethod",
    ].into_iter().collect();

    if skip_names.contains(func_name.as_str()) {
        return;
    }

    if func_name.starts_with('_') {
        return;
    }

    let params = collect_param_names(sig_node, source);
    if params.is_empty() {
        return;
    }

    let body = find_body_after_sig(sig_node, parent, source);
    if body.is_empty() {
        return;
    }

    for (param_name, param_line) in &params {
        if param_name.starts_with('_') {
            continue;
        }
        if !body.contains(param_name.as_str()) {
            issues.push(Issue {
                rule: "unused-method-parameter".to_string(),
                message: format!(
                    "Parameter '{}' in function '{}' is not used.",
                    param_name, func_name
                ),
                severity: Severity::Info,
                file: file.to_path_buf(),
                line: *param_line,
                column: 1,
            });
        }
    }
}

fn get_func_name(sig_node: tree_sitter::Node, source: &str) -> String {
    let mut cursor = sig_node.walk();
    for child in sig_node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        }
        if child.kind() == "function_signature" {
            let mut inner = child.walk();
            for inner_child in child.children(&mut inner) {
                if inner_child.kind() == "identifier" {
                    return inner_child.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                }
            }
        }
    }
    String::new()
}

fn collect_param_names(sig_node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut params = Vec::new();
    collect_params_recursive(sig_node, source, &mut params);
    params
}

fn collect_params_recursive(
    node: tree_sitter::Node,
    source: &str,
    params: &mut Vec<(String, usize)>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();

        if kind == "formal_parameter" || kind == "simple_formal_parameter"
            || kind == "default_formal_parameter" || kind == "field_formal_parameter"
        {
            let mut inner = child.walk();
            let mut last_ident = None;
            for inner_child in child.children(&mut inner) {
                if inner_child.kind() == "identifier" {
                    last_ident = Some((
                        inner_child.utf8_text(source.as_bytes()).unwrap_or("").to_string(),
                        inner_child.start_position().row + 1,
                    ));
                }
            }
            if let Some(param) = last_ident {
                if !param.0.is_empty() {
                    params.push(param);
                }
            }
        } else {
            collect_params_recursive(child, source, params);
        }
    }
}

fn find_body_after_sig(
    sig_node: tree_sitter::Node,
    parent: tree_sitter::Node,
    source: &str,
) -> String {
    let sig_end = sig_node.end_byte();
    let parent_end = parent.end_byte();
    if sig_end < parent_end && sig_end < source.len() {
        source[sig_end..parent_end.min(source.len())].to_string()
    } else {
        String::new()
    }
}
