use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::cache::McpCache;
use super::schema::{
    explain_input_schema, fix_safe_input_schema, lint_diff_input_schema, lint_file_input_schema,
    review_input_schema, ExplainArgs, FixSafeArgs, LintDiffArgs, LintFileArgs, ReviewArgs,
};

/// MCP tool definitions for Falcon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Deprecated MCP tool names retained as aliases for one release.
///
/// Mapping is `(old_name, Some(new_name))` when there's a direct equivalent,
/// or `(old_name, None)` when no canonical replacement exists (legacy callers
/// keep working, but the tool is no longer advertised in discovery).
pub const DEPRECATED_TOOLS: &[(&str, Option<&str>)] = &[
    ("falcon_check_file", Some("lint_file")),
    ("falcon_analyze", Some("review")),
    ("falcon_explain_rule", Some("explain")),
    ("falcon_fix", Some("fix_safe")),
    ("falcon_ai_score", None),
    ("falcon_conventions", None),
    ("falcon_provenance", None),
];

/// The locked surface — exactly 5 tools advertised to clients.
pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "lint_file".to_string(),
            description: "Analyze a single Dart file for lint issues. Faster than full project analysis. Use after generating or modifying code.".to_string(),
            input_schema: lint_file_input_schema(),
        },
        ToolDefinition {
            name: "lint_diff".to_string(),
            description: "Analyze only Dart files changed relative to a base git ref (default: origin/main).".to_string(),
            input_schema: lint_diff_input_schema(),
        },
        ToolDefinition {
            name: "review".to_string(),
            description: "Run Falcon static analysis on a Flutter/Dart project. Returns issues with rule name, severity, file, line, and message.".to_string(),
            input_schema: review_input_schema(),
        },
        ToolDefinition {
            name: "explain".to_string(),
            description: "Explain a Falcon lint rule with examples, rationale, and fix suggestions.".to_string(),
            input_schema: explain_input_schema(),
        },
        ToolDefinition {
            name: "fix_safe".to_string(),
            description: "Generate auto-fix suggestions for lint issues in a project. Returns original and replacement code. Safe by default (preview-only).".to_string(),
            input_schema: fix_safe_input_schema(),
        },
    ]
}

/// Execute a tool by name with the given arguments.
///
/// Accepts both the new canonical names and the deprecated old names. When an
/// old name is used we log a deprecation warning and route to the same handler.
pub fn execute_tool(name: &str, args: &Value) -> Result<Value, String> {
    // Deprecation routing: log + map old name to handler.
    if let Some((_, new_opt)) = DEPRECATED_TOOLS.iter().find(|(old, _)| *old == name) {
        match new_opt {
            Some(new) => log::warn!(
                "deprecated MCP tool name '{}' \u{2014} use '{}'",
                name,
                new
            ),
            None => log::warn!(
                "deprecated MCP tool name '{}' \u{2014} no canonical replacement; this tool will be removed in a future release",
                name
            ),
        }
    }

    match name {
        // New canonical names.
        "lint_file" => execute_check_file(args),
        "lint_diff" => execute_lint_diff(args),
        "review" => execute_analyze(args),
        "explain" => execute_explain_rule(args),
        "fix_safe" => execute_fix(args),

        // Backward-compat aliases (still dispatchable, not in list_tools()).
        "falcon_check_file" => execute_check_file(args),
        "falcon_analyze" => execute_analyze(args),
        "falcon_explain_rule" => execute_explain_rule(args),
        "falcon_fix" => execute_fix(args),
        "falcon_ai_score" => execute_ai_score(args),
        "falcon_conventions" => execute_conventions(args),
        "falcon_provenance" => execute_provenance(args),

        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn parse_args<T>(tool: &str, args: &Value) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args.clone()).map_err(|e| format!("Invalid {} arguments: {}", tool, e))
}

fn execute_analyze(args: &Value) -> Result<Value, String> {
    let args: ReviewArgs = parse_args("review", args)?;
    let path = args.path;
    let path = PathBuf::from(&path);

    let mut config = crate::config::FalconConfig::load(&path)
        .map_err(|e| format!("Failed to load config: {}", e))?;

    if let Some(preset_name) = args.preset.as_deref() {
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
    let args: ReviewArgs = parse_args("falcon_ai_score", args)?;
    let path = args.path;
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
    let args: LintFileArgs = parse_args("lint_file", args)?;
    let LintFileArgs {
        file_path,
        source,
        project_root,
    } = args;
    let file_path = PathBuf::from(&file_path);
    let use_cache = source.is_none() && project_root.is_none();

    let source = if let Some(src) = source {
        src
    } else {
        std::fs::read_to_string(&file_path).map_err(|e| format!("Failed to read file: {}", e))?
    };

    if let Some(project_root) = project_root {
        let sdk = crate::sdk::FalconSdk::new();
        let sdk_issues = sdk
            .analyze_source_with_project_context(
                &source,
                &file_path.to_string_lossy(),
                &project_root,
            )
            .map_err(|e| format!("Analysis failed: {}", e))?;

        let issue_list: Vec<Value> = sdk_issues
            .iter()
            .map(|i| {
                serde_json::json!({
                    "rule": i.rule,
                    "message": i.message,
                    "severity": i.severity,
                    "line": i.line,
                    "column": i.column
                })
            })
            .collect();

        return Ok(serde_json::json!({
            "file": file_path.to_string_lossy(),
            "issue_count": issue_list.len(),
            "issues": issue_list
        }));
    }

    let cache = use_cache.then(McpCache::new);
    if let Some(cache) = &cache {
        if let Some(value) = cache.load_lint_file(&file_path, &source) {
            return Ok(value);
        }
    }

    let result = analyze_lint_file_source(&file_path, &source)?;
    if let Some(cache) = &cache {
        if let Err(err) = cache.store_lint_file(&file_path, &source, &result) {
            log::warn!(
                "failed to store MCP lint_file cache for {}: {}",
                file_path.display(),
                err
            );
        }
    }
    Ok(result)
}

fn analyze_lint_file_source(file_path: &Path, source: &str) -> Result<Value, String> {
    let mut parser =
        crate::parser::DartParser::new().map_err(|e| format!("Parser init failed: {}", e))?;
    let tree = parser
        .parse(source)
        .ok_or_else(|| "Failed to parse Dart source".to_string())?;

    let config = crate::config::FalconConfig::default();
    let mut registry = crate::rules::RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = registry.check(tree.root_node(), source, file_path);

    let metrics = crate::metrics::calculate_file_metrics(tree.root_node(), source, &config.metrics);
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
    let args: ExplainArgs = parse_args("explain", args)?;
    let rule = args.rule;

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
    let args: FixSafeArgs = parse_args("fix_safe", args)?;
    let path = args.path;
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

    let preview = args.preview;
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
    let args: ReviewArgs = parse_args("falcon_conventions", args)?;
    let path = args.path;
    let path = PathBuf::from(&path);

    let report = crate::ai_score::convention::detect_conventions(&path)
        .map_err(|e| format!("Convention detection failed: {}", e))?;

    serde_json::to_value(&report).map_err(|e| format!("Serialization failed: {}", e))
}

fn execute_provenance(args: &Value) -> Result<Value, String> {
    let args: ReviewArgs = parse_args("falcon_provenance", args)?;
    let path = args.path;
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

fn execute_lint_diff(args: &Value) -> Result<Value, String> {
    let args: LintDiffArgs = parse_args("lint_diff", args)?;
    let path = args.path;
    let root = PathBuf::from(&path);
    let base_ref = args.base_ref;

    let changed_files = crate::review::pr_review::changed_dart_files(&root, &base_ref)
        .map_err(|e| e.to_string())?;

    if changed_files.is_empty() {
        return Ok(serde_json::json!({
            "path": path,
            "base_ref": base_ref,
            "changed_file_count": 0,
            "file_count": 0,
            "issue_count": 0,
            "changed_files": [],
            "issues": []
        }));
    }

    let config = crate::config::FalconConfig::load(&root)
        .map_err(|e| format!("Failed to load config: {}", e))?;
    let falcon =
        crate::Falcon::new(config).map_err(|e| format!("Failed to initialize Falcon: {}", e))?;
    let report = falcon
        .analyze_files_with_project_context(&root, &changed_files)
        .map_err(|e| format!("Diff analysis failed: {}", e))?;

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
    let changed_file_values: Vec<Value> = changed_files
        .iter()
        .map(|f| {
            let display_path = f.strip_prefix(&root).unwrap_or(f);
            serde_json::json!(display_path.to_string_lossy())
        })
        .collect();

    Ok(serde_json::json!({
        "path": path,
        "base_ref": base_ref,
        "changed_file_count": changed_file_values.len(),
        "file_count": report.file_count,
        "issue_count": issues.len(),
        "changed_files": changed_file_values,
        "issues": issues
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;
    use std::process::Command;

    /// The 5 canonical tool names — locked surface.
    const CANONICAL_TOOLS: &[&str] = &["lint_file", "lint_diff", "review", "explain", "fix_safe"];

    #[test]
    fn list_tools_returns_exactly_five() {
        let tools = list_tools();
        assert_eq!(
            tools.len(),
            5,
            "MCP surface must be exactly 5 tools, got {}",
            tools.len()
        );
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        for canon in CANONICAL_TOOLS {
            assert!(
                names.contains(canon),
                "missing canonical tool '{}' in list_tools()",
                canon
            );
        }
        // No old names should appear in discovery.
        for (old, _) in DEPRECATED_TOOLS {
            assert!(
                !names.contains(old),
                "deprecated tool '{}' must not appear in list_tools()",
                old
            );
        }
    }

    #[test]
    fn list_tools_have_valid_input_schemas() {
        for tool in list_tools() {
            let schema = &tool.input_schema;
            assert_eq!(
                schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "tool '{}' inputSchema must have type=object",
                tool.name
            );
            assert!(
                schema.get("properties").is_some(),
                "tool '{}' inputSchema must declare properties",
                tool.name
            );
        }
    }

    #[test]
    fn canonical_names_are_dispatchable() {
        // Use intentionally-bad args so handlers exit early without doing work.
        // We only assert that we DO NOT get the "Unknown tool" error path.
        for name in CANONICAL_TOOLS {
            let result = execute_tool(name, &json!({}));
            if let Err(e) = &result {
                assert!(
                    !e.starts_with("Unknown tool"),
                    "canonical tool '{}' returned Unknown tool error: {}",
                    name,
                    e
                );
            }
        }
    }

    #[test]
    fn deprecated_names_still_dispatchable() {
        for (old, _) in DEPRECATED_TOOLS {
            let result = execute_tool(old, &json!({}));
            if let Err(e) = &result {
                assert!(
                    !e.starts_with("Unknown tool"),
                    "deprecated tool '{}' must remain dispatchable (alias path), got: {}",
                    old,
                    e
                );
            }
        }
    }

    #[test]
    fn unknown_tool_returns_clear_error() {
        let result = execute_tool("totally_made_up_tool", &json!({}));
        match result {
            Err(e) => assert!(
                e.starts_with("Unknown tool"),
                "expected 'Unknown tool' prefix, got: {}",
                e
            ),
            Ok(v) => panic!("expected error for unknown tool, got Ok({:?})", v),
        }
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

        let result = execute_tool(
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
    fn lint_diff_defaults_base_ref() {
        let repo = temp_git_repo();
        git(repo.path(), &["commit", "--allow-empty", "-m", "initial"]);

        let result = execute_tool(
            "lint_diff",
            &json!({ "path": repo.path().to_string_lossy() }),
        )
        .expect_err("missing default origin/main should surface git diff error");
        assert!(
            result.contains("origin/main...HEAD"),
            "default base ref should be origin/main in git error: {}",
            result
        );

        let result = execute_tool(
            "lint_diff",
            &json!({ "path": repo.path().to_string_lossy(), "base_ref": "HEAD" }),
        )
        .expect("explicit HEAD base should work");
        assert_eq!(result.get("base_ref"), Some(&json!("HEAD")));
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

    #[test]
    fn deprecated_tools_table_covers_all_old_names() {
        // Sanity: every old name must have an entry in DEPRECATED_TOOLS.
        let old_names: Vec<&str> = DEPRECATED_TOOLS.iter().map(|(o, _)| *o).collect();
        for expected in &[
            "falcon_check_file",
            "falcon_analyze",
            "falcon_explain_rule",
            "falcon_fix",
            "falcon_ai_score",
            "falcon_conventions",
            "falcon_provenance",
        ] {
            assert!(
                old_names.contains(expected),
                "DEPRECATED_TOOLS missing entry for '{}'",
                expected
            );
        }
    }
}
