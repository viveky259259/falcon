use std::path::PathBuf;

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
    use hf_hub::api::sync::Api;

    let api = Api::new()?;
    let repo = api.model(cfg.model_id.clone());
    eprintln!(
        "falcon: ensuring embedded model {} ({})",
        cfg.model_id, cfg.model_file
    );
    repo.get(&cfg.model_file).map_err(|err| {
        anyhow::anyhow!(
            "failed to resolve model file '{}' from '{}': {err}. \
Pre-download the file into the Hugging Face cache or update ai.embedded.model_id/model_file.",
            cfg.model_file,
            cfg.model_id
        )
    })
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
