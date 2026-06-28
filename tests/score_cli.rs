use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[test]
fn score_json_outputs_ai_score() {
    let project = temp_dart_project();

    let output = falcon_cmd()
        .args(["score", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run falcon score");

    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid score json: {}\nstdout:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert!(json["overall"].as_u64().is_some(), "json: {json:?}");
    assert!(json["grade"].as_str().is_some(), "json: {json:?}");
}

#[test]
fn score_format_json_alias_outputs_ai_score() {
    let project = temp_dart_project();
    let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args([
            "score",
            project.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("run falcon score");

    assert!(
        output.status.success(),
        "score --format json should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["overall"].as_u64(), Some(100));
}

#[test]
fn ai_score_alias_warns_and_still_outputs_json() {
    let project = temp_dart_project();

    let output = falcon_cmd()
        .args(["ai-score", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run falcon ai-score");

    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`falcon ai-score`")
            && stderr.contains("`falcon score`")
            && stderr.contains("removed in v1.0"),
        "stderr:\n{}",
        stderr
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid score json: {}\nstdout:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert!(json["overall"].as_u64().is_some(), "json: {json:?}");
}

fn falcon_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}

fn temp_dart_project() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    let lib = project.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();
    write_pubspec(project.path());
    project
}

fn write_pubspec(root: &Path) {
    std::fs::write(
        root.join("pubspec.yaml"),
        "name: falcon_score_test\nenvironment:\n  sdk: '>=3.0.0 <4.0.0'\n",
    )
    .unwrap();
}
