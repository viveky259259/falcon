use crate::config::Severity;
use crate::reporters::{AnalysisReport, Issue};
use std::collections::HashMap;
use std::path::Path;

/// Format analysis results as a GitHub PR comment body (markdown).
pub fn format_pr_comment(report: &AnalysisReport, project_root: &Path) -> String {
    let mut md = String::new();

    let errors = report.error_count();
    let warnings = report.warning_count();
    let infos = report.info_count();
    let total = report.issues.len();

    let status = if errors > 0 {
        "❌"
    } else if warnings > 0 {
        "⚠️"
    } else {
        "✅"
    };

    md.push_str(&format!("## {} Falcon Analysis\n\n", status));

    md.push_str(&format!(
        "| Metric | Value |\n|---|---|\n| Files analyzed | {} |\n| Errors | {} |\n| Warnings | {} |\n| Info | {} |\n| **Total issues** | **{}** |\n\n",
        report.file_count, errors, warnings, infos, total
    ));

    if total == 0 {
        md.push_str("No issues found — great job! 🎉\n");
        return md;
    }

    let mut by_rule: HashMap<String, (usize, Severity)> = HashMap::new();
    for issue in &report.issues {
        let entry = by_rule
            .entry(issue.rule.clone())
            .or_insert((0, issue.severity));
        entry.0 += 1;
    }
    let mut sorted_rules: Vec<(String, usize, Severity)> = by_rule
        .into_iter()
        .map(|(rule, (count, sev))| (rule, count, sev))
        .collect();
    sorted_rules.sort_by(|a, b| b.1.cmp(&a.1));

    md.push_str("<details>\n<summary>📋 Issues by rule (click to expand)</summary>\n\n");
    md.push_str("| Rule | Count | Severity |\n|---|---|---|\n");
    for (rule, count, severity) in sorted_rules.iter().take(20) {
        let sev_icon = match severity {
            Severity::Error => "🔴",
            Severity::Warning => "🟡",
            Severity::Info => "🔵",
        };
        md.push_str(&format!("| `{}` | {} | {} |\n", rule, count, sev_icon));
    }
    if sorted_rules.len() > 20 {
        md.push_str(&format!(
            "| ... | +{} more rules | |\n",
            sorted_rules.len() - 20
        ));
    }
    md.push_str("\n</details>\n\n");

    let error_issues: Vec<&Issue> = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .collect();

    if !error_issues.is_empty() {
        md.push_str("<details>\n<summary>🔴 Errors (must fix)</summary>\n\n");
        let shown = error_issues.len().min(30);
        for issue in error_issues.iter().take(shown) {
            let rel = issue
                .file
                .strip_prefix(project_root)
                .unwrap_or(&issue.file)
                .to_string_lossy();
            md.push_str(&format!(
                "- **{}:{}** `{}` — {}\n",
                rel, issue.line, issue.rule, issue.message
            ));
        }
        if error_issues.len() > shown {
            md.push_str(&format!(
                "- ... and {} more errors\n",
                error_issues.len() - shown
            ));
        }
        md.push_str("\n</details>\n\n");
    }

    md.push_str(&format!(
        "\n---\n*Analyzed by [Falcon](https://github.com/viveky259259/falcon) — Rust-powered static analysis for Flutter/Dart*\n"
    ));

    md
}

/// Post a comment to a GitHub PR using the `gh` CLI.
pub fn post_pr_comment(owner: &str, repo: &str, pr_number: u32, body: &str) -> anyhow::Result<()> {
    let pr_ref = format!("{}", pr_number);

    let output = std::process::Command::new("gh")
        .args([
            "pr",
            "comment",
            &pr_ref,
            "--repo",
            &format!("{}/{}", owner, repo),
            "--body",
            body,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to post PR comment: {}", stderr);
    }

    Ok(())
}

/// Post a comment to the current PR (auto-detects from CI environment).
pub fn post_comment_auto(body: &str) -> anyhow::Result<()> {
    if let Ok(pr_url) = detect_pr_from_env() {
        let output = std::process::Command::new("gh")
            .args(["pr", "comment", &pr_url, "--body", body])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to post PR comment: {}", stderr);
        }
        Ok(())
    } else {
        anyhow::bail!(
            "Could not detect PR. Set GITHUB_PR_NUMBER or run in a GitHub Actions workflow. \
             Or use: falcon pr-comment --owner OWNER --repo REPO --pr NUMBER"
        )
    }
}

fn detect_pr_from_env() -> Result<String, ()> {
    if let Ok(pr_num) = std::env::var("GITHUB_PR_NUMBER") {
        return Ok(pr_num);
    }

    if let Ok(ref_name) = std::env::var("GITHUB_REF") {
        if ref_name.starts_with("refs/pull/") {
            let parts: Vec<&str> = ref_name.split('/').collect();
            if parts.len() >= 3 {
                return Ok(parts[2].to_string());
            }
        }
    }

    if let Ok(event_path) = std::env::var("GITHUB_EVENT_PATH") {
        if let Ok(content) = std::fs::read_to_string(&event_path) {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(number) = event
                    .get("pull_request")
                    .and_then(|pr| pr.get("number"))
                    .and_then(|n| n.as_u64())
                {
                    return Ok(number.to_string());
                }
            }
        }
    }

    Err(())
}

/// Format a compact single-line summary for CI logs.
pub fn format_ci_summary(report: &AnalysisReport) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let status = if errors > 0 {
        "FAIL"
    } else if warnings > 0 {
        "WARN"
    } else {
        "PASS"
    };

    format!(
        "Falcon [{}]: {} files, {} errors, {} warnings, {} total issues",
        status,
        report.file_count,
        errors,
        warnings,
        report.issues.len()
    )
}

/// Generate a GitHub Actions summary (GITHUB_STEP_SUMMARY).
pub fn write_github_step_summary(
    report: &AnalysisReport,
    project_root: &Path,
) -> anyhow::Result<()> {
    if let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY") {
        let md = format_pr_comment(report, project_root);
        std::fs::write(&summary_path, &md)?;
        log::info!("Wrote GitHub step summary to {}", summary_path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use crate::reporters::{AnalysisReport, Issue};
    use std::path::PathBuf;
    use std::sync::Mutex;

    // Mutex to serialize env-var tests so they don't interfere with each other.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn make_report_empty() -> AnalysisReport {
        AnalysisReport {
            issues: vec![],
            metrics: vec![],
            file_count: 5,
            project_path: None,
        }
    }

    fn make_issue(rule: &str, msg: &str, severity: Severity, file: &str, line: usize) -> Issue {
        Issue {
            rule: rule.to_string(),
            message: msg.to_string(),
            severity,
            file: PathBuf::from(file),
            line,
            column: 1,
        }
    }

    fn make_report_with_issues(issues: Vec<Issue>, file_count: usize) -> AnalysisReport {
        AnalysisReport {
            issues,
            metrics: vec![],
            file_count,
            project_path: None,
        }
    }

    // ── format_pr_comment ────────────────────────────────────────────────────

    #[test]
    fn test_format_pr_comment_no_issues_contains_pass_icon() {
        let report = make_report_empty();
        let root = PathBuf::from("/project");
        let md = format_pr_comment(&report, &root);
        assert!(md.contains("✅"), "clean report should use ✅");
        assert!(md.contains("No issues found"), "should mention no issues");
    }

    #[test]
    fn test_format_pr_comment_no_issues_has_file_count() {
        let report = make_report_empty();
        let root = PathBuf::from("/project");
        let md = format_pr_comment(&report, &root);
        assert!(md.contains("5"), "file count 5 should appear in the table");
    }

    #[test]
    fn test_format_pr_comment_warnings_icon() {
        let issues = vec![make_issue(
            "lint/foo",
            "some warning",
            Severity::Warning,
            "/project/lib/foo.dart",
            10,
        )];
        let report = make_report_with_issues(issues, 3);
        let root = PathBuf::from("/project");
        let md = format_pr_comment(&report, &root);
        assert!(md.contains("⚠️"), "warnings-only report should use ⚠️");
    }

    #[test]
    fn test_format_pr_comment_errors_icon() {
        let issues = vec![make_issue(
            "lint/bar",
            "bad error",
            Severity::Error,
            "/project/lib/bar.dart",
            5,
        )];
        let report = make_report_with_issues(issues, 2);
        let root = PathBuf::from("/project");
        let md = format_pr_comment(&report, &root);
        assert!(md.contains("❌"), "error report should use ❌");
    }

    #[test]
    fn test_format_pr_comment_error_detail_section() {
        let issues = vec![make_issue(
            "lint/bar",
            "bad error",
            Severity::Error,
            "/project/lib/bar.dart",
            5,
        )];
        let report = make_report_with_issues(issues, 2);
        let root = PathBuf::from("/project");
        let md = format_pr_comment(&report, &root);
        assert!(
            md.contains("Errors (must fix)"),
            "should include error details section"
        );
        // Path should be relative to project root
        assert!(
            md.contains("lib/bar.dart"),
            "file path should be relative to project root"
        );
    }

    #[test]
    fn test_format_pr_comment_relative_path_stripping() {
        let issues = vec![make_issue(
            "rule/x",
            "msg",
            Severity::Error,
            "/my/project/src/widget.dart",
            1,
        )];
        let report = make_report_with_issues(issues, 1);
        let root = PathBuf::from("/my/project");
        let md = format_pr_comment(&report, &root);
        assert!(
            md.contains("src/widget.dart"),
            "should strip project root prefix"
        );
        assert!(
            !md.contains("/my/project/src/widget.dart"),
            "full absolute path should not appear"
        );
    }

    #[test]
    fn test_format_pr_comment_rules_by_rule_table() {
        let issues = vec![
            make_issue("rule/alpha", "m1", Severity::Warning, "/p/a.dart", 1),
            make_issue("rule/alpha", "m2", Severity::Warning, "/p/b.dart", 2),
            make_issue("rule/beta", "m3", Severity::Info, "/p/c.dart", 3),
        ];
        let report = make_report_with_issues(issues, 3);
        let root = PathBuf::from("/p");
        let md = format_pr_comment(&report, &root);
        assert!(md.contains("rule/alpha"), "rule alpha should appear");
        assert!(md.contains("rule/beta"), "rule beta should appear");
        // alpha has 2 occurrences, beta has 1 — both should show up in the table
        assert!(md.contains("Issues by rule"), "should have issues-by-rule section");
    }

    #[test]
    fn test_format_pr_comment_more_than_20_rules_shows_overflow() {
        let issues: Vec<Issue> = (0..25)
            .map(|i| {
                make_issue(
                    &format!("rule/r{}", i),
                    "msg",
                    Severity::Warning,
                    "/p/f.dart",
                    i + 1,
                )
            })
            .collect();
        let report = make_report_with_issues(issues, 1);
        let root = PathBuf::from("/p");
        let md = format_pr_comment(&report, &root);
        assert!(
            md.contains("more rules"),
            "overflow line should appear for >20 rules"
        );
    }

    #[test]
    fn test_format_pr_comment_contains_falcon_attribution() {
        let report = make_report_empty();
        let root = PathBuf::from("/project");
        let md = format_pr_comment(&report, &root);
        assert!(md.contains("Falcon"), "should credit Falcon");
    }

    // ── format_ci_summary ────────────────────────────────────────────────────

    #[test]
    fn test_format_ci_summary_pass() {
        let report = make_report_empty();
        let s = format_ci_summary(&report);
        assert!(s.contains("PASS"), "no issues => PASS");
        assert!(s.contains("5 files"), "file count should appear");
        assert!(s.contains("0 errors"), "zero errors");
        assert!(s.contains("0 warnings"), "zero warnings");
    }

    #[test]
    fn test_format_ci_summary_warn() {
        let issues = vec![make_issue(
            "r",
            "m",
            Severity::Warning,
            "/p/f.dart",
            1,
        )];
        let report = make_report_with_issues(issues, 10);
        let s = format_ci_summary(&report);
        assert!(s.contains("WARN"), "warnings-only => WARN");
        assert!(s.contains("0 errors"), "no errors");
        assert!(s.contains("1 warnings"), "1 warning");
    }

    #[test]
    fn test_format_ci_summary_fail() {
        let issues = vec![
            make_issue("r", "m", Severity::Error, "/p/f.dart", 1),
            make_issue("r", "m", Severity::Warning, "/p/g.dart", 2),
        ];
        let report = make_report_with_issues(issues, 4);
        let s = format_ci_summary(&report);
        assert!(s.contains("FAIL"), "errors => FAIL");
        assert!(s.contains("1 errors"), "1 error");
        assert!(s.contains("2 total issues"), "total issues count");
    }

    // ── detect_pr_from_env ───────────────────────────────────────────────────

    #[test]
    fn test_detect_pr_github_pr_number_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // Remove any vars that could interfere
        std::env::remove_var("GITHUB_REF");
        std::env::remove_var("GITHUB_EVENT_PATH");
        std::env::set_var("GITHUB_PR_NUMBER", "42");
        let result = detect_pr_from_env();
        std::env::remove_var("GITHUB_PR_NUMBER");
        assert_eq!(result, Ok("42".to_string()));
    }

    #[test]
    fn test_detect_pr_from_github_ref() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GITHUB_PR_NUMBER");
        std::env::remove_var("GITHUB_EVENT_PATH");
        std::env::set_var("GITHUB_REF", "refs/pull/99/merge");
        let result = detect_pr_from_env();
        std::env::remove_var("GITHUB_REF");
        assert_eq!(result, Ok("99".to_string()));
    }

    #[test]
    fn test_detect_pr_github_ref_non_pr_ignored() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GITHUB_PR_NUMBER");
        std::env::remove_var("GITHUB_EVENT_PATH");
        std::env::set_var("GITHUB_REF", "refs/heads/main");
        let result = detect_pr_from_env();
        std::env::remove_var("GITHUB_REF");
        assert!(result.is_err(), "non-PR ref should not produce a PR number");
    }

    #[test]
    fn test_detect_pr_from_event_path() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GITHUB_PR_NUMBER");
        std::env::remove_var("GITHUB_REF");

        let dir = tempfile::tempdir().expect("tempdir");
        let event_file = dir.path().join("event.json");
        std::fs::write(
            &event_file,
            r#"{"pull_request":{"number":7}}"#,
        )
        .expect("write event");
        std::env::set_var("GITHUB_EVENT_PATH", event_file.to_str().unwrap());
        let result = detect_pr_from_env();
        std::env::remove_var("GITHUB_EVENT_PATH");
        assert_eq!(result, Ok("7".to_string()));
    }

    #[test]
    fn test_detect_pr_no_env_vars() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GITHUB_PR_NUMBER");
        std::env::remove_var("GITHUB_REF");
        std::env::remove_var("GITHUB_EVENT_PATH");
        let result = detect_pr_from_env();
        assert!(result.is_err(), "no env vars => Err");
    }

    // ── write_github_step_summary ────────────────────────────────────────────

    #[test]
    fn test_write_github_step_summary_writes_file() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let summary_path = dir.path().join("step_summary.md");
        std::env::set_var("GITHUB_STEP_SUMMARY", summary_path.to_str().unwrap());

        let report = make_report_empty();
        let root = PathBuf::from("/project");
        write_github_step_summary(&report, &root).expect("should succeed");
        std::env::remove_var("GITHUB_STEP_SUMMARY");

        let content = std::fs::read_to_string(&summary_path).expect("file should exist");
        assert!(content.contains("Falcon"), "written markdown should contain Falcon");
        assert!(content.contains("✅"), "clean report should have ✅");
    }

    #[test]
    fn test_write_github_step_summary_no_env_var_is_noop() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GITHUB_STEP_SUMMARY");
        let report = make_report_empty();
        let root = PathBuf::from("/project");
        // Should not error even though no summary path is set
        let result = write_github_step_summary(&report, &root);
        assert!(result.is_ok(), "no env var => noop, not an error");
    }

    #[test]
    fn test_write_github_step_summary_content_matches_format_pr_comment() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let summary_path = dir.path().join("step_summary2.md");
        std::env::set_var("GITHUB_STEP_SUMMARY", summary_path.to_str().unwrap());

        let issues = vec![make_issue(
            "rule/x",
            "oops",
            Severity::Error,
            "/project/src/main.dart",
            12,
        )];
        let report = make_report_with_issues(issues, 2);
        let root = PathBuf::from("/project");

        let expected_md = format_pr_comment(&report, &root);
        write_github_step_summary(&report, &root).expect("should succeed");
        std::env::remove_var("GITHUB_STEP_SUMMARY");

        let actual = std::fs::read_to_string(&summary_path).expect("file should exist");
        assert_eq!(actual, expected_md, "written content should match format_pr_comment output");
    }
}
