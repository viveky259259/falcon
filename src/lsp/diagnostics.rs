use crate::config::{FalconConfig, Severity};
use crate::metrics;
use crate::parser::DartParser;
use crate::reporters::Issue;
use crate::rules::RuleRegistry;
use std::path::Path;
use tower_lsp::lsp_types::*;

pub fn analyze_source(
    source: &str,
    file_path: &Path,
    config: &FalconConfig,
    registry: &RuleRegistry,
) -> Vec<Issue> {
    let mut parser = match DartParser::new() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let tree = match parser.parse(source) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let root = tree.root_node();
    let mut issues = Vec::new();

    let metric_results = metrics::calculate_file_metrics(root, source, &config.metrics);
    for violation in metric_results.violations() {
        issues.push(violation);
    }

    let rule_issues = registry.check(root, source, file_path);
    issues.extend(rule_issues);

    issues
}

pub fn issues_to_diagnostics(issues: &[Issue]) -> Vec<Diagnostic> {
    issues
        .iter()
        .map(|issue| {
            let line = issue.line.saturating_sub(1) as u32;
            let col = issue.column.saturating_sub(1) as u32;

            Diagnostic {
                range: Range {
                    start: Position::new(line, col),
                    end: Position::new(line, col + 20),
                },
                severity: Some(severity_to_lsp(&issue.severity)),
                code: Some(NumberOrString::String(issue.rule.clone())),
                code_description: None,
                source: Some("falcon".to_string()),
                message: issue.message.clone(),
                related_information: None,
                tags: None,
                data: Some(serde_json::json!({
                    "rule": issue.rule,
                    "line": issue.line,
                    "column": issue.column,
                })),
            }
        })
        .collect()
}

fn severity_to_lsp(severity: &Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// Convert parser byte columns using the actual document's UTF-16 text.
pub fn issues_to_diagnostics_with_source(issues: &[Issue], source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = issues_to_diagnostics(issues);
    for (issue, diagnostic) in issues.iter().zip(&mut diagnostics) {
        let line = issue.line.saturating_sub(1);
        let content = source.lines().nth(line).unwrap_or("");
        let requested = issue.column.saturating_sub(1).min(content.len());
        let byte = if content.is_char_boundary(requested) {
            requested
        } else {
            content
                .char_indices()
                .map(|(offset, _)| offset)
                .take_while(|offset| *offset <= requested)
                .last()
                .unwrap_or(0)
        };
        let character = content[..byte].encode_utf16().count() as u32;
        let width = content[byte..]
            .chars()
            .next()
            .map_or(0, |character| character.len_utf16() as u32);
        diagnostic.range = Range::new(
            Position::new(line as u32, character),
            Position::new(line as u32, character + width),
        );
    }
    diagnostics
}

pub fn position_at_byte(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let last = prefix.rsplit('\n').next().unwrap_or("");
    Position::new(line, last.encode_utf16().count() as u32)
}
