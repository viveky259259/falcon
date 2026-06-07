//! Falcon SDK — embeddable static analysis engine for Flutter/Dart.
//!
//! Use this module to integrate Falcon analysis into Rust-based tools,
//! AI pipelines, or custom automation without spawning a CLI process.
//!
//! # Quick Start
//! ```no_run
//! use falcon::sdk::{FalconSdk, AnalysisOptions};
//! let sdk = FalconSdk::new();
//! let result = sdk.analyze_project("/path/to/flutter/app", None).unwrap();
//! println!("Issues: {}", result.issue_count);
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The main entry point for embedding Falcon analysis.
pub struct FalconSdk;

/// Options to customize analysis behavior.
#[derive(Debug, Clone, Default)]
pub struct AnalysisOptions {
    pub preset: Option<String>,
    pub fail_on_error: bool,
    pub include_metrics: bool,
    pub include_score: bool,
}

/// Complete analysis result with issues, score, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub project_path: String,
    pub file_count: usize,
    pub issue_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub issues: Vec<SdkIssue>,
    pub score: Option<SdkScore>,
    pub passed: bool,
}

/// A single analysis issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkIssue {
    pub rule: String,
    pub message: String,
    pub severity: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
}

/// AI Code Quality Score breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkScore {
    pub overall: u32,
    pub grade: String,
    pub resource_safety: u32,
    pub error_handling: u32,
    pub type_safety: u32,
    pub security: u32,
    pub convention_match: u32,
    pub complexity: u32,
}

impl Default for FalconSdk {
    fn default() -> Self {
        Self::new()
    }
}

impl FalconSdk {
    /// Create a new Falcon SDK instance.
    pub fn new() -> Self {
        Self
    }

    /// Analyze a Flutter/Dart project directory.
    pub fn analyze_project(
        &self,
        path: &str,
        options: Option<AnalysisOptions>,
    ) -> anyhow::Result<AnalysisResult> {
        let path = PathBuf::from(path);
        let opts = options.unwrap_or_default();

        let mut config = crate::config::FalconConfig::load(&path)?;

        if let Some(ref preset_name) = opts.preset {
            if let Some(p) = crate::plugins::presets::get_preset(preset_name) {
                config.rules = p.rules;
            }
        }

        let falcon = crate::Falcon::new(config)?;
        let report = falcon.analyze(&path)?;

        let score = if opts.include_score {
            match crate::ai_score::score::score_from_report(&report) {
                Ok(s) => Some(SdkScore {
                    overall: s.overall,
                    grade: s.grade.to_string(),
                    resource_safety: s.resource_safety.score,
                    error_handling: s.error_handling.score,
                    type_safety: s.type_safety.score,
                    security: s.security.score,
                    convention_match: s.convention_match.score,
                    complexity: s.complexity.score,
                }),
                Err(_) => None,
            }
        } else {
            None
        };

        let issues: Vec<SdkIssue> = report
            .issues
            .iter()
            .map(|i| SdkIssue {
                rule: i.rule.clone(),
                message: i.message.clone(),
                severity: format!("{:?}", i.severity),
                file: i.file.to_string_lossy().to_string(),
                line: i.line,
                column: i.column,
            })
            .collect();

        let errors = report.error_count();
        let warnings = report.warning_count();
        let infos = report.info_count();
        let passed = if opts.fail_on_error {
            errors == 0
        } else {
            true
        };

        Ok(AnalysisResult {
            project_path: path.to_string_lossy().to_string(),
            file_count: report.file_count,
            issue_count: issues.len(),
            error_count: errors,
            warning_count: warnings,
            info_count: infos,
            issues,
            score,
            passed,
        })
    }

    /// Analyze a single Dart file (source string).
    pub fn analyze_source(&self, source: &str, file_name: &str) -> anyhow::Result<Vec<SdkIssue>> {
        let file_path = PathBuf::from(file_name);

        let mut parser = crate::parser::DartParser::new()?;
        let tree = parser
            .parse(source)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Dart source"))?;

        let config = crate::config::FalconConfig::default();
        let mut registry = crate::rules::RuleRegistry::new();
        registry.register_defaults(&config);

        let mut all_issues = registry.check(tree.root_node(), source, &file_path);

        let metrics =
            crate::metrics::calculate_file_metrics(tree.root_node(), source, &config.metrics);
        all_issues.extend(metrics.violations());

        let issues: Vec<SdkIssue> = all_issues
            .iter()
            .map(|i| SdkIssue {
                rule: i.rule.clone(),
                message: i.message.clone(),
                severity: format!("{:?}", i.severity),
                file: file_name.to_string(),
                line: i.line,
                column: i.column,
            })
            .collect();

        Ok(issues)
    }

    /// Get the AI Code Quality Score for a project.
    pub fn score_project(&self, path: &str) -> anyhow::Result<SdkScore> {
        let path = PathBuf::from(path);
        let score = crate::ai_score::score::calculate_ai_score(&path)?;

        Ok(SdkScore {
            overall: score.overall,
            grade: score.grade.to_string(),
            resource_safety: score.resource_safety.score,
            error_handling: score.error_handling.score,
            type_safety: score.type_safety.score,
            security: score.security.score,
            convention_match: score.convention_match.score,
            complexity: score.complexity.score,
        })
    }

    /// Detect team conventions from a project.
    pub fn detect_conventions(&self, path: &str) -> anyhow::Result<serde_json::Value> {
        let path = PathBuf::from(path);
        let report = crate::ai_score::convention::detect_conventions(&path)?;
        serde_json::to_value(&report).map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Get analysis result as JSON string.
    pub fn analyze_to_json(
        &self,
        path: &str,
        options: Option<AnalysisOptions>,
    ) -> anyhow::Result<String> {
        let result = self.analyze_project(path, options)?;
        Ok(serde_json::to_string_pretty(&result)?)
    }
}
