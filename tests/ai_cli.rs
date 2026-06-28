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
fn ai_status_shows_embedded_model_details_when_configured() {
    let dir = tempfile::tempdir().expect("temp project");
    std::fs::write(
        dir.path().join("falcon.yaml"),
        r#"
rules: []
ai:
  enabled: true
  provider: embedded
  embedded:
    model_id: "local/status-model"
    model_file: "status-model.gguf"
    max_issues: 7
"#,
    )
    .expect("write falcon config");

    let output = falcon_cmd()
        .args(["ai", "status", dir.path().to_str().unwrap()])
        .output()
        .expect("run falcon ai status");

    assert!(
        output.status.success(),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Embedded model:"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("Model id:   local/status-model"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Model file: status-model.gguf"),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains("Max issues: 7"), "stdout:\n{stdout}");
    #[cfg(feature = "ai-local")]
    assert!(
        stdout.contains("Engine:     compiled in (ai-local)"),
        "stdout:\n{stdout}"
    );
    #[cfg(not(feature = "ai-local"))]
    assert!(
        stdout.contains("Engine:     NOT compiled (rebuild with --features ai-local)"),
        "stdout:\n{stdout}"
    );
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
fn ai_triage_ai_local_json_clean_project_skips_engine_load() {
    let dir = tempfile::tempdir().expect("temp project");
    std::fs::create_dir_all(dir.path().join("lib")).expect("create lib");
    std::fs::write(dir.path().join("lib/main.dart"), "void main() {}\n").expect("write dart file");

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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("ensuring embedded model"),
        "stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("ensuring embedded tokenizer"),
        "stderr:\n{stderr}"
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "invalid triage json: {err}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(json["triaged"], 0);
    assert_eq!(json["verdicts"].as_array().map(Vec::len), Some(0));
}

#[test]
#[cfg(feature = "ai-local")]
fn ai_triage_ai_local_uses_embedded_defaults_even_when_ai_config_disabled() {
    let dir = tempfile::tempdir().expect("temp project");
    std::fs::create_dir_all(dir.path().join("lib")).expect("create lib");
    std::fs::write(dir.path().join("lib/main.dart"), "void main() {}\n").expect("write dart file");
    std::fs::write(
        dir.path().join("falcon.yaml"),
        r#"
rules: []
ai:
  enabled: false
  provider: openai
"#,
    )
    .expect("write falcon config");

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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("ensuring embedded model"),
        "stderr:\n{stderr}"
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "invalid triage json: {err}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(json["triaged"], 0);
}

#[test]
#[cfg(feature = "ai-local")]
#[ignore = "downloads the embedded model/tokenizer and runs local inference"]
fn ai_triage_ai_local_json_runs_with_embedded_engine() {
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
    assert_eq!(json["verdicts"][0]["degraded"], false, "json: {json}");
    assert!(json["verdicts"][0]["is_real"].is_boolean(), "json: {json}");
    assert!(
        json["verdicts"][0]["confidence"].as_u64().is_some(),
        "json: {json}"
    );
    assert!(
        json["verdicts"][0]["rationale"]
            .as_str()
            .is_some_and(|rationale| !rationale.is_empty()),
        "json: {json}"
    );
}

fn falcon_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}
