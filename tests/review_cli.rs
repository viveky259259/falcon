use serde_json::Value;
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
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();
    std::fs::write(root.join("README.md"), "initial\n").unwrap();
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
