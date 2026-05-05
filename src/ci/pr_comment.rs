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
