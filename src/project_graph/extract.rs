use super::{identity, types::*};
use crate::parser::{dart_ast, find_descendants_by_kind, walk_tree};
use std::collections::HashMap;
use tree_sitter::Node;

pub(super) fn extract(file: &mut ParsedFile) {
    collect_declarations(file);
    file.directives = directives(file.tree.root_node(), &file.source);
    extract_types(file);
    extract_calls(file);
}
fn collect_declarations(file: &mut ParsedFile) {
    let root = file.tree.root_node();
    let file_id = identity(&["file", &file.path]);
    let mut declarations = Vec::new();
    walk_tree(root, &mut |node| {
        let kind = match node.kind() {
            "class_declaration" => NodeKind::Class,
            "mixin_declaration" => NodeKind::Mixin,
            "enum_declaration" => NodeKind::Enum,
            "extension_declaration" => NodeKind::Extension,
            "type_alias" => NodeKind::TypeAlias,
            "function_signature" => {
                if ancestor_class(node) {
                    NodeKind::Method
                } else {
                    NodeKind::Function
                }
            }
            "constructor_signature" => NodeKind::Constructor,
            "initialized_identifier" | "initialized_variable_definition" => NodeKind::Variable,
            "formal_parameter" => NodeKind::Parameter,
            "type_parameter" => NodeKind::TypeParameter,
            _ => return,
        };
        let parameter_name = if kind == NodeKind::Parameter {
            find_descendants_by_kind(node, "identifier")
                .last()
                .map(|name| &file.source[name.byte_range()])
        } else {
            None
        };
        let name = parameter_name.or_else(|| {
            node.child_by_field_name("name")
                .map(|name| &file.source[name.byte_range()])
                .or_else(|| dart_ast::get_declaration_name(node, &file.source))
        });
        let Some(name) = name else {
            return;
        };
        let mut span = SourceSpan::at(node);
        if matches!(
            kind,
            NodeKind::Function | NodeKind::Method | NodeKind::Constructor
        ) {
            if let Some(body) = function_body(node) {
                span.end_byte = body.end_byte();
                span.end_line = body.end_position().row + 1;
                span.end_column = body.end_position().column + 1;
            }
        }
        declarations.push((kind, name.to_owned(), span));
    });
    // Outer declarations precede nested ones; location does not enter unique symbol IDs.
    declarations.sort_by_key(|(_, _, span)| (span.start_byte, std::cmp::Reverse(span.end_byte)));
    let mut occurrences: HashMap<String, usize> = HashMap::new();
    for (kind, name, span) in declarations {
        let owner = file
            .declarations
            .iter()
            .rev()
            .find(|d| d.span.start_byte <= span.start_byte && d.span.end_byte >= span.end_byte);
        let qualified = owner.map_or_else(
            || name.clone(),
            |owner| format!("{}.{}", owner.qualified, name),
        );
        let owner = owner.map_or_else(|| file_id.clone(), |owner| owner.id.clone());
        let key = format!("{kind:?}:{qualified}");
        let ordinal = occurrences.entry(key.clone()).or_default();
        let id = identity(&["symbol", &file.path, &key, &ordinal.to_string()]);
        *ordinal += 1;
        file.declarations.push(Declaration {
            id,
            name,
            qualified,
            kind,
            span,
            owner,
        });
    }
}
fn extract_types(file: &mut ParsedFile) {
    let root = file.tree.root_node();
    for node in find_descendants_by_kind(root, "class_declaration") {
        let Some(class) = file
            .declarations
            .iter()
            .find(|d| d.kind == NodeKind::Class && d.span.start_byte == node.start_byte())
        else {
            continue;
        };
        for (syntax, kind) in [
            ("superclass", EdgeKind::Extends),
            ("mixins", EdgeKind::MixesIn),
            ("interfaces", EdgeKind::Implements),
        ] {
            for clause in find_descendants_by_kind(node, syntax) {
                if clause.start_byte()
                    >= node
                        .child_by_field_name("body")
                        .map_or(node.end_byte(), |body| body.start_byte())
                {
                    continue;
                }
                for (target, span) in clause_types(clause, &file.source) {
                    file.relations.push(Relation {
                        kind,
                        from: class.id.clone(),
                        target,
                        span,
                    });
                }
            }
        }
    }
}
fn extract_calls(file: &mut ParsedFile) {
    let root = file.tree.root_node();
    for args in find_descendants_by_kind(root, "arguments") {
        let Some(selector) = args
            .parent()
            .and_then(|p| p.parent())
            .filter(|n| n.kind() == "selector")
        else {
            continue;
        };
        let Some(previous) = selector.prev_named_sibling() else {
            continue;
        };
        let target = &file.source[previous.byte_range()];
        let Some(owner) = file.declarations.iter().rev().find(|d| {
            matches!(
                d.kind,
                NodeKind::Function | NodeKind::Method | NodeKind::Constructor
            ) && d.span.start_byte <= args.start_byte()
                && d.span.end_byte >= args.end_byte()
        }) else {
            continue;
        };
        // Receiver calls cannot be resolved by terminal name without type information.
        let target = if previous.kind() == "identifier" {
            target.to_owned()
        } else {
            format!("<receiver>{target}")
        };
        file.relations.push(Relation {
            kind: EdgeKind::Calls,
            from: owner.id.clone(),
            target,
            span: SourceSpan::at(previous),
        });
    }
}
fn ancestor_class(mut node: Node) -> bool {
    while let Some(parent) = node.parent() {
        if dart_ast::is_class_like(parent) {
            return true;
        }
        node = parent;
    }
    false
}
fn function_body(node: Node) -> Option<Node> {
    let signature = if node
        .parent()
        .is_some_and(|p| p.kind() == "method_signature")
    {
        node.parent()?
    } else {
        node
    };
    signature
        .next_named_sibling()
        .filter(|n| n.kind() == "function_body")
        .or_else(|| dart_ast::get_function_body(node))
}
fn clause_types(clause: Node, source: &str) -> Vec<(String, SourceSpan)> {
    let mut cursor = clause.walk();
    let types: Vec<_> = clause
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "type_identifier")
        .collect();
    let mut names: Vec<(String, SourceSpan)> = Vec::new();
    for node in types {
        if let Some((name, span)) = names
            .last_mut()
            .filter(|(_, span)| source[span.end_byte..node.start_byte()].trim() == ".")
        {
            name.push('.');
            name.push_str(&source[node.byte_range()]);
            span.end_byte = node.end_byte();
            span.end_column = node.end_position().column + 1;
        } else {
            names.push((source[node.byte_range()].into(), SourceSpan::at(node)));
        }
    }
    names
}
fn directives(root: Node, source: &str) -> Vec<Directive> {
    let mut result = Vec::new();
    walk_tree(root, &mut |node| {
        if !matches!(
            node.kind(),
            "library_import" | "library_export" | "part_directive"
        ) {
            return;
        }
        let kind = match node.kind() {
            "library_import" => EdgeKind::Imports,
            "library_export" => EdgeKind::Exports,
            _ => EdgeKind::Part,
        };
        let uris = find_descendants_by_kind(node, "uri");
        let prefix = find_descendants_by_kind(node, "import_specification")
            .first()
            .and_then(|spec| spec.child_by_field_name("alias"))
            .map(|n| source[n.byte_range()].to_owned());
        let mut show: Option<Vec<String>> = None;
        let mut hide = Vec::new();
        for combinator in find_descendants_by_kind(node, "combinator") {
            let names = find_descendants_by_kind(combinator, "identifier")
                .into_iter()
                .map(|n| source[n.byte_range()].to_owned());
            if source[combinator.byte_range()]
                .trim_start()
                .starts_with("show")
            {
                let names: Vec<String> = names.collect();
                match &mut show {
                    Some(allowed) => allowed.retain(|name| names.contains(name)),
                    None => show = Some(names),
                }
            } else {
                hide.extend(names);
            }
        }
        let conditional = uris.len() > 1;
        for uri in uris {
            let text = source[uri.byte_range()].trim();
            let text = text.strip_prefix('r').unwrap_or(text);
            let value = text.trim_matches(['\'', '"']).to_owned();
            result.push(Directive {
                kind,
                uri: value,
                prefix: prefix.clone(),
                show: show.clone(),
                hide: hide.clone(),
                conditional,
                span: SourceSpan::at(uri),
                target: None,
            });
        }
    });
    result
}
