use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn analyze_since_uses_project_resolver_context() {
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
            "analyze",
            repo.path().to_str().unwrap(),
            "--since",
            "HEAD~1",
            "--format",
            "json",
        ])
        .output()
        .expect("run falcon analyze --since");

    assert_success_or_findings_failure(&output);
    let json = output_json(&output);
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
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid analyze json: {}\nstdout:\n{}\nstderr:\n{}",
            e,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
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
