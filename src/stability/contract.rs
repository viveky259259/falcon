use colored::Colorize;
use serde::{Deserialize, Serialize};

/// Falcon's stability contract — what teams can rely on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityContract {
    pub version: String,
    pub guarantees: Vec<Guarantee>,
    pub deprecation_policy: DeprecationPolicy,
    pub migration_policy: MigrationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guarantee {
    pub area: String,
    pub promise: String,
    pub since_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationPolicy {
    pub notice_period_months: u32,
    pub deprecation_stages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPolicy {
    pub auto_migration: bool,
    pub migration_guides: bool,
    pub backwards_compatible_configs: bool,
}

impl Default for StabilityContract {
    fn default() -> Self {
        Self {
            version: "1.2".to_string(),
            guarantees: vec![
                Guarantee {
                    area: "Configuration".to_string(),
                    promise: "No breaking falcon.yaml changes without migration path".to_string(),
                    since_version: "1.0".to_string(),
                },
                Guarantee {
                    area: "Rule naming".to_string(),
                    promise: "Rule names never change — only aliases added".to_string(),
                    since_version: "1.0".to_string(),
                },
                Guarantee {
                    area: "Exit codes".to_string(),
                    promise: "Exit code semantics stable for CI integration".to_string(),
                    since_version: "0.4".to_string(),
                },
                Guarantee {
                    area: "Output formats".to_string(),
                    promise: "SARIF, CodeClimate, Checkstyle schema stable".to_string(),
                    since_version: "0.4".to_string(),
                },
                Guarantee {
                    area: "Performance".to_string(),
                    promise: "No more than 20% regression between minor versions".to_string(),
                    since_version: "1.0".to_string(),
                },
                Guarantee {
                    area: "Rule behavior".to_string(),
                    promise: "Rules never become stricter without opt-in".to_string(),
                    since_version: "1.2".to_string(),
                },
            ],
            deprecation_policy: DeprecationPolicy {
                notice_period_months: 6,
                deprecation_stages: vec![
                    "Announced — rule marked @deprecated in docs, no behavior change".to_string(),
                    "Warning — rule emits deprecation notice when triggered".to_string(),
                    "Disabled — rule no longer runs by default, still available with --include-deprecated".to_string(),
                    "Removed — rule code deleted in next major version".to_string(),
                ],
            },
            migration_policy: MigrationPolicy {
                auto_migration: true,
                migration_guides: true,
                backwards_compatible_configs: true,
            },
        }
    }
}

pub fn print_stability_contract(contract: &StabilityContract) {
    println!();
    println!(
        "  {} Stability Contract (v{})",
        "falcon".bright_cyan().bold(),
        contract.version
    );
    println!();

    println!("  {} Guarantees:", "📋".bright_white());
    for g in &contract.guarantees {
        println!(
            "    {} {} (since v{})",
            "✓".green().bold(),
            g.promise.bright_white(),
            g.since_version.dimmed()
        );
        println!("      Area: {}", g.area.dimmed());
    }

    println!();
    println!(
        "  {} Deprecation Policy ({}-month notice):",
        "⏳".bright_white(),
        contract.deprecation_policy.notice_period_months
    );
    for (i, stage) in contract
        .deprecation_policy
        .deprecation_stages
        .iter()
        .enumerate()
    {
        println!("    {}. {}", i + 1, stage);
    }

    println!();
    println!("  {} Migration Policy:", "🔄".bright_white());
    println!(
        "    Auto-migration:          {}",
        if contract.migration_policy.auto_migration {
            "Yes".green()
        } else {
            "No".red()
        }
    );
    println!(
        "    Migration guides:        {}",
        if contract.migration_policy.migration_guides {
            "Yes".green()
        } else {
            "No".red()
        }
    );
    println!(
        "    Backwards-compatible:    {}",
        if contract.migration_policy.backwards_compatible_configs {
            "Yes".green()
        } else {
            "No".red()
        }
    );
    println!();
}
