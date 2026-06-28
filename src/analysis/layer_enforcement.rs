use crate::config::Severity;
use crate::reporters::Issue;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Defines an architectural layer and what it's allowed to import.
#[derive(Debug, Clone)]
pub struct LayerConfig {
    pub name: String,
    pub path_pattern: String,
    pub allowed_imports: Vec<String>,
}

impl LayerConfig {
    pub fn clean_architecture() -> Vec<LayerConfig> {
        vec![
            LayerConfig {
                name: "domain".to_string(),
                path_pattern: "domain".to_string(),
                allowed_imports: vec![],
            },
            LayerConfig {
                name: "data".to_string(),
                path_pattern: "data".to_string(),
                allowed_imports: vec!["domain".to_string()],
            },
            LayerConfig {
                name: "presentation".to_string(),
                path_pattern: "presentation".to_string(),
                allowed_imports: vec!["domain".to_string()],
            },
            LayerConfig {
                name: "application".to_string(),
                path_pattern: "application".to_string(),
                allowed_imports: vec!["domain".to_string(), "data".to_string()],
            },
        ]
    }

    pub fn feature_first() -> Vec<LayerConfig> {
        vec![
            LayerConfig {
                name: "core".to_string(),
                path_pattern: "core".to_string(),
                allowed_imports: vec![],
            },
            LayerConfig {
                name: "shared".to_string(),
                path_pattern: "shared".to_string(),
                allowed_imports: vec!["core".to_string()],
            },
            LayerConfig {
                name: "features".to_string(),
                path_pattern: "features".to_string(),
                allowed_imports: vec!["core".to_string(), "shared".to_string()],
            },
        ]
    }
}

/// Detect architecture preset from directory structure.
pub fn detect_architecture(root: &Path) -> Option<Vec<LayerConfig>> {
    let lib_path = root.join("lib");
    let base = if lib_path.exists() {
        lib_path
    } else {
        root.to_path_buf()
    };

    let has_domain = base.join("domain").exists();
    let has_data = base.join("data").exists();
    let has_presentation = base.join("presentation").exists();

    if has_domain && (has_data || has_presentation) {
        return Some(LayerConfig::clean_architecture());
    }

    let has_core = base.join("core").exists();
    let has_features = base.join("features").exists();

    if has_core && has_features {
        return Some(LayerConfig::feature_first());
    }

    None
}

/// Enforce layer dependency rules on a project.
pub fn enforce_layers(
    root: &Path,
    layers: &[LayerConfig],
    exclude: &[glob::Pattern],
) -> Vec<Issue> {
    let mut issues = Vec::new();
    let lib_path = root.join("lib");
    let base = if lib_path.exists() { &lib_path } else { root };

    let dart_files: Vec<PathBuf> = WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    for file in &dart_files {
        let rel = file.strip_prefix(base).unwrap_or(file);
        let rel_str = rel.to_string_lossy();

        let file_layer = layers.iter().find(|l| {
            rel_str.starts_with(&l.path_pattern)
                || rel_str.starts_with(&format!("{}/", l.path_pattern))
        });

        let file_layer = match file_layer {
            Some(l) => l,
            None => continue,
        };

        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("import") {
                continue;
            }

            for layer in layers {
                if layer.name == file_layer.name {
                    continue;
                }

                let import_markers = [
                    format!("/{}/", layer.path_pattern),
                    format!("/{}'", layer.path_pattern),
                    format!("/{}\"", layer.path_pattern),
                ];

                let imports_layer = import_markers.iter().any(|m| trimmed.contains(m.as_str()));

                if imports_layer && !file_layer.allowed_imports.contains(&layer.name) {
                    issues.push(Issue {
                        rule: "layer-violation".to_string(),
                        message: format!(
                            "Layer '{}' cannot import from '{}'. Allowed imports: [{}].",
                            file_layer.name,
                            layer.name,
                            if file_layer.allowed_imports.is_empty() {
                                "none".to_string()
                            } else {
                                file_layer.allowed_imports.join(", ")
                            }
                        ),
                        severity: Severity::Error,
                        file: file.clone(),
                        line: line_num + 1,
                        column: 1,
                    });
                }
            }
        }
    }

    issues
}
