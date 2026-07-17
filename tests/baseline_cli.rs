use serde_json::Value;
use std::env;
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

#[test]
fn check_does_not_auto_run_semantic_for_dart_project() {
    let project = temp_dart_project();
    write_package_config(project.path());
    let fake_dart_dir = fake_dart_on_path();

    let output = falcon_cmd()
        .env("PATH", path_with(fake_dart_dir.path()))
        .args([
            "check",
            project.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("run falcon check");

    assert_success_or_findings_failure(&output);
    let json = check_json(&output);
    assert_has_rule(&json, "avoid-print-in-production");
}

#[test]
fn check_semantic_suppresses_same_line_falcon_issue() {
    let project = temp_dart_project();
    write_package_config(project.path());
    let fake_dart_dir = fake_dart_on_path();

    let output = falcon_cmd()
        .env("PATH", path_with(fake_dart_dir.path()))
        .args([
            "check",
            project.path().to_str().unwrap(),
            "--format",
            "json",
            "--semantic",
        ])
        .output()
        .expect("run falcon check --semantic");

    assert_success_or_findings_failure(&output);
    let json = check_json(&output);
    assert_no_rule(&json, "avoid-print-in-production");
}

#[test]
fn check_semantic_no_defer_keeps_same_line_falcon_issue() {
    let project = temp_dart_project();
    write_package_config(project.path());
    let fake_dart_dir = fake_dart_on_path();

    let output = falcon_cmd()
        .env("PATH", path_with(fake_dart_dir.path()))
        .args([
            "check",
            project.path().to_str().unwrap(),
            "--format",
            "json",
            "--semantic",
            "--no-defer-to-analyzer",
        ])
        .output()
        .expect("run falcon check --semantic --no-defer-to-analyzer");

    assert_success_or_findings_failure(&output);
    let json = check_json(&output);
    assert_has_rule(&json, "avoid-print-in-production");
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

fn write_package_config(root: &Path) {
    let dart_tool = root.join(".dart_tool");
    fs::create_dir_all(&dart_tool).unwrap();
    fs::write(
        dart_tool.join("package_config.json"),
        r#"{"configVersion":2,"packages":[]}"#,
    )
    .unwrap();
}

fn fake_dart_on_path() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let dart = dir.path().join("dart");
    fs::write(
        &dart,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "Dart SDK version: 3.0.0"
  exit 0
fi
cat <<'JSON'
{"diagnostics":[{"code":"avoid_print","severity":"INFO","problemMessage":"Avoid print.","location":{"file":"lib/main.dart","range":{"start":{"line":2,"column":3}}}}]}
JSON
exit 1
"#,
    )
    .unwrap();
    make_executable(&dart);
    dir
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }
}

fn path_with(dir: &Path) -> String {
    let current = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(env::split_paths(&current));
    env::join_paths(paths)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn check_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid check json: {}\nstdout:\n{}\nstderr:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_has_rule(json: &Value, rule_id: &str) {
    let issues = json["issues"].as_array().expect("issues array");
    assert!(
        issues
            .iter()
            .any(|issue| issue["rule"].as_str() == Some(rule_id)),
        "expected rule {} in {}",
        rule_id,
        json
    );
}

fn assert_no_rule(json: &Value, rule_id: &str) {
    let issues = json["issues"].as_array().expect("issues array");
    assert!(
        !issues
            .iter()
            .any(|issue| issue["rule"].as_str() == Some(rule_id)),
        "did not expect rule {} in {}",
        rule_id,
        json
    );
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
