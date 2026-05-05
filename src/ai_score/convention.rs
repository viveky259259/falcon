use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConventionReport {
    pub naming: NamingConventions,
    pub architecture: ArchitectureConventions,
    pub error_handling: ErrorHandlingConventions,
    pub state_management: Option<String>,
    pub consistency_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamingConventions {
    pub file_naming: String,
    pub class_naming: String,
    pub private_prefix: bool,
    pub uses_underscore_params: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureConventions {
    pub pattern: String,
    pub layers_detected: Vec<String>,
    pub has_separate_models: bool,
    pub has_separate_services: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConventions {
    pub uses_result_type: bool,
    pub uses_either: bool,
    pub uses_try_catch: bool,
    pub uses_custom_exceptions: bool,
}

/// Auto-detect team conventions from the codebase at `root`.
pub fn detect_conventions(root: &Path) -> anyhow::Result<ConventionReport> {
    if !root.exists() {
        anyhow::bail!("Path does not exist: {}", root.display());
    }

    let mut file_names = Vec::new();
    let mut class_names = Vec::new();
    let mut dir_names: HashMap<String, usize> = HashMap::new();
    let mut uses_result = false;
    let mut uses_either = false;
    let mut uses_try_catch = false;
    let mut uses_custom_exceptions = false;
    let mut state_mgmt: HashMap<String, usize> = HashMap::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) => Some(entry),
            Err(err) => {
                log::warn!("Failed to read directory entry: {}", err);
                None
            }
        })
    {
        if entry.file_type().is_dir() {
            if let Some(name) = entry.path().file_name() {
                let n = name.to_string_lossy().to_string();
                *dir_names.entry(n).or_default() += 1;
            }
        }

        if !entry.file_type().is_file() {
            continue;
        }

        if entry.path().extension().map_or(true, |ext| ext != "dart") {
            continue;
        }

        if let Some(fname) = entry.path().file_name() {
            file_names.push(fname.to_string_lossy().to_string());
        }

        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("class ") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() > 1 {
                        class_names.push(parts[1].to_string());
                    }
                }

                if trimmed.contains("Result<") || trimmed.contains("Result.success") {
                    uses_result = true;
                }
                if trimmed.contains("Either<") || trimmed.contains("dartz") {
                    uses_either = true;
                }
                if trimmed.contains("try {") || trimmed.contains("} catch") {
                    uses_try_catch = true;
                }
                if trimmed.contains("class ")
                    && trimmed.contains("Exception")
                    && trimmed.contains("implements")
                {
                    uses_custom_exceptions = true;
                }

                if trimmed.contains("BlocProvider") || trimmed.contains("extends Bloc") {
                    *state_mgmt.entry("BLoC".to_string()).or_default() += 1;
                }
                if trimmed.contains("ref.watch")
                    || trimmed.contains("ref.read")
                    || trimmed.contains("ConsumerWidget")
                {
                    *state_mgmt.entry("Riverpod".to_string()).or_default() += 1;
                }
                if trimmed.contains("Provider.of") || trimmed.contains("ChangeNotifier") {
                    *state_mgmt.entry("Provider".to_string()).or_default() += 1;
                }
                if trimmed.contains("GetxController") || trimmed.contains("Get.find") {
                    *state_mgmt.entry("GetX".to_string()).or_default() += 1;
                }
                if trimmed.contains("setState(") {
                    *state_mgmt.entry("setState".to_string()).or_default() += 1;
                }
            }
        }
    }

    let file_naming = if file_names.iter().all(|f| f == &f.to_lowercase()) {
        "snake_case".to_string()
    } else {
        "mixed".to_string()
    };

    let pascal = class_names
        .iter()
        .filter(|c| c.chars().next().map_or(false, |ch| ch.is_uppercase()))
        .count();
    let class_naming = if pascal == class_names.len() {
        "PascalCase".to_string()
    } else {
        "mixed".to_string()
    };

    let mut layers = Vec::new();
    let arch_dirs = [
        "domain",
        "data",
        "presentation",
        "models",
        "services",
        "repositories",
        "views",
        "controllers",
        "features",
        "core",
        "shared",
        "widgets",
    ];
    for dir in &arch_dirs {
        if dir_names.contains_key(*dir) {
            layers.push(dir.to_string());
        }
    }

    let pattern = if layers.contains(&"domain".to_string()) && layers.contains(&"data".to_string())
    {
        "Clean Architecture"
    } else if layers.contains(&"features".to_string()) {
        "Feature-First"
    } else if layers.contains(&"models".to_string()) && layers.contains(&"views".to_string()) {
        "MVC/MVVM"
    } else {
        "Flat/Custom"
    };

    let dominant_state = state_mgmt
        .iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k.clone());

    let consistency_score = calculate_consistency(&file_naming, &class_naming, &layers);

    Ok(ConventionReport {
        naming: NamingConventions {
            file_naming,
            class_naming,
            private_prefix: true,
            uses_underscore_params: false,
        },
        architecture: ArchitectureConventions {
            pattern: pattern.to_string(),
            layers_detected: layers.clone(),
            has_separate_models: layers.contains(&"models".to_string()),
            has_separate_services: layers.contains(&"services".to_string()),
        },
        error_handling: ErrorHandlingConventions {
            uses_result_type: uses_result,
            uses_either,
            uses_try_catch,
            uses_custom_exceptions,
        },
        state_management: dominant_state,
        consistency_score,
    })
}

fn calculate_consistency(file_naming: &str, class_naming: &str, layers: &[String]) -> f64 {
    let mut score: f64 = 50.0;
    if file_naming == "snake_case" {
        score += 15.0;
    }
    if class_naming == "PascalCase" {
        score += 15.0;
    }
    if !layers.is_empty() {
        score += 10.0;
    }
    if layers.len() >= 3 {
        score += 10.0;
    }
    score.min(100.0)
}

pub fn print_convention_report(report: &ConventionReport) {
    println!();
    println!("  {} Convention Detection", "falcon".bright_cyan().bold());
    println!();

    println!("  Naming:");
    println!(
        "    File naming:       {}",
        report.naming.file_naming.bright_white()
    );
    println!(
        "    Class naming:      {}",
        report.naming.class_naming.bright_white()
    );

    println!();
    println!("  Architecture:");
    println!(
        "    Pattern:           {}",
        report.architecture.pattern.bright_white().bold()
    );
    if !report.architecture.layers_detected.is_empty() {
        println!(
            "    Layers:            {}",
            report
                .architecture
                .layers_detected
                .join(", ")
                .bright_white()
        );
    }

    println!();
    println!("  Error Handling:");
    if report.error_handling.uses_result_type {
        println!("    {} Result type pattern", "✓".green());
    }
    if report.error_handling.uses_either {
        println!("    {} Either/dartz pattern", "✓".green());
    }
    if report.error_handling.uses_try_catch {
        println!("    {} try/catch pattern", "✓".green());
    }
    if report.error_handling.uses_custom_exceptions {
        println!("    {} Custom exception classes", "✓".green());
    }

    if let Some(ref sm) = report.state_management {
        println!();
        println!("  State Management:    {}", sm.bright_white().bold());
    }

    println!();
    println!("  Consistency Score:   {:.0}%", report.consistency_score);
    println!();
}
