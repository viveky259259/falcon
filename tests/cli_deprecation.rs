use std::process::{Command, Output};

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
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("removed in v1.0"),
            "unexpected deprecation warning:\n{}",
            stderr
        );
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
