use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JSON_SCHEMA_DRAFT_07: &str = "http://json-schema.org/draft-07/schema#";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintFileArgs {
    pub file_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintDiffArgs {
    pub path: String,
    #[serde(default = "default_base_ref")]
    pub base_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewArgs {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainArgs {
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixSafeArgs {
    pub path: String,
    #[serde(default = "default_preview")]
    pub preview: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorArgs {
    pub path: String,
    #[serde(default)]
    pub execute: bool,
    #[serde(default)]
    pub decisions: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only: Vec<String>,
}

pub fn doctor_input_schema() -> Value {
    serde_json::json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to the Flutter project to diagnose"
            },
            "execute": {
                "type": "boolean",
                "default": false,
                "description": "Apply the fixes. Requires `decisions` to answer every question returned by the diagnosis call. Never available over the HTTP bridge."
            },
            "decisions": {
                "type": "object",
                "additionalProperties": { "type": "string" },
                "description": "Answers keyed by question id, e.g. {\"flutter.channel\":\"stable\",\"flutter.version\":\"3.24.5\"}"
            },
            "only": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Restrict to these checks: flutter, dart, cocoapods, android, xcode"
            }
        },
        "required": ["path"]
    })
}

pub fn default_base_ref() -> String {
    "origin/main".to_string()
}

pub fn default_preview() -> bool {
    true
}

pub fn lint_file_input_schema() -> Value {
    serde_json::json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Path to the Dart file to check"
            },
            "source": {
                "type": "string",
                "description": "Optional: Dart source code to analyze (if provided, file_path is used only for context)"
            },
            "project_root": {
                "type": "string",
                "description": "Optional: project root for resolver-backed cross-file rules"
            }
        },
        "required": ["file_path"]
    })
}

pub fn lint_diff_input_schema() -> Value {
    serde_json::json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to the Flutter/Dart project directory (repository root)"
            },
            "base_ref": {
                "type": "string",
                "description": "Git ref to diff against (default: origin/main)",
                "default": "origin/main"
            }
        },
        "required": ["path"]
    })
}

pub fn review_input_schema() -> Value {
    serde_json::json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
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
    })
}

pub fn explain_input_schema() -> Value {
    serde_json::json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
        "type": "object",
        "properties": {
            "rule": {
                "type": "string",
                "description": "Rule name to explain (e.g. 'avoid-empty-catch', 'ensure-dispose-lifecycle')"
            }
        },
        "required": ["rule"]
    })
}

pub fn fix_safe_input_schema() -> Value {
    serde_json::json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lint_diff_args_default_base_ref() {
        let args: LintDiffArgs = serde_json::from_value(json!({"path": "."})).unwrap();
        assert_eq!(args.base_ref, "origin/main");
    }

    #[test]
    fn fix_safe_args_default_preview() {
        let args: FixSafeArgs = serde_json::from_value(json!({"path": "."})).unwrap();
        assert!(args.preview);
    }

    #[test]
    fn schemas_declare_required_fields() {
        let cases = [
            (lint_file_input_schema(), "file_path"),
            (lint_diff_input_schema(), "path"),
            (review_input_schema(), "path"),
            (explain_input_schema(), "rule"),
            (fix_safe_input_schema(), "path"),
        ];

        for (schema, required) in cases {
            assert_eq!(
                schema.get("$schema").and_then(|v| v.as_str()),
                Some(JSON_SCHEMA_DRAFT_07)
            );
            assert!(
                schema
                    .get("required")
                    .and_then(|v| v.as_array())
                    .is_some_and(|items| items.contains(&json!(required))),
                "schema missing required field '{}': {}",
                required,
                schema
            );
        }
    }
}
