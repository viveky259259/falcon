//! End-to-end test for `falcon check-assets`.
//!
//! Builds and invokes the actual CLI binary against a TempDir fixture, asserts
//! exit code + presence of substrings in stdout.

use std::process::Command;
use tempfile::TempDir;

fn falcon_bin() -> std::path::PathBuf {
    // CARGO_BIN_EXE_<name> is set by cargo for integration tests.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_falcon"))
}

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn check_assets_exits_zero_when_all_present() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/a.png\n",
    );
    write(&tmp.path().join("assets/a.png"), "");

    let output = Command::new(falcon_bin())
        .arg("check-assets")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_assets_exits_two_on_missing_file() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/missing.png\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-assets")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Missing Asset"), "stdout: {stdout}");
    assert!(stdout.contains("assets/missing.png"), "stdout: {stdout}");
}

#[test]
fn check_assets_json_format_is_valid_json() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/missing.png\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-assets")
        .arg(tmp.path())
        .args(["--format", "json"])
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(parsed["schema_version"], serde_json::json!(1));
    assert_eq!(parsed["issues"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["issues"][0]["rule_id"], "assets/missing-file");
}
