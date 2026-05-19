//! Integration test: PR-E MCP tool surface lockdown.
//!
//! Pins the contract: exactly 5 advertised tools, with old names accepted
//! as aliases. Council mapping: EPIC 2.1.

use serde_json::json;

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
fn lint_diff_stub_response_carries_marker() {
    let result = falcon::mcp::tools::execute_tool(
        "lint_diff",
        &json!({ "path": "/tmp/dummy", "base_ref": "origin/main" }),
    )
    .expect("stub should succeed");

    assert_eq!(
        result.get("not_yet_implemented_changed_file_scoping"),
        Some(&json!(true)),
        "stub marker missing from response: {:?}",
        result
    );
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
