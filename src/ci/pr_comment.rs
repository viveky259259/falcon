use crate::config::Severity;
use crate::reporters::{AnalysisReport, Issue};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Threshold above which non-error findings are collapsed into `<details>` blocks.
const TRUNCATION_THRESHOLD: usize = 60;

/// Falcon signature footer shown on every non-zero-findings PR comment.
const FOOTER: &str = "_Posted by Falcon — see [docs](https://github.com/viveky259259/falcon)._";

fn severity_icon(sev: Severity) -> &'static str {
    match sev {
        Severity::Error => "🔴",
        Severity::Warning => "🟡",
        Severity::Info => "🔵",
    }
}

fn sanitize_md_inline(value: &str) -> String {
    value
        .replace(['\r', '\n'], " ")
        .replace('`', "\\`")
}

/// Render a single issue as a bullet line with severity icon, location, rule and fix hint.
fn render_issue_line(issue: &Issue, project_root: &Path) -> String {
    let rel: PathBuf = issue
        .file
        .strip_prefix(project_root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| issue.file.clone());
    let rel_str = sanitize_md_inline(&rel.to_string_lossy());
    let rule = sanitize_md_inline(&issue.rule);
    let message = sanitize_md_inline(&issue.message);
    let icon = severity_icon(issue.severity);
    // `Issue.message` is rendered as the why/fix hint — there is no separate fix field.
    format!(
        "- {} **{}:{}** `{}` — {}\n",
        icon, rel_str, issue.line, rule, message
    )
}

/// Group issues per file (path relative to `project_root` when possible) with files
/// sorted alphabetically and issues within each file sorted by `(line, column)`.
fn group_by_file<'a>(
    issues: &'a [&'a Issue],
    project_root: &Path,
) -> Vec<(String, Vec<&'a Issue>)> {
    let mut grouped: BTreeMap<String, Vec<&'a Issue>> = BTreeMap::new();
    for issue in issues {
        let rel = issue
            .file
            .strip_prefix(project_root)
            .unwrap_or(&issue.file)
            .to_string_lossy()
            .to_string();
        grouped.entry(rel).or_default().push(*issue);
    }
    for v in grouped.values_mut() {
        v.sort_by(|a, b| {
            (a.line, a.column, &a.rule, &a.message)
                .cmp(&(b.line, b.column, &b.rule, &b.message))
        });
    }
    grouped.into_iter().collect()
}

/// Render the per-file grouped section for a given set of issues.
fn render_grouped_section(issues: &[&Issue], project_root: &Path) -> String {
    let mut out = String::new();
    for (file, file_issues) in group_by_file(issues, project_root) {
        out.push_str(&format!("### `{}`\n\n", file));
        for issue in file_issues {
            out.push_str(&render_issue_line(issue, project_root));
        }
        out.push('\n');
    }
    out
}

/// Format analysis results as a GitHub PR comment body (markdown).
///
/// Layout (council spec, EPIC 2.3):
/// - Top status icon header + summary table (preserved from earlier impl).
/// - Findings grouped **per file** (alphabetical), each line prefixed with a severity icon
///   and including the `Issue.message` as the why/fix hint.
/// - When total findings exceed [`TRUNCATION_THRESHOLD`], non-error findings collapse into
///   `<details>` blocks; errors always render inline. File group headers are always shown
///   for whichever severities are rendered inline (errors above, collapsed sections inside).
/// - Falcon signature footer line on every non-zero-findings output.
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

    let error_issues: Vec<&Issue> = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .collect();
    let warning_issues: Vec<&Issue> = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .collect();
    let info_issues: Vec<&Issue> = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Info)
        .collect();

    let truncate = total > TRUNCATION_THRESHOLD;

    if truncate {
        // Errors always inline.
        if !error_issues.is_empty() {
            md.push_str(&format!(
                "### 🔴 Errors ({} — must fix)\n\n",
                error_issues.len()
            ));
            md.push_str(&render_grouped_section(&error_issues, project_root));
        }

        // Warnings and info collapsed into <details> blocks (still grouped per file inside).
        if !warning_issues.is_empty() {
            md.push_str(&format!(
                "<details>\n<summary>🟡 Warnings ({})</summary>\n\n",
                warning_issues.len()
            ));
            md.push_str(&render_grouped_section(&warning_issues, project_root));
            md.push_str("</details>\n\n");
        }

        if !info_issues.is_empty() {
            md.push_str(&format!(
                "<details>\n<summary>🔵 Info ({})</summary>\n\n",
                info_issues.len()
            ));
            md.push_str(&render_grouped_section(&info_issues, project_root));
            md.push_str("</details>\n\n");
        }
    } else {
        // Below threshold: render every finding inline, grouped per file, mixed severities.
        let all_issues: Vec<&Issue> = report.issues.iter().collect();
        md.push_str(&render_grouped_section(&all_issues, project_root));
    }

    md.push_str("---\n");
    md.push_str(FOOTER);
    md.push('\n');

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
