use std::path::PathBuf;

/// Resolve the cache directory for a given hf model id. Honors `XDG_CACHE_HOME`,
/// then falls back to `~/.cache`. The model id's '/' is replaced with '_' so it
/// is a single path segment under `falcon/models/`.
pub fn model_cache_dir(model_id: &str, home: Option<&str>, xdg: Option<&str>) -> PathBuf {
    let base = match xdg {
        Some(x) if !x.is_empty() => PathBuf::from(x),
        _ => PathBuf::from(home.unwrap_or(".")).join(".cache"),
    };
    base.join("falcon").join("models").join(model_id.replace('/', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_xdg_cache_home() {
        let p = model_cache_dir("Qwen/Qwen2.5-0.5B-Instruct-GGUF", Some("/home/u"), Some("/xdg"));
        assert_eq!(p, PathBuf::from("/xdg/falcon/models/Qwen_Qwen2.5-0.5B-Instruct-GGUF"));
    }

    #[test]
    fn falls_back_to_home_cache() {
        let p = model_cache_dir("a/b", Some("/home/u"), None);
        assert_eq!(p, PathBuf::from("/home/u/.cache/falcon/models/a_b"));
    }

    #[test]
    fn empty_xdg_falls_back() {
        let p = model_cache_dir("a/b", Some("/home/u"), Some(""));
        assert_eq!(p, PathBuf::from("/home/u/.cache/falcon/models/a_b"));
    }
}

/// Download (if missing) the model file for `cfg`, returning the local path to
/// the model file. Only compiled with the `ai-local` feature. `hf-hub` manages
/// its own on-disk cache; `model_cache_dir` above documents Falcon's intended
/// layout and is used for status/reporting.
#[cfg(feature = "ai-local")]
pub fn ensure_model(cfg: &crate::ai::config::EmbeddedModelConfig) -> anyhow::Result<PathBuf> {
    use hf_hub::api::sync::Api;

    let api = Api::new()?;
    let repo = api.model(cfg.model_id.clone());
    eprintln!("falcon: ensuring model {} ({})", cfg.model_id, cfg.model_file);
    let model_path = repo.get(&cfg.model_file).map_err(|e| {
        anyhow::anyhow!(
            "failed to download model file '{}' from '{}': {e}. \
You can pre-download it manually into the hf-hub cache.",
            cfg.model_file, cfg.model_id
        )
    })?;
    Ok(model_path)
}
