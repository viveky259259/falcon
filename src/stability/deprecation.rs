use colored::Colorize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecatedRule {
    pub name: String,
    pub deprecated_since: String,
    pub removal_version: String,
    pub reason: String,
    pub replacement: Option<String>,
    pub stage: DeprecationStage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeprecationStage {
    Announced,
    Warning,
    Disabled,
    Removed,
}

impl std::fmt::Display for DeprecationStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeprecationStage::Announced => write!(f, "Announced"),
            DeprecationStage::Warning => write!(f, "Warning"),
            DeprecationStage::Disabled => write!(f, "Disabled"),
            DeprecationStage::Removed => write!(f, "Removed"),
        }
    }
}

/// Registry of deprecated rules. Currently empty — no rules deprecated yet.
pub fn deprecated_rules() -> Vec<DeprecatedRule> {
    vec![]
}

/// Check if a rule is deprecated.
pub fn is_deprecated(rule_name: &str) -> Option<DeprecatedRule> {
    deprecated_rules().into_iter().find(|r| r.name == rule_name)
}

/// Check config for deprecated rules and warn.
pub fn check_deprecated_in_config(config: &crate::config::FalconConfig) -> Vec<DeprecatedRule> {
    let mut found = Vec::new();
    for rule in &config.rules {
        let name = rule.name();
        if let Some(dep) = is_deprecated(&name) {
            found.push(dep);
        }
    }
    found
}

pub fn print_deprecation_warnings(deprecated: &[DeprecatedRule]) {
    if deprecated.is_empty() {
        return;
    }

    println!();
    println!("  {} Deprecated Rules Warning", "⚠".yellow());
    for dep in deprecated {
        println!(
            "    {} '{}' — {} (stage: {}, removal: v{})",
            "⚠".yellow(),
            dep.name.bright_white(),
            dep.reason,
            dep.stage,
            dep.removal_version
        );
        if let Some(ref replacement) = dep.replacement {
            println!("      Replace with: {}", replacement.bright_green());
        }
    }
    println!();
}

pub fn print_deprecation_status() {
    let rules = deprecated_rules();

    println!();
    println!(
        "  {} Rule Deprecation Status",
        "falcon".bright_cyan().bold()
    );
    println!();

    if rules.is_empty() {
        println!(
            "  {} No rules are currently deprecated.",
            "✓".green().bold()
        );
    } else {
        for rule in &rules {
            let stage_color = match rule.stage {
                DeprecationStage::Announced => rule.stage.to_string().blue(),
                DeprecationStage::Warning => rule.stage.to_string().yellow(),
                DeprecationStage::Disabled => rule.stage.to_string().red(),
                DeprecationStage::Removed => rule.stage.to_string().dimmed(),
            };
            println!(
                "    {} {:<40} {} → removed in v{}",
                stage_color,
                rule.name.bright_white(),
                rule.reason.dimmed(),
                rule.removal_version
            );
        }
    }
    println!();
}
