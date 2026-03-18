use colored::Colorize;
use std::path::Path;

#[derive(Debug)]
pub struct ConfigError {
    pub field: String,
    pub message: String,
    pub severity: ConfigErrorSeverity,
}

#[derive(Debug)]
pub enum ConfigErrorSeverity {
    Error,
    Warning,
}

/// Validate falcon.yaml with clear, actionable error messages.
pub fn validate_config(path: &Path) -> Vec<ConfigError> {
    let config_path = path.join("falcon.yaml");
    let mut errors = Vec::new();

    if !config_path.exists() {
        return errors;
    }

    let contents = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            errors.push(ConfigError {
                field: "file".to_string(),
                message: format!("Cannot read falcon.yaml: {}", e),
                severity: ConfigErrorSeverity::Error,
            });
            return errors;
        }
    };

    let yaml: serde_yaml::Value = match serde_yaml::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            errors.push(ConfigError {
                field: "yaml".to_string(),
                message: format!("Invalid YAML syntax: {}", e),
                severity: ConfigErrorSeverity::Error,
            });
            return errors;
        }
    };

    let valid_top_level = ["metrics", "rules", "unused", "exclude", "ai"];
    if let Some(map) = yaml.as_mapping() {
        for key in map.keys() {
            if let Some(key_str) = key.as_str() {
                if !valid_top_level.contains(&key_str) {
                    errors.push(ConfigError {
                        field: key_str.to_string(),
                        message: format!(
                            "Unknown top-level key '{}'. Valid keys: {}",
                            key_str,
                            valid_top_level.join(", ")
                        ),
                        severity: ConfigErrorSeverity::Warning,
                    });
                }
            }
        }
    }

    if let Some(metrics) = yaml.get("metrics") {
        let valid_metrics = [
            "cyclomatic_complexity",
            "lines_of_code",
            "number_of_parameters",
            "maximum_nesting_level",
            "number_of_methods",
            "maintainability_index",
            "source_lines_of_code",
        ];
        if let Some(map) = metrics.as_mapping() {
            for key in map.keys() {
                if let Some(key_str) = key.as_str() {
                    if !valid_metrics.contains(&key_str) {
                        errors.push(ConfigError {
                            field: format!("metrics.{}", key_str),
                            message: format!(
                                "Unknown metric '{}'. Valid metrics: {}",
                                key_str,
                                valid_metrics.join(", ")
                            ),
                            severity: ConfigErrorSeverity::Warning,
                        });
                    }
                }
            }
            for key in map.keys() {
                if let Some(key_str) = key.as_str() {
                    if valid_metrics.contains(&key_str) {
                        if let Some(val) = map.get(key) {
                            if !val.is_number() {
                                errors.push(ConfigError {
                                    field: format!("metrics.{}", key_str),
                                    message: format!(
                                        "Metric '{}' must be a number, got: {:?}",
                                        key_str, val
                                    ),
                                    severity: ConfigErrorSeverity::Error,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(rules) = yaml.get("rules") {
        if !rules.is_sequence() {
            errors.push(ConfigError {
                field: "rules".to_string(),
                message: "'rules' must be a list/sequence".to_string(),
                severity: ConfigErrorSeverity::Error,
            });
        }
    }

    if let Some(exclude) = yaml.get("exclude") {
        if !exclude.is_sequence() {
            errors.push(ConfigError {
                field: "exclude".to_string(),
                message: "'exclude' must be a list of glob patterns".to_string(),
                severity: ConfigErrorSeverity::Error,
            });
        }
    }

    errors
}

pub fn print_validation_results(errors: &[ConfigError]) {
    if errors.is_empty() {
        println!("{} falcon.yaml is valid", "✓".green().bold());
        return;
    }

    for error in errors {
        match error.severity {
            ConfigErrorSeverity::Error => {
                println!(
                    "  {} [{}] {}",
                    "✗".red().bold(),
                    error.field.bright_white(),
                    error.message
                );
            }
            ConfigErrorSeverity::Warning => {
                println!(
                    "  {} [{}] {}",
                    "⚠".yellow().bold(),
                    error.field.bright_white(),
                    error.message
                );
            }
        }
    }
}
