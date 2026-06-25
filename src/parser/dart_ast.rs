use tree_sitter::Node;

pub const CLASS_DECLARATION: &str = "class_declaration";
pub const FUNCTION_DECLARATION: &str = "function_signature";
pub const FUNCTION_BODY: &str = "function_body";
pub const METHOD_DECLARATION: &str = "method_signature";
pub const CONSTRUCTOR_DECLARATION: &str = "constructor_signature";
pub const FORMAL_PARAMETER_LIST: &str = "formal_parameter_list";
pub const FORMAL_PARAMETER: &str = "formal_parameter";
pub const BLOCK: &str = "block";
pub const IF_STATEMENT: &str = "if_statement";
pub const FOR_STATEMENT: &str = "for_statement";
pub const WHILE_STATEMENT: &str = "while_statement";
pub const DO_STATEMENT: &str = "do_statement";
pub const SWITCH_STATEMENT: &str = "switch_statement";
pub const SWITCH_CASE: &str = "switch_expression_case";
pub const TRY_STATEMENT: &str = "try_statement";
pub const CATCH_CLAUSE: &str = "catch_clause";
pub const CONDITIONAL_EXPRESSION: &str = "conditional_expression";
pub const BINARY_EXPRESSION: &str = "binary_expression";
pub const RETURN_STATEMENT: &str = "return_statement";
pub const VARIABLE_DECLARATION: &str = "initialized_variable_definition";
pub const IMPORT_DIRECTIVE: &str = "import_or_export";
pub const LIBRARY_DIRECTIVE: &str = "library_name";
pub const PART_DIRECTIVE: &str = "part_directive";
pub const PART_OF_DIRECTIVE: &str = "part_of_directive";
pub const ENUM_DECLARATION: &str = "enum_declaration";
pub const MIXIN_DECLARATION: &str = "mixin_declaration";
pub const EXTENSION_DECLARATION: &str = "extension_declaration";
pub const TYPE_ALIAS: &str = "type_alias";
pub const TOP_LEVEL_DEFINITION: &str = "top_level_definition";
pub const IDENTIFIER: &str = "identifier";
pub const TYPE_IDENTIFIER: &str = "type_identifier";
pub const STRING_LITERAL: &str = "string_literal";
pub const NUMBER_LITERAL: &str = "decimal_integer_literal";
pub const FLOATING_POINT_LITERAL: &str = "decimal_floating_point_literal";
pub const HEX_LITERAL: &str = "hex_integer_literal";
pub const COMMENT: &str = "comment";
pub const DOCUMENTATION_COMMENT: &str = "documentation_comment";
pub const EXPRESSION_STATEMENT: &str = "expression_statement";
pub const INVOCATION: &str = "selector";
pub const ARGUMENT_LIST: &str = "arguments";
pub const ASSIGNMENT_EXPRESSION: &str = "assignment_expression";
pub const CLASS_BODY: &str = "class_body";
pub const DECLARATION: &str = "declaration";
pub const SUPER_CLASS: &str = "superclass";
pub const INTERFACES: &str = "interfaces";
pub const MIXINS: &str = "mixins";

pub fn is_function_like(node: Node) -> bool {
    matches!(
        node.kind(),
        "function_signature" | "method_signature" | "constructor_signature"
    )
}

pub fn is_class_like(node: Node) -> bool {
    matches!(
        node.kind(),
        "class_declaration" | "enum_declaration" | "mixin_declaration" | "extension_declaration"
    )
}

pub fn is_declaration(node: Node) -> bool {
    matches!(
        node.kind(),
        "class_declaration"
            | "enum_declaration"
            | "mixin_declaration"
            | "extension_declaration"
            | "type_alias"
            | "function_signature"
            | "initialized_variable_definition"
    )
}

pub fn is_control_flow(node: Node) -> bool {
    matches!(
        node.kind(),
        "if_statement"
            | "for_statement"
            | "while_statement"
            | "do_statement"
            | "switch_statement"
            | "try_statement"
    )
}

pub fn get_declaration_name<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return Some(&source[child.byte_range()]);
        }
    }
    // For method_signature, the name lives inside a nested function_signature
    let mut cursor2 = node.walk();
    for child in node.children(&mut cursor2) {
        if child.kind() == "function_signature" {
            let mut inner = child.walk();
            for inner_child in child.children(&mut inner) {
                if inner_child.kind() == "identifier" {
                    return Some(&source[inner_child.byte_range()]);
                }
            }
        }
    }
    None
}

pub fn get_function_parameters(node: Node) -> Vec<Node> {
    let mut params = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "formal_parameter_list" {
            let mut param_cursor = child.walk();
            for param_child in child.children(&mut param_cursor) {
                if param_child.kind() == "formal_parameter" {
                    params.push(param_child);
                }
            }
            break;
        }
    }

    params
}

pub fn get_function_body(node: Node) -> Option<Node> {
    let parent = node.parent()?;
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.kind() == "function_body" {
            return Some(child);
        }
    }
    None
}

pub fn get_class_methods(node: Node) -> Vec<Node> {
    let mut methods = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body" {
            let mut body_cursor = child.walk();
            for body_child in child.children(&mut body_cursor) {
                if body_child.kind() == "class_member" || body_child.kind() == "declaration" {
                    let mut decl_cursor = body_child.walk();
                    for decl_child in body_child.children(&mut decl_cursor) {
                        if is_function_like(decl_child) {
                            methods.push(decl_child);
                        }
                    }
                }
            }
        }
    }
    methods
}

pub fn get_class_superclass(node: Node, source: &str) -> Option<String> {
    class_relation_names(node, source, SUPER_CLASS)
        .into_iter()
        .next()
}

pub fn get_class_mixins(node: Node, source: &str) -> Vec<String> {
    class_relation_names(node, source, MIXINS)
}

pub fn get_class_interfaces(node: Node, source: &str) -> Vec<String> {
    class_relation_names(node, source, INTERFACES)
}

pub fn get_method_names(node: Node, source: &str) -> Vec<String> {
    get_class_methods(node)
        .into_iter()
        .filter_map(|method| get_declaration_name(method, source).map(ToString::to_string))
        .collect()
}

fn class_relation_names(node: Node, source: &str, relation_kind: &str) -> Vec<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == relation_kind {
            return relation_clause_names(&source[child.byte_range()]);
        }
    }
    class_header_relation_names(&source[node.byte_range()], relation_kind)
}

fn class_header_relation_names(class_text: &str, relation_kind: &str) -> Vec<String> {
    let keyword = match relation_kind {
        SUPER_CLASS => "extends",
        MIXINS => "with",
        INTERFACES => "implements",
        _ => return Vec::new(),
    };

    let header = class_text.split('{').next().unwrap_or(class_text);
    let Some(start) = find_keyword(header, keyword) else {
        return Vec::new();
    };

    let after_keyword = &header[start + keyword.len()..];
    let end = match keyword {
        "extends" => next_keyword_start(after_keyword, &["with", "implements"]),
        "with" => next_keyword_start(after_keyword, &["implements"]),
        "implements" => None,
        _ => None,
    }
    .unwrap_or(after_keyword.len());

    relation_clause_names(after_keyword[..end].trim())
}

fn find_keyword(haystack: &str, keyword: &str) -> Option<usize> {
    haystack.match_indices(keyword).find_map(|(idx, _)| {
        let before = haystack[..idx].chars().next_back();
        let after = haystack[idx + keyword.len()..].chars().next();
        let before_ok = before.is_none_or(|c| !is_ident_char(c));
        let after_ok = after.is_none_or(|c| !is_ident_char(c));
        (before_ok && after_ok).then_some(idx)
    })
}

fn next_keyword_start(haystack: &str, keywords: &[&str]) -> Option<usize> {
    keywords
        .iter()
        .filter_map(|keyword| find_keyword(haystack, keyword))
        .min()
}

fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

fn relation_clause_names(clause: &str) -> Vec<String> {
    let body = clause
        .trim()
        .strip_prefix("extends")
        .or_else(|| clause.trim().strip_prefix("with"))
        .or_else(|| clause.trim().strip_prefix("implements"))
        .unwrap_or_else(|| clause.trim());

    body.split(',').filter_map(normalize_type_name).collect()
}

fn normalize_type_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let end = trimmed
        .find(|c: char| c == '<' || c == '?' || c.is_whitespace())
        .unwrap_or(trimmed.len());
    let name = trimmed[..end].trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    //! Helper coverage for AST classification predicates and name extraction.
    use super::*;
    use crate::parser::{find_descendants_by_kind, DartParser};

    fn parse(src: &str) -> tree_sitter::Tree {
        let mut p = DartParser::new().unwrap();
        p.parse(src).unwrap()
    }

    #[test]
    fn classification_predicates_split_class_like_and_function_like() {
        let src = r#"
            class C {}
            enum E { a, b }
            mixin M {}
            extension X on String { void foo() {} }
            int top() => 1;
        "#;
        let tree = parse(src);
        let root = tree.root_node();

        for kind in [
            "class_declaration",
            "enum_declaration",
            "mixin_declaration",
            "extension_declaration",
        ] {
            for n in find_descendants_by_kind(root, kind) {
                assert!(is_class_like(n), "{} should be class-like", kind);
                assert!(is_declaration(n), "{} should be a declaration", kind);
                assert!(!is_function_like(n), "{} should NOT be function-like", kind);
            }
        }

        let fns = find_descendants_by_kind(root, "function_signature");
        assert!(!fns.is_empty(), "expected at least one function_signature");
        for n in fns {
            assert!(is_function_like(n));
            assert!(!is_class_like(n));
        }
    }

    #[test]
    fn get_declaration_name_extracts_class_name_and_handles_anonymous() {
        let src = "class Widget {} void main() { { } }";
        let tree = parse(src);
        let root = tree.root_node();

        let class = find_descendants_by_kind(root, "class_declaration")
            .into_iter()
            .next()
            .expect("class_declaration");
        assert_eq!(get_declaration_name(class, src), Some("Widget"));

        // A `block` has no identifier child, so name extraction returns None.
        let blocks = find_descendants_by_kind(root, "block");
        assert!(!blocks.is_empty());
        for b in blocks {
            assert!(get_declaration_name(b, src).is_none());
        }
    }

    #[test]
    fn function_param_and_class_method_helpers_agree_with_grammar() {
        // Two-arg function → two formal parameters.
        let src1 = "int add(int a, int b) => a + b;";
        let tree1 = parse(src1);
        let sig = find_descendants_by_kind(tree1.root_node(), "function_signature")
            .into_iter()
            .next()
            .expect("function_signature");
        assert_eq!(get_function_parameters(sig).len(), 2);

        // `class A` has two methods (`m`, `n`); `field` must NOT be counted.
        let src2 = "class A { void m() {} int n(int x) => x; int field = 0; }";
        let tree2 = parse(src2);
        let class = find_descendants_by_kind(tree2.root_node(), "class_declaration")
            .into_iter()
            .next()
            .unwrap();
        let methods = get_class_methods(class);
        assert!(methods.len() >= 2, "got {}", methods.len());
        for m in methods {
            assert!(is_function_like(m));
        }
    }

    #[test]
    fn class_relationship_helpers_extract_extends_with_implements() {
        let src = "class _S extends State<W> with M, N implements I, J { void dispose() {} }";
        let tree = parse(src);
        let class = find_descendants_by_kind(tree.root_node(), "class_declaration")
            .into_iter()
            .next()
            .unwrap();

        assert_eq!(get_class_superclass(class, src), Some("State".to_string()));
        assert_eq!(get_class_mixins(class, src), vec!["M", "N"]);
        assert_eq!(get_class_interfaces(class, src), vec!["I", "J"]);
        assert_eq!(get_method_names(class, src), vec!["dispose"]);
    }
}
