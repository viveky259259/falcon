//! The `generate_tests` MCP tool: golden (widget screenshot) tests or unit
//! test stubs for a project. Preview by default; `write: true` writes files
//! that don't exist yet and never overwrites.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::schema::JSON_SCHEMA_DRAFT_07;
use super::tools::ToolDefinition;
use crate::doctor::Trust;

/// Longest test file content returned inline per file in preview mode.
const MAX_INLINE_CHARS: usize = 6_000;
const MAX_INLINE_FILES: usize = 10;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerateTestsArgs {
    path: String,
    kind: String,
    #[serde(default)]
    write: bool,
    #[serde(default)]
    output_dir: Option<String>,
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "generate_tests".to_string(),
        description: "Generate tests for the project: kind=golden writes golden (screenshot) tests for each widget, kind=unit writes unit test stubs for classes and functions. Previews by default and returns the test code; pass write:true to write files that don't exist yet (never overwrites).".to_string(),
        input_schema: json!({
            "$schema": JSON_SCHEMA_DRAFT_07,
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "Flutter/Dart project directory" },
                "kind": { "type": "string", "enum": ["golden", "unit"], "description": "golden: screenshot tests per widget; unit: test stubs per class" },
                "write": { "type": "boolean", "default": false, "description": "Write the files (default false: preview only). Never overwrites." },
                "output_dir": { "type": "string", "description": "golden only: directory under the project (default test/golden_generated)" }
            },
            "required": ["path", "kind"]
        }),
    }
}

pub fn execute(args: &Value, trust: Trust) -> Result<Value, String> {
    let a: GenerateTestsArgs = serde_json::from_value(args.clone())
        .map_err(|e| format!("Invalid generate_tests arguments: {}", e))?;
    if a.write && trust == Trust::Remote {
        return Err("writing tests is not permitted over this transport; preview only".to_string());
    }
    let project = PathBuf::from(&a.path);
    if !project.is_dir() {
        return Err(format!("{} is not a directory", project.display()));
    }
    match a.kind.as_str() {
        "golden" => golden(&project, a.output_dir.as_deref(), a.write),
        "unit" => {
            if a.output_dir.is_some() {
                return Err("`output_dir` applies to kind=golden only".to_string());
            }
            unit(&project, a.write)
        }
        other => Err(format!("Unknown kind '{other}': use golden or unit")),
    }
}

fn inline(content: &str, index: usize) -> Value {
    if index >= MAX_INLINE_FILES {
        return Value::Null;
    }
    if content.len() > MAX_INLINE_CHARS {
        let cut = (0..=MAX_INLINE_CHARS)
            .rev()
            .find(|i| content.is_char_boundary(*i))
            .unwrap_or(0);
        json!(format!("{}\n// … truncated", &content[..cut]))
    } else {
        json!(content)
    }
}

fn golden(project: &Path, output_dir: Option<&str>, write: bool) -> Result<Value, String> {
    let rel = output_dir.unwrap_or("test/golden_generated");
    if Path::new(rel).is_absolute() || rel.split(['/', '\\']).any(|p| p == "..") {
        return Err("`output_dir` must be a relative path inside the project".to_string());
    }
    let report = crate::golden_gen::generate_golden_tests(project, &project.join(rel), !write)
        .map_err(|e| format!("Golden test generation failed: {:#}", e))?;
    let files: Vec<Value> = report
        .generated
        .iter()
        .enumerate()
        .map(|(i, g)| {
            json!({
                "widget": g.widget_name,
                "source_file": g.source_file.display().to_string(),
                "test_file": g.test_file.display().to_string(),
                "content": if write { Value::Null } else { inline(&g.test_content, i) },
            })
        })
        .collect();
    Ok(json!({
        "kind": "golden",
        "written": write,
        "widgets_found": report.widgets_found,
        "already_tested": report.widgets_already_tested,
        "files": files,
        "skipped": report.skipped,
        "next_step": if write { "Run `flutter test --update-goldens` to record the golden images." } else { "Call again with write:true to write these files." },
    }))
}

fn unit(project: &Path, write: bool) -> Result<Value, String> {
    let stubs = crate::analysis::test_gen::generate_test_stubs(project);
    let mut files = Vec::new();
    let mut written = 0;
    for (i, stub) in stubs
        .iter()
        .filter(|s| !s.test_cases.is_empty())
        .enumerate()
    {
        let test_path = project.join(&stub.test_file);
        let exists = test_path.exists();
        let content = crate::analysis::test_gen::render_test_file(stub);
        let mut wrote = false;
        if write && !exists {
            if let Some(parent) = test_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
            }
            std::fs::write(&test_path, &content)
                .map_err(|e| format!("Failed to write {}: {}", test_path.display(), e))?;
            written += 1;
            wrote = true;
        }
        files.push(json!({
            "class": stub.class_name,
            "target_file": stub.target_file,
            "test_file": stub.test_file,
            "test_cases": stub.test_cases.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
            "already_exists": exists,
            "written": wrote,
            "content": if write { Value::Null } else { inline(&content, i) },
        }));
    }
    Ok(json!({ "kind": "unit", "written": written, "files": files }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_kind_and_escaping_output_dir() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().display().to_string();
        let err = execute(&json!({"path": p, "kind": "e2e"}), Trust::Local).unwrap_err();
        assert!(err.contains("Unknown kind"));
        let err = execute(
            &json!({"path": p, "kind": "golden", "output_dir": "../x"}),
            Trust::Local,
        )
        .unwrap_err();
        assert!(err.contains("inside the project"));
    }

    #[test]
    fn remote_may_preview_but_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().display().to_string();
        let err = execute(
            &json!({"path": p, "kind": "unit", "write": true}),
            Trust::Remote,
        )
        .unwrap_err();
        assert!(err.contains("not permitted"));
        assert!(execute(&json!({"path": p, "kind": "unit"}), Trust::Remote).is_ok());
    }

    #[test]
    fn unit_preview_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(
            dir.path().join("lib/calc.dart"),
            "class Calc {\n  int add(int a, int b) => a + b;\n}\n",
        )
        .unwrap();
        let p = dir.path().display().to_string();
        let v = execute(&json!({"path": p, "kind": "unit"}), Trust::Local).unwrap();
        assert_eq!(v["written"], 0);
        assert!(!dir.path().join("test").exists());
    }

    #[test]
    fn inline_truncates_long_content() {
        let long = "x".repeat(MAX_INLINE_CHARS + 10);
        assert!(inline(&long, 0).as_str().unwrap().ends_with("truncated"));
        assert!(inline("short", MAX_INLINE_FILES).is_null());
    }
}
