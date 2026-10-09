use tower_lsp::lsp_types::*;

pub fn generate_code_actions(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for diag in diagnostics {
        if let Some(ref data) = diag.data {
            let rule = data.get("rule").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(fix) = create_fix_for_rule(rule, uri, diag, source) {
                actions.push(CodeActionOrCommand::CodeAction(fix));
            }
        }

        actions.push(CodeActionOrCommand::CodeAction(create_suppress_action(
            uri, diag,
        )));

        actions.push(CodeActionOrCommand::CodeAction(
            create_suppress_file_action(uri, diag),
        ));
    }

    actions
}

fn create_fix_for_rule(
    rule: &str,
    uri: &Url,
    diag: &Diagnostic,
    source: &str,
) -> Option<CodeAction> {
    let edits = syntax_edits_for_diagnostic(source, rule, diag);
    if edits.is_empty() {
        return None;
    }
    let title = format!("Fix {}", rule);
    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

fn create_suppress_action(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let rule_name = diag
        .data
        .as_ref()
        .and_then(|d| d.get("rule"))
        .and_then(|v| v.as_str())
        .unwrap_or("falcon");

    let suppress_line = format!("  // ignore: {}", rule_name);
    let insert_pos = Position::new(diag.range.start.line, 0);

    let edit = TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: format!("{}\n", suppress_line),
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    CodeAction {
        title: format!("Suppress '{}' for this line", rule_name),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

fn create_suppress_file_action(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let rule_name = diag
        .data
        .as_ref()
        .and_then(|d| d.get("rule"))
        .and_then(|v| v.as_str())
        .unwrap_or("falcon");

    let edit = TextEdit {
        range: Range {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
        },
        new_text: format!("// ignore_for_file: {}\n", rule_name),
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    CodeAction {
        title: format!("Suppress '{}' for entire file", rule_name),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

fn syntax_edits_for_diagnostic(source: &str, rule: &str, diagnostic: &Diagnostic) -> Vec<TextEdit> {
    use crate::ai::syntax_fixes::fixes_for_issue;
    let line = diagnostic.range.start.line as usize + 1;
    let column = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("column"))
        .and_then(|value| value.as_u64())
        .unwrap_or(1) as usize;
    fixes_for_issue(source, rule, line, column)
        .into_iter()
        .map(|fix| TextEdit {
            range: Range::new(
                crate::lsp::diagnostics::position_at_byte(source, fix.range.start),
                crate::lsp::diagnostics::position_at_byte(source, fix.range.end),
            ),
            new_text: fix.replacement,
        })
        .collect()
}

/// Combine only non-overlapping syntax edits; suppression actions are excluded.
pub fn fix_all_edits(source: &str, diagnostics: &[Diagnostic]) -> Vec<TextEdit> {
    use crate::ai::syntax_fixes::{fixes_for_issue, non_overlapping};
    let edits = diagnostics
        .iter()
        .flat_map(|diagnostic| {
            let rule = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("rule"))
                .and_then(|rule| rule.as_str())
                .unwrap_or("");
            let column = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("column"))
                .and_then(|value| value.as_u64())
                .unwrap_or(1) as usize;
            fixes_for_issue(
                source,
                rule,
                diagnostic.range.start.line as usize + 1,
                column,
            )
        })
        .collect();
    non_overlapping(edits)
        .into_iter()
        .map(|fix| TextEdit {
            range: Range::new(
                crate::lsp::diagnostics::position_at_byte(source, fix.range.start),
                crate::lsp::diagnostics::position_at_byte(source, fix.range.end),
            ),
            new_text: fix.replacement,
        })
        .collect()
}
