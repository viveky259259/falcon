use tower_lsp::lsp_types::*;
use crate::reporters::Issue;

/// Enrich diagnostics with AI-specific metadata.
pub fn enrich_diagnostics_with_ai(
    diagnostics: &mut Vec<Diagnostic>,
    _issues: &[Issue],
    source: &str,
    file_path: &std::path::Path,
) {
    let provenance = crate::ai_score::provenance::analyze_provenance(file_path, source);

    for diag in diagnostics.iter_mut() {
        if let Some(data) = &diag.data {
            if let Some(rule) = data.get("rule").and_then(|r| r.as_str()) {
                let ai_severity = ai_rule_severity(rule);
                let ai_data = serde_json::json!({
                    "rule": rule,
                    "line": data.get("line"),
                    "column": data.get("column"),
                    "ai_provenance": provenance.origin.to_string(),
                    "ai_confidence": provenance.confidence,
                    "ai_severity": ai_severity,
                });
                diag.data = Some(ai_data);
            }
        }
    }

    if provenance.origin == crate::ai_score::provenance::CodeOrigin::LikelyAiGenerated {
        diagnostics.push(Diagnostic {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 0),
            },
            severity: Some(DiagnosticSeverity::HINT),
            code: Some(NumberOrString::String("falcon-ai-provenance".to_string())),
            source: Some("falcon".to_string()),
            message: format!(
                "This file appears to be AI-generated ({:.0}% confidence). Extra scrutiny recommended.",
                provenance.confidence * 100.0
            ),
            related_information: None,
            code_description: None,
            tags: None,
            data: None,
        });
    }
}

fn ai_rule_severity(rule: &str) -> &'static str {
    match rule {
        "avoid-empty-catch" | "ensure-dispose-lifecycle" | "avoid-hardcoded-credentials"
        | "avoid-unawaited-futures" | "ensure-stream-subscription-cancel" => "critical-in-ai-code",
        "avoid-dynamic" | "avoid-print-in-production" | "avoid-throw-in-catch" => "high-in-ai-code",
        _ => "standard",
    }
}

/// Custom LSP commands for AI features.
pub fn ai_commands() -> Vec<String> {
    vec![
        "falcon.aiScore".to_string(),
        "falcon.provenance".to_string(),
        "falcon.explainRule".to_string(),
        "falcon.drift".to_string(),
    ]
}
