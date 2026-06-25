//! Integration test: PR-E MCP tool surface lockdown.
//!
//! Pins the contract: exactly 5 advertised tools, with old names accepted
//! as aliases. Council mapping: EPIC 2.1.

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
    assert_eq!(tools.len(), 5, "locked surface size is 5");
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
        let result = falcon::mcp::tools::execute_tool(name, &json!({}));
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
        let result = falcon::mcp::tools::execute_tool(name, &json!({}));
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
    let result = falcon::mcp::tools::execute_tool("this_does_not_exist", &json!({}));
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
    )
    .expect("lint_diff should analyze changed Dart files with project context");

    assert_eq!(result.get("changed_file_count"), Some(&json!(1)));
    assert_eq!(result.get("file_count"), Some(&json!(1)));
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
