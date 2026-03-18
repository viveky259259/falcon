mod defaults;

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
}

impl Default for FalconConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            rules: default_rules(),
            unused: UnusedConfig::default(),
            exclude: default_excludes(),
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

fn default_excludes() -> Vec<String> {
    vec![
        "build/**".to_string(),
        ".dart_tool/**".to_string(),
        "**/*.g.dart".to_string(),
        "**/*.freezed.dart".to_string(),
    ]
}
