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
