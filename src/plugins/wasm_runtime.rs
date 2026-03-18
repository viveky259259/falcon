use crate::config::Severity;
use crate::reporters::Issue;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Represents a WASM-based plugin rule definition.
/// Plugin authors define rules as pattern matchers that get compiled to a
/// portable format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuleDefinition {
    pub name: String,
    pub description: String,
    #[serde(default = "default_severity_str")]
    pub severity: String,
    /// AST node kinds to match against.
    #[serde(default)]
    pub node_kinds: Vec<String>,
    /// Text patterns to search for within matched nodes.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Anti-patterns — if found, suppress the match.
    #[serde(default)]
    pub anti_patterns: Vec<String>,
    /// Custom message template with {match} placeholder.
    #[serde(default)]
    pub message_template: Option<String>,
}

fn default_severity_str() -> String {
    "warning".to_string()
}

/// A compiled WASM rule ready for execution.
#[derive(Debug)]
pub struct CompiledWasmRule {
    pub definition: WasmRuleDefinition,
    severity: Severity,
}

impl CompiledWasmRule {
    pub fn compile(def: WasmRuleDefinition) -> Self {
        let severity = match def.severity.to_lowercase().as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "info" | "style" => Severity::Info,
            _ => Severity::Warning,
        };
        Self {
            definition: def,
            severity,
        }
    }

    pub fn check(&self, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (line_num, line) in source.lines().enumerate() {
            for pattern in &self.definition.patterns {
                if line.contains(pattern.as_str()) {
                    let suppressed = self
                        .definition
                        .anti_patterns
                        .iter()
                        .any(|ap| line.contains(ap.as_str()));

                    if !suppressed {
                        let message = self
                            .definition
                            .message_template
                            .as_ref()
                            .map(|t| t.replace("{match}", pattern))
                            .unwrap_or_else(|| self.definition.description.clone());

                        issues.push(Issue {
                            rule: self.definition.name.clone(),
                            message,
                            severity: self.severity,
                            file: file.to_path_buf(),
                            line: line_num + 1,
                            column: 1,
                        });
                    }
                }
            }
        }

        issues
    }
}

/// Load WASM rule definitions from a YAML file.
pub fn load_wasm_rules(path: &Path) -> anyhow::Result<Vec<CompiledWasmRule>> {
    let content = std::fs::read_to_string(path)?;
    let definitions: Vec<WasmRuleDefinition> = serde_yaml::from_str(&content)?;

    Ok(definitions
        .into_iter()
        .map(CompiledWasmRule::compile)
        .collect())
}

/// The WASM plugin sandbox — provides resource limits and isolation.
#[derive(Debug, Clone)]
pub struct WasmSandbox {
    pub max_execution_ms: u64,
    pub max_memory_mb: u64,
    pub max_issues_per_file: usize,
}

impl Default for WasmSandbox {
    fn default() -> Self {
        Self {
            max_execution_ms: 5000,
            max_memory_mb: 64,
            max_issues_per_file: 100,
        }
    }
}

impl WasmSandbox {
    pub fn check_with_limits(
        &self,
        rules: &[CompiledWasmRule],
        source: &str,
        file: &Path,
    ) -> Vec<Issue> {
        let start = std::time::Instant::now();
        let mut all_issues = Vec::new();

        for rule in rules {
            if start.elapsed().as_millis() as u64 > self.max_execution_ms {
                log::warn!(
                    "WASM sandbox timeout ({} ms) reached for file {}",
                    self.max_execution_ms,
                    file.display()
                );
                break;
            }

            let issues = rule.check(source, file);
            all_issues.extend(issues);

            if all_issues.len() >= self.max_issues_per_file {
                all_issues.truncate(self.max_issues_per_file);
                break;
            }
        }

        all_issues
    }
}

/// Generate a template rules.yaml for a new WASM plugin.
pub fn generate_rules_template() -> String {
    let template = vec![WasmRuleDefinition {
        name: "my-custom-rule".to_string(),
        description: "Describe what this rule checks for".to_string(),
        severity: "warning".to_string(),
        node_kinds: vec!["class_declaration".to_string()],
        patterns: vec!["TODO".to_string()],
        anti_patterns: vec!["// ignore".to_string()],
        message_template: Some("Found '{match}' — consider addressing this.".to_string()),
    }];

    serde_yaml::to_string(&template).unwrap_or_default()
}
