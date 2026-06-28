use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line, walk_tree};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

pub fn collect_declarations(root: Node, source: &str) -> Vec<(String, usize)> {
    let mut declarations = Vec::new();

    let classes = find_descendants_by_kind(root, "class_declaration");
    for node in classes {
        if let Some(name) = dart_ast::get_declaration_name(node, source) {
            declarations.push((name.to_string(), node_start_line(node)));
        }
    }

    let enums = find_descendants_by_kind(root, "enum_declaration");
    for node in enums {
        if let Some(name) = dart_ast::get_declaration_name(node, source) {
            declarations.push((name.to_string(), node_start_line(node)));
        }
    }

    let mixins = find_descendants_by_kind(root, "mixin_declaration");
    for node in mixins {
        if let Some(name) = dart_ast::get_declaration_name(node, source) {
            declarations.push((name.to_string(), node_start_line(node)));
        }
    }

    let extensions = find_descendants_by_kind(root, "extension_declaration");
    for node in extensions {
        if let Some(name) = dart_ast::get_declaration_name(node, source) {
            declarations.push((name.to_string(), node_start_line(node)));
        }
    }

    let type_aliases = find_descendants_by_kind(root, "type_alias");
    for node in type_aliases {
        if let Some(name) = dart_ast::get_declaration_name(node, source) {
            declarations.push((name.to_string(), node_start_line(node)));
        }
    }

    let functions = find_descendants_by_kind(root, "function_signature");
    for node in functions {
        if is_top_level_function(node) {
            if let Some(name) = dart_ast::get_declaration_name(node, source) {
                declarations.push((name.to_string(), node_start_line(node)));
            }
        }
    }

    declarations
}

fn is_top_level_function(node: Node) -> bool {
    let Some(parent) = node.parent() else {
        return true;
    };
    if parent.kind() == "source_file" || parent.kind() == "program" {
        return true;
    }
    let Some(grandparent) = parent.parent() else {
        return true;
    };
    grandparent.kind() == "source_file" || grandparent.kind() == "program"
}

/// Collects all identifier references, excluding names at declaration sites.
/// Returns a map of (identifier_name -> set of files where it's referenced).
pub fn collect_references_by_file(
    root: Node,
    source: &str,
    file: &Path,
) -> HashMap<String, HashSet<PathBuf>> {
    let mut refs: HashMap<String, HashSet<PathBuf>> = HashMap::new();

    let declaration_names = collect_declaration_name_positions(root, source);

    walk_tree(root, &mut |node| {
        if node.kind() != "identifier" && node.kind() != "type_identifier" {
            return;
        }

        let byte_start = node.start_byte();
        if declaration_names.contains(&byte_start) {
            return;
        }

        let text = &source[node.byte_range()];
        refs.entry(text.to_string())
            .or_default()
            .insert(file.to_path_buf());
    });

    refs
}

/// Returns byte offsets of identifiers that are the declared name of a
/// class, enum, mixin, extension, type alias, or top-level function.
fn collect_declaration_name_positions(root: Node, _source: &str) -> HashSet<usize> {
    let mut positions = HashSet::new();

    let decl_kinds = [
        "class_declaration",
        "enum_declaration",
        "mixin_declaration",
        "extension_declaration",
        "type_alias",
    ];

    for kind in &decl_kinds {
        for node in find_descendants_by_kind(root, kind) {
            if let Some(name_node) = first_identifier_child(node) {
                positions.insert(name_node.start_byte());
            }
        }
    }

    for node in find_descendants_by_kind(root, "function_signature") {
        if is_top_level_function(node) {
            if let Some(name_node) = first_identifier_child(node) {
                positions.insert(name_node.start_byte());
            }
        }
    }

    positions
}

fn first_identifier_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    let first = node
        .children(&mut cursor)
        .find(|&child| child.kind() == "identifier" || child.kind() == "type_identifier");
    first
}
