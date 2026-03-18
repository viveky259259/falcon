use crate::config::{RuleConfig, Severity};
use colored::Colorize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulePreset {
    pub name: String,
    pub description: String,
    pub rules: Vec<RuleConfig>,
}

/// Built-in rule presets.
pub fn get_preset(name: &str) -> Option<RulePreset> {
    match name {
        "recommended" => Some(preset_recommended()),
        "strict" => Some(preset_strict()),
        "flutter" => Some(preset_flutter()),
        "riverpod" => Some(preset_riverpod()),
        "bloc" => Some(preset_bloc()),
        "performance" => Some(preset_performance()),
        "ai-generated" => Some(preset_ai_generated()),
        _ => None,
    }
}

pub fn list_presets() -> Vec<RulePreset> {
    vec![
        preset_recommended(),
        preset_strict(),
        preset_flutter(),
        preset_riverpod(),
        preset_bloc(),
        preset_performance(),
        preset_ai_generated(),
    ]
}

fn preset_recommended() -> RulePreset {
    RulePreset {
        name: "recommended".to_string(),
        description: "Balanced set of rules for most projects — catches real issues without excessive noise.".to_string(),
        rules: vec![
            rule("avoid-dynamic", Severity::Warning),
            rule("prefer-trailing-comma", Severity::Info),
            rule("avoid-late-keyword", Severity::Warning),
            rule("avoid-nested-conditionals", Severity::Warning),
            rule("avoid-long-functions", Severity::Warning),
            rule("avoid-long-parameter-list", Severity::Warning),
            rule("avoid-double-negation", Severity::Warning),
            rule("prefer-match-file-name", Severity::Info),
            rule("avoid-returning-widgets", Severity::Warning),
            rule("avoid-unnecessary-set-state", Severity::Error),
            rule("prefer-const-constructors", Severity::Info),
            rule("avoid-unused-parameters", Severity::Warning),
            rule("avoid-throw-in-catch", Severity::Error),
            rule("always-override-equals-hashcode", Severity::Warning),
        ],
    }
}

fn preset_strict() -> RulePreset {
    RulePreset {
        name: "strict".to_string(),
        description: "Maximum safety — all rules enabled at highest severity. For teams that want zero tolerance.".to_string(),
        rules: vec![
            rule("avoid-dynamic", Severity::Error),
            rule("prefer-trailing-comma", Severity::Warning),
            rule("avoid-late-keyword", Severity::Error),
            rule("no-magic-numbers", Severity::Error),
            rule("avoid-nested-conditionals", Severity::Error),
            rule("avoid-long-functions", Severity::Error),
            rule("avoid-long-parameter-list", Severity::Error),
            rule("avoid-double-negation", Severity::Error),
            rule("prefer-match-file-name", Severity::Warning),
            rule("avoid-global-state", Severity::Error),
            rule("avoid-returning-widgets", Severity::Error),
            rule("prefer-extracting-callbacks", Severity::Warning),
            rule("avoid-unnecessary-set-state", Severity::Error),
            rule("avoid-expanded-as-spacer", Severity::Error),
            rule("prefer-const-constructors", Severity::Warning),
            rule("avoid-unused-parameters", Severity::Error),
            rule("prefer-correct-identifier-length", Severity::Warning),
            rule("avoid-cascade-after-if-null", Severity::Error),
            rule("avoid-collection-methods-unrelated-types", Severity::Error),
            rule("avoid-duplicate-exports", Severity::Error),
            rule("avoid-missing-enum-constant-in-map", Severity::Error),
            rule("avoid-non-ascii-symbols", Severity::Warning),
            rule("avoid-throw-in-catch", Severity::Error),
            rule("avoid-top-level-members-in-tests", Severity::Error),
            rule("avoid-unnecessary-type-assertions", Severity::Warning),
            rule("avoid-unnecessary-type-casts", Severity::Warning),
            rule("binary-expression-operand-order", Severity::Info),
            rule("double-literal-format", Severity::Info),
            rule("newline-before-return", Severity::Info),
            rule("prefer-first-last", Severity::Info),
            rule("always-override-equals-hashcode", Severity::Error),
            rule("avoid-mutable-equatable", Severity::Error),
            rule("prefer-equatable", Severity::Warning),
        ],
    }
}

fn preset_flutter() -> RulePreset {
    RulePreset {
        name: "flutter".to_string(),
        description: "Flutter-specific rules — widget best practices, performance, and UI patterns.".to_string(),
        rules: vec![
            rule("avoid-returning-widgets", Severity::Warning),
            rule("prefer-extracting-callbacks", Severity::Info),
            rule("avoid-unnecessary-set-state", Severity::Error),
            rule("avoid-expanded-as-spacer", Severity::Warning),
            rule("prefer-const-constructors", Severity::Warning),
            rule("avoid-long-functions", Severity::Warning),
            rule("avoid-nested-conditionals", Severity::Warning),
            rule("avoid-dynamic", Severity::Warning),
            rule("prefer-trailing-comma", Severity::Info),
        ],
    }
}

fn preset_riverpod() -> RulePreset {
    RulePreset {
        name: "riverpod".to_string(),
        description: "Riverpod state management rules — ref.read/watch patterns, provider hygiene.".to_string(),
        rules: vec![
            rule("avoid-ref-read-inside-build", Severity::Error),
            rule("avoid-watch-outside-build", Severity::Error),
            rule("prefer-async-value-when", Severity::Warning),
            rule("avoid-public-notifier-properties", Severity::Warning),
            rule("prefer-ref-read-for-methods", Severity::Info),
            rule("avoid-returning-widgets", Severity::Warning),
            rule("avoid-unnecessary-set-state", Severity::Error),
            rule("prefer-const-constructors", Severity::Info),
        ],
    }
}

fn preset_bloc() -> RulePreset {
    RulePreset {
        name: "bloc".to_string(),
        description: "BLoC pattern rules — emit safety, public API, and provider patterns.".to_string(),
        rules: vec![
            rule("avoid-bloc-public-methods", Severity::Warning),
            rule("avoid-emit-outside-bloc", Severity::Error),
            rule("prefer-multi-bloc-provider", Severity::Info),
            rule("avoid-passing-bloc-to-widget", Severity::Warning),
            rule("prefer-bloc-extensions", Severity::Info),
            rule("avoid-returning-widgets", Severity::Warning),
            rule("avoid-unnecessary-set-state", Severity::Error),
            rule("prefer-const-constructors", Severity::Info),
        ],
    }
}

fn preset_performance() -> RulePreset {
    RulePreset {
        name: "performance".to_string(),
        description: "Performance-focused rules — rebuild prevention, const usage, efficient patterns.".to_string(),
        rules: vec![
            rule("prefer-const-constructors", Severity::Error),
            rule("avoid-unnecessary-set-state", Severity::Error),
            rule("avoid-returning-widgets", Severity::Error),
            rule("avoid-expanded-as-spacer", Severity::Warning),
            rule("avoid-long-functions", Severity::Warning),
            rule("prefer-trailing-comma", Severity::Info),
        ],
    }
}

fn preset_ai_generated() -> RulePreset {
    RulePreset {
        name: "ai-generated".to_string(),
        description: "Rules targeting the most common issues in AI-generated Flutter code (Cursor, Copilot, Claude, Gemini).".to_string(),
        rules: vec![
            rule("avoid-empty-catch", Severity::Error),
            rule("avoid-print-in-production", Severity::Error),
            rule("avoid-hardcoded-credentials", Severity::Error),
            rule("ensure-dispose-lifecycle", Severity::Error),
            rule("avoid-unawaited-futures", Severity::Error),
            rule("ensure-stream-subscription-cancel", Severity::Error),
            rule("prefer-specific-catch-type", Severity::Warning),
            rule("avoid-excessive-widget-nesting", Severity::Warning),
            rule("prefer-named-boolean-parameters", Severity::Warning),
            rule("avoid-dynamic", Severity::Error),
            rule("avoid-global-state", Severity::Error),
            rule("avoid-returning-widgets", Severity::Warning),
            rule("avoid-long-functions", Severity::Warning),
            rule("avoid-nested-conditionals", Severity::Warning),
            rule("avoid-unnecessary-set-state", Severity::Error),
            rule("avoid-unused-parameters", Severity::Warning),
            rule("avoid-throw-in-catch", Severity::Error),
            rule("prefer-const-constructors", Severity::Warning),
            rule("prefer-trailing-comma", Severity::Info),
            rule("no-magic-numbers", Severity::Warning),
        ],
    }
}

fn rule(name: &str, severity: Severity) -> RuleConfig {
    RuleConfig::simple(name.to_string(), severity)
}

pub fn print_presets(presets: &[RulePreset]) {
    println!();
    println!(
        "  {} Available Presets",
        "falcon".bright_cyan().bold()
    );
    println!();

    for preset in presets {
        println!(
            "  {} {} ({} rules)",
            "•".bright_white(),
            preset.name.bright_white().bold(),
            preset.rules.len()
        );
        println!("    {}", preset.description.dimmed());
    }

    println!();
    println!("  Usage:");
    println!("    falcon preset apply recommended");
    println!("    falcon preset show strict");
    println!();
}

pub fn print_preset_detail(preset: &RulePreset) {
    println!();
    println!(
        "  {} Preset: {}",
        "falcon".bright_cyan().bold(),
        preset.name.bright_white().bold()
    );
    println!("  {}", preset.description);
    println!();
    println!("  Rules:");
    for rule in &preset.rules {
        let sev = match rule.severity() {
            Severity::Error => "ERROR".red(),
            Severity::Warning => "WARN".yellow(),
            Severity::Info => "INFO".blue(),
        };
        println!("    {} {}", sev, rule.name());
    }
    println!();
}

/// Apply a preset to an existing falcon.yaml.
pub fn apply_preset(
    preset: &RulePreset,
    config_path: &std::path::Path,
) -> anyhow::Result<()> {
    let mut config = if config_path.join("falcon.yaml").exists() {
        crate::config::FalconConfig::load(config_path)?
    } else {
        crate::config::FalconConfig::default()
    };

    config.rules = preset.rules.clone();

    let yaml = serde_yaml::to_string(&config)?;
    std::fs::write(config_path.join("falcon.yaml"), yaml)?;

    println!(
        "  {} Applied preset '{}' ({} rules) to falcon.yaml",
        "✓".green().bold(),
        preset.name.bright_cyan(),
        preset.rules.len()
    );

    Ok(())
}
