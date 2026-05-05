//! Refactoring simulation — analyze impact of migrating state management,
//! architecture patterns, or dependency changes.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported refactoring scenarios.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum RefactorScenario {
    /// Migrate from setState to Riverpod
    SetStateToRiverpod,
    /// Migrate from setState to BLoC
    SetStateToBloc,
    /// Migrate from Provider to Riverpod
    ProviderToRiverpod,
    /// Migrate from BLoC to Riverpod
    BlocToRiverpod,
    /// Extract feature modules from flat structure
    FeatureFirst,
    /// Adopt Clean Architecture layers
    CleanArchitecture,
}

impl std::fmt::Display for RefactorScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetStateToRiverpod => write!(f, "setState → Riverpod"),
            Self::SetStateToBloc => write!(f, "setState → BLoC"),
            Self::ProviderToRiverpod => write!(f, "Provider → Riverpod"),
            Self::BlocToRiverpod => write!(f, "BLoC → Riverpod"),
            Self::FeatureFirst => write!(f, "Flat → Feature-First"),
            Self::CleanArchitecture => write!(f, "Flat → Clean Architecture"),
        }
    }
}

/// Impact analysis result for a refactoring scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorImpact {
    pub scenario: String,
    pub files_affected: usize,
    pub estimated_changes: usize,
    pub complexity: String,
    pub estimated_hours: f64,
    pub affected_files: Vec<AffectedFile>,
    pub risks: Vec<String>,
    pub benefits: Vec<String>,
    pub migration_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedFile {
    pub file: String,
    pub reason: String,
    pub change_type: String,
}

/// Simulate a refactoring and produce an impact report.
pub fn simulate_refactor(
    root: &Path,
    scenario: &RefactorScenario,
) -> anyhow::Result<RefactorImpact> {
    match scenario {
        RefactorScenario::SetStateToRiverpod => {
            simulate_state_migration(root, "setState", "Riverpod")
        }
        RefactorScenario::SetStateToBloc => simulate_state_migration(root, "setState", "BLoC"),
        RefactorScenario::ProviderToRiverpod => {
            simulate_state_migration(root, "Provider", "Riverpod")
        }
        RefactorScenario::BlocToRiverpod => simulate_state_migration(root, "BLoC", "Riverpod"),
        RefactorScenario::FeatureFirst => simulate_architecture_migration(root, "Feature-First"),
        RefactorScenario::CleanArchitecture => {
            simulate_architecture_migration(root, "Clean Architecture")
        }
    }
}

fn simulate_state_migration(root: &Path, from: &str, to: &str) -> anyhow::Result<RefactorImpact> {
    let mut affected = Vec::new();
    let mut total_occurrences = 0;

    let patterns: Vec<&str> = match from {
        "setState" => vec!["setState(", "State<", "StatefulWidget"],
        "Provider" => vec![
            "Provider.of",
            "ChangeNotifier",
            "Consumer(",
            "context.read",
            "context.watch",
        ],
        "BLoC" => vec![
            "BlocProvider",
            "BlocBuilder",
            "extends Bloc",
            "extends Cubit",
            "emit(",
        ],
        _ => vec![],
    };

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| !e.path().to_string_lossy().contains("/test/"))
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let count: usize = patterns.iter().map(|p| source.matches(p).count()).sum();
        if count > 0 {
            total_occurrences += count;
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();

            let change = if source.contains("StatefulWidget") || source.contains("extends Bloc") {
                "major rewrite"
            } else if count > 3 {
                "significant changes"
            } else {
                "minor changes"
            };

            affected.push(AffectedFile {
                file: rel,
                reason: format!("{} occurrences of {} patterns", count, from),
                change_type: change.to_string(),
            });
        }
    }

    let files_affected = affected.len();
    let complexity = if files_affected > 30 {
        "High"
    } else if files_affected > 10 {
        "Medium"
    } else {
        "Low"
    };
    let hours_per_file = match complexity {
        "High" => 1.5,
        "Medium" => 1.0,
        _ => 0.5,
    };

    let removal_step = match from {
        "setState" => None,
        "Provider" => Some("Remove provider dependency from pubspec.yaml".to_string()),
        "BLoC" => Some("Remove flutter_bloc and bloc dependencies from pubspec.yaml".to_string()),
        other => Some(format!(
            "Remove {} dependency from pubspec.yaml",
            other.to_lowercase()
        )),
    };

    let mut migration_steps: Vec<String> = match to {
        "Riverpod" => vec![
            "Add riverpod and flutter_riverpod to pubspec.yaml".to_string(),
            "Wrap app with ProviderScope".to_string(),
            format!(
                "Convert {} widgets to ConsumerWidget/ConsumerStatefulWidget",
                files_affected
            ),
            "Replace state with providers (StateNotifier/AsyncNotifier)".to_string(),
            "Update tests to use ProviderContainer".to_string(),
        ],
        "BLoC" => vec![
            "Add flutter_bloc to pubspec.yaml".to_string(),
            format!(
                "Create Bloc/Cubit classes for {} stateful widgets",
                files_affected
            ),
            "Define events and states for each Bloc".to_string(),
            "Replace setState with BlocBuilder/BlocListener".to_string(),
            "Update tests to use blocTest".to_string(),
        ],
        _ => vec![],
    };

    if let Some(step) = removal_step {
        migration_steps.push(step);
    }

    let risks = vec![
        format!(
            "{} files need changes — risk of regressions",
            files_affected
        ),
        "Widget tests will need updating for new state management".to_string(),
        format!("Team needs to learn {} patterns", to),
    ];

    let benefits = vec![
        format!("Better testability with {}", to),
        "Separation of UI and business logic".to_string(),
        format!("Eliminates {} anti-patterns", from),
        "More predictable state updates".to_string(),
    ];

    Ok(RefactorImpact {
        scenario: format!("{} → {}", from, to),
        files_affected,
        estimated_changes: total_occurrences,
        complexity: complexity.to_string(),
        estimated_hours: files_affected as f64 * hours_per_file,
        affected_files: affected,
        risks,
        benefits,
        migration_steps,
    })
}

fn simulate_architecture_migration(root: &Path, target: &str) -> anyhow::Result<RefactorImpact> {
    let mut affected = Vec::new();
    let mut file_count = 0;

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| !e.path().to_string_lossy().contains("/test/"))
    {
        file_count += 1;
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        let source = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let change = if source.contains("class ") && source.contains("Repository") {
            "move to data layer"
        } else if source.contains("Widget build") {
            "move to presentation layer"
        } else if source.contains("class ") && !source.contains("Widget") {
            "move to domain/data layer"
        } else {
            "classify and relocate"
        };

        affected.push(AffectedFile {
            file: rel,
            reason: "Needs layer assignment".to_string(),
            change_type: change.to_string(),
        });
    }

    let layers = match target {
        "Clean Architecture" => vec![
            "domain/entities/",
            "domain/repositories/",
            "domain/usecases/",
            "data/repositories/",
            "data/datasources/",
            "presentation/pages/",
            "presentation/widgets/",
        ],
        "Feature-First" => vec![
            "features/<name>/data/",
            "features/<name>/domain/",
            "features/<name>/presentation/",
            "core/",
            "shared/",
        ],
        _ => vec![],
    };

    let migration_steps = vec![
        format!("Create directory structure: {}", layers.join(", ")),
        format!("Move {} files to appropriate layers", file_count),
        "Update all import paths".to_string(),
        "Add dependency injection (get_it/injectable)".to_string(),
        "Define abstract repository interfaces in domain".to_string(),
        "Implement repository interfaces in data layer".to_string(),
    ];

    Ok(RefactorImpact {
        scenario: format!("→ {}", target),
        files_affected: file_count,
        estimated_changes: file_count * 2,
        complexity: if file_count > 50 { "High" } else { "Medium" }.to_string(),
        estimated_hours: file_count as f64 * 0.3,
        affected_files: affected,
        risks: vec![
            "All import paths change — large diff".to_string(),
            "Potential circular dependency issues during migration".to_string(),
        ],
        benefits: vec![
            "Clear separation of concerns".to_string(),
            "Better testability with dependency inversion".to_string(),
            "Scalable project structure".to_string(),
        ],
        migration_steps,
    })
}

/// Print a refactoring impact report.
pub fn print_refactor_impact(impact: &RefactorImpact) {
    println!();
    println!(
        "  {} Refactoring Simulation: {}",
        "falcon".bright_cyan().bold(),
        impact.scenario.bright_white().bold()
    );
    println!();

    let complexity_color = match impact.complexity.as_str() {
        "High" => impact.complexity.red().bold(),
        "Medium" => impact.complexity.yellow().bold(),
        _ => impact.complexity.green().bold(),
    };

    println!(
        "  Files affected:      {}",
        impact.files_affected.to_string().bright_white()
    );
    println!("  Estimated changes:   {}", impact.estimated_changes);
    println!("  Complexity:          {}", complexity_color);
    println!("  Estimated effort:    {:.0} hours", impact.estimated_hours);

    println!();
    println!("  {} Migration Steps", "▸".bright_cyan());
    for (i, step) in impact.migration_steps.iter().enumerate() {
        println!("    {}. {}", i + 1, step);
    }

    println!();
    println!("  {} Benefits", "▸".green());
    for b in &impact.benefits {
        println!("    {} {}", "+".green(), b);
    }

    println!();
    println!("  {} Risks", "▸".red());
    for r in &impact.risks {
        println!("    {} {}", "!".red(), r);
    }

    if !impact.affected_files.is_empty() {
        println!();
        println!("  {} Affected Files (top 10)", "▸".bright_cyan());
        for f in impact.affected_files.iter().take(10) {
            println!(
                "    {} {} — {}",
                "·".dimmed(),
                f.file.bright_white(),
                f.change_type.dimmed()
            );
        }
        if impact.affected_files.len() > 10 {
            println!(
                "    {} ... and {} more",
                "·".dimmed(),
                impact.affected_files.len() - 10
            );
        }
    }

    println!();
}
