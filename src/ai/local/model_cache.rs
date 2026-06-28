use std::path::PathBuf;

#[cfg(feature = "ai-local")]
const TOKENIZER_FILE: &str = "tokenizer.json";

pub fn model_cache_dir(model_id: &str, home: Option<&str>, xdg: Option<&str>) -> PathBuf {
    let base = match xdg {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(home.unwrap_or(".")).join(".cache"),
    };
    base.join("falcon")
        .join("models")
        .join(model_id.replace('/', "_"))
}

#[cfg(feature = "ai-local")]
pub fn ensure_model(cfg: &crate::ai::config::EmbeddedModelConfig) -> anyhow::Result<PathBuf> {
    let api = falcon_hf_api(&cfg.model_id)?;
    let repo = api.model(cfg.model_id.clone());
    eprintln!(
        "falcon: ensuring embedded model {} ({})",
        cfg.model_id, cfg.model_file
    );
    repo.get(&cfg.model_file).map_err(|err| {
        anyhow::anyhow!(
            "failed to resolve model file '{}' from '{}': {err}. \
Pre-download the file into the Falcon model cache or update ai.embedded.model_id/model_file.",
            cfg.model_file,
            cfg.model_id
        )
    })
}

#[cfg(feature = "ai-local")]
pub fn ensure_tokenizer(model_id: &str) -> anyhow::Result<PathBuf> {
    let api = falcon_hf_api(model_id)?;
    eprintln!(
        "falcon: ensuring embedded tokenizer {} ({})",
        model_id, TOKENIZER_FILE
    );
    api.model(model_id.to_string())
        .get(TOKENIZER_FILE)
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to resolve tokenizer file '{}' from '{}': {err}. \
Pre-download the file into the Falcon model cache or configure network access.",
                TOKENIZER_FILE,
                model_id
            )
        })
}

#[cfg(feature = "ai-local")]
fn falcon_hf_api(model_id: &str) -> anyhow::Result<hf_hub::api::sync::Api> {
    let home = dirs::home_dir().and_then(|path| path.to_str().map(str::to_string));
    let xdg = std::env::var("XDG_CACHE_HOME").ok();
    let cache_dir = model_cache_dir(model_id, home.as_deref(), xdg.as_deref());
    hf_hub::api::sync::ApiBuilder::from_env()
        .with_cache_dir(cache_dir)
        .build()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_xdg_cache_home() {
        let path = model_cache_dir(
            "Qwen/Qwen2.5-0.5B-Instruct-GGUF",
            Some("/home/u"),
            Some("/xdg"),
        );
        assert_eq!(
            path,
            PathBuf::from("/xdg/falcon/models/Qwen_Qwen2.5-0.5B-Instruct-GGUF")
        );
    }

    #[test]
    fn falls_back_to_home_cache() {
        let path = model_cache_dir("a/b", Some("/home/u"), None);
        assert_eq!(path, PathBuf::from("/home/u/.cache/falcon/models/a_b"));
    }

    #[test]
    fn empty_xdg_falls_back_to_home_cache() {
        let path = model_cache_dir("a/b", Some("/home/u"), Some(""));
        assert_eq!(path, PathBuf::from("/home/u/.cache/falcon/models/a_b"));
    }
}
