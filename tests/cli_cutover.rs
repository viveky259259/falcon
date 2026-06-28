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
    for legacy in ["analyze", "metrics", "asset-audit", "vuln-scan", "test-gen"] {
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
    for command in ["analyze", "metrics", "asset-audit", "vuln-scan", "test-gen"] {
        assert!(
            stdout.contains(command),
            "missing legacy command {command} in legacy help:\n{stdout}"
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
