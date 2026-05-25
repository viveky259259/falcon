//! End-to-end test for `falcon check-a11y`.

use std::process::Command;
use tempfile::TempDir;

fn falcon_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_falcon"))
}

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn check_a11y_exits_two_on_missing_ensure_semantics() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("lib/main.dart"),
        "void main() {\n  runApp(MyApp());\n}\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-a11y")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ensureSemantics"), "stdout: {stdout}");
}

#[test]
fn check_a11y_exits_zero_when_clean() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("lib/main.dart"),
        "void main() {\n  SemanticsBinding.instance.ensureSemantics();\n  runApp(MyApp());\n}\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-a11y")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn check_a11y_json_format() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("lib/main.dart"),
        "void main() {\n  runApp(MyApp());\n}\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-a11y")
        .arg(tmp.path())
        .args(["--format", "json"])
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["schema_version"], serde_json::json!(1));
    assert!(parsed["issues"].as_array().unwrap().len() >= 1);
    assert_eq!(
        parsed["issues"][0]["rule_id"],
        "a11y/missing-ensure-semantics"
    );
}
