//! Shared types for Falcon's pre-flight checks (check-assets, check-a11y,
//! check-pods, check-platform-deps).

use crate::config::Severity;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single finding from any pre-flight check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightIssue {
    pub rule_id: String,
    pub severity: Severity,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Output format selector for any `falcon check-*` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
}

/// Platform selector for commands that scope to one platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TargetPlatform {
    Ios,
    Android,
    Both,
}

/// Compute the exit code from a slice of issues.
/// Returns 2 if any Error, 1 if any Warning, 0 otherwise.
pub fn exit_code_for_issues(issues: &[PreflightIssue]) -> i32 {
    let mut code = 0;
    for issue in issues {
        match issue.severity {
            Severity::Error => return 2,
            Severity::Warning => code = code.max(1),
            Severity::Info => {}
        }
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(severity: Severity) -> PreflightIssue {
        PreflightIssue {
            rule_id: "x/y".into(),
            severity,
            title: "t".into(),
            file: None,
            line: None,
            plugin: None,
            message: "m".into(),
            suggestion: None,
        }
    }

    #[test]
    fn exit_code_empty_is_zero() {
        assert_eq!(exit_code_for_issues(&[]), 0);
    }

    #[test]
    fn exit_code_info_only_is_zero() {
        assert_eq!(exit_code_for_issues(&[issue(Severity::Info)]), 0);
    }

    #[test]
    fn exit_code_warning_is_one() {
        assert_eq!(
            exit_code_for_issues(&[issue(Severity::Warning), issue(Severity::Info)]),
            1
        );
    }

    #[test]
    fn exit_code_error_short_circuits_to_two() {
        assert_eq!(
            exit_code_for_issues(&[
                issue(Severity::Warning),
                issue(Severity::Error),
                issue(Severity::Info)
            ]),
            2
        );
    }

    #[test]
    fn preflight_issue_serializes_omitting_none_fields() {
        let issue = PreflightIssue {
            rule_id: "assets/missing-file".into(),
            severity: Severity::Error,
            title: "Missing asset".into(),
            file: None,
            line: None,
            plugin: None,
            message: ".env not found".into(),
            suggestion: None,
        };
        let json = serde_json::to_value(&issue).unwrap();
        assert!(json.get("file").is_none(), "file should be omitted: {json}");
        assert!(json.get("line").is_none(), "line should be omitted: {json}");
        assert!(json.get("plugin").is_none(), "plugin should be omitted: {json}");
        assert!(json.get("suggestion").is_none(), "suggestion should be omitted: {json}");
    }
}
