//! Syntax-bounded edits shared by disk fixes and editor quick fixes.
use crate::parser::{walk_tree, DartParser};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxFix {
    pub rule: String,
    pub range: Range<usize>,
    pub replacement: String,
}

/// Only edit parsed syntax at the reported location; strings/comments stay intact.
pub fn fixes_for_issue(source: &str, rule: &str, line: usize, column: usize) -> Vec<SyntaxFix> {
    let Ok(mut parser) = DartParser::new() else {
        return Vec::new();
    };
    let Some(tree) = parser.parse(source) else {
        return Vec::new();
    };
    let mut fixes = Vec::new();
    walk_tree(tree.root_node(), &mut |node| {
        if node.start_position().row + 1 != line {
            return;
        }
        if column > 1 && node.start_position().column + 1 != column {
            return;
        }
        let text = &source[node.byte_range()];
        let edit = match rule {
            "prefer-first-last" if node.kind() == "selector" && text == "[0]" => {
                // Index writes cannot be replaced with an Iterable getter.
                let following = source[node.end_byte()..].trim_start();
                if (following.starts_with('=') && !following.starts_with("=="))
                    || ["+=", "-=", "++", "--"]
                        .iter()
                        .any(|operator| following.starts_with(operator))
                {
                    return;
                }
                Some((node.byte_range(), ".first".into()))
            }
            "prefer-first-last" if node.kind() == "selector" && text.ends_with(".length - 1]") => {
                Some((node.byte_range(), ".last".into()))
            }
            "prefer-first-last" if node.kind() == "selector" && text == ".elementAt" => node
                .next_named_sibling()
                .filter(|next| &source[next.byte_range()] == "(0)")
                .map(|next| (node.start_byte()..next.end_byte(), ".first".into())),
            "avoid-double-negation"
                if matches!(node.kind(), "unary_expression" | "prefix_expression")
                    && text.starts_with("!!") =>
            {
                Some((node.start_byte()..node.start_byte() + 2, String::new()))
            }
            "double-literal-format" if node.kind() == "decimal_floating_point_literal" => {
                if text.starts_with('.') {
                    Some((node.byte_range(), format!("0{text}")))
                } else if text.ends_with('.') {
                    Some((node.byte_range(), format!("{text}0")))
                } else {
                    None
                }
            }
            "prefer-trailing-comma"
                if matches!(
                    node.kind(),
                    "arguments" | "formal_parameter_list" | "list_literal" | "set_or_map_literal"
                ) =>
            {
                let mut cursor = node.walk();
                let last = node
                    .named_children(&mut cursor)
                    .filter(|child| !child.is_extra())
                    .last();
                last.filter(|child| {
                    !source[child.end_byte()..node.end_byte()]
                        .trim_start()
                        .starts_with(',')
                })
                .map(|child| (child.end_byte()..child.end_byte(), ",".into()))
            }
            "avoid-expanded-as-spacer"
                if matches!(node.kind(), "identifier" | "type_identifier")
                    && text == "Expanded" =>
            {
                node.next_named_sibling().and_then(|arguments| {
                    let compact: String =
                        source[arguments.byte_range()].split_whitespace().collect();
                    if [
                        "(child:SizedBox())",
                        "(child:SizedBox.shrink())",
                        "(child:Container())",
                    ]
                    .contains(&compact.as_str())
                    {
                        Some((
                            node.start_byte()..arguments.end_byte(),
                            if source[..node.start_byte()].split_whitespace().last()
                                == Some("const")
                            {
                                "Spacer()".into()
                            } else {
                                "const Spacer()".into()
                            },
                        ))
                    } else {
                        None
                    }
                })
            }
            "newline-before-return" if node.kind() == "return_statement" => {
                let offset = source[..node.start_byte()]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
                Some((offset..offset, "\n".into()))
            }
            _ => None,
        };
        if let Some((range, replacement)) = edit {
            fixes.push(SyntaxFix {
                rule: rule.into(),
                range,
                replacement,
            });
        }
    });
    non_overlapping(fixes)
}

/// Deduplicate nested diagnostics and conservatively discard overlapping edits.
pub fn non_overlapping(mut fixes: Vec<SyntaxFix>) -> Vec<SyntaxFix> {
    fixes.sort_by_key(|fix| (fix.range.start, fix.range.end));
    fixes.dedup_by(|a, b| a.range == b.range && a.replacement == b.replacement);
    let mut selected: Vec<SyntaxFix> = Vec::new();
    for fix in fixes {
        if selected.last().is_some_and(|previous| {
            previous.range.end > fix.range.start || previous.range.start == fix.range.start
        }) {
            continue;
        }
        selected.push(fix);
    }
    selected
}
