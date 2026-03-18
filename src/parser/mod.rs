pub mod dart_ast;
pub mod visitor;

use anyhow::Result;
use tree_sitter::{Node, Parser, Tree};

pub struct DartParser {
    parser: Parser,
}

impl DartParser {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_dart::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| anyhow::anyhow!("Failed to set Dart language: {}", e))?;
        Ok(Self { parser })
    }

    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }
}

pub fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

pub fn node_start_line(node: Node) -> usize {
    node.start_position().row + 1
}

pub fn node_end_line(node: Node) -> usize {
    node.end_position().row + 1
}

pub fn find_children_by_kind<'a>(node: Node<'a>, kind: &str) -> Vec<Node<'a>> {
    let mut results = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            results.push(child);
        }
    }
    results
}

pub fn find_descendants_by_kind<'a>(node: Node<'a>, kind: &str) -> Vec<Node<'a>> {
    let mut results = Vec::new();
    collect_descendants(node, kind, &mut results);
    results
}

fn collect_descendants<'a>(node: Node<'a>, kind: &str, results: &mut Vec<Node<'a>>) {
    if node.kind() == kind {
        results.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_descendants(child, kind, results);
    }
}

pub fn walk_tree<F>(node: Node, callback: &mut F)
where
    F: FnMut(Node),
{
    callback(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree(child, callback);
    }
}
