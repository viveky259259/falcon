use crate::config::Severity;
use crate::reporters::Issue;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::manifest::{PluginManifest, PluginType};

/// A loaded plugin that can run analysis rules.
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub source_path: PathBuf,
    plugin_impl: PluginImpl,
}

enum PluginImpl {
    Wasm(WasmPlugin),
    Preset(PresetPlugin),
}

struct WasmPlugin {
    rules: Vec<WasmRule>,
}

struct WasmRule {
    name: String,
    description: String,
    severity: Severity,
    patterns: Vec<String>,
}

struct PresetPlugin {
    rules: HashMap<String, Severity>,
}

impl LoadedPlugin {
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    pub fn rule_count(&self) -> usize {
        match &self.plugin_impl {
            PluginImpl::Wasm(w) => w.rules.len(),
            PluginImpl::Preset(p) => p.rules.len(),
        }
    }

    pub fn check(&self, source: &str, file: &Path) -> Vec<Issue> {
        match &self.plugin_impl {
            PluginImpl::Wasm(w) => check_wasm_rules(w, source, file),
            PluginImpl::Preset(_) => Vec::new(),
        }
    }

    pub fn rule_names(&self) -> Vec<String> {
        match &self.plugin_impl {
            PluginImpl::Wasm(w) => w.rules.iter().map(|r| r.name.clone()).collect(),
            PluginImpl::Preset(p) => p.rules.keys().cloned().collect(),
        }
    }
}

fn check_wasm_rules(wasm: &WasmPlugin, source: &str, file: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    for rule in &wasm.rules {
        for (line_num, line) in source.lines().enumerate() {
            for pattern in &rule.patterns {
                if line.contains(pattern.as_str()) {
                    issues.push(Issue {
                        rule: rule.name.clone(),
                        message: rule.description.clone(),
                        severity: rule.severity,
                        file: file.to_path_buf(),
                        line: line_num + 1,
                        column: 1,
                    });
                }
            }
        }
    }

    issues
}

/// Plugin loader — discovers and loads plugins from a directory.
pub struct PluginLoader {
    plugin_dirs: Vec<PathBuf>,
    loaded: Vec<LoadedPlugin>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugin_dirs: Vec::new(),
            loaded: Vec::new(),
        }
    }

    pub fn add_plugin_dir(&mut self, dir: PathBuf) {
        self.plugin_dirs.push(dir);
    }

    /// Discover and load all plugins from configured directories.
    pub fn discover(&mut self) -> anyhow::Result<usize> {
        let mut count = 0;

        for dir in self.plugin_dirs.clone() {
            if !dir.exists() {
                continue;
            }

            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    let manifest_path = path.join("falcon-plugin.yaml");
                    if manifest_path.exists() {
                        match self.load_plugin(&path) {
                            Ok(plugin) => {
                                self.loaded.push(plugin);
                                count += 1;
                            }
                            Err(e) => {
                                log::warn!("Failed to load plugin at {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    fn load_plugin(&self, path: &Path) -> anyhow::Result<LoadedPlugin> {
        let manifest = PluginManifest::load(path)?;

        let plugin_impl = match manifest.plugin_type {
            PluginType::Wasm => {
                let rules = manifest
                    .rules
                    .iter()
                    .map(|r| WasmRule {
                        name: format!("{}:{}", manifest.name, r.name),
                        description: r.description.clone(),
                        severity: parse_severity(&r.default_severity),
                        patterns: Vec::new(),
                    })
                    .collect();
                PluginImpl::Wasm(WasmPlugin { rules })
            }
            PluginType::Native => {
                let rules = manifest
                    .rules
                    .iter()
                    .map(|r| WasmRule {
                        name: format!("{}:{}", manifest.name, r.name),
                        description: r.description.clone(),
                        severity: parse_severity(&r.default_severity),
                        patterns: Vec::new(),
                    })
                    .collect();
                PluginImpl::Wasm(WasmPlugin { rules })
            }
            PluginType::Preset => {
                let rules = manifest
                    .rules
                    .iter()
                    .map(|r| (r.name.clone(), parse_severity(&r.default_severity)))
                    .collect();
                PluginImpl::Preset(PresetPlugin { rules })
            }
        };

        Ok(LoadedPlugin {
            manifest,
            source_path: path.to_path_buf(),
            plugin_impl,
        })
    }

    pub fn loaded_plugins(&self) -> &[LoadedPlugin] {
        &self.loaded
    }

    pub fn check_all(&self, source: &str, file: &Path) -> Vec<Issue> {
        let mut all_issues = Vec::new();
        for plugin in &self.loaded {
            all_issues.extend(plugin.check(source, file));
        }
        all_issues
    }
}

fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "info" | "style" => Severity::Info,
        _ => Severity::Warning,
    }
}
