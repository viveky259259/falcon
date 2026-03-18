use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line, walk_tree};
use std::collections::HashSet;
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
        if node.parent().map_or(true, |p| {
            p.parent().map_or(true, |gp| gp.kind() == "program")
        }) {
            if let Some(name) = dart_ast::get_declaration_name(node, source) {
                declarations.push((name.to_string(), node_start_line(node)));
            }
        }
    }

    declarations
}

pub fn collect_references(root: Node, source: &str) -> HashSet<String> {
    let mut references = HashSet::new();

    walk_tree(root, &mut |node| {
        if node.kind() == "identifier" || node.kind() == "type_identifier" {
            let text = &source[node.byte_range()];
            references.insert(text.to_string());
        }
    });

    references
}
