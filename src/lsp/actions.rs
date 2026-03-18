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
    let line = diag.range.start.line as usize;
    let lines: Vec<&str> = source.lines().collect();
    let line_content = lines.get(line)?;

    let (title, replacement) = match rule {
        "prefer-trailing-comma" => {
            let mut fixed = line_content.trim_end().to_string();
            if fixed.ends_with(')') || fixed.ends_with(']') || fixed.ends_with('}') {
                let last = fixed.pop()?;
                fixed.push(',');
                fixed.push(last);
            }
            ("Add trailing comma".to_string(), fixed)
        }
        "avoid-double-negation" => {
            let fixed = line_content.replace("!!", "");
            ("Remove double negation".to_string(), fixed)
        }
        "avoid-expanded-as-spacer" => {
            let fixed = line_content
                .replace("Expanded(child: SizedBox())", "const Spacer()")
                .replace("Expanded(child: SizedBox.shrink())", "const Spacer()")
                .replace("Expanded(child: Container())", "const Spacer()");
            ("Replace with Spacer()".to_string(), fixed)
        }
        "prefer-first-last" => {
            let fixed = line_content
                .replace(".elementAt(0)", ".first")
                .replace("[0]", ".first");
            if fixed == *line_content {
                return None;
            }
            ("Use .first instead of index access".to_string(), fixed)
        }
        "double-literal-format" => {
            let fixed = line_content.replace(".0)", "0.0)").replace(".0;", "0.0;");
            if fixed == *line_content {
                return None;
            }
            ("Fix double literal format".to_string(), fixed)
        }
        _ => return None,
    };

    let line_end = line_content.len() as u32;
    let edit = TextEdit {
        range: Range {
            start: Position::new(diag.range.start.line, 0),
            end: Position::new(diag.range.start.line, line_end),
        },
        new_text: replacement,
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

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
