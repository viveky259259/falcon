//! Falcon Marketplace — browse, install, and publish rule packs,
//! convention configs, and integrations.

use colored::Colorize;
use serde::{Deserialize, Serialize};
/// A marketplace listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceListing {
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub category: ListingCategory,
    pub downloads: usize,
    pub rating: f64,
    pub rules_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ListingCategory {
    RulePack,
    ConventionConfig,
    Integration,
    Preset,
}

impl std::fmt::Display for ListingCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RulePack => write!(f, "Rule Pack"),
            Self::ConventionConfig => write!(f, "Convention Config"),
            Self::Integration => write!(f, "Integration"),
            Self::Preset => write!(f, "Preset"),
        }
    }
}

/// Sample marketplace listings (simulated registry).
pub fn browse_marketplace(query: Option<&str>) -> Vec<MarketplaceListing> {
    let all = sample_listings();
    match query {
        Some(q) => {
            let q_lower = q.to_lowercase();
            all.into_iter()
                .filter(|l| {
                    l.name.to_lowercase().contains(&q_lower)
                        || l.description.to_lowercase().contains(&q_lower)
                        || l.author.to_lowercase().contains(&q_lower)
                })
                .collect()
        }
        None => all,
    }
}

fn sample_listings() -> Vec<MarketplaceListing> {
    vec![
        MarketplaceListing {
            name: "stripe-flutter-conventions".to_string(),
            description: "Stripe's Flutter team coding conventions — architecture, naming, error handling".to_string(),
            author: "stripe-engineering".to_string(),
            version: "1.2.0".to_string(),
            category: ListingCategory::ConventionConfig,
            downloads: 12500,
            rating: 4.8,
            rules_count: 0,
        },
        MarketplaceListing {
            name: "riverpod-strict".to_string(),
            description: "Strict rules for Riverpod best practices — no ref.read in build, proper scoping".to_string(),
            author: "riverpod-community".to_string(),
            version: "2.0.1".to_string(),
            category: ListingCategory::RulePack,
            downloads: 8300,
            rating: 4.6,
            rules_count: 12,
        },
        MarketplaceListing {
            name: "security-hardened".to_string(),
            description: "Enterprise security rules — credential detection, encryption checks, OWASP compliance".to_string(),
            author: "falcon-security".to_string(),
            version: "1.0.0".to_string(),
            category: ListingCategory::RulePack,
            downloads: 5600,
            rating: 4.9,
            rules_count: 25,
        },
        MarketplaceListing {
            name: "clean-arch-enforcer".to_string(),
            description: "Enforce Clean Architecture layer boundaries with import restrictions".to_string(),
            author: "arch-experts".to_string(),
            version: "1.1.0".to_string(),
            category: ListingCategory::RulePack,
            downloads: 7200,
            rating: 4.5,
            rules_count: 8,
        },
        MarketplaceListing {
            name: "sonarqube-bridge".to_string(),
            description: "Export Falcon results to SonarQube for unified quality dashboards".to_string(),
            author: "devops-tools".to_string(),
            version: "1.0.2".to_string(),
            category: ListingCategory::Integration,
            downloads: 3400,
            rating: 4.3,
            rules_count: 0,
        },
        MarketplaceListing {
            name: "accessibility-complete".to_string(),
            description: "Full WCAG 2.1 AA compliance rules for Flutter — semantics, contrast, touch targets".to_string(),
            author: "a11y-flutter".to_string(),
            version: "1.3.0".to_string(),
            category: ListingCategory::RulePack,
            downloads: 4100,
            rating: 4.7,
            rules_count: 18,
        },
        MarketplaceListing {
            name: "startup-minimal".to_string(),
            description: "Minimal but effective preset for startups — catch critical bugs without slowing velocity".to_string(),
            author: "falcon-presets".to_string(),
            version: "1.0.0".to_string(),
            category: ListingCategory::Preset,
            downloads: 9800,
            rating: 4.4,
            rules_count: 15,
        },
        MarketplaceListing {
            name: "google-style".to_string(),
            description: "Google's internal Flutter style conventions (community interpretation)".to_string(),
            author: "flutter-community".to_string(),
            version: "2.1.0".to_string(),
            category: ListingCategory::ConventionConfig,
            downloads: 15200,
            rating: 4.6,
            rules_count: 0,
        },
    ]
}

/// Print marketplace listings.
pub fn print_marketplace(listings: &[MarketplaceListing], query: Option<&str>) {
    println!();
    println!("  {} Marketplace", "falcon".bright_cyan().bold());
    if let Some(q) = query {
        println!("  Search: \"{}\"", q);
    }
    println!();

    if listings.is_empty() {
        println!("  No results found.");
        println!();
        return;
    }

    for listing in listings {
        let cat_color = match listing.category {
            ListingCategory::RulePack => listing.category.to_string().bright_yellow(),
            ListingCategory::ConventionConfig => listing.category.to_string().bright_blue(),
            ListingCategory::Integration => listing.category.to_string().bright_green(),
            ListingCategory::Preset => listing.category.to_string().bright_magenta(),
        };

        println!(
            "  {} {} v{} [{}]",
            "▸".bright_cyan(),
            listing.name.bright_white().bold(),
            listing.version,
            cat_color
        );
        println!("    {}", listing.description.dimmed());
        println!(
            "    by {} · {} downloads · ★ {:.1}{}",
            listing.author,
            listing.downloads,
            listing.rating,
            if listing.rules_count > 0 {
                format!(" · {} rules", listing.rules_count)
            } else {
                String::new()
            }
        );
        println!();
    }
}
