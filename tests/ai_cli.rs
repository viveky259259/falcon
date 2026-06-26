use serde_json::Value;
use std::process::Command;

#[test]
fn ai_help_lists_triage_subcommand() {
    let output = falcon_cmd()
        .args(["ai", "--help"])
        .output()
        .expect("run falcon ai --help");

    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("triage"), "stdout:\n{stdout}");
}

#[test]
#[cfg(not(feature = "ai-local"))]
fn ai_triage_default_build_json_reports_unavailable() {
    let output = falcon_cmd()
        .args(["ai", "triage", ".", "--format", "json"])
        .output()
        .expect("run falcon ai triage");

    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "invalid triage json: {err}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(json["available"], false);
    assert!(
        json["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("--features ai-local")),
        "json: {json:?}"
    );
}

#[test]
#[cfg(feature = "ai-local")]
fn ai_triage_ai_local_json_runs_with_degraded_verdicts() {
    let dir = tempfile::tempdir().expect("temp project");
    std::fs::create_dir_all(dir.path().join("lib")).expect("create lib");
    std::fs::write(
        dir.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .expect("write dart file");

    let output = falcon_cmd()
        .args([
            "ai",
            "triage",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("run falcon ai triage");

    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "invalid triage json: {err}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    });

    assert!(json["triaged"].as_u64().unwrap_or(0) >= 1, "json: {json}");
    assert_eq!(json["verdicts"][0]["degraded"], true);
    assert_eq!(json["verdicts"][0]["is_real"], true);
    assert!(
        json["verdicts"][0]["rationale"]
            .as_str()
            .is_some_and(|rationale| rationale.contains("Inference failed")),
        "json: {json}"
    );
}

fn falcon_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}
