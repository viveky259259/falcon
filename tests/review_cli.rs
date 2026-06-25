use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn review_format_gh_renders_pr_comment_for_changed_files() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    std::fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let output = falcon_cmd()
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "gh",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Falcon Analysis"), "stdout:\n{}", stdout);
    assert!(stdout.contains("lib/main.dart"), "stdout:\n{}", stdout);
}

#[test]
fn review_format_json_uses_diff_alias_and_reports_changed_file_count() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    std::fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  final value = 1;\n  print(value);\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let output = falcon_cmd()
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--diff",
            "HEAD~1",
            "--format",
            "json",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid review json: {}\nstdout:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(json["summary"]["files_analyzed"], 1);
}

#[test]
fn review_format_sarif_reports_changed_file_findings() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    std::fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let output = falcon_cmd()
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "sarif",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid review sarif: {}\nstdout:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(json["version"], "2.1.0");
    assert!(json["$schema"].as_str().unwrap_or("").contains("sarif"));
    assert_eq!(
        json["runs"][0]["results"][0]["ruleId"],
        "avoid-print-in-production"
    );
    assert_eq!(
        json["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
            ["uri"],
        repo.path().join("lib/main.dart").to_string_lossy().as_ref()
    );
}

#[test]
fn review_quick_strictness_reports_only_errors() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    std::fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let output = falcon_cmd()
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "json",
            "--strictness",
            "quick",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json = review_json(&output);
    assert_no_rule(&json, "avoid-print-in-production");
}

#[test]
fn review_analyzer_copilot_suppresses_same_line_falcon_issue() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    write_package_config(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let fake_dart_dir = fake_dart_on_path();
    let output = falcon_cmd()
        .env("PATH", path_with(fake_dart_dir.path()))
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "json",
            "--analyzer-copilot",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json = review_json(&output);
    assert_no_rule(&json, "avoid-print-in-production");
}

#[test]
fn review_auto_runs_analyzer_copilot_for_dart_project() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    write_package_config(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let fake_dart_dir = fake_dart_on_path();
    let output = falcon_cmd()
        .env("PATH", path_with(fake_dart_dir.path()))
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "json",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json = review_json(&output);
    assert_no_rule(&json, "avoid-print-in-production");
}

#[test]
fn review_no_defer_to_analyzer_keeps_same_line_falcon_issue() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    write_package_config(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let fake_dart_dir = fake_dart_on_path();
    let output = falcon_cmd()
        .env("PATH", path_with(fake_dart_dir.path()))
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "json",
            "--analyzer-copilot",
            "--no-defer-to-analyzer",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json = review_json(&output);
    assert_has_rule(&json, "avoid-print-in-production");
}

#[test]
fn review_analyzer_copilot_skips_without_package_config() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    fs::write(
        repo.path().join("lib/main.dart"),
        "void main() {\n  print('debug');\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let fake_dart_dir = fake_dart_on_path();
    let output = falcon_cmd()
        .env("PATH", path_with(fake_dart_dir.path()))
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "json",
            "--analyzer-copilot",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json = review_json(&output);
    assert_has_rule(&json, "avoid-print-in-production");
}

#[test]
fn review_uses_project_resolver_context_for_changed_files() {
    let repo = temp_git_repo();
    write_initial_project(repo.path());
    fs::write(
        repo.path().join("falcon.yaml"),
        "rules:\n  - dispose-not-called:\n      severity: error\nunused:\n  enabled: false\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("lib/base.dart"),
        "class BaseState extends State<W> {}\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("lib/screen.dart"),
        "class _ScreenState extends BaseState {}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    fs::write(
        repo.path().join("lib/screen.dart"),
        r#"
class _ScreenState extends BaseState {
  final TextEditingController controller = TextEditingController();
}
"#,
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "add disposable field"]);

    let output = falcon_cmd()
        .args([
            "review",
            repo.path().to_str().unwrap(),
            "--base-ref",
            "HEAD~1",
            "--format",
            "json",
        ])
        .output()
        .expect("run falcon review");

    assert_success_or_findings_failure(&output);
    let json = review_json(&output);
    assert_eq!(json["summary"]["files_analyzed"], 1);
    assert_has_rule(&json, "dispose-not-called");
}

fn falcon_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}

fn temp_git_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init"]);
    git(
        repo.path(),
        &["config", "user.email", "falcon@example.test"],
    );
    git(repo.path(), &["config", "user.name", "Falcon Test"]);
    repo
}

fn write_initial_project(root: &Path) {
    let lib = root.join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();
    fs::write(root.join("README.md"), "initial\n").unwrap();
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

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn path_with(prefix: &Path) -> std::ffi::OsString {
    let mut paths = vec![prefix.to_path_buf()];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    env::join_paths(paths).unwrap()
}

fn review_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid review json: {}\nstdout:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn assert_has_rule(json: &Value, rule: &str) {
    assert!(
        json["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["rule"] == rule),
        "expected rule {rule} in json:\n{}",
        serde_json::to_string_pretty(json).unwrap()
    );
}

fn assert_no_rule(json: &Value, rule: &str) {
    assert!(
        json["issues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|issue| issue["rule"] != rule),
        "did not expect rule {rule} in json:\n{}",
        serde_json::to_string_pretty(json).unwrap()
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {:?}: {}", args, e));
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
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
