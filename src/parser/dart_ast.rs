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
    // `.find()` cannot be used: the yielded `Node` borrows from `cursor`,
    // so returning it from a closure outlives the cursor (E0597).
    #[allow(clippy::manual_find)]
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
    find_named_type_after_keyword(node, source, "extends")
}

pub fn get_class_mixins(node: Node, source: &str) -> Vec<String> {
    find_named_types_after_keyword(node, source, "with")
}

pub fn get_class_interfaces(node: Node, source: &str) -> Vec<String> {
    find_named_types_after_keyword(node, source, "implements")
}

pub fn get_method_names(node: Node, source: &str) -> Vec<String> {
    get_class_methods(node)
        .into_iter()
        .filter_map(|method| get_declaration_name(method, source).map(ToString::to_string))
        .collect()
}

fn find_named_type_after_keyword(node: Node, source: &str, keyword: &str) -> Option<String> {
    find_named_types_after_keyword(node, source, keyword)
        .into_iter()
        .next()
}

fn find_named_types_after_keyword(node: Node, source: &str, keyword: &str) -> Vec<String> {
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");
    let Some(after_keyword) = text.split_once(keyword).map(|(_, after)| after) else {
        return Vec::new();
    };
    let mut end = after_keyword
        .find(['{', '\n'])
        .unwrap_or(after_keyword.len());
    for boundary in [" extends ", " with ", " implements "] {
        if boundary.trim() != keyword {
            if let Some(pos) = after_keyword.find(boundary) {
                end = end.min(pos);
            }
        }
    }
    after_keyword[..end]
        .split([',', '<', '>', ' '])
        .map(str::trim)
        .filter(|part| {
            !part.is_empty()
                && *part != "with"
                && *part != "implements"
                && part
                    .chars()
                    .next()
                    .is_some_and(|ch| ch == '_' || ch.is_alphabetic())
        })
        .map(ToString::to_string)
        .collect()
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
}
