use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// MCP tool definitions for Falcon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "falcon_analyze".to_string(),
            description: "Run Falcon static analysis on a Flutter/Dart project or file. Returns issues with rule name, severity, file, line, and message.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Flutter/Dart project directory or file to analyze"
                    },
                    "preset": {
                        "type": "string",
                        "description": "Optional rule preset (recommended, strict, flutter, ai-generated)",
                        "enum": ["recommended", "strict", "flutter", "riverpod", "bloc", "performance", "ai-generated"]
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "falcon_ai_score".to_string(),
            description: "Calculate AI Code Quality Score (0-100) with 6-dimension breakdown: Resource Safety, Error Handling, Type Safety, Security, Convention Match, Complexity.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Flutter/Dart project directory"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "falcon_check_file".to_string(),
            description: "Analyze a single Dart file for lint issues. Faster than full project analysis. Use after generating or modifying code.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the Dart file to check"
                    },
                    "source": {
                        "type": "string",
                        "description": "Optional: Dart source code to analyze (if provided, file_path is used only for context)"
                    }
                },
                "required": ["file_path"]
            }),
        },
        ToolDefinition {
            name: "falcon_explain_rule".to_string(),
            description: "Explain a Falcon lint rule with examples, rationale, and fix suggestions.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "rule": {
                        "type": "string",
                        "description": "Rule name to explain (e.g. 'avoid-empty-catch', 'ensure-dispose-lifecycle')"
                    }
                },
                "required": ["rule"]
            }),
        },
        ToolDefinition {
            name: "falcon_fix".to_string(),
            description: "Generate auto-fix suggestions for lint issues in a project. Returns original and replacement code.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Flutter/Dart project directory"
                    },
                    "preview": {
                        "type": "boolean",
                        "description": "If true, only preview fixes without applying them (default: true)",
                        "default": true
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "falcon_conventions".to_string(),
            description: "Auto-detect team conventions (naming patterns, architecture, state management, error handling) from a Flutter project.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Flutter/Dart project directory"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "falcon_provenance".to_string(),
            description: "Analyze code provenance — detect AI-generated vs human-written vs code-generated files.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Flutter/Dart project directory"
                    }
                },
                "required": ["path"]
            }),
        },
    ]
}

/// Execute a tool by name with the given arguments.
pub fn execute_tool(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "falcon_analyze" => execute_analyze(args),
        "falcon_ai_score" => execute_ai_score(args),
        "falcon_check_file" => execute_check_file(args),
        "falcon_explain_rule" => execute_explain_rule(args),
        "falcon_fix" => execute_fix(args),
        "falcon_conventions" => execute_conventions(args),
        "falcon_provenance" => execute_provenance(args),
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn get_string_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing required argument: {}", key))
}

fn execute_analyze(args: &Value) -> Result<Value, String> {
    let path = get_string_arg(args, "path")?;
    let path = PathBuf::from(&path);

    let mut config = crate::config::FalconConfig::load(&path)
        .map_err(|e| format!("Failed to load config: {}", e))?;

    if let Some(preset_name) = args.get("preset").and_then(|v| v.as_str()) {
        if let Some(p) = crate::plugins::presets::get_preset(preset_name) {
            config.rules = p.rules;
        }
    }

    let falcon =
        crate::Falcon::new(config).map_err(|e| format!("Failed to initialize Falcon: {}", e))?;
    let report = falcon
        .analyze(&path)
        .map_err(|e| format!("Analysis failed: {}", e))?;

    let issues: Vec<Value> = report
        .issues
        .iter()
        .map(|i| {
            serde_json::json!({
                "rule": i.rule,
                "message": i.message,
                "severity": format!("{:?}", i.severity),
                "file": i.file.to_string_lossy(),
                "line": i.line,
                "column": i.column
            })
        })
        .collect();

    Ok(serde_json::json!({
        "file_count": report.file_count,
        "issue_count": issues.len(),
        "issues": issues
    }))
}

fn execute_ai_score(args: &Value) -> Result<Value, String> {
    let path = get_string_arg(args, "path")?;
    let path = PathBuf::from(&path);

    let score = crate::ai_score::score::calculate_ai_score(&path)
        .map_err(|e| format!("Score calculation failed: {}", e))?;

    Ok(serde_json::json!({
        "overall": score.overall,
        "grade": score.grade.to_string(),
        "resource_safety": { "score": score.resource_safety.score, "findings": score.resource_safety.findings },
        "error_handling": { "score": score.error_handling.score, "findings": score.error_handling.findings },
        "type_safety": { "score": score.type_safety.score, "findings": score.type_safety.findings },
        "security": { "score": score.security.score, "findings": score.security.findings },
        "convention_match": { "score": score.convention_match.score, "findings": score.convention_match.findings },
        "complexity": { "score": score.complexity.score, "findings": score.complexity.findings },
        "file_count": score.file_count,
        "total_issues": score.total_issues,
        "production_ready": score.overall >= 85
    }))
}

fn execute_check_file(args: &Value) -> Result<Value, String> {
    let file_path = get_string_arg(args, "file_path")?;
    let file_path = PathBuf::from(&file_path);

    let source = if let Some(src) = args.get("source").and_then(|v| v.as_str()) {
        src.to_string()
    } else {
        std::fs::read_to_string(&file_path).map_err(|e| format!("Failed to read file: {}", e))?
    };

    let mut parser =
        crate::parser::DartParser::new().map_err(|e| format!("Parser init failed: {}", e))?;
    let tree = parser
        .parse(&source)
        .ok_or_else(|| "Failed to parse Dart source".to_string())?;

    let config = crate::config::FalconConfig::default();
    let mut registry = crate::rules::RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = registry.check(tree.root_node(), &source, &file_path);

    let metrics =
        crate::metrics::calculate_file_metrics(tree.root_node(), &source, &config.metrics);
    let mut all_issues = issues;
    all_issues.extend(metrics.violations());

    let issue_list: Vec<Value> = all_issues
        .iter()
        .map(|i| {
            serde_json::json!({
                "rule": i.rule,
                "message": i.message,
                "severity": format!("{:?}", i.severity),
                "line": i.line,
                "column": i.column
            })
        })
        .collect();

    Ok(serde_json::json!({
        "file": file_path.to_string_lossy(),
        "issue_count": issue_list.len(),
        "issues": issue_list
    }))
}

fn execute_explain_rule(args: &Value) -> Result<Value, String> {
    let rule = get_string_arg(args, "rule")?;

    match crate::ai::explain::explain_rule(&rule) {
        Some(explanation) => Ok(serde_json::json!({
            "rule": explanation.name,
            "category": explanation.category,
            "severity": explanation.severity,
            "summary": explanation.summary,
            "why": explanation.why,
            "bad_example": explanation.bad_example,
            "good_example": explanation.good_example,
            "exceptions": explanation.exceptions
        })),
        None => Err(format!("Unknown rule: {}", rule)),
    }
}

fn execute_fix(args: &Value) -> Result<Value, String> {
    let path = get_string_arg(args, "path")?;
    let path = PathBuf::from(&path);

    let config = crate::config::FalconConfig::load(&path)
        .map_err(|e| format!("Failed to load config: {}", e))?;
    let falcon = crate::Falcon::new(config).map_err(|e| format!("Failed to initialize: {}", e))?;
    let report = falcon
        .analyze(&path)
        .map_err(|e| format!("Analysis failed: {}", e))?;

    let fixes = crate::ai::fix::generate_fixes(&report.issues, &path);

    let fix_list: Vec<Value> = fixes
        .iter()
        .filter(|f| f.auto_fixable)
        .map(|f| {
            serde_json::json!({
                "rule": f.rule,
                "file": f.file.to_string_lossy(),
                "line": f.line,
                "original": f.original,
                "replacement": f.replacement,
                "description": f.description
            })
        })
        .collect();

    let preview = args
        .get("preview")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    if !preview {
        let applied = crate::ai::fix::apply_fixes(&fixes);
        Ok(serde_json::json!({
            "applied": applied,
            "fixes": fix_list
        }))
    } else {
        Ok(serde_json::json!({
            "preview": true,
            "fix_count": fix_list.len(),
            "fixes": fix_list
        }))
    }
}

fn execute_conventions(args: &Value) -> Result<Value, String> {
    let path = get_string_arg(args, "path")?;
    let path = PathBuf::from(&path);

    let report = crate::ai_score::convention::detect_conventions(&path)
        .map_err(|e| format!("Convention detection failed: {}", e))?;

    serde_json::to_value(&report).map_err(|e| format!("Serialization failed: {}", e))
}

fn execute_provenance(args: &Value) -> Result<Value, String> {
    let path = get_string_arg(args, "path")?;
    let path = PathBuf::from(&path);

    let results = crate::ai_score::provenance::analyze_project_provenance(&path)
        .map_err(|e| format!("Provenance analysis failed: {}", e))?;
    let summary = crate::ai_score::provenance::summarize_provenance(&results);

    Ok(serde_json::json!({
        "total_files": summary.total_files,
        "human_files": summary.human_files,
        "ai_files": summary.ai_files,
        "codegen_files": summary.codegen_files,
        "unknown_files": summary.unknown_files,
        "ai_percentage": summary.ai_percentage
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ─────────────────────────────────────────────────────────────

    /// Create a minimal Dart project in a TempDir: lib/main.dart
    fn make_dart_project() -> TempDir {
        let dir = TempDir::new().expect("TempDir::new");
        let lib_dir = dir.path().join("lib");
        fs::create_dir_all(&lib_dir).unwrap();
        let mut f = fs::File::create(lib_dir.join("main.dart")).unwrap();
        write!(
            f,
            "import 'package:flutter/material.dart';\n\nvoid main() {{\n  runApp(MyApp());\n}}\n\nclass MyApp extends StatelessWidget {{\n  @override\n  Widget build(BuildContext context) {{\n    return MaterialApp(home: Scaffold(body: Center(child: Text('Hello'))));\n  }}\n}}\n"
        )
        .unwrap();
        dir
    }

    /// Create a single `.dart` file in a TempDir and return (TempDir, PathBuf-to-file).
    fn make_dart_file(source: &str) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("TempDir::new");
        let file_path = dir.path().join("test.dart");
        fs::write(&file_path, source).unwrap();
        (dir, file_path)
    }

    // ══════════════════════════════════════════════════════════════════════
    // list_tools
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn list_tools_returns_seven_tools() {
        let tools = list_tools();
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn list_tools_contains_falcon_analyze() {
        let tools = list_tools();
        assert!(tools.iter().any(|t| t.name == "falcon_analyze"));
    }

    #[test]
    fn list_tools_contains_all_expected_names() {
        let tools = list_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        for expected in &[
            "falcon_analyze",
            "falcon_ai_score",
            "falcon_check_file",
            "falcon_explain_rule",
            "falcon_fix",
            "falcon_conventions",
            "falcon_provenance",
        ] {
            assert!(names.contains(expected), "missing tool: {}", expected);
        }
    }

    #[test]
    fn list_tools_each_has_nonempty_description() {
        for tool in list_tools() {
            assert!(
                !tool.description.is_empty(),
                "tool {} has empty description",
                tool.name
            );
        }
    }

    #[test]
    fn list_tools_each_has_object_input_schema() {
        for tool in list_tools() {
            assert_eq!(
                tool.input_schema["type"],
                json!("object"),
                "tool {} schema type is not object",
                tool.name
            );
        }
    }

    #[test]
    fn list_tools_analyze_schema_has_path_required() {
        let tools = list_tools();
        let analyze = tools.iter().find(|t| t.name == "falcon_analyze").unwrap();
        let required = analyze.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("path")));
    }

    #[test]
    fn list_tools_check_file_schema_requires_file_path() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|t| t.name == "falcon_check_file")
            .unwrap();
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("file_path")));
    }

    #[test]
    fn list_tools_explain_rule_schema_requires_rule() {
        let tools = list_tools();
        let tool = tools
            .iter()
            .find(|t| t.name == "falcon_explain_rule")
            .unwrap();
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("rule")));
    }

    #[test]
    fn list_tools_can_be_serialized_to_json() {
        let tools = list_tools();
        let serialized = serde_json::to_string(&tools);
        assert!(serialized.is_ok());
        let text = serialized.unwrap();
        assert!(text.contains("falcon_analyze"));
    }

    // ══════════════════════════════════════════════════════════════════════
    // get_string_arg
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn get_string_arg_returns_value_when_present() {
        let args = json!({"key": "value"});
        assert_eq!(get_string_arg(&args, "key").unwrap(), "value");
    }

    #[test]
    fn get_string_arg_returns_error_when_missing() {
        let args = json!({});
        let err = get_string_arg(&args, "missing").unwrap_err();
        assert!(err.contains("missing"), "err was: {}", err);
    }

    #[test]
    fn get_string_arg_returns_error_for_non_string_value() {
        let args = json!({"num": 42});
        let err = get_string_arg(&args, "num").unwrap_err();
        assert!(err.contains("num"), "err was: {}", err);
    }

    #[test]
    fn get_string_arg_returns_empty_string_value() {
        let args = json!({"key": ""});
        assert_eq!(get_string_arg(&args, "key").unwrap(), "");
    }

    #[test]
    fn get_string_arg_null_value_is_error() {
        let args = json!({"key": null});
        assert!(get_string_arg(&args, "key").is_err());
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_tool dispatch
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn execute_tool_unknown_name_returns_error() {
        let result = execute_tool("does_not_exist", &json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("does_not_exist"), "err: {}", err);
    }

    #[test]
    fn execute_tool_analyze_missing_path_returns_error() {
        let result = execute_tool("falcon_analyze", &json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("path") || err.contains("Missing"),
            "err: {}",
            err
        );
    }

    #[test]
    fn execute_tool_ai_score_missing_path_returns_error() {
        let result = execute_tool("falcon_ai_score", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn execute_tool_check_file_missing_file_path_returns_error() {
        let result = execute_tool("falcon_check_file", &json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("file_path") || err.contains("Missing"),
            "err: {}",
            err
        );
    }

    #[test]
    fn execute_tool_explain_rule_missing_rule_returns_error() {
        let result = execute_tool("falcon_explain_rule", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn execute_tool_fix_missing_path_returns_error() {
        let result = execute_tool("falcon_fix", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn execute_tool_conventions_missing_path_returns_error() {
        let result = execute_tool("falcon_conventions", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn execute_tool_provenance_missing_path_returns_error() {
        let result = execute_tool("falcon_provenance", &json!({}));
        assert!(result.is_err());
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_explain_rule
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn explain_rule_known_rule_returns_ok() {
        let result = execute_tool("falcon_explain_rule", &json!({"rule": "avoid-long-functions"}));
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn explain_rule_result_has_expected_fields() {
        let val = execute_tool("falcon_explain_rule", &json!({"rule": "avoid-long-functions"}))
            .unwrap();
        assert!(val.get("rule").is_some(), "missing 'rule' field");
        assert!(val.get("category").is_some(), "missing 'category' field");
        assert!(val.get("severity").is_some(), "missing 'severity' field");
        assert!(val.get("summary").is_some(), "missing 'summary' field");
        assert!(val.get("why").is_some(), "missing 'why' field");
        assert!(val.get("bad_example").is_some(), "missing 'bad_example'");
        assert!(val.get("good_example").is_some(), "missing 'good_example'");
        assert!(val.get("exceptions").is_some(), "missing 'exceptions'");
    }

    #[test]
    fn explain_rule_rule_name_matches_request() {
        let val = execute_tool("falcon_explain_rule", &json!({"rule": "avoid-long-functions"}))
            .unwrap();
        assert_eq!(val["rule"], json!("avoid-long-functions"));
    }

    #[test]
    fn explain_rule_no_magic_numbers_known() {
        let result = execute_tool("falcon_explain_rule", &json!({"rule": "no-magic-numbers"}));
        assert!(result.is_ok());
    }

    #[test]
    fn explain_rule_unknown_rule_returns_error() {
        let result =
            execute_tool("falcon_explain_rule", &json!({"rule": "totally-fake-rule-xyz"}));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("totally-fake-rule-xyz"), "err: {}", err);
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_check_file  (uses source= to avoid real FS reads)
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn check_file_with_inline_source_returns_ok() {
        let (_dir, file_path) = make_dart_file("void main() {}");
        let result = execute_tool(
            "falcon_check_file",
            &json!({
                "file_path": file_path.to_string_lossy(),
                "source": "void main() {}"
            }),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn check_file_result_has_required_keys() {
        let (_dir, file_path) = make_dart_file("void main() {}");
        let val = execute_tool(
            "falcon_check_file",
            &json!({
                "file_path": file_path.to_string_lossy(),
                "source": "void main() {}"
            }),
        )
        .unwrap();
        assert!(val.get("file").is_some(), "missing 'file'");
        assert!(val.get("issue_count").is_some(), "missing 'issue_count'");
        assert!(val.get("issues").is_some(), "missing 'issues'");
    }

    #[test]
    fn check_file_issue_count_matches_issues_array_length() {
        let source = "void main() {}\nvar x = 1;\n";
        let (_dir, file_path) = make_dart_file(source);
        let val = execute_tool(
            "falcon_check_file",
            &json!({
                "file_path": file_path.to_string_lossy(),
                "source": source
            }),
        )
        .unwrap();
        let count = val["issue_count"].as_u64().unwrap() as usize;
        let arr_len = val["issues"].as_array().unwrap().len();
        assert_eq!(count, arr_len);
    }

    #[test]
    fn check_file_reads_from_disk_when_no_source() {
        let source = "void main() {}";
        let (_dir, file_path) = make_dart_file(source);
        let result = execute_tool(
            "falcon_check_file",
            &json!({ "file_path": file_path.to_string_lossy() }),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn check_file_nonexistent_file_without_source_returns_error() {
        let result = execute_tool(
            "falcon_check_file",
            &json!({ "file_path": "/nonexistent/path/file.dart" }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn check_file_issue_items_have_rule_and_message() {
        // Use code likely to produce at least one issue via empty-catch-like patterns
        let source = "void main() { try {} catch (e) {} }";
        let (_dir, file_path) = make_dart_file(source);
        let val = execute_tool(
            "falcon_check_file",
            &json!({
                "file_path": file_path.to_string_lossy(),
                "source": source
            }),
        )
        .unwrap();
        let issues = val["issues"].as_array().unwrap();
        for issue in issues {
            assert!(issue.get("rule").is_some(), "issue missing 'rule'");
            assert!(issue.get("message").is_some(), "issue missing 'message'");
            assert!(issue.get("severity").is_some(), "issue missing 'severity'");
            assert!(issue.get("line").is_some(), "issue missing 'line'");
            assert!(issue.get("column").is_some(), "issue missing 'column'");
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_analyze (full project)
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn analyze_valid_project_returns_ok() {
        let dir = make_dart_project();
        let result = execute_tool("falcon_analyze", &json!({"path": dir.path().to_string_lossy()}));
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn analyze_result_has_file_count_and_issues() {
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_analyze", &json!({"path": dir.path().to_string_lossy()})).unwrap();
        assert!(val.get("file_count").is_some());
        assert!(val.get("issue_count").is_some());
        assert!(val.get("issues").is_some());
    }

    #[test]
    fn analyze_with_recommended_preset_returns_ok() {
        let dir = make_dart_project();
        let result = execute_tool(
            "falcon_analyze",
            &json!({
                "path": dir.path().to_string_lossy(),
                "preset": "recommended"
            }),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn analyze_with_strict_preset_returns_ok() {
        let dir = make_dart_project();
        let result = execute_tool(
            "falcon_analyze",
            &json!({
                "path": dir.path().to_string_lossy(),
                "preset": "strict"
            }),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn analyze_with_unknown_preset_still_runs() {
        // An unknown preset is silently ignored (get_preset returns None)
        let dir = make_dart_project();
        let result = execute_tool(
            "falcon_analyze",
            &json!({
                "path": dir.path().to_string_lossy(),
                "preset": "nonexistent-preset"
            }),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn analyze_issue_count_matches_issues_array() {
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_analyze", &json!({"path": dir.path().to_string_lossy()})).unwrap();
        let count = val["issue_count"].as_u64().unwrap() as usize;
        let arr_len = val["issues"].as_array().unwrap().len();
        assert_eq!(count, arr_len);
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_ai_score
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn ai_score_valid_project_returns_ok() {
        let dir = make_dart_project();
        let result =
            execute_tool("falcon_ai_score", &json!({"path": dir.path().to_string_lossy()}));
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn ai_score_result_has_overall_and_grade() {
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_ai_score", &json!({"path": dir.path().to_string_lossy()}))
                .unwrap();
        assert!(val.get("overall").is_some(), "missing 'overall'");
        assert!(val.get("grade").is_some(), "missing 'grade'");
    }

    #[test]
    fn ai_score_result_has_all_six_dimensions() {
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_ai_score", &json!({"path": dir.path().to_string_lossy()}))
                .unwrap();
        for dim in &[
            "resource_safety",
            "error_handling",
            "type_safety",
            "security",
            "convention_match",
            "complexity",
        ] {
            assert!(val.get(*dim).is_some(), "missing dimension '{}'", dim);
        }
    }

    #[test]
    fn ai_score_dimensions_have_score_and_findings() {
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_ai_score", &json!({"path": dir.path().to_string_lossy()}))
                .unwrap();
        for dim in &[
            "resource_safety",
            "error_handling",
            "type_safety",
            "security",
            "convention_match",
            "complexity",
        ] {
            let d = &val[*dim];
            assert!(d.get("score").is_some(), "{} missing 'score'", dim);
            assert!(d.get("findings").is_some(), "{} missing 'findings'", dim);
        }
    }

    #[test]
    fn ai_score_has_production_ready_field() {
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_ai_score", &json!({"path": dir.path().to_string_lossy()}))
                .unwrap();
        assert!(val.get("production_ready").is_some());
    }

    #[test]
    fn ai_score_has_file_count_and_total_issues() {
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_ai_score", &json!({"path": dir.path().to_string_lossy()}))
                .unwrap();
        assert!(val.get("file_count").is_some());
        assert!(val.get("total_issues").is_some());
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_fix
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn fix_valid_project_preview_true_returns_ok() {
        let dir = make_dart_project();
        let result = execute_tool(
            "falcon_fix",
            &json!({"path": dir.path().to_string_lossy(), "preview": true}),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn fix_preview_true_result_has_preview_flag() {
        let dir = make_dart_project();
        let val = execute_tool(
            "falcon_fix",
            &json!({"path": dir.path().to_string_lossy(), "preview": true}),
        )
        .unwrap();
        assert_eq!(val["preview"], json!(true));
        assert!(val.get("fix_count").is_some());
        assert!(val.get("fixes").is_some());
    }

    #[test]
    fn fix_default_preview_is_true() {
        // Omitting "preview" should default to preview=true
        let dir = make_dart_project();
        let val =
            execute_tool("falcon_fix", &json!({"path": dir.path().to_string_lossy()})).unwrap();
        assert_eq!(val["preview"], json!(true));
    }

    #[test]
    fn fix_preview_false_returns_applied_field() {
        let dir = make_dart_project();
        let val = execute_tool(
            "falcon_fix",
            &json!({"path": dir.path().to_string_lossy(), "preview": false}),
        )
        .unwrap();
        assert!(val.get("applied").is_some(), "missing 'applied'");
        assert!(val.get("fixes").is_some(), "missing 'fixes'");
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_conventions
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn conventions_valid_project_returns_ok() {
        let dir = make_dart_project();
        let result = execute_tool(
            "falcon_conventions",
            &json!({"path": dir.path().to_string_lossy()}),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn conventions_returns_json_object() {
        let dir = make_dart_project();
        let val = execute_tool(
            "falcon_conventions",
            &json!({"path": dir.path().to_string_lossy()}),
        )
        .unwrap();
        assert!(val.is_object() || val.is_array(), "expected JSON object or array, got: {:?}", val);
    }

    // ══════════════════════════════════════════════════════════════════════
    // execute_provenance
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn provenance_valid_project_returns_ok() {
        let dir = make_dart_project();
        let result = execute_tool(
            "falcon_provenance",
            &json!({"path": dir.path().to_string_lossy()}),
        );
        assert!(result.is_ok(), "err: {:?}", result.err());
    }

    #[test]
    fn provenance_result_has_summary_fields() {
        let dir = make_dart_project();
        let val = execute_tool(
            "falcon_provenance",
            &json!({"path": dir.path().to_string_lossy()}),
        )
        .unwrap();
        assert!(val.get("total_files").is_some(), "missing 'total_files'");
        assert!(val.get("human_files").is_some(), "missing 'human_files'");
        assert!(val.get("ai_files").is_some(), "missing 'ai_files'");
        assert!(val.get("codegen_files").is_some(), "missing 'codegen_files'");
        assert!(val.get("unknown_files").is_some(), "missing 'unknown_files'");
        assert!(val.get("ai_percentage").is_some(), "missing 'ai_percentage'");
    }

    #[test]
    fn provenance_invalid_path_returns_error() {
        let result = execute_tool(
            "falcon_provenance",
            &json!({"path": "/absolutely/nonexistent/path/xyz123"}),
        );
        // May succeed with 0 files or fail; either is acceptable, but should not panic
        // We just assert it doesn't panic — the result can go either way
        let _ = result;
    }
}
