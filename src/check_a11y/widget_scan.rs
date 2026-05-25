//! Find interactive Flutter widgets and check whether each is wrapped in a
//! `Semantics(identifier: ...)` within 3 ancestor widgets.

use crate::parser::DartParser;
use tree_sitter::Node;

/// The set of widget identifiers Falcon considers "interactive" for Maestro
/// testing purposes. Wrapping any of these in `Semantics(identifier: ...)`
/// makes them targetable from Maestro flows.
pub const INTERACTIVE_WIDGETS: &[&str] = &[
    "ElevatedButton",
    "TextButton",
    "OutlinedButton",
    "IconButton",
    "FloatingActionButton",
    "TextField",
    "TextFormField",
    "GestureDetector",
    "InkWell",
    "InkResponse",
    "DropdownButton",
    "DropdownButtonFormField",
    "Checkbox",
    "CheckboxListTile",
    "Switch",
    "SwitchListTile",
    "Radio",
    "RadioListTile",
];

/// Maximum number of enclosing widgets we consider when looking for a wrapping
/// `Semantics(identifier:)`. The spec's "within 3 ancestors" rule.
const ANCESTOR_LOOKBACK: usize = 3;

/// A single interactive-widget occurrence found in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetOccurrence {
    /// Which interactive widget kind (e.g. "TextField").
    pub kind: String,
    /// 1-based line number where the widget appears.
    pub line: usize,
    /// Whether one of the nearest `ANCESTOR_LOOKBACK` enclosing widgets is
    /// `Semantics(identifier: ...)`.
    pub wrapped: bool,
}

/// Walk a Dart source string and return every interactive-widget occurrence
/// together with whether it is wrapped in `Semantics(identifier:)`.
pub fn scan_widgets(source: &str) -> Vec<WidgetOccurrence> {
    let mut parser = match DartParser::new() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let tree = match parser.parse(source) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut occurrences = Vec::new();
    walk(tree.root_node(), source, &mut occurrences);
    occurrences
}

fn walk<'a>(node: Node<'a>, source: &str, out: &mut Vec<WidgetOccurrence>) {
    // Check if this node is an identifier that is the start of a widget call
    // (i.e. next sibling is a selector with argument_part).
    if let Some(name) = widget_call_name_at_identifier(node, source) {
        if INTERACTIVE_WIDGETS.iter().any(|w| *w == name) {
            let wrapped = nearest_semantics_with_identifier(node, source, ANCESTOR_LOOKBACK);
            out.push(WidgetOccurrence {
                kind: name.to_string(),
                line: node.start_position().row + 1,
                wrapped,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, out);
    }
}

/// Look up at most `lookback` widget calls in the ancestor chain (via
/// tree-sitter parent links) and check whether any is `Semantics(...)` with
/// an `identifier:` argument.
///
/// In the tree-sitter-dart grammar, a widget call like `Semantics(identifier:
/// 'x', child: ...)` does NOT produce a single wrapper node. Instead the
/// widget name appears as an `identifier` sibling of the `selector` node that
/// holds the argument list.  We therefore walk up the parent chain and, for
/// every ancestor, inspect whether any immediate child is the `Semantics`
/// identifier followed by a `selector` that contains `identifier:`.
fn nearest_semantics_with_identifier(node: Node<'_>, source: &str, lookback: usize) -> bool {
    let mut current = node.parent();
    let mut widgets_seen = 0usize;
    // Walk up enough layers. Each "widget" occupies several AST levels
    // (named_argument → argument → arguments → argument_part → selector).
    let mut layers_remaining = lookback * 8;
    while let Some(n) = current {
        if layers_remaining == 0 {
            break;
        }
        layers_remaining -= 1;
        // Check each child of this ancestor: is it an identifier named
        // "Semantics" whose next sibling is a selector containing
        // `identifier:`?
        if ancestor_node_is_semantics_with_identifier(n, source) {
            return true;
        }
        // Count widgets we've passed so we don't go too far up the tree.
        if ancestor_node_contains_widget_call(n, source) {
            widgets_seen += 1;
            if widgets_seen >= lookback {
                break;
            }
        }
        current = n.parent();
    }
    false
}

/// Returns true if `ancestor` has a direct child `identifier` == "Semantics"
/// whose immediately following sibling is a `selector` that contains the text
/// `identifier:`.
fn ancestor_node_is_semantics_with_identifier(ancestor: Node<'_>, source: &str) -> bool {
    let mut cursor = ancestor.walk();
    let children: Vec<Node<'_>> = ancestor.children(&mut cursor).collect();
    for (i, child) in children.iter().enumerate() {
        if (child.kind() == "identifier" || child.kind() == "type_identifier")
            && child.utf8_text(source.as_bytes()).unwrap_or("") == "Semantics"
        {
            // Check the next sibling for `selector` with `identifier:`
            if let Some(next) = children.get(i + 1) {
                if next.kind() == "selector" {
                    let selector_text = next.utf8_text(source.as_bytes()).unwrap_or("");
                    if selector_text.contains("identifier:") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Returns true if `ancestor` has a direct child `identifier` that is a known
/// widget kind (any widget, not just Semantics) followed by a `selector`.
/// Used only for the widget-count heuristic in `nearest_semantics_with_identifier`.
fn ancestor_node_contains_widget_call(ancestor: Node<'_>, source: &str) -> bool {
    let mut cursor = ancestor.walk();
    let children: Vec<Node<'_>> = ancestor.children(&mut cursor).collect();
    for (i, child) in children.iter().enumerate() {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            let name = child.utf8_text(source.as_bytes()).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            // Check if next sibling is a `selector` (argument invocation)
            if let Some(next) = children.get(i + 1) {
                if next.kind() == "selector" {
                    // Heuristic: PascalCase → likely a widget constructor
                    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Return the constructor/identifier name if `node` looks like a widget call
/// site in the tree-sitter-dart grammar.
///
/// In this grammar, `Foo(...)` is represented as:
///   `(identifier:"Foo") (selector (argument_part (arguments ...)))`
/// as siblings under their parent.  So we recognise a widget call by finding
/// an `identifier` (or `type_identifier`) node whose *next sibling* is a
/// `selector` containing an `argument_part`.
fn widget_call_name_at_identifier<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    if node.kind() != "identifier" && node.kind() != "type_identifier" {
        return None;
    }
    let parent = node.parent()?;
    // Find this node's index among its parent's children, then check next sibling.
    let mut cursor = parent.walk();
    let children: Vec<Node<'_>> = parent.children(&mut cursor).collect();
    let my_idx = children.iter().position(|c| c.id() == node.id())?;
    if let Some(next) = children.get(my_idx + 1) {
        if next.kind() == "selector" {
            // Confirm the selector has an argument_part (i.e. it's a call, not
            // just field access like `.length`).
            let mut sc = next.walk();
            for child in next.children(&mut sc) {
                if child.kind() == "argument_part" {
                    return node.utf8_text(source.as_bytes()).ok();
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_widgets_finds_textfield() {
        let src = r#"
import 'package:flutter/material.dart';
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return TextField(controller: c);
  }
}
"#;
        let result = scan_widgets(src);
        assert!(result.iter().any(|o| o.kind == "TextField"), "got: {result:?}");
    }

    #[test]
    fn scan_widgets_marks_wrapped_when_inside_semantics_with_identifier() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Semantics(
      identifier: 'email-field',
      child: TextField(controller: c),
    );
  }
}
"#;
        let result = scan_widgets(src);
        let tf = result.iter().find(|o| o.kind == "TextField").expect("TextField found");
        assert!(tf.wrapped, "expected wrapped TextField, got {tf:?}");
    }

    #[test]
    fn scan_widgets_marks_unwrapped_when_semantics_missing_identifier() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: 'email field',
      child: TextField(controller: c),
    );
  }
}
"#;
        let result = scan_widgets(src);
        let tf = result.iter().find(|o| o.kind == "TextField").expect("TextField found");
        assert!(!tf.wrapped, "expected unwrapped TextField (no identifier:), got {tf:?}");
    }

    #[test]
    fn scan_widgets_includes_all_interactive_kinds() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        ElevatedButton(onPressed: () {}, child: const Text('a')),
        TextButton(onPressed: () {}, child: const Text('b')),
        IconButton(onPressed: () {}, icon: const Icon(Icons.add)),
        TextField(controller: c),
        GestureDetector(onTap: () {}, child: const SizedBox()),
        InkWell(onTap: () {}, child: const SizedBox()),
        Checkbox(value: true, onChanged: (_) {}),
        Switch(value: false, onChanged: (_) {}),
      ],
    );
  }
}
"#;
        let result = scan_widgets(src);
        let kinds: std::collections::HashSet<_> = result.iter().map(|o| o.kind.as_str()).collect();
        assert!(kinds.contains("ElevatedButton"));
        assert!(kinds.contains("TextButton"));
        assert!(kinds.contains("IconButton"));
        assert!(kinds.contains("TextField"));
        assert!(kinds.contains("GestureDetector"));
        assert!(kinds.contains("InkWell"));
        assert!(kinds.contains("Checkbox"));
        assert!(kinds.contains("Switch"));
    }

    #[test]
    fn scan_widgets_empty_source_returns_empty() {
        let result = scan_widgets("");
        assert!(result.is_empty());
    }

    #[test]
    fn scan_widgets_non_interactive_widgets_ignored() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(child: const Text('hi'));
  }
}
"#;
        let result = scan_widgets(src);
        assert!(result.is_empty(), "got: {result:?}");
    }

    #[test]
    fn scan_widgets_reports_line_numbers() {
        let src = "class Foo extends StatelessWidget {\n  @override\n  Widget build(BuildContext context) {\n    return TextField(controller: c);\n  }\n}\n";
        let result = scan_widgets(src);
        let tf = result.iter().find(|o| o.kind == "TextField").expect("TextField found");
        assert_eq!(tf.line, 4);
    }

    #[test]
    fn interactive_widgets_list_is_exhaustive() {
        assert!(INTERACTIVE_WIDGETS.len() >= 18, "expected >=18 widgets");
    }
}
