use crate::rules::RuleRegistry;
use crate::config::FalconConfig;
use colored::Colorize;

pub struct RuleDoc {
    pub name: String,
    pub description: String,
    pub severity: String,
    pub category: String,
}

/// Generate documentation for all registered rules.
pub fn generate_rule_docs() -> Vec<RuleDoc> {
    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    registry
        .rules()
        .iter()
        .map(|rule| {
            let category = categorize_rule(rule.name());
            RuleDoc {
                name: rule.name().to_string(),
                description: rule.description().to_string(),
                severity: format!("{:?}", rule.default_severity()),
                category,
            }
        })
        .collect()
}

fn categorize_rule(name: &str) -> String {
    if name.contains("bloc") {
        "BLoC".to_string()
    } else if name.contains("ref-") || name.contains("watch") || name.contains("notifier") || name.contains("async-value") {
        "Riverpod".to_string()
    } else if name.contains("equatable") || name.contains("equals") || name.contains("mutable-equat") {
        "Equatable".to_string()
    } else if name.contains("widget") || name.contains("setState") || name.contains("expanded") || name.contains("const-constructor") || name.contains("callback") || name.contains("returning-widget") {
        "Flutter".to_string()
    } else {
        "Dart".to_string()
    }
}

/// Print rule docs in console format.
pub fn print_rule_docs(docs: &[RuleDoc]) {
    println!();
    println!(
        "  {} Rule Reference ({} rules)",
        "falcon".bright_cyan().bold(),
        docs.len()
    );
    println!();

    let categories: Vec<&str> = vec!["Dart", "Flutter", "Riverpod", "BLoC", "Equatable"];

    for category in &categories {
        let cat_rules: Vec<&RuleDoc> = docs
            .iter()
            .filter(|d| d.category == *category)
            .collect();

        if cat_rules.is_empty() {
            continue;
        }

        println!("  {} ({} rules)", category.bright_yellow().bold(), cat_rules.len());
        for rule in &cat_rules {
            let sev = match rule.severity.as_str() {
                "Error" => "ERR".red(),
                "Warning" => "WRN".yellow(),
                "Info" => "INF".blue(),
                _ => "???".dimmed(),
            };
            println!(
                "    {} {:<45} {}",
                sev,
                rule.name.bright_white(),
                rule.description.dimmed()
            );
        }
        println!();
    }
}

/// Generate Markdown documentation.
pub fn generate_markdown_docs(docs: &[RuleDoc]) -> String {
    let mut md = String::new();
    md.push_str("# Falcon Rule Reference\n\n");
    md.push_str(&format!("**{} rules** across 5 categories.\n\n", docs.len()));

    md.push_str("## Table of Contents\n\n");
    let categories = ["Dart", "Flutter", "Riverpod", "BLoC", "Equatable"];
    for cat in &categories {
        let count = docs.iter().filter(|d| d.category == *cat).count();
        if count > 0 {
            md.push_str(&format!("- [{}](#{}--{} rules)\n", cat, cat.to_lowercase(), count));
        }
    }
    md.push('\n');

    for cat in &categories {
        let cat_rules: Vec<&RuleDoc> = docs.iter().filter(|d| d.category == *cat).collect();
        if cat_rules.is_empty() {
            continue;
        }

        md.push_str(&format!("## {}\n\n", cat));
        md.push_str("| Rule | Severity | Description |\n");
        md.push_str("|------|----------|-------------|\n");
        for rule in &cat_rules {
            md.push_str(&format!(
                "| `{}` | {} | {} |\n",
                rule.name, rule.severity, rule.description
            ));
        }
        md.push('\n');
    }

    md
}
