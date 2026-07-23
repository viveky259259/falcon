mod defaults;
pub mod validator;

pub use defaults::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FalconConfig {
    #[serde(default = "MetricsConfig::default")]
    pub metrics: MetricsConfig,

    #[serde(default)]
    pub rules: Vec<RuleConfig>,

    #[serde(default = "UnusedConfig::default")]
    pub unused: UnusedConfig,

    #[serde(default = "default_excludes")]
    pub exclude: Vec<String>,

    #[serde(default)]
    pub ai: crate::ai::config::AiConfig,

    #[serde(default)]
    pub preflight: PreflightConfig,

    #[serde(default)]
    pub analyze: AnalyzeConfig,
}

impl Default for FalconConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            rules: default_rules(),
            unused: UnusedConfig::default(),
            exclude: default_excludes(),
            ai: crate::ai::config::AiConfig::default(),
            preflight: PreflightConfig::default(),
            analyze: AnalyzeConfig::default(),
        }
    }
}

impl FalconConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let config_path = path.join("falcon.yaml");
        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: FalconConfig = serde_yaml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(FalconConfig::default())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "defaults::cyclomatic_complexity")]
    pub cyclomatic_complexity: u32,

    #[serde(default = "defaults::lines_of_code")]
    pub lines_of_code: u32,

    #[serde(default = "defaults::number_of_parameters")]
    pub number_of_parameters: u32,

    #[serde(default = "defaults::maximum_nesting_level")]
    pub maximum_nesting_level: u32,

    #[serde(default = "defaults::number_of_methods")]
    pub number_of_methods: u32,

    #[serde(default = "defaults::maintainability_index")]
    pub maintainability_index: u32,

    #[serde(default = "defaults::source_lines_of_code")]
    pub source_lines_of_code: u32,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            cyclomatic_complexity: defaults::cyclomatic_complexity(),
            lines_of_code: defaults::lines_of_code(),
            number_of_parameters: defaults::number_of_parameters(),
            maximum_nesting_level: defaults::maximum_nesting_level(),
            number_of_methods: defaults::number_of_methods(),
            maintainability_index: defaults::maintainability_index(),
            source_lines_of_code: defaults::source_lines_of_code(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleConfig {
    Simple(String),
    WithSeverity {
        #[serde(flatten)]
        entries: HashMap<String, RuleSettings>,
    },
}

impl RuleConfig {
    pub fn name(&self) -> &str {
        match self {
            RuleConfig::Simple(name) => name,
            RuleConfig::WithSeverity { entries } => {
                entries.keys().next().map(|s| s.as_str()).unwrap_or("")
            }
        }
    }

    pub fn severity(&self) -> Severity {
        match self {
            RuleConfig::Simple(_) => Severity::Warning,
            RuleConfig::WithSeverity { entries } => entries
                .values()
                .next()
                .map(|s| s.severity)
                .unwrap_or(Severity::Warning),
        }
    }

    pub fn options(&self) -> HashMap<String, serde_yaml::Value> {
        match self {
            RuleConfig::Simple(_) => HashMap::new(),
            RuleConfig::WithSeverity { entries } => entries
                .values()
                .next()
                .map(|s| s.options.clone())
                .unwrap_or_default(),
        }
    }

    pub fn simple(name: String, severity: Severity) -> Self {
        if severity == Severity::Warning {
            RuleConfig::Simple(name)
        } else {
            let mut entries = HashMap::new();
            entries.insert(
                name,
                RuleSettings {
                    severity,
                    options: HashMap::new(),
                },
            );
            RuleConfig::WithSeverity { entries }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSettings {
    #[serde(default = "default_severity")]
    pub severity: Severity,

    #[serde(flatten)]
    pub options: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

fn default_severity() -> Severity {
    Severity::Warning
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,

    #[serde(default = "default_unused_excludes")]
    pub exclude: Vec<String>,
}

impl Default for UnusedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            exclude: default_unused_excludes(),
        }
    }
}

/// Configuration for pre-flight checks (check-assets, check-a11y, check-pods,
/// check-platform-deps).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PreflightConfig {
    /// Issues to skip entirely.
    #[serde(default)]
    pub suppress: Vec<PreflightSuppression>,

    /// Tuning knobs per check, free-form so each command can read its own keys.
    #[serde(default)]
    pub config: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightSuppression {
    pub rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub reason: String,
}

/// Configuration controlling how `falcon analyze` invokes the four pre-flight checks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyzeConfig {
    #[serde(default)]
    pub preflight: AnalyzePreflightConfig,
}

/// Tuning for the analyze-rollup of pre-flight checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzePreflightConfig {
    /// Whether to run the four pre-flight checks during `falcon analyze`.
    /// Default: false (soft-rollout default in v0.5; will flip to true in v0.6).
    #[serde(default)]
    pub enabled: bool,
    /// List of check names to skip. Valid values: "check-assets", "check-a11y",
    /// "check-pods", "check-platform-deps".
    #[serde(default)]
    pub skip: Vec<String>,
}

impl Default for AnalyzePreflightConfig {
    fn default() -> Self {
        Self {
            enabled: false, // soft-rollout default; will flip to true in next major
            skip: Vec::new(),
        }
    }
}

fn bool_true() -> bool {
    true
}

fn default_unused_excludes() -> Vec<String> {
    vec![
        "**/*.g.dart".to_string(),
        "**/*.freezed.dart".to_string(),
        "**/*.gen.dart".to_string(),
    ]
}

pub fn default_excludes() -> Vec<String> {
    vec![
        "build/**".to_string(),
        ".dart_tool/**".to_string(),
        "**/*.g.dart".to_string(),
        "**/*.freezed.dart".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_config_default_is_empty() {
        let cfg = FalconConfig::default();
        assert!(cfg.preflight.suppress.is_empty());
        assert!(cfg.preflight.config.is_empty());
    }

    #[test]
    fn preflight_config_parses_from_yaml() {
        let yaml = r#"
preflight:
  suppress:
    - rule_id: assets/missing-file
      reason: "Bootstrap step documented in README."
  config:
    check-assets:
      warn_on_empty_directory: false
"#;
        let cfg: FalconConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.preflight.suppress.len(), 1);
        assert_eq!(cfg.preflight.suppress[0].rule_id, "assets/missing-file");
        assert!(cfg.preflight.config.contains_key("check-assets"));
    }

    #[test]
    fn analyze_preflight_default_is_disabled_no_skips() {
        let cfg = FalconConfig::default();
        assert!(!cfg.analyze.preflight.enabled);
        assert!(cfg.analyze.preflight.skip.is_empty());
    }

    #[test]
    fn analyze_preflight_parses_disabled() {
        let yaml = "analyze:\n  preflight:\n    enabled: false\n";
        let cfg: FalconConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!cfg.analyze.preflight.enabled);
    }

    #[test]
    fn analyze_preflight_parses_skip_list() {
        let yaml = "analyze:\n  preflight:\n    skip: [check-pods, check-platform-deps]\n";
        let cfg: FalconConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!cfg.analyze.preflight.enabled); // still default-false
        assert_eq!(
            cfg.analyze.preflight.skip,
            vec!["check-pods", "check-platform-deps"]
        );
    }
}
