//! Auto-Rule Generation — propose new rules from observed code patterns.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

/// A proposed new rule based on observed patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedRule {
    pub name: String,
    pub description: String,
    pub pattern: String,
    pub occurrences: usize,
    pub files_affected: usize,
    pub confidence: f64,
    pub category: String,
}

type PatternCheck = (&'static str, Box<dyn Fn(&str) -> bool>);

/// Scan a project for repeated anti-patterns that could become new rules.
pub fn discover_patterns(root: &Path) -> Vec<ProposedRule> {
    let mut proposed = Vec::new();
    let mut pattern_counts: HashMap<String, (usize, usize)> = HashMap::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| !e.path().to_string_lossy().contains("/test/"))
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        detect_patterns(&source, &mut pattern_counts);
    }

    for (pattern, (occurrences, files)) in &pattern_counts {
        if *occurrences >= 3 {
            if let Some(rule) = propose_rule(pattern, *occurrences, *files) {
                proposed.push(rule);
            }
        }
    }

    proposed.sort_by_key(|rule| Reverse(rule.occurrences));
    proposed
}

fn detect_patterns(source: &str, counts: &mut HashMap<String, (usize, usize)>) {
    let checks: Vec<PatternCheck> = vec![
        (
            "toString-in-interpolation",
            Box::new(|l: &str| l.contains(".toString()") && l.contains("${")),
        ),
        (
            "bang-operator",
            Box::new(|l: &str| {
                l.contains("!.") || l.contains("!,") || (l.contains('!') && l.contains("null"))
            }),
        ),
        (
            "nested-ternary",
            Box::new(|l: &str| l.matches('?').count() >= 2 && l.contains(':')),
        ),
        (
            "string-concat",
            Box::new(|l: &str| l.matches(" + '").count() >= 1 || l.matches(" + \"").count() >= 1),
        ),
        (
            "force-unwrap-list",
            Box::new(|l: &str| l.contains(".first!") || l.contains(".last!") || l.contains("[0]!")),
        ),
        (
            "raw-map-access",
            Box::new(|l: &str| l.contains("['") && l.contains("']") && !l.contains("?[")),
        ),
    ];

    let mut file_patterns = std::collections::HashSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        for (name, check) in &checks {
            if check(trimmed) {
                let entry = counts.entry(name.to_string()).or_insert((0, 0));
                entry.0 += 1;
                file_patterns.insert(name.to_string());
            }
        }
    }

    for p in file_patterns {
        counts.entry(p).or_insert((0, 0)).1 += 1;
    }
}

fn propose_rule(pattern: &str, occurrences: usize, files: usize) -> Option<ProposedRule> {
    let (name, desc, cat) = match pattern {
        "toString-in-interpolation" => (
            "avoid-tostring-in-interpolation",
            "Unnecessary .toString() in string interpolation — Dart handles this automatically",
            "style",
        ),
        "bang-operator" => (
            "avoid-bang-operator",
            "Avoid null assertion operator (!) — use null-aware operators or explicit checks instead",
            "safety",
        ),
        "nested-ternary" => (
            "avoid-nested-ternary",
            "Nested ternary expressions reduce readability — extract to if/else or helper method",
            "readability",
        ),
        "string-concat" => (
            "prefer-interpolation-over-concat",
            "Prefer string interpolation over concatenation for readability",
            "style",
        ),
        "force-unwrap-list" => (
            "avoid-force-unwrap-collection",
            "Force unwrapping collection elements can throw — use .firstOrNull or bounds checking",
            "safety",
        ),
        "raw-map-access" => (
            "prefer-null-aware-map-access",
            "Use null-aware map access (?[]) to avoid NoSuchMethodError on missing keys",
            "safety",
        ),
        _ => return None,
    };

    let confidence = if occurrences >= 10 {
        0.9
    } else if occurrences >= 5 {
        0.7
    } else {
        0.5
    };

    Some(ProposedRule {
        name: name.to_string(),
        description: desc.to_string(),
        pattern: pattern.to_string(),
        occurrences,
        files_affected: files,
        confidence,
        category: cat.to_string(),
    })
}

/// Print proposed rules.
pub fn print_proposed_rules(rules: &[ProposedRule]) {
    println!();
    println!("  {} Auto-Rule Discovery", "falcon".bright_cyan().bold());
    println!();

    if rules.is_empty() {
        println!("  {} No new rule patterns detected.", "✓".green().bold());
        println!();
        return;
    }

    println!(
        "  {} proposed rule(s) based on observed patterns:\n",
        rules.len()
    );

    for rule in rules {
        let confidence_color = if rule.confidence >= 0.8 {
            format!("{:.0}%", rule.confidence * 100.0).green()
        } else {
            format!("{:.0}%", rule.confidence * 100.0).yellow()
        };

        println!(
            "  {} {} ({})",
            "▸".bright_cyan(),
            rule.name.bright_white().bold(),
            confidence_color
        );
        println!("    {}", rule.description);
        println!(
            "    {} occurrences across {} files [{}]",
            rule.occurrences, rule.files_affected, rule.category
        );
        println!();
    }
}
