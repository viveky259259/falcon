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

    let falcon = crate::Falcon::new(config)
        .map_err(|e| format!("Failed to initialize Falcon: {}", e))?;
    let report = falcon.analyze(&path)
        .map_err(|e| format!("Analysis failed: {}", e))?;

    let issues: Vec<Value> = report.issues.iter().map(|i| {
        serde_json::json!({
            "rule": i.rule,
            "message": i.message,
            "severity": format!("{:?}", i.severity),
            "file": i.file.to_string_lossy(),
            "line": i.line,
            "column": i.column
        })
    }).collect();

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
        std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?
    };

    let mut parser = crate::parser::DartParser::new()
        .map_err(|e| format!("Parser init failed: {}", e))?;
    let tree = parser.parse(&source)
        .ok_or_else(|| "Failed to parse Dart source".to_string())?;

    let config = crate::config::FalconConfig::default();
    let mut registry = crate::rules::RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = registry.check(tree.root_node(), &source, &file_path);

    let metrics = crate::metrics::calculate_file_metrics(
        tree.root_node(), &source, &config.metrics
    );
    let mut all_issues = issues;
    all_issues.extend(metrics.violations());

    let issue_list: Vec<Value> = all_issues.iter().map(|i| {
        serde_json::json!({
            "rule": i.rule,
            "message": i.message,
            "severity": format!("{:?}", i.severity),
            "line": i.line,
            "column": i.column
        })
    }).collect();

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
    let falcon = crate::Falcon::new(config)
        .map_err(|e| format!("Failed to initialize: {}", e))?;
    let report = falcon.analyze(&path)
        .map_err(|e| format!("Analysis failed: {}", e))?;

    let fixes = crate::ai::fix::generate_fixes(&report.issues, &path);

    let fix_list: Vec<Value> = fixes.iter().filter(|f| f.auto_fixable).map(|f| {
        serde_json::json!({
            "rule": f.rule,
            "file": f.file.to_string_lossy(),
            "line": f.line,
            "original": f.original,
            "replacement": f.replacement,
            "description": f.description
        })
    }).collect();

    let preview = args.get("preview").and_then(|v| v.as_bool()).unwrap_or(true);
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
