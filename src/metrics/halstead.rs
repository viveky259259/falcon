use crate::parser::walk_tree;
use std::collections::HashSet;
use tree_sitter::Node;

#[derive(Debug, Clone)]
pub struct HalsteadMetrics {
    pub unique_operators: u32,
    pub unique_operands: u32,
    pub total_operators: u32,
    pub total_operands: u32,
    pub vocabulary: u32,
    pub length: u32,
    pub volume: f64,
    pub difficulty: f64,
    pub effort: f64,
}

const OPERATOR_KINDS: &[&str] = &[
    "+", "-", "*", "/", "%", "~/", "==", "!=", "<", ">", "<=", ">=", "&&", "||", "!", "&", "|",
    "^", "~", "<<", ">>", "=", "+=", "-=", "*=", "/=", "%=", "??", "??=", "?.", "?", ".", "..",
    "...", "=>", "->", "as", "is", "is!", "++", "--",
];

const OPERATOR_NODE_KINDS: &[&str] = &[
    "if_statement",
    "for_statement",
    "while_statement",
    "do_statement",
    "switch_statement",
    "try_statement",
    "catch_clause",
    "return_statement",
    "throw_expression",
    "assert_statement",
    "yield_statement",
    "break_statement",
    "continue_statement",
];

pub fn calculate(node: Node, source: &str) -> HalsteadMetrics {
    let mut operators = Vec::new();
    let mut operands = Vec::new();

    walk_tree(node, &mut |n| match n.kind() {
        "identifier" | "type_identifier" => {
            operands.push(source[n.byte_range()].to_string());
        }
        "decimal_integer_literal"
        | "decimal_floating_point_literal"
        | "hex_integer_literal"
        | "null_literal"
        | "true"
        | "false" => {
            operands.push(source[n.byte_range()].to_string());
        }
        "string_literal" => {
            operands.push(source[n.byte_range()].to_string());
        }
        kind if OPERATOR_NODE_KINDS.contains(&kind) => {
            operators.push(kind.to_string());
        }
        _ => {
            let text = &source[n.byte_range()];
            if !n.is_named() && OPERATOR_KINDS.iter().any(|op| text == *op) {
                operators.push(text.to_string());
            }
        }
    });

    let unique_ops: HashSet<&str> = operators.iter().map(|s| s.as_str()).collect();
    let unique_opnds: HashSet<&str> = operands.iter().map(|s| s.as_str()).collect();

    let n1 = unique_ops.len() as u32;
    let n2 = unique_opnds.len() as u32;
    let big_n1 = operators.len() as u32;
    let big_n2 = operands.len() as u32;

    let vocabulary = n1 + n2;
    let length = big_n1 + big_n2;
    let volume = if vocabulary > 0 {
        (length as f64) * (vocabulary as f64).log2()
    } else {
        0.0
    };

    let difficulty = if n2 > 0 {
        (n1 as f64 / 2.0) * (big_n2 as f64 / n2.max(1) as f64)
    } else {
        0.0
    };

    let effort = volume * difficulty;

    HalsteadMetrics {
        unique_operators: n1,
        unique_operands: n2,
        total_operators: big_n1,
        total_operands: big_n2,
        vocabulary,
        length,
        volume: (volume * 100.0).round() / 100.0,
        difficulty: (difficulty * 100.0).round() / 100.0,
        effort: (effort * 100.0).round() / 100.0,
    }
}
