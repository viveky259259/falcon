#[cfg(feature = "ai-local")]
use crate::ai::config::EmbeddedModelConfig;
#[cfg(feature = "ai-local")]
use crate::ai::local::completer::{Completer, GenOpts};
#[cfg(feature = "ai-local")]
use crate::ai::local::model_cache::ensure_model;
#[cfg(feature = "ai-local")]
use anyhow::Result;
#[cfg(feature = "ai-local")]
use candle_core::{Device, Tensor};
#[cfg(feature = "ai-local")]
use candle_transformers::generation::LogitsProcessor;
#[cfg(feature = "ai-local")]
use candle_transformers::models::quantized_qwen2::ModelWeights;
#[cfg(feature = "ai-local")]
use tokenizers::Tokenizer;

#[cfg(feature = "ai-local")]
pub struct LocalEngine {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
}

#[cfg(feature = "ai-local")]
impl LocalEngine {
    /// Load the configured quantized model from cache (downloading on first use).
    pub fn load(cfg: &EmbeddedModelConfig) -> Result<Self> {
        let model_path = ensure_model(cfg)?;
        let device = Device::Cpu;

        let mut file = std::fs::File::open(&model_path)?;
        let content = candle_core::quantized::gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow::anyhow!("failed to read GGUF '{}': {e}", model_path.display()))?;
        let model = ModelWeights::from_gguf(content, &mut file, &device)?;

        let tok_path = hf_hub::api::sync::Api::new()?
            .model("Qwen/Qwen2.5-0.5B-Instruct".to_string())
            .get("tokenizer.json")?;
        let tokenizer = Tokenizer::from_file(tok_path)
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {e}"))?;

        Ok(Self { model, tokenizer, device })
    }
}

#[cfg(feature = "ai-local")]
impl Completer for LocalEngine {
    fn complete(&mut self, prompt: &str, opts: &GenOpts) -> Result<String> {
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("tokenize failed: {e}"))?;
        let mut tokens: Vec<u32> = encoding.get_ids().to_vec();

        let mut logits_processor = LogitsProcessor::new(42, Some(opts.temperature as f64), None);
        let mut out_tokens: Vec<u32> = Vec::new();
        let eos = self.tokenizer.token_to_id("<|im_end|>");

        for index in 0..opts.max_tokens {
            let context =
                if index == 0 { &tokens[..] } else { &tokens[tokens.len() - 1..] };
            let input = Tensor::new(context, &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, tokens.len() - context.len())?;
            let logits = logits.squeeze(0)?;
            let next = logits_processor.sample(&logits)?;
            if Some(next) == eos {
                break;
            }
            tokens.push(next);
            out_tokens.push(next);
        }

        let text = self
            .tokenizer
            .decode(&out_tokens, true)
            .map_err(|e| anyhow::anyhow!("decode failed: {e}"))?;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "ai-local")]
    use super::*;
    #[cfg(feature = "ai-local")]
    use crate::ai::config::EmbeddedModelConfig;
    #[cfg(feature = "ai-local")]
    use crate::ai::local::completer::GenOpts;

    #[test]
    #[cfg(feature = "ai-local")]
    #[ignore = "downloads ~350MB model; run manually with --features ai-local -- --ignored"]
    fn loads_and_emits_json_like_text() {
        let cfg = EmbeddedModelConfig::default();
        let mut engine = LocalEngine::load(&cfg).expect("load model");
        let out = engine
            .complete(
                "Reply with JSON: {\"is_real\": true, \"confidence\": 50, \"rationale\": \"x\"}",
                &GenOpts::default(),
            )
            .expect("complete");
        assert!(out.contains('{'));
    }
}
