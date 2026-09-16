//! Integration test: PR-E MCP tool surface lockdown.
//!
//! Pins the contract: exactly 6 advertised tools, with old names accepted
//! as aliases. Council mapping: EPIC 2.1.

use falcon::doctor::Trust;
use serde_json::json;
use std::path::Path;
use std::process::Command;

const CANONICAL: &[&str] = &["lint_file", "lint_diff", "review", "explain", "fix_safe"];

const DEPRECATED: &[&str] = &[
    "falcon_analyze",
    "falcon_ai_score",
    "falcon_check_file",
    "falcon_explain_rule",
    "falcon_fix",
    "falcon_conventions",
    "falcon_provenance",
];

#[test]
fn list_tools_returns_exactly_five_canonical_entries() {
    let tools = falcon::mcp::tools::list_tools();
    assert_eq!(tools.len(), 6, "locked surface size is 6");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    for canon in CANONICAL {
        assert!(names.contains(canon), "missing canonical '{}'", canon);
    }
}

#[test]
fn deprecated_names_absent_from_list_tools() {
    let tools = falcon::mcp::tools::list_tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    for old in DEPRECATED {
        assert!(
            !names.contains(old),
            "deprecated '{}' must not be advertised",
            old
        );
    }
}

#[test]
fn each_canonical_name_is_dispatchable() {
    for name in CANONICAL {
        let result = falcon::mcp::tools::execute_tool(name, &json!({}), Trust::Local);
        if let Err(e) = &result {
            assert!(
                !e.starts_with("Unknown tool"),
                "canonical '{}' must be dispatchable, got: {}",
                name,
                e
            );
        }
    }
}

#[test]
fn each_deprecated_name_is_still_dispatchable_as_alias() {
    for name in DEPRECATED {
        let result = falcon::mcp::tools::execute_tool(name, &json!({}), Trust::Local);
        if let Err(e) = &result {
            assert!(
                !e.starts_with("Unknown tool"),
                "deprecated '{}' must remain dispatchable (alias), got: {}",
                name,
                e
            );
        }
    }
}

#[test]
fn unknown_tool_yields_clear_error() {
    let result = falcon::mcp::tools::execute_tool("this_does_not_exist", &json!({}), Trust::Local);
    let err = result.expect_err("unknown tool must error");
    assert!(
        err.starts_with("Unknown tool"),
        "expected 'Unknown tool' prefix, got: {}",
        err
    );
}

#[test]
fn lint_diff_analyzes_changed_dart_files() {
    let repo = temp_git_repo();
    let lib = repo.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "class Good {}\n").unwrap();
    std::fs::write(repo.path().join("README.md"), "initial\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    std::fs::write(
        lib.join("main.dart"),
        "class badName {\n  void run() {\n    print('debug');\n  }\n}\n",
    )
    .unwrap();
    std::fs::write(repo.path().join("README.md"), "changed\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "change Dart file"]);

    let result = falcon::mcp::tools::execute_tool(
        "lint_diff",
        &json!({ "path": repo.path().to_string_lossy(), "base_ref": "HEAD~1" }),
        Trust::Local,
    )
    .expect("lint_diff should analyze changed Dart files");

    assert_eq!(
        result.get("changed_file_count"),
        Some(&json!(1)),
        "lint_diff should ignore non-Dart changed files: {:?}",
        result
    );
    assert_eq!(result.get("file_count"), Some(&json!(1)));
    assert!(
        result
            .get("changed_files")
            .and_then(|v| v.as_array())
            .is_some_and(|files| files.contains(&json!("lib/main.dart"))),
        "changed_files should include lib/main.dart: {:?}",
        result
    );
    assert!(
        result
            .get("issue_count")
            .and_then(|v| v.as_u64())
            .unwrap_or_default()
            > 0,
        "changed Dart file should be analyzed and report issues: {:?}",
        result
    );
}

#[test]
fn lint_diff_uses_project_resolver_context_for_changed_files() {
    let repo = temp_git_repo();
    let lib = repo.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        repo.path().join("falcon.yaml"),
        "rules:\n  - dispose-not-called:\n      severity: error\nunused:\n  enabled: false\n",
    )
    .unwrap();
    std::fs::write(
        lib.join("base.dart"),
        "class BaseState extends State<W> {}\n",
    )
    .unwrap();
    std::fs::write(
        lib.join("screen.dart"),
        "class _ScreenState extends BaseState {}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "initial"]);

    std::fs::write(
        lib.join("screen.dart"),
        r#"
class _ScreenState extends BaseState {
  final TextEditingController controller = TextEditingController();
}
"#,
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "add disposable field"]);

    let result = falcon::mcp::tools::execute_tool(
        "lint_diff",
        &json!({ "path": repo.path().to_string_lossy(), "base_ref": "HEAD~1" }),
        Trust::Local,
    )
    .expect("lint_diff should analyze changed Dart files with project context");

    assert_eq!(result.get("changed_file_count"), Some(&json!(1)));
    assert_eq!(result.get("file_count"), Some(&json!(1)));
    assert_has_rule(&result, "dispose-not-called");
}

#[test]
fn lint_file_can_opt_into_project_resolver_context() {
    let repo = temp_git_repo();
    let lib = repo.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        repo.path().join("falcon.yaml"),
        "rules:\n  - dispose-not-called:\n      severity: error\nunused:\n  enabled: false\n",
    )
    .unwrap();
    std::fs::write(
        lib.join("base.dart"),
        "class BaseState extends State<W> {}\n",
    )
    .unwrap();

    let result = falcon::mcp::tools::execute_tool(
        "lint_file",
        &json!({
            "file_path": "lib/screen.dart",
            "project_root": repo.path().to_string_lossy(),
            "source": r#"
class _ScreenState extends BaseState {
  final TextEditingController controller = TextEditingController();
}
"#
        }),
        Trust::Local,
    )
    .expect("lint_file should analyze source with project context");

    assert_eq!(result.get("file"), Some(&json!("lib/screen.dart")));
    assert_has_rule(&result, "dispose-not-called");
}

#[test]
fn schemas_have_draft_07_marker_and_required_fields() {
    for tool in falcon::mcp::tools::list_tools() {
        let schema = &tool.input_schema;
        let dollar_schema = schema.get("$schema").and_then(|v| v.as_str());
        assert_eq!(
            dollar_schema,
            Some("http://json-schema.org/draft-07/schema#"),
            "tool '{}' must declare draft-07 $schema",
            tool.name
        );
        assert_eq!(
            schema.get("type").and_then(|v| v.as_str()),
            Some("object"),
            "tool '{}' must be an object schema",
            tool.name
        );
        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| panic!("tool '{}' missing properties", tool.name));
        for (pname, pval) in props {
            assert!(
                pval.get("type").is_some(),
                "tool '{}' property '{}' missing type",
                tool.name,
                pname
            );
            assert!(
                pval.get("description").is_some(),
                "tool '{}' property '{}' missing description",
                tool.name,
                pname
            );
        }
    }
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

fn assert_has_rule(result: &serde_json::Value, rule: &str) {
    assert!(
        result["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["rule"] == rule),
        "expected rule {rule} in result:\n{}",
        serde_json::to_string_pretty(result).unwrap()
    );
}

// ─── `doctor` tool: surface and the Trust gate ─────────────────────────────
//
// `doctor`'s diagnosis path (`execute: false`, the default) calls
// `falcon::doctor::diagnose`, which unconditionally shells out to `curl` to
// fetch the Flutter release manifest — regardless of the project path, even
// a tempdir. Per the task's "no test touches the network" constraint, none
// of these tests drive a real diagnosis to completion; they check the
// surface, the schema default, and — the security-relevant part — that the
// Trust gate refuses a `Remote` execute *before* any network- or
// filesystem-mutating code runs, and that it does not refuse diagnosis.

#[test]
fn doctor_is_advertised_on_the_local_surface() {
    let tools = falcon::mcp::tools::list_tools();
    assert!(
        tools.iter().any(|t| t.name == "doctor"),
        "doctor must be advertised"
    );
}

#[test]
fn doctor_defaults_to_not_executing() {
    let schema = falcon::mcp::schema::doctor_input_schema();
    assert_eq!(schema["properties"]["execute"]["default"], json!(false));
}

#[test]
fn remote_trust_refuses_to_execute() {
    // `execute: true` must be refused for `Trust::Remote` before the call
    // ever reaches `doctor::execute` (which would download and run
    // installers) — this is the actual security boundary, and it must not
    // require network access to prove.
    let args = json!({ "path": ".", "execute": true, "decisions": {} });
    let err = falcon::mcp::tools::execute_tool("doctor", &args, Trust::Remote)
        .expect_err("remote execution must be refused");
    assert!(
        err.to_lowercase().contains("not permitted") || err.to_lowercase().contains("refus"),
        "refusal must be explicit: {}",
        err
    );
}

#[test]
fn remote_trust_does_not_gate_diagnosis_mode() {
    // The Trust gate must trip only on `execute: true`, never on `execute:
    // false` (the default). To prove that without running the
    // network-touching diagnosis pipeline, this omits the required `path`
    // field: argument parsing fails before `diagnose()` is ever called, so
    // the resulting error is a parse error, not the Trust refusal — showing
    // the refusal is specific to `execute: true` and not a blanket block on
    // `Trust::Remote`.
    let args = json!({ "execute": false });
    let err = falcon::mcp::tools::execute_tool("doctor", &args, Trust::Remote)
        .expect_err("missing required `path` must fail to parse");
    assert!(
        !err.to_lowercase().contains("not permitted") && !err.to_lowercase().contains("refus"),
        "diagnosis mode must never be refused on trust grounds: {}",
        err
    );
    assert!(
        err.contains("invalid doctor args"),
        "expected an argument-parsing error, not a trust refusal: {}",
        err
    );
}

#[test]
fn remote_listing_marks_doctor_diagnosis_only() {
    let remote = falcon::mcp::tools::list_tools_for(Trust::Remote);
    let doctor = remote
        .iter()
        .find(|t| t.name == "doctor")
        .expect("still listed");
    assert!(
        doctor.description.contains("diagnosis only"),
        "remote description must say so: {}",
        doctor.description
    );
}

#[test]
fn local_listing_keeps_the_full_doctor_description() {
    let local = falcon::mcp::tools::list_tools_for(Trust::Local);
    let doctor = local
        .iter()
        .find(|t| t.name == "doctor")
        .expect("still listed");
    assert!(
        !doctor.description.contains("diagnosis only"),
        "local (stdio) transport may install, so it must not be relabeled: {}",
        doctor.description
    );
}
