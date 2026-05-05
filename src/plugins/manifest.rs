use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub license: String,
    pub plugin_type: PluginType,
    #[serde(default)]
    pub rules: Vec<PluginRuleEntry>,
    #[serde(default)]
    pub min_falcon_version: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Wasm,
    Native,
    Preset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRuleEntry {
    pub name: String,
    pub description: String,
    #[serde(default = "default_severity")]
    pub default_severity: String,
}

fn default_severity() -> String {
    "warning".to_string()
}

impl PluginManifest {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let manifest_path = if path.is_file() {
            path.to_path_buf()
        } else {
            path.join("falcon-plugin.yaml")
        };

        let content = std::fs::read_to_string(&manifest_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read plugin manifest at {}: {}",
                manifest_path.display(),
                e
            )
        })?;

        let manifest: PluginManifest = serde_yaml::from_str(&content)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            anyhow::bail!("Plugin name cannot be empty");
        }
        if self.version.is_empty() {
            anyhow::bail!("Plugin version cannot be empty");
        }
        if !self
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            anyhow::bail!(
                "Plugin name can only contain alphanumeric characters, underscores, and hyphens"
            );
        }
        Ok(())
    }

    pub fn save(&self, dir: &Path) -> anyhow::Result<PathBuf> {
        let path = dir.join("falcon-plugin.yaml");
        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(&path, yaml)?;
        Ok(path)
    }
}

impl Default for PluginManifest {
    fn default() -> Self {
        Self {
            name: "my-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "A custom Falcon plugin".to_string(),
            author: String::new(),
            repository: String::new(),
            license: "MIT".to_string(),
            plugin_type: PluginType::Wasm,
            rules: Vec::new(),
            min_falcon_version: "0.1.0".to_string(),
            tags: Vec::new(),
        }
    }
}
