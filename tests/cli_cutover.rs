use std::process::{Command, Output};

#[test]
fn default_help_lists_only_cutover_verbs() {
    let output = falcon_cmd()
        .arg("--help")
        .output()
        .expect("run falcon --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in ["review", "check", "fix", "score", "x"] {
        assert!(
            stdout.contains(command),
            "missing command {command} in help:\n{stdout}"
        );
    }
    for legacy in [
        "analyze",
        "check-unused-code",
        "check-unused-files",
        "check-dependencies",
        "check-cycles",
        "check-unused-params",
        "check-dead-code",
        "check-unused-l10n",
        "check-promoted-deps",
        "upgrade-check",
        "check-platform",
        "check-codegen",
        "check-perf",
        "check-unused-confidence",
        "check-layers",
        "check-imports",
        "cognitive-complexity",
        "check-widgets",
        "check-async",
        "codebase-intel",
        "ai-report",
        "provenance",
        "ai-profile",
        "discover-rules",
        "predict",
        "drift",
        "conventions",
        "compare",
        "compare-reports",
        "compare-branches",
        "history",
        "baseline",
        "validate",
        "explain",
        "preset",
        "rule-docs",
        "stability-contract",
        "deprecation-status",
        "suppress",
        "dashboard",
        "trends",
        "rule-impact",
        "benchmark",
        "benchmark-db",
        "score-track",
        "perf-track",
        "fix-track",
        "self-tune",
        "learn",
        "smells",
        "metrics",
        "asset-audit",
        "theme-audit",
        "l10n-coverage",
        "deeplink-validate",
        "animation-audit",
        "golden-gen",
        "dep-graph",
        "workspace",
        "docs",
        "vuln-scan",
        "refactor-sim",
        "test-gen",
    ] {
        assert!(
            !stdout.contains(legacy),
            "legacy command {legacy} leaked into default help:\n{stdout}"
        );
    }
    assert!(stdout.contains("--legacy-help"));
}

#[test]
fn legacy_help_lists_historical_commands() {
    let output = falcon_cmd()
        .arg("--legacy-help")
        .output()
        .expect("run falcon --legacy-help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in ["analyze"] {
        assert!(
            stdout.contains(command),
            "missing legacy command {command} in legacy help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_flutter_quality_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "asset-audit",
        "theme-audit",
        "l10n-coverage",
        "deeplink-validate",
        "animation-audit",
        "golden-gen",
    ] {
        assert!(
            stdout.contains(command),
            "missing x command {command} in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_core_analysis_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in ["metrics", "smells"] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_moved_check_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "check-unused-code",
        "check-unused-files",
        "check-dependencies",
        "check-cycles",
        "check-unused-params",
        "check-dead-code",
        "check-unused-l10n",
        "check-promoted-deps",
    ] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_platform_diagnostics() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "upgrade-check",
        "check-platform",
        "check-codegen",
        "check-perf",
    ] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_specialized_code_checks() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "check-unused-confidence",
        "check-layers",
        "check-imports",
        "cognitive-complexity",
        "check-widgets",
        "check-async",
        "codebase-intel",
    ] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_ai_insight_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "ai-report",
        "provenance",
        "ai-profile",
        "discover-rules",
        "predict",
        "drift",
        "conventions",
    ] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_comparison_history_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in ["compare", "compare-reports", "compare-branches", "history"] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_dashboard_analytics_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in ["dashboard", "trends", "rule-impact"] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_tracking_learning_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "benchmark",
        "benchmark-db",
        "score-track",
        "perf-track",
        "fix-track",
        "self-tune",
        "learn",
    ] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_config_rule_admin_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "baseline",
        "validate",
        "explain",
        "preset",
        "rule-docs",
        "stability-contract",
        "deprecation-status",
        "suppress",
    ] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn x_help_lists_documented_migration_commands() {
    let output = falcon_cmd()
        .args(["x", "--help"])
        .output()
        .expect("run falcon x --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "dep-graph",
        "workspace",
        "docs",
        "vuln-scan",
        "refactor-sim",
        "test-gen",
    ] {
        assert!(
            stdout.contains(command),
            "missing x {command} command in help:\n{stdout}"
        );
    }
}

#[test]
fn check_help_is_available_as_stable_top_level_verb() {
    let output = falcon_cmd()
        .args(["check", "--help"])
        .output()
        .expect("run falcon check --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run project checks and static analysis"));
    assert!(stdout.contains("--fail-on"));
    assert!(stdout.contains("--since"));
    assert!(stdout.contains("--semantic"));
    assert!(stdout.contains("--no-defer-to-analyzer"));
}

#[test]
fn review_help_lists_semantic_mode() {
    let output = falcon_cmd()
        .args(["review", "--help"])
        .output()
        .expect("run falcon review --help");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Review code changes"));
    assert!(stdout.contains("--semantic"));
    assert!(stdout.contains("--no-defer-to-analyzer"));
}

fn falcon_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
