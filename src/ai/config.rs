use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub provider: AiProvider,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default)]
    pub api_key_env: Option<String>,

    #[serde(default)]
    pub model: Option<String>,

    #[serde(default)]
    pub local: Option<LocalModelConfig>,

    #[serde(default)]
    pub features: AiFeatureToggles,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: AiProvider::None,
            api_key: None,
            api_key_env: None,
            model: None,
            local: None,
            features: AiFeatureToggles::default(),
        }
    }
}

impl AiConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(ref key) = self.api_key {
            return Some(key.clone());
        }
        if let Some(ref env_var) = self.api_key_env {
            return std::env::var(env_var).ok();
        }
        match self.provider {
            AiProvider::OpenAi => std::env::var("OPENAI_API_KEY").ok(),
            AiProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
            AiProvider::Gemini => std::env::var("GOOGLE_API_KEY").ok(),
            AiProvider::Local | AiProvider::None => None,
        }
    }

    pub fn is_available(&self) -> bool {
        if !self.enabled {
            return false;
        }
        match self.provider {
            AiProvider::Local => self.local.is_some(),
            AiProvider::None => false,
            _ => self.resolve_api_key().is_some(),
        }
    }

    pub fn effective_model(&self) -> String {
        if let Some(ref model) = self.model {
            return model.clone();
        }
        match self.provider {
            AiProvider::OpenAi => "gpt-4o".to_string(),
            AiProvider::Anthropic => "claude-sonnet-4-20250514".to_string(),
            AiProvider::Gemini => "gemini-2.0-flash".to_string(),
            AiProvider::Local => self
                .local
                .as_ref()
                .map(|l| l.model.clone())
                .unwrap_or_else(|| "codellama".to_string()),
            AiProvider::None => String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[serde(alias = "openai")]
    OpenAi,
    Anthropic,
    Gemini,
    Local,
    None,
}

impl Default for AiProvider {
    fn default() -> Self {
        AiProvider::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelConfig {
    #[serde(default = "default_local_endpoint")]
    pub endpoint: String,

    #[serde(default = "default_local_model")]
    pub model: String,
}

fn default_local_endpoint() -> String {
    "http://localhost:11434".to_string()
}

fn default_local_model() -> String {
    "codellama".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiFeatureToggles {
    #[serde(default)]
    pub confidence_scoring: bool,

    #[serde(default)]
    pub smart_fixes: bool,

    #[serde(default)]
    pub explanations: bool,

    #[serde(default)]
    pub false_positive_reduction: bool,
}

impl Default for AiFeatureToggles {
    fn default() -> Self {
        Self {
            confidence_scoring: false,
            smart_fixes: false,
            explanations: false,
            false_positive_reduction: false,
        }
    }
}

pub fn generate_ai_setup(path: &Path) -> anyhow::Result<()> {
    let config_path = path.join("falcon.yaml");
    let mut content = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    if content.contains("ai:") {
        anyhow::bail!("AI configuration already exists in falcon.yaml. Edit it directly.");
    }

    let ai_block = r#"
# AI Configuration (all features off by default)
ai:
  enabled: false
  # Provider: openai, anthropic, gemini, local, none
  provider: none
  # API key (or use api_key_env to reference an environment variable)
  # api_key: "sk-..."
  # api_key_env: "OPENAI_API_KEY"
  # Model override (uses provider default if not set)
  # model: "gpt-4o"
  # Local model config (for Ollama/llama.cpp)
  # local:
  #   endpoint: "http://localhost:11434"
  #   model: "codellama"
  features:
    confidence_scoring: false
    smart_fixes: false
    explanations: false
    false_positive_reduction: false
"#;

    content.push_str(ai_block);
    std::fs::write(&config_path, content)?;

    Ok(())
}
