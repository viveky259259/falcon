use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn legacy_analyze_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let output = falcon_cmd()
        .args(["analyze", temp.path().to_str().unwrap(), "--format", "json"])
        .output()
        .expect("run falcon analyze");

    assert_success(&output);
    assert_deprecation_warning(&output, "analyze", "check");
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid analyze json: {}\nstdout:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout)
        )
    });
}

#[test]
fn legacy_pr_comment_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let output = falcon_cmd()
        .args(["pr-comment", temp.path().to_str().unwrap(), "--dry-run"])
        .output()
        .expect("run falcon pr-comment");

    assert_success(&output);
    assert_deprecation_warning(&output, "pr-comment", "review --format gh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Falcon Analysis"), "stdout:\n{}", stdout);
}

#[test]
fn legacy_metrics_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let output = falcon_cmd()
        .args(["metrics", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon metrics");

    assert_success(&output);
    assert_deprecation_warning(&output, "metrics", "x metrics");
}

#[test]
fn legacy_smells_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let output = falcon_cmd()
        .args(["smells", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon smells");

    assert_success(&output);
    assert_deprecation_warning(&output, "smells", "x smells");
}

#[test]
fn legacy_moved_check_commands_warn_and_still_run() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        ("check-unused-code", "x check-unused-code"),
        ("check-unused-files", "x check-unused-files"),
        ("check-dependencies", "x check-dependencies"),
        ("check-cycles", "x check-cycles"),
        ("check-unused-params", "x check-unused-params"),
        ("check-dead-code", "x check-dead-code"),
        ("check-unused-l10n", "x check-unused-l10n"),
        ("check-promoted-deps", "x check-promoted-deps"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, root])
            .output()
            .expect("run falcon legacy check command");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_platform_diagnostics_warn_and_still_run() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        ("upgrade-check", "x upgrade-check"),
        ("check-platform", "x check-platform"),
        ("check-codegen", "x check-codegen"),
        ("check-perf", "x check-perf"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, root])
            .output()
            .expect("run falcon legacy platform diagnostic");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_specialized_code_checks_warn_and_still_run() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        ("check-unused-confidence", "x check-unused-confidence"),
        ("check-layers", "x check-layers"),
        ("check-imports", "x check-imports"),
        ("cognitive-complexity", "x cognitive-complexity"),
        ("check-widgets", "x check-widgets"),
        ("check-async", "x check-async"),
        ("codebase-intel", "x codebase-intel"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, root])
            .output()
            .expect("run falcon legacy specialized code check");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_ai_insight_commands_warn_and_still_run() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        ("ai-report", "x ai-report"),
        ("provenance", "x provenance"),
        ("ai-profile", "x ai-profile"),
        ("discover-rules", "x discover-rules"),
        ("predict", "x predict"),
        ("drift", "x drift"),
        ("conventions", "x conventions"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, root])
            .output()
            .expect("run falcon legacy AI insight command");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_comparison_history_commands_warn_and_show_help() {
    let cases = [
        ("compare", "x compare"),
        ("compare-reports", "x compare-reports"),
        ("compare-branches", "x compare-branches"),
        ("history", "x history"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, "--help"])
            .output()
            .expect("run falcon legacy comparison command help");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_dashboard_analytics_commands_warn_and_show_help() {
    let cases = [
        (
            vec!["dashboard", "snapshot", "--help"],
            "dashboard",
            "x dashboard",
        ),
        (
            vec!["dashboard", "history", "--help"],
            "dashboard",
            "x dashboard",
        ),
        (
            vec!["dashboard", "serve", "--help"],
            "dashboard",
            "x dashboard",
        ),
        (vec!["trends", "--help"], "trends", "x trends"),
        (
            vec!["rule-impact", "--help"],
            "rule-impact",
            "x rule-impact",
        ),
    ];

    for (args, old, new) in cases {
        let output = falcon_cmd()
            .args(args)
            .output()
            .expect("run falcon legacy dashboard analytics help");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_tracking_learning_commands_warn_and_show_help() {
    let cases = [
        ("benchmark", "x benchmark"),
        ("benchmark-db", "x benchmark-db"),
        ("score-track", "x score-track"),
        ("perf-track", "x perf-track"),
        ("fix-track", "x fix-track"),
        ("self-tune", "x self-tune"),
        ("learn", "x learn"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, "--help"])
            .output()
            .expect("run falcon legacy tracking command help");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_config_rule_admin_commands_warn_and_show_help() {
    let cases = [
        ("baseline", "x baseline"),
        ("validate", "x validate"),
        ("explain", "x explain"),
        ("preset", "x preset"),
        ("rule-docs", "x rule-docs"),
        ("stability-contract", "x stability-contract"),
        ("deprecation-status", "x deprecation-status"),
        ("suppress", "x suppress"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, "--help"])
            .output()
            .expect("run falcon legacy config admin command help");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_platform_business_commands_warn_and_show_help() {
    let cases = [
        ("cloud", "x cloud"),
        ("enterprise", "x enterprise"),
        ("marketplace", "x marketplace"),
        ("certify", "x certify"),
        ("partners", "x partners"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, "--help"])
            .output()
            .expect("run falcon legacy platform command help");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_app_runtime_commands_warn_and_show_help() {
    let cases = [
        ("watch", "x watch"),
        ("run", "x run"),
        ("runtime-check", "x runtime-check"),
        ("live", "x live"),
        ("devtools", "x devtools"),
        ("manage", "x manage"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, "--help"])
            .output()
            .expect("run falcon legacy app runtime command help");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_sdk_passthrough_commands_warn_and_forward() {
    let cases = [
        ("flutter", "x flutter", ["doctor", "--verbose"].as_slice()),
        ("fvm", "x fvm", ["use", "stable"].as_slice()),
    ];

    for (old, new, forwarded_args) in cases {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let log = temp.path().join("sdk.log");
        make_fake_executable(&bin, old);

        let output = falcon_cmd()
            .arg(old)
            .args(forwarded_args)
            .env("PATH", path_with_front(&bin))
            .env("FAKE_SDK_LOG", &log)
            .output()
            .expect("run falcon legacy sdk passthrough command");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
        assert_forwarded_args(&log, forwarded_args);
    }
}

#[test]
fn legacy_ecosystem_ops_commands_warn_and_show_help() {
    let cases = [
        ("plugin", "x plugin"),
        ("export", "x export"),
        ("webhook", "x webhook"),
        ("migrate-from-dcm", "x migrate-from-dcm"),
        ("feature-gap", "x feature-gap"),
        ("showcase", "x showcase"),
        ("community", "x community"),
    ];

    for (old, new) in cases {
        let output = falcon_cmd()
            .args([old, "--help"])
            .output()
            .expect("run falcon legacy ecosystem ops command help");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn legacy_docs_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    let output_dir = temp.path().join("docs");
    let output = falcon_cmd()
        .args(["docs", output_dir.to_str().unwrap()])
        .output()
        .expect("run falcon docs");

    assert_success(&output);
    assert!(output_dir.join("RULES.md").is_file());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`falcon docs`")
            && stderr.contains("`falcon x docs`")
            && stderr.contains("removed in v1.0"),
        "stderr:\n{}",
        stderr
    );
}

#[test]
fn x_docs_command_does_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    let output_dir = temp.path().join("docs-x");
    let output = falcon_cmd()
        .args(["x", "docs", output_dir.to_str().unwrap()])
        .output()
        .expect("run falcon x docs");

    assert_success(&output);
    assert!(output_dir.join("RULES.md").is_file());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("removed in v1.0"),
        "unexpected deprecation warning:\n{}",
        stderr
    );
}

#[test]
fn legacy_workspace_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    let output = falcon_cmd()
        .args(["workspace", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon workspace");

    assert_success(&output);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`falcon workspace`")
            && stderr.contains("`falcon x workspace`")
            && stderr.contains("removed in v1.0"),
        "stderr:\n{}",
        stderr
    );
}

#[test]
fn legacy_dep_graph_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    let output = falcon_cmd()
        .args(["dep-graph", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon dep-graph");

    assert_success(&output);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`falcon dep-graph`")
            && stderr.contains("`falcon x dep-graph`")
            && stderr.contains("removed in v1.0"),
        "stderr:\n{}",
        stderr
    );
}

#[test]
fn legacy_vuln_scan_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    let output = falcon_cmd()
        .args(["vuln-scan", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon vuln-scan");

    assert_success(&output);
    assert_deprecation_warning(&output, "vuln-scan", "x vuln-scan");
}

#[test]
fn legacy_refactor_sim_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    let output = falcon_cmd()
        .args([
            "refactor-sim",
            temp.path().to_str().unwrap(),
            "--scenario",
            "set-state-to-riverpod",
        ])
        .output()
        .expect("run falcon refactor-sim");

    assert_success(&output);
    assert_deprecation_warning(&output, "refactor-sim", "x refactor-sim");
}

#[test]
fn legacy_test_gen_command_warns_and_still_runs() {
    let temp = tempfile::tempdir().unwrap();
    let output = falcon_cmd()
        .args(["test-gen", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon test-gen");

    assert_success(&output);
    assert_deprecation_warning(&output, "test-gen", "x test-gen");
}

#[test]
fn legacy_audit_commands_warn_and_still_run() {
    let temp = tempfile::tempdir().unwrap();
    write_l10n_fixture(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        (
            vec!["asset-audit", root, "--no-html"],
            "asset-audit",
            "x asset-audit",
        ),
        (
            vec!["theme-audit", root, "--no-html"],
            "theme-audit",
            "x theme-audit",
        ),
        (
            vec!["l10n-coverage", root, "--no-html"],
            "l10n-coverage",
            "x l10n-coverage",
        ),
        (
            vec!["deeplink-validate", root, "--no-html"],
            "deeplink-validate",
            "x deeplink-validate",
        ),
        (
            vec!["animation-audit", root, "--no-html"],
            "animation-audit",
            "x animation-audit",
        ),
        (
            vec!["golden-gen", root, "--dry-run", "--no-html"],
            "golden-gen",
            "x golden-gen",
        ),
    ];

    for (args, old, new) in cases {
        let output = falcon_cmd()
            .args(args)
            .output()
            .expect("run falcon legacy audit command");

        assert_success(&output);
        assert_deprecation_warning(&output, old, new);
    }
}

#[test]
fn x_analysis_tools_do_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_str().unwrap();
    let cases = [
        vec!["x", "vuln-scan", root],
        vec![
            "x",
            "refactor-sim",
            root,
            "--scenario",
            "set-state-to-riverpod",
        ],
        vec!["x", "test-gen", root],
    ];

    for args in cases {
        let output = falcon_cmd()
            .args(args)
            .output()
            .expect("run falcon x analysis tool");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_metrics_command_does_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let output = falcon_cmd()
        .args(["x", "metrics", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon x metrics");

    assert_success(&output);
    assert_no_deprecation_warning(&output);
}

#[test]
fn x_smells_command_does_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let output = falcon_cmd()
        .args(["x", "smells", temp.path().to_str().unwrap()])
        .output()
        .expect("run falcon x smells");

    assert_success(&output);
    assert_no_deprecation_warning(&output);
}

#[test]
fn x_moved_check_commands_do_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        "check-unused-code",
        "check-unused-files",
        "check-dependencies",
        "check-cycles",
        "check-unused-params",
        "check-dead-code",
        "check-unused-l10n",
        "check-promoted-deps",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, root])
            .output()
            .expect("run falcon x check command");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_platform_diagnostics_do_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        "upgrade-check",
        "check-platform",
        "check-codegen",
        "check-perf",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, root])
            .output()
            .expect("run falcon x platform diagnostic");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_specialized_code_checks_do_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        "check-unused-confidence",
        "check-layers",
        "check-imports",
        "cognitive-complexity",
        "check-widgets",
        "check-async",
        "codebase-intel",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, root])
            .output()
            .expect("run falcon x specialized code check");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_ai_insight_commands_do_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    write_dart_project(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        "ai-report",
        "provenance",
        "ai-profile",
        "discover-rules",
        "predict",
        "drift",
        "conventions",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, root])
            .output()
            .expect("run falcon x AI insight command");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_comparison_history_commands_do_not_warn_on_help() {
    let cases = ["compare", "compare-reports", "compare-branches", "history"];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, "--help"])
            .output()
            .expect("run falcon x comparison command help");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_dashboard_analytics_commands_do_not_warn_on_help() {
    let cases = [
        vec!["x", "dashboard", "snapshot", "--help"],
        vec!["x", "dashboard", "history", "--help"],
        vec!["x", "dashboard", "serve", "--help"],
        vec!["x", "trends", "--help"],
        vec!["x", "rule-impact", "--help"],
    ];

    for args in cases {
        let output = falcon_cmd()
            .args(args)
            .output()
            .expect("run falcon x dashboard analytics help");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_tracking_learning_commands_do_not_warn_on_help() {
    let cases = [
        "benchmark",
        "benchmark-db",
        "score-track",
        "perf-track",
        "fix-track",
        "self-tune",
        "learn",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, "--help"])
            .output()
            .expect("run falcon x tracking command help");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_config_rule_admin_commands_do_not_warn_on_help() {
    let cases = [
        "baseline",
        "validate",
        "explain",
        "preset",
        "rule-docs",
        "stability-contract",
        "deprecation-status",
        "suppress",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, "--help"])
            .output()
            .expect("run falcon x config admin command help");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_platform_business_commands_do_not_warn_on_help() {
    let cases = ["cloud", "enterprise", "marketplace", "certify", "partners"];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, "--help"])
            .output()
            .expect("run falcon x platform command help");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_app_runtime_commands_do_not_warn_on_help() {
    let cases = [
        "watch",
        "run",
        "runtime-check",
        "live",
        "devtools",
        "manage",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, "--help"])
            .output()
            .expect("run falcon x app runtime command help");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_sdk_passthrough_commands_do_not_warn_and_forward() {
    let cases = [
        ("flutter", ["doctor", "--verbose"].as_slice()),
        ("fvm", ["use", "stable"].as_slice()),
    ];

    for (command, forwarded_args) in cases {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let log = temp.path().join("sdk.log");
        make_fake_executable(&bin, command);

        let output = falcon_cmd()
            .args(["x", command])
            .args(forwarded_args)
            .env("PATH", path_with_front(&bin))
            .env("FAKE_SDK_LOG", &log)
            .output()
            .expect("run falcon x sdk passthrough command");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
        assert_forwarded_args(&log, forwarded_args);
    }
}

#[test]
fn x_ecosystem_ops_commands_do_not_warn_on_help() {
    let cases = [
        "plugin",
        "export",
        "webhook",
        "migrate-from-dcm",
        "feature-gap",
        "showcase",
        "community",
    ];

    for command in cases {
        let output = falcon_cmd()
            .args(["x", command, "--help"])
            .output()
            .expect("run falcon x ecosystem ops command help");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

#[test]
fn x_audit_commands_do_not_warn() {
    let temp = tempfile::tempdir().unwrap();
    write_l10n_fixture(temp.path());
    let root = temp.path().to_str().unwrap();
    let cases = [
        vec!["x", "asset-audit", root, "--no-html"],
        vec!["x", "theme-audit", root, "--no-html"],
        vec!["x", "l10n-coverage", root, "--no-html"],
        vec!["x", "deeplink-validate", root, "--no-html"],
        vec!["x", "animation-audit", root, "--no-html"],
        vec!["x", "golden-gen", root, "--dry-run", "--no-html"],
    ];

    for args in cases {
        let output = falcon_cmd()
            .args(args)
            .output()
            .expect("run falcon x audit command");

        assert_success(&output);
        assert_no_deprecation_warning(&output);
    }
}

fn falcon_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}

fn make_fake_executable(bin_dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(bin_dir).unwrap();
    let path = bin_dir.join(name);
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_SDK_LOG\"\nexit 0\n",
    )
    .unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    path
}

fn path_with_front(dir: &Path) -> OsString {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).unwrap()
}

fn assert_forwarded_args(log: &Path, expected: &[&str]) {
    let logged = std::fs::read_to_string(log).unwrap();
    let actual: Vec<_> = logged.lines().collect();
    assert_eq!(actual, expected, "forwarded args log:\n{}", logged);
}

fn assert_deprecation_warning(output: &Output, old: &str, new: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("`falcon {old}`"))
            && stderr.contains(&format!("`falcon {new}`"))
            && stderr.contains("removed in v1.0"),
        "stderr:\n{}",
        stderr
    );
}

fn assert_no_deprecation_warning(output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("removed in v1.0"),
        "unexpected deprecation warning:\n{}",
        stderr
    );
}

fn write_l10n_fixture(root: &std::path::Path) {
    let l10n_dir = root.join("lib/l10n");
    std::fs::create_dir_all(&l10n_dir).unwrap();
    std::fs::write(
        l10n_dir.join("app_en.arb"),
        r#"{"@@locale":"en","hello":"Hello"}"#,
    )
    .unwrap();
    std::fs::write(
        l10n_dir.join("app_es.arb"),
        r#"{"@@locale":"es","hello":"Hola"}"#,
    )
    .unwrap();
}

fn write_dart_project(root: &std::path::Path) {
    let lib = root.join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();
    std::fs::write(
        root.join("pubspec.yaml"),
        "name: falcon_cli_deprecation_test\nenvironment:\n  sdk: '>=3.0.0 <4.0.0'\n",
    )
    .unwrap();
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
