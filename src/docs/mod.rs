use crate::config::FalconConfig;
use crate::rules::RuleRegistry;
use colored::Colorize;
use std::path::Path;

/// Generate markdown documentation for all registered rules.
pub fn generate_rule_docs(output_dir: &Path) -> anyhow::Result<()> {
    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    std::fs::create_dir_all(output_dir)?;

    let rules = registry.rules();
    let mut index = String::new();
    index.push_str("# Falcon Lint Rules\n\n");
    index.push_str(&format!("**{} rules** available across {} categories.\n\n", rules.len(), 5));
    index.push_str("| Rule | Category | Severity | Description |\n");
    index.push_str("|------|----------|----------|-------------|\n");

    let mut by_category: std::collections::BTreeMap<String, Vec<(String, String, String)>> =
        std::collections::BTreeMap::new();

    for rule in &rules {
        let name = rule.name().to_string();
        let category = categorize(&name);
        let severity = rule.default_severity().to_string();
        let description = rule.description().to_string();

        index.push_str(&format!(
            "| [`{}`](rules/{}.md) | {} | {} | {} |\n",
            name,
            name,
            category,
            severity,
            description
        ));

        by_category
            .entry(category.clone())
            .or_default()
            .push((name.clone(), severity.clone(), description.clone()));

        let rule_doc = format!(
            "# {}\n\n\
             **Category:** {}\n\
             **Default severity:** {}\n\n\
             ## Description\n\n\
             {}\n\n\
             ## Configuration\n\n\
             ```yaml\n\
             rules:\n\
               - {}\n\
             ```\n\n\
             To change severity:\n\n\
             ```yaml\n\
             rules:\n\
               - {}:\n\
                   severity: error\n\
             ```\n\n\
             ## Suppression\n\n\
             ```dart\n\
             // ignore: {}\n\
             offendingCode();\n\
             ```\n\n\
             Or for the entire file:\n\n\
             ```dart\n\
             // ignore_for_file: {}\n\
             ```\n",
            name, category, severity, description,
            name, name, name, name,
        );

        let rules_dir = output_dir.join("rules");
        std::fs::create_dir_all(&rules_dir)?;
        std::fs::write(rules_dir.join(format!("{}.md", name)), rule_doc)?;
    }

    index.push_str("\n## By Category\n\n");
    for (category, cat_rules) in &by_category {
        index.push_str(&format!("### {} ({} rules)\n\n", category, cat_rules.len()));
        for (name, _sev, desc) in cat_rules {
            index.push_str(&format!("- [`{}`](rules/{}.md) — {}\n", name, name, desc));
        }
        index.push('\n');
    }

    std::fs::write(output_dir.join("RULES.md"), index)?;

    println!(
        "{} Generated documentation for {} rules at {}",
        "✓".green().bold(),
        rules.len(),
        output_dir.display()
    );

    Ok(())
}

fn categorize(name: &str) -> String {
    if name.contains("ref-") || name.contains("watch") || name.contains("notifier") || name.contains("async-value") {
        "Provider/Riverpod".to_string()
    } else if name.contains("bloc") || name.contains("emit") || name.contains("multi-bloc") {
        "BLoC".to_string()
    } else if name.contains("equatable") || (name.contains("equals") && name.contains("hashcode")) {
        "Equatable".to_string()
    } else if name.contains("widget") || name.contains("setstate") || name.contains("expanded") || name.contains("const-constructor") || name.contains("extracting-callback") || name.contains("returning-widget") {
        "Flutter".to_string()
    } else {
        "Dart".to_string()
    }
}
