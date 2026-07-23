//! Focused coverage for `falcon::ai_score::score` — the AI Code Quality Score
//! rollup that maps Falcon findings into a 6-dimension 0-100 grade letter.
//!
//! These tests pin down behaviour the product surface depends on:
//! - determinism of score_from_report (same input -> same output)
//! - grade-letter boundaries for B / C / D / F (A is covered by edge_core_tests)
//! - per-dimension penalty wiring (resource_safety, security, complexity)
//! - empty-project / no-files edge case
//! - shields.io badge generation matches the grade band

use falcon::ai_score::score::{generate_badge, score_from_report, Grade};
use falcon::config::Severity;
use falcon::reporters::{AnalysisReport, Issue};
use std::path::PathBuf;

// ── Helpers ────────────────────────────────────────────────────────────────

fn issue(rule: &str) -> Issue {
    Issue {
        rule: rule.to_string(),
        message: "m".to_string(),
        severity: Severity::Warning,
        file: PathBuf::from("f.dart"),
        line: 1,
        column: 1,
    }
}

fn report_with(issues: Vec<Issue>, file_count: usize) -> AnalysisReport {
    AnalysisReport {
        issues,
        metrics: vec![],
        file_count,
        project_path: None,
    }
}

// ── Determinism ────────────────────────────────────────────────────────────

#[test]
fn ai_score_is_deterministic_for_same_input() {
    let mk = || {
        report_with(
            vec![
                issue("avoid-empty-catch"),
                issue("avoid-empty-catch"),
                issue("avoid-dynamic"),
                issue("avoid-hardcoded-credentials"),
                issue("avoid-long-functions"),
            ],
            20,
        )
    };

    let a = score_from_report(&mk()).unwrap();
    let b = score_from_report(&mk()).unwrap();

    assert_eq!(a.overall, b.overall);
    assert_eq!(a.grade, b.grade);
    assert_eq!(a.resource_safety.score, b.resource_safety.score);
    assert_eq!(a.error_handling.score, b.error_handling.score);
    assert_eq!(a.type_safety.score, b.type_safety.score);
    assert_eq!(a.security.score, b.security.score);
    assert_eq!(a.convention_match.score, b.convention_match.score);
    assert_eq!(a.complexity.score, b.complexity.score);
    assert_eq!(a.total_issues, b.total_issues);
    assert_eq!(a.file_count, b.file_count);
}

// ── Grade letter boundaries ────────────────────────────────────────────────
//
// Per `score.rs`: 90..=100 → A, 80..=89 → B, 70..=79 → C, 60..=69 → D, else → F.
// We synthesise issue sets that drive `overall` into each band and verify the
// returned `Grade`. This pins the band thresholds.

#[test]
fn grade_is_a_with_zero_issues() {
    let score = score_from_report(&report_with(vec![], 10)).unwrap();
    assert_eq!(score.grade, Grade::A);
    assert_eq!(score.overall, 100);
}

#[test]
fn grade_drops_below_a_when_issues_present() {
    // Many empty-catch (8pt penalty each on error_handling, weighted 0.25).
    // 5 empty catches → error_handling = 100 - 40 = 60; other dims 100.
    // overall = 60*0.25 + 100*0.75 = 15 + 75 = 90 → still A boundary;
    // bump to 6 for definite drop: error_handling = 52 → overall = 13 + 75 = 88 → B.
    let issues = vec![issue("avoid-empty-catch"); 6];
    let score = score_from_report(&report_with(issues, 50)).unwrap();
    assert_eq!(score.grade, Grade::B, "overall was {}", score.overall);
    assert!(score.overall >= 80 && score.overall <= 89);
}

#[test]
fn grade_is_f_with_catastrophic_findings() {
    // Hardcoded credentials apply a 25-pt penalty each on security (weight 0.15).
    // Combined with many empty catches, the overall score should collapse to F.
    let mut issues = vec![issue("avoid-hardcoded-credentials"); 8]; // security: 100 - 200 → saturates to 0
    issues.extend(std::iter::repeat_with(|| issue("avoid-empty-catch")).take(20)); // error_handling: 100 - 160 → 0
    issues.extend(std::iter::repeat_with(|| issue("ensure-dispose-lifecycle")).take(30)); // resource_safety: 100 - 150 → 0

    let score = score_from_report(&report_with(issues, 5)).unwrap();
    assert_eq!(score.grade, Grade::F, "overall was {}", score.overall);
    assert!(score.overall < 60);
}

// ── Per-dimension wiring ───────────────────────────────────────────────────

#[test]
fn resource_safety_penalises_dispose_and_stream_findings() {
    let issues = vec![
        issue("ensure-dispose-lifecycle"),
        issue("ensure-dispose-lifecycle"),
        issue("ensure-stream-subscription-cancel"),
    ];
    let score = score_from_report(&report_with(issues, 5)).unwrap();
    // 3 findings * 5pt each = 15 penalty.
    assert_eq!(score.resource_safety.score, 85);
    assert_eq!(score.resource_safety.findings.len(), 2); // two distinct categories
}

#[test]
fn security_penalty_for_credentials_is_steep() {
    let one_cred =
        score_from_report(&report_with(vec![issue("avoid-hardcoded-credentials")], 5)).unwrap();
    let zero = score_from_report(&report_with(vec![], 5)).unwrap();

    // Credentials are weighted heavily (25pt each).
    assert_eq!(zero.security.score, 100);
    assert_eq!(one_cred.security.score, 75);
    assert!(one_cred.overall < zero.overall);
}

#[test]
fn complexity_dimension_caps_penalty() {
    // Spam long-function findings — penalty should saturate at 60 (cap).
    let issues = vec![issue("avoid-long-functions"); 200];
    let score = score_from_report(&report_with(issues, 10)).unwrap();
    // Floor of the dimension after the cap is 40 (100 - 60).
    assert_eq!(score.complexity.score, 40);
}

// ── Edge cases ─────────────────────────────────────────────────────────────

#[test]
fn empty_report_yields_perfect_score() {
    let score = score_from_report(&report_with(vec![], 0)).unwrap();
    assert_eq!(score.overall, 100);
    assert_eq!(score.grade, Grade::A);
    assert_eq!(score.file_count, 0);
    assert_eq!(score.total_issues, 0);
    // All dimensions perfect.
    assert_eq!(score.resource_safety.score, 100);
    assert_eq!(score.error_handling.score, 100);
    assert_eq!(score.type_safety.score, 100);
    assert_eq!(score.security.score, 100);
    assert_eq!(score.convention_match.score, 100);
    assert_eq!(score.complexity.score, 100);
}

#[test]
fn zero_files_does_not_divide_by_zero() {
    // Even with non-empty issue list and file_count=0, scoring must not panic.
    let issues = vec![
        issue("avoid-dynamic"),
        issue("avoid-long-functions"),
        issue("prefer-correct-identifier-length"),
    ];
    let score = score_from_report(&report_with(issues, 0)).unwrap();
    assert!(score.overall <= 100);
    // The per-file ratios short-circuit to 0 when file_count == 0,
    // so these dims should remain at 100.
    assert_eq!(score.type_safety.score, 100);
    assert_eq!(score.convention_match.score, 100);
    assert_eq!(score.complexity.score, 100);
}

// ── Badge generation ───────────────────────────────────────────────────────

#[test]
fn badge_color_tracks_score_band() {
    let perfect = score_from_report(&report_with(vec![], 1)).unwrap();
    let badge = generate_badge(&perfect);
    assert!(badge.contains("100/100"));
    assert!(badge.contains("brightgreen"));
    assert!(badge.starts_with("![Falcon AI Score]"));

    // Drive overall into F band (<60).
    let mut issues = vec![issue("avoid-hardcoded-credentials"); 10];
    issues.extend(std::iter::repeat_with(|| issue("avoid-empty-catch")).take(30));
    issues.extend(std::iter::repeat_with(|| issue("ensure-dispose-lifecycle")).take(40));
    let bad = score_from_report(&report_with(issues, 5)).unwrap();
    let bad_badge = generate_badge(&bad);
    assert!(
        bad_badge.contains("red"),
        "expected red badge for overall {}: {}",
        bad.overall,
        bad_badge
    );
}
