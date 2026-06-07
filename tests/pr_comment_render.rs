// Integration tests for the PR comment renderer (council EPIC 2.3 spec).
//
// These tests pin down the contract of `format_pr_comment` rather than the
// exact rendered bytes — golden-file matching the full output would be too
// brittle. Each test asserts the key invariants of one AC bullet.

use falcon::ci::pr_comment::format_pr_comment;
use falcon::config::Severity;
use falcon::reporters::{AnalysisReport, Issue};
use std::path::{Path, PathBuf};

const PROJECT_ROOT: &str = "/project";

const FOOTER_LINE: &str = "_Posted by Falcon — Rust-powered static analysis for Flutter/Dart._";

fn project_root() -> &'static Path {
    Path::new(PROJECT_ROOT)
}

fn issue(sev: Severity, file: &str, line: usize, rule: &str, message: &str) -> Issue {
    Issue {
        rule: rule.to_string(),
        message: message.to_string(),
        severity: sev,
        file: PathBuf::from(format!("{}/{}", PROJECT_ROOT, file)),
        line,
        column: 1,
    }
}

fn report(issues: Vec<Issue>, file_count: usize) -> AnalysisReport {
    AnalysisReport {
        issues,
        metrics: vec![],
        file_count,
        project_path: None,
    }
}

// AC #?: zero findings -> returns the "great job" message AND the summary header.
#[test]
fn zero_findings_renders_great_job_and_summary_header() {
    let report = report(vec![], 10);
    let out = format_pr_comment(&report, project_root());

    assert!(
        out.contains("✅ Falcon Analysis"),
        "expected clean status header, got:\n{}",
        out
    );
    assert!(
        out.contains("No issues found — great job! 🎉"),
        "expected great-job line, got:\n{}",
        out
    );
    // Summary table still rendered.
    assert!(
        out.contains("Files analyzed | 10"),
        "expected summary table with file count, got:\n{}",
        out
    );
    // Zero-findings output deliberately omits the footer (kept lean per spec).
    assert!(
        !out.contains(FOOTER_LINE),
        "footer should not render on zero-findings output"
    );
}

// AC #1 + #2: one error in one file -> file-group header rendered with
// the file path relative to project_root and a 🔴 prefix on the bullet.
#[test]
fn single_error_renders_file_group_header_with_relative_path_and_red_icon() {
    let issues = vec![issue(
        Severity::Error,
        "lib/main.dart",
        42,
        "avoid-dynamic",
        "Avoid using dynamic type",
    )];
    let out = format_pr_comment(&report(issues, 1), project_root());

    // Per-file group header with the path relative to project_root.
    assert!(
        out.contains("### `lib/main.dart`"),
        "expected per-file group header, got:\n{}",
        out
    );
    // Severity icon prefix and the file:line + rule + message rendered.
    assert!(
        out.contains("- 🔴 **lib/main.dart:42** `avoid-dynamic` — Avoid using dynamic type"),
        "expected red-icon bullet with file:line, rule and fix hint, got:\n{}",
        out
    );
}

// AC #1: multiple files -> each file gets its own group, file headers
// emitted in alphabetical order.
#[test]
fn multiple_files_render_groups_in_alphabetical_order() {
    let issues = vec![
        issue(Severity::Error, "lib/zeta.dart", 1, "r1", "msg z"),
        issue(Severity::Error, "lib/alpha.dart", 2, "r2", "msg a"),
        issue(Severity::Error, "lib/mid.dart", 3, "r3", "msg m"),
    ];
    let out = format_pr_comment(&report(issues, 3), project_root());

    let alpha_at = out
        .find("### `lib/alpha.dart`")
        .expect("alpha header missing");
    let mid_at = out.find("### `lib/mid.dart`").expect("mid header missing");
    let zeta_at = out
        .find("### `lib/zeta.dart`")
        .expect("zeta header missing");

    assert!(
        alpha_at < mid_at && mid_at < zeta_at,
        "expected alphabetical file ordering (alpha < mid < zeta), got positions: \
         alpha={} mid={} zeta={}\nfull output:\n{}",
        alpha_at,
        mid_at,
        zeta_at,
        out
    );
}

// AC #2: mix of severities -> each gets its correct icon (🔴, 🟡, 🔵).
#[test]
fn mixed_severities_get_correct_icons() {
    let issues = vec![
        issue(Severity::Error, "lib/a.dart", 1, "r-err", "err hint"),
        issue(Severity::Warning, "lib/b.dart", 2, "r-warn", "warn hint"),
        issue(Severity::Info, "lib/c.dart", 3, "r-info", "info hint"),
    ];
    let out = format_pr_comment(&report(issues, 3), project_root());

    assert!(
        out.contains("- 🔴 **lib/a.dart:1** `r-err` — err hint"),
        "error icon prefix missing, got:\n{}",
        out
    );
    assert!(
        out.contains("- 🟡 **lib/b.dart:2** `r-warn` — warn hint"),
        "warning icon prefix missing, got:\n{}",
        out
    );
    assert!(
        out.contains("- 🔵 **lib/c.dart:3** `r-info` — info hint"),
        "info icon prefix missing, got:\n{}",
        out
    );
}

// AC #5: >60 findings, all errors -> errors stay inline, no <details> for them.
#[test]
fn over_threshold_all_errors_keeps_them_inline_no_details() {
    let mut issues = Vec::with_capacity(70);
    for i in 0..70 {
        // Spread across two files so we still verify file-group headers render.
        let file = if i % 2 == 0 {
            "lib/a.dart"
        } else {
            "lib/b.dart"
        };
        issues.push(issue(
            Severity::Error,
            file,
            i + 1,
            "err-rule",
            "fix this error",
        ));
    }
    let out = format_pr_comment(&report(issues, 2), project_root());

    // Errors-section header renders inline.
    assert!(
        out.contains("### 🔴 Errors"),
        "expected inline errors header, got:\n{}",
        out
    );
    // Per-file group headers still render for the error group.
    assert!(
        out.contains("### `lib/a.dart`"),
        "expected per-file group header for lib/a.dart, got:\n{}",
        out
    );
    assert!(
        out.contains("### `lib/b.dart`"),
        "expected per-file group header for lib/b.dart, got:\n{}",
        out
    );
    // No <details> block for errors.
    assert!(
        !out.contains("<summary>🔴 Errors"),
        "errors must not be collapsed under <details>, got:\n{}",
        out
    );
    // No warnings/info sections at all (none in this report).
    assert!(
        !out.contains("<summary>🟡 Warnings"),
        "warnings section should not appear for all-errors case, got:\n{}",
        out
    );
    assert!(
        !out.contains("<summary>🔵 Info"),
        "info section should not appear for all-errors case, got:\n{}",
        out
    );
}

// AC #5: >60 findings, mixed severities -> warnings/info collapsed under
// <details>, errors stay inline. Per-file group headers still rendered.
#[test]
fn over_threshold_mixed_collapses_warnings_and_info_keeps_errors_inline() {
    let mut issues = Vec::with_capacity(70);
    // 30 errors, 25 warnings, 15 infos = 70 total.
    for i in 0..30 {
        issues.push(issue(
            Severity::Error,
            "lib/err.dart",
            i + 1,
            "err-rule",
            "fix the error",
        ));
    }
    for i in 0..25 {
        issues.push(issue(
            Severity::Warning,
            "lib/warn.dart",
            i + 1,
            "warn-rule",
            "address the warning",
        ));
    }
    for i in 0..15 {
        issues.push(issue(
            Severity::Info,
            "lib/info.dart",
            i + 1,
            "info-rule",
            "informational",
        ));
    }
    let out = format_pr_comment(&report(issues, 3), project_root());

    // Errors stay inline (no <details> wrapping the errors group itself).
    assert!(
        out.contains("### 🔴 Errors"),
        "expected inline errors header, got:\n{}",
        out
    );
    assert!(
        !out.contains("<summary>🔴 Errors"),
        "errors must not be collapsed when total > 60"
    );

    // Warnings + info collapsed under <details>.
    assert!(
        out.contains("<details>\n<summary>🟡 Warnings"),
        "warnings should be collapsed under <details>, got:\n{}",
        out
    );
    assert!(
        out.contains("<details>\n<summary>🔵 Info"),
        "info should be collapsed under <details>, got:\n{}",
        out
    );

    // Per-file group headers are emitted across all three severity blocks.
    assert!(
        out.contains("### `lib/err.dart`"),
        "expected header for lib/err.dart in output, got:\n{}",
        out
    );
    assert!(
        out.contains("### `lib/warn.dart`"),
        "expected header for lib/warn.dart in output, got:\n{}",
        out
    );
    assert!(
        out.contains("### `lib/info.dart`"),
        "expected header for lib/info.dart in output, got:\n{}",
        out
    );
}

// AC #4: footer signature line is present in every non-zero-findings output.
#[test]
fn footer_signature_present_on_every_nonzero_output() {
    // Single low-severity finding (below threshold).
    let low = format_pr_comment(
        &report(
            vec![issue(Severity::Info, "lib/a.dart", 1, "info-r", "hint")],
            1,
        ),
        project_root(),
    );
    assert!(
        low.contains(FOOTER_LINE),
        "footer missing from below-threshold output, got:\n{}",
        low
    );
    assert_eq!(
        low.matches(FOOTER_LINE).count(),
        1,
        "footer should render exactly once in below-threshold output"
    );
    assert!(
        low.contains("\n---\n"),
        "separator should render before footer in below-threshold output"
    );

    // Many findings (above threshold) — footer must still be present once.
    let mut many = Vec::with_capacity(80);
    for i in 0..80 {
        many.push(issue(
            Severity::Warning,
            "lib/many.dart",
            i + 1,
            "warn-r",
            "fix it",
        ));
    }
    let above = format_pr_comment(&report(many, 1), project_root());
    assert!(
        above.contains(FOOTER_LINE),
        "footer missing from above-threshold output, got:\n{}",
        above
    );
    assert!(
        above.contains("\n---\n"),
        "separator should render before footer in above-threshold output"
    );
    // And exactly once.
    assert_eq!(
        above.matches(FOOTER_LINE).count(),
        1,
        "footer should render exactly once"
    );
}
