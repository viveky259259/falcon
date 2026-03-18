use colored::Colorize;
use serde::{Deserialize, Serialize};

/// A registry entry for a published plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub rating: f32,
    pub tags: Vec<String>,
    pub url: String,
}

/// Search results from the plugin registry.
pub fn search_registry(query: &str) -> Vec<RegistryEntry> {
    let all = built_in_registry();

    if query.is_empty() || query == "*" {
        return all;
    }

    let query_lower = query.to_lowercase();
    all.into_iter()
        .filter(|e| {
            e.name.to_lowercase().contains(&query_lower)
                || e.description.to_lowercase().contains(&query_lower)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
        })
        .collect()
}

fn built_in_registry() -> Vec<RegistryEntry> {
    vec![
        RegistryEntry {
            name: "falcon-flutter-hooks".to_string(),
            version: "1.0.0".to_string(),
            description: "Rules for flutter_hooks package — lifecycle and state management best practices.".to_string(),
            author: "Falcon Community".to_string(),
            downloads: 1250,
            rating: 4.7,
            tags: vec!["flutter".to_string(), "hooks".to_string(), "state".to_string()],
            url: "https://github.com/falcon-plugins/flutter-hooks".to_string(),
        },
        RegistryEntry {
            name: "falcon-clean-arch".to_string(),
            version: "1.2.0".to_string(),
            description: "Enforce clean architecture boundaries with custom layer rules.".to_string(),
            author: "Falcon Community".to_string(),
            downloads: 3400,
            rating: 4.9,
            tags: vec!["architecture".to_string(), "clean-arch".to_string(), "layers".to_string()],
            url: "https://github.com/falcon-plugins/clean-arch".to_string(),
        },
        RegistryEntry {
            name: "falcon-freezed".to_string(),
            version: "0.5.0".to_string(),
            description: "Rules for freezed package — data class conventions and union type patterns.".to_string(),
            author: "Falcon Community".to_string(),
            downloads: 890,
            rating: 4.5,
            tags: vec!["freezed".to_string(), "code-gen".to_string(), "data-class".to_string()],
            url: "https://github.com/falcon-plugins/freezed".to_string(),
        },
        RegistryEntry {
            name: "falcon-firebase".to_string(),
            version: "1.0.0".to_string(),
            description: "Firebase SDK best practices — security rules, query patterns, offline handling.".to_string(),
            author: "Falcon Community".to_string(),
            downloads: 2100,
            rating: 4.6,
            tags: vec!["firebase".to_string(), "backend".to_string(), "security".to_string()],
            url: "https://github.com/falcon-plugins/firebase".to_string(),
        },
        RegistryEntry {
            name: "falcon-getx".to_string(),
            version: "0.3.0".to_string(),
            description: "GetX state management rules — controller patterns, reactive variables, routing.".to_string(),
            author: "Falcon Community".to_string(),
            downloads: 670,
            rating: 4.2,
            tags: vec!["getx".to_string(), "state".to_string(), "routing".to_string()],
            url: "https://github.com/falcon-plugins/getx".to_string(),
        },
        RegistryEntry {
            name: "falcon-accessibility".to_string(),
            version: "1.1.0".to_string(),
            description: "A11y rules — semantics, contrast ratios, touch target sizes, screen reader hints.".to_string(),
            author: "Falcon Community".to_string(),
            downloads: 1800,
            rating: 4.8,
            tags: vec!["accessibility".to_string(), "a11y".to_string(), "semantics".to_string()],
            url: "https://github.com/falcon-plugins/accessibility".to_string(),
        },
    ]
}

pub fn print_search_results(results: &[RegistryEntry], query: &str) {
    println!();
    if results.is_empty() {
        println!("  No plugins found matching '{}'.", query);
        println!();
        return;
    }

    println!(
        "  {} Plugin Registry — {} result(s) for '{}'",
        "falcon".bright_cyan().bold(),
        results.len(),
        query
    );
    println!();

    for entry in results {
        let stars = "★".repeat(entry.rating.round() as usize);
        println!(
            "  {} v{} — {} {} ({} downloads)",
            entry.name.bright_white().bold(),
            entry.version,
            stars.bright_yellow(),
            format!("{:.1}", entry.rating).dimmed(),
            entry.downloads
        );
        println!("    {}", entry.description.dimmed());
        println!(
            "    tags: {}",
            entry.tags.join(", ").bright_blue()
        );
        println!("    {}", entry.url.underline());
        println!();
    }
}
