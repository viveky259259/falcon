use crate::parser::{dart_ast, node_text, walk_tree};
use std::collections::HashSet;
use tree_sitter::Node;

/// Coupling Between Objects: count of distinct types referenced by a class
/// (via field types, parameter types, return types, local variable types).
pub fn coupling_between_objects(class_node: Node, source: &str) -> u32 {
    let class_name = dart_ast::get_declaration_name(class_node, source).unwrap_or("");
    let mut referenced_types: HashSet<String> = HashSet::new();

    walk_tree(class_node, &mut |node| {
        if node.kind() == "type_identifier" {
            let text = &source[node.byte_range()];
            if text != class_name
                && !is_builtin_type(text)
                && text.chars().next().map_or(false, |c| c.is_uppercase())
            {
                referenced_types.insert(text.to_string());
            }
        }
    });

    referenced_types.len() as u32
}

/// Depth of Inheritance Tree: number of superclass levels.
/// Only counts the immediate `extends` clause (deeper resolution needs full project context).
pub fn depth_of_inheritance(class_node: Node, _source: &str) -> u32 {
    let mut cursor = class_node.walk();
    for child in class_node.children(&mut cursor) {
        if child.kind() == "superclass" {
            return 1;
        }
    }
    0
}

/// Number of interfaces implemented via `implements` clause.
pub fn number_of_interfaces(class_node: Node, _source: &str) -> u32 {
    let mut cursor = class_node.walk();
    for child in class_node.children(&mut cursor) {
        if child.kind() == "interfaces" {
            let mut count = 0u32;
            let mut ic = child.walk();
            for iface_child in child.children(&mut ic) {
                if iface_child.kind() == "type_identifier" || iface_child.kind() == "identifier" {
                    count += 1;
                }
            }
            return count;
        }
    }
    0
}

/// Number of methods with `@override` annotation.
pub fn number_of_overridden_methods(class_node: Node, source: &str) -> u32 {
    let mut count = 0u32;
    let mut cursor = class_node.walk();
    for child in class_node.children(&mut cursor) {
        if child.kind() == "class_body" {
            let mut body_cursor = child.walk();
            for member in child.children(&mut body_cursor) {
                if member.kind() == "class_member" {
                    let member_text = node_text(member, source);
                    if member_text.contains("@override") {
                        let mut mc = member.walk();
                        for mc_child in member.children(&mut mc) {
                            if dart_ast::is_function_like(mc_child) {
                                count += 1;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    count
}

/// Number of Added Methods: methods that do NOT have @override.
pub fn number_of_added_methods(class_node: Node, source: &str) -> u32 {
    let total = dart_ast::get_class_methods(class_node).len() as u32;
    let overridden = number_of_overridden_methods(class_node, source);
    total.saturating_sub(overridden)
}

/// Response For Class: number of distinct methods + distinct methods called by those methods.
pub fn response_for_class(class_node: Node, source: &str) -> u32 {
    let methods = dart_ast::get_class_methods(class_node);
    let own_count = methods.len() as u32;

    let mut called_methods: HashSet<String> = HashSet::new();
    for method in &methods {
        if let Some(body) = dart_ast::get_function_body(*method) {
            walk_tree(body, &mut |node| {
                if node.kind() == "identifier" {
                    let text = &source[node.byte_range()];
                    if let Some(parent) = node.parent() {
                        if parent.kind() == "selector"
                            || parent.kind() == "unconditional_assignable_selector"
                        {
                            called_methods.insert(text.to_string());
                        }
                    }
                }
            });
        }
    }

    own_count + called_methods.len() as u32
}

/// Weight of Class: ratio of public methods to total methods.
pub fn weight_of_class(class_node: Node, source: &str) -> f64 {
    let methods = dart_ast::get_class_methods(class_node);
    if methods.is_empty() {
        return 0.0;
    }

    let public_count = methods
        .iter()
        .filter(|m| {
            dart_ast::get_declaration_name(**m, source)
                .map(|name| !name.starts_with('_'))
                .unwrap_or(false)
        })
        .count();

    let ratio = public_count as f64 / methods.len() as f64;
    (ratio * 100.0).round() / 100.0
}

/// Weighted Methods per Class: sum of cyclomatic complexities of all methods.
pub fn weighted_methods_per_class(class_node: Node, source: &str) -> u32 {
    let methods = dart_ast::get_class_methods(class_node);
    let mut total = 0u32;

    for method in methods {
        if let Some(body) = dart_ast::get_function_body(method) {
            total += crate::metrics::cyclomatic::calculate(body, source);
        } else {
            total += 1;
        }
    }

    total
}

/// Tight Class Cohesion: fraction of method pairs that access at least one common field.
pub fn tight_class_cohesion(class_node: Node, source: &str) -> f64 {
    let fields = collect_class_fields(class_node, source);
    let methods = dart_ast::get_class_methods(class_node);

    if methods.len() < 2 || fields.is_empty() {
        return 1.0;
    }

    let method_fields: Vec<HashSet<String>> = methods
        .iter()
        .map(|m| {
            let mut used = HashSet::new();
            if let Some(body) = dart_ast::get_function_body(*m) {
                walk_tree(body, &mut |node| {
                    if node.kind() == "identifier" {
                        let text = &source[node.byte_range()];
                        if fields.contains(text) {
                            used.insert(text.to_string());
                        }
                    }
                });
            }
            used
        })
        .collect();

    let n = method_fields.len();
    let total_pairs = n * (n - 1) / 2;
    if total_pairs == 0 {
        return 1.0;
    }

    let mut connected = 0u32;
    for i in 0..n {
        for j in (i + 1)..n {
            if !method_fields[i].is_disjoint(&method_fields[j]) {
                connected += 1;
            }
        }
    }

    let ratio = connected as f64 / total_pairs as f64;
    (ratio * 100.0).round() / 100.0
}

/// Lack of Cohesion of Methods (LCOM):
/// number of method pairs with no shared field access minus
/// number of pairs with shared field access (clamped to 0).
pub fn lack_of_cohesion(class_node: Node, source: &str) -> u32 {
    let fields = collect_class_fields(class_node, source);
    let methods = dart_ast::get_class_methods(class_node);

    if methods.len() < 2 || fields.is_empty() {
        return 0;
    }

    let method_fields: Vec<HashSet<String>> = methods
        .iter()
        .map(|m| {
            let mut used = HashSet::new();
            if let Some(body) = dart_ast::get_function_body(*m) {
                walk_tree(body, &mut |node| {
                    if node.kind() == "identifier" {
                        let text = &source[node.byte_range()];
                        if fields.contains(text) {
                            used.insert(text.to_string());
                        }
                    }
                });
            }
            used
        })
        .collect();

    let n = method_fields.len();
    let mut p = 0i32; // pairs with no shared fields
    let mut q = 0i32; // pairs with shared fields

    for i in 0..n {
        for j in (i + 1)..n {
            if method_fields[i].is_disjoint(&method_fields[j]) {
                p += 1;
            } else {
                q += 1;
            }
        }
    }

    if p > q {
        (p - q) as u32
    } else {
        0
    }
}

fn collect_class_fields(class_node: Node, source: &str) -> HashSet<String> {
    let mut fields = HashSet::new();

    let mut cursor = class_node.walk();
    for child in class_node.children(&mut cursor) {
        if child.kind() == "class_body" {
            let mut body_cursor = child.walk();
            for member in child.children(&mut body_cursor) {
                if member.kind() == "class_member" {
                    let mut mc = member.walk();
                    for mc_child in member.children(&mut mc) {
                        if !dart_ast::is_function_like(mc_child)
                            && mc_child.kind() != "constructor_signature"
                            && mc_child.kind() != "function_body"
                        {
                            walk_tree(mc_child, &mut |node| {
                                if node.kind() == "identifier" {
                                    let text = &source[node.byte_range()];
                                    if !is_type_keyword(text) {
                                        fields.insert(text.to_string());
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
    }

    fields
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "int"
            | "double"
            | "String"
            | "bool"
            | "void"
            | "dynamic"
            | "num"
            | "Object"
            | "Null"
            | "List"
            | "Map"
            | "Set"
            | "Future"
            | "Stream"
            | "Iterable"
            | "Type"
            | "Function"
            | "Symbol"
            | "Never"
            | "Record"
    )
}

fn is_type_keyword(name: &str) -> bool {
    matches!(
        name,
        "int"
            | "double"
            | "String"
            | "bool"
            | "void"
            | "dynamic"
            | "num"
            | "var"
            | "final"
            | "const"
            | "late"
            | "static"
            | "required"
            | "abstract"
            | "override"
    )
}
