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
