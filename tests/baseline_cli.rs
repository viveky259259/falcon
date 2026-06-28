use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn check_baseline_filters_existing_findings_and_can_update_file() {
    let project = temp_dart_project();
    let baseline = project.path().join(".falcon-baseline.json");

    let update_output = falcon_cmd()
        .args([
            "check",
            project.path().to_str().unwrap(),
            "--format",
            "json",
            "--update-baseline",
            baseline.to_str().unwrap(),
        ])
        .output()
        .expect("run falcon check --update-baseline");

    assert_success_or_findings_failure(&update_output);
    assert!(baseline.is_file());
    let baseline_json: Value =
        serde_json::from_str(&fs::read_to_string(&baseline).unwrap()).unwrap();
    assert_eq!(baseline_json["schema_version"], 1);

    let filtered_output = falcon_cmd()
        .args([
            "check",
            project.path().to_str().unwrap(),
            "--format",
            "json",
            "--baseline",
            baseline.to_str().unwrap(),
        ])
        .output()
        .expect("run falcon check --baseline");

    assert_success_or_findings_failure(&filtered_output);
    let json: Value = serde_json::from_slice(&filtered_output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid check json: {}\nstdout:\n{}",
            e,
            String::from_utf8_lossy(&filtered_output.stdout)
        )
    });
    assert_eq!(json["issues"].as_array().unwrap().len(), 0);
}

fn falcon_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}

fn temp_dart_project() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    let lib = project.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(
        lib.join("main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    write_pubspec(project.path());
    project
}

fn write_pubspec(root: &Path) {
    fs::write(
        root.join("pubspec.yaml"),
        "name: falcon_baseline_test\nenvironment:\n  sdk: '>=3.0.0 <4.0.0'\n",
    )
    .unwrap();
}

fn assert_success_or_findings_failure(output: &Output) {
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "unexpected status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
