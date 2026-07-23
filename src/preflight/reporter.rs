//! Output emitters for pre-flight issues — text (ANSI) and JSON.
//!
//! SARIF is intentionally deferred to the analyze-rollup PR.

use super::{OutputFormat, PreflightIssue};
use crate::config::Severity;
use colored::Colorize;

/// Render a slice of issues as ANSI-colored text. Returns the rendered string;
/// the caller decides whether to print to stdout/stderr.
pub fn render_text(issues: &[PreflightIssue]) -> String {
    if issues.is_empty() {
        return format!("  {} No pre-flight issues found.\n", "✓".green().bold());
    }
    let mut out = String::new();
    for issue in issues {
        let badge = match issue.severity {
            Severity::Error => "Error".red().bold(),
            Severity::Warning => "Warning".yellow().bold(),
            Severity::Info => "Info".blue().bold(),
        };
        out.push_str(&format!("{} {}\n", badge, issue.title.bold()));
        if let Some(plugin) = &issue.plugin {
            out.push_str(&format!("  Plugin: {}\n", plugin));
        }
        if let Some(file) = &issue.file {
            match issue.line {
                Some(line) => out.push_str(&format!("  File: {}:{}\n", file.display(), line)),
                None => out.push_str(&format!("  File: {}\n", file.display())),
            }
        }
        out.push_str(&format!("  Rule: {}\n", issue.rule_id));
        out.push_str(&format!("  {}\n", issue.message));
        if let Some(suggestion) = &issue.suggestion {
            out.push_str(&format!("  {} {}\n", "→".bright_cyan(), suggestion));
        }
        out.push('\n');
    }
    out
}

/// Render as `{"schema_version": 1, "issues": [...]}` JSON.
pub fn render_json(issues: &[PreflightIssue]) -> String {
    let doc = serde_json::json!({
        "schema_version": 1,
        "issues": issues,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{\"issues\":[]}".into())
}

/// Render with the format the caller picked.
pub fn render(issues: &[PreflightIssue], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => render_text(issues),
        OutputFormat::Json => render_json(issues),
        OutputFormat::Sarif => {
            eprintln!(
                "  ⚠ Falcon: --format sarif not yet implemented (lands in analyze-rollup follow-up). \
Falling back to JSON. Track at docs/superpowers/specs/2026-05-25-preflight-checks-design.md."
            );
            render_json(issues)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn disable_colors() {
        INIT.call_once(|| {
            colored::control::set_override(false);
        });
    }

    fn sample_error() -> PreflightIssue {
        PreflightIssue {
            rule_id: "assets/missing-file".into(),
            severity: Severity::Error,
            title: "Missing Asset".into(),
            file: Some(PathBuf::from("pubspec.yaml")),
            line: Some(109),
            plugin: None,
            message: "The asset `.env` is declared in pubspec.yaml but not on disk.".into(),
            suggestion: Some("Create the file or remove the declaration.".into()),
        }
    }

    #[test]
    fn render_text_empty_returns_ok_line() {
        disable_colors();
        let out = render_text(&[]);
        assert!(out.contains("No pre-flight issues found"), "got: {out}");
    }

    #[test]
    fn render_text_includes_title_file_line_rule_message_suggestion() {
        disable_colors();
        let out = render_text(&[sample_error()]);
        assert!(out.contains("Missing Asset"));
        assert!(out.contains("pubspec.yaml:109"));
        assert!(out.contains("assets/missing-file"));
        assert!(out.contains("The asset `.env` is declared"));
        assert!(out.contains("Create the file or remove the declaration."));
    }

    #[test]
    fn render_text_severity_labels() {
        disable_colors();
        let mut iss = sample_error();

        iss.severity = Severity::Error;
        assert!(render_text(&[iss.clone()]).contains("Error"));

        iss.severity = Severity::Warning;
        assert!(render_text(&[iss.clone()]).contains("Warning"));

        iss.severity = Severity::Info;
        assert!(render_text(&[iss.clone()]).contains("Info"));
    }

    #[test]
    fn render_json_has_schema_version_and_issues_array() {
        let json = render_json(&[sample_error()]);
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["schema_version"], serde_json::json!(1));
        assert_eq!(val["issues"].as_array().unwrap().len(), 1);
        assert_eq!(val["issues"][0]["rule_id"], "assets/missing-file");
        assert_eq!(val["issues"][0]["line"], 109);
    }

    #[test]
    fn render_json_empty_returns_empty_issues_array() {
        let json = render_json(&[]);
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["schema_version"], serde_json::json!(1));
        assert_eq!(val["issues"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn render_sarif_falls_back_to_json_for_now() {
        let sarif = render(&[sample_error()], OutputFormat::Sarif);
        let val: serde_json::Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(val["schema_version"], serde_json::json!(1));
    }
}
