use crate::ai::config::EmbeddedModelConfig;
use crate::ai::local::completer::{Completer, GenOpts};
use crate::ai::local::model_cache::ensure_model;
use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;

const TOKENIZER_MODEL_ID: &str = "Qwen/Qwen2.5-0.5B-Instruct";
const TOKENIZER_FILE: &str = "tokenizer.json";
const DEFAULT_EOS_TOKEN: &str = "<|im_end|>";

pub struct LocalEngine {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    eos_token_id: Option<u32>,
}

impl LocalEngine {
    pub fn load(cfg: &EmbeddedModelConfig) -> Result<Self> {
        let model_path = ensure_model(cfg)?;
        let tokenizer_path = ensure_tokenizer()?;
        let device = Device::Cpu;

        let mut file = std::fs::File::open(&model_path).map_err(|err| {
            anyhow::anyhow!(
                "failed to open GGUF model '{}': {err}",
                model_path.display()
            )
        })?;
        let content =
            candle_core::quantized::gguf_file::Content::read(&mut file).map_err(|err| {
                anyhow::anyhow!(
                    "failed to read GGUF model '{}': {err}",
                    model_path.display()
                )
            })?;
        let model = ModelWeights::from_gguf(content, &mut file, &device).map_err(|err| {
            anyhow::anyhow!(
                "failed to load GGUF model '{}': {err}",
                model_path.display()
            )
        })?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|err| {
            anyhow::anyhow!(
                "failed to load tokenizer '{}': {err}",
                tokenizer_path.display()
            )
        })?;
        let eos_token_id = tokenizer.token_to_id(DEFAULT_EOS_TOKEN);

        Ok(Self {
            model,
            tokenizer,
            device,
            eos_token_id,
        })
    }
}

impl Completer for LocalEngine {
    fn complete(&mut self, prompt: &str, opts: &GenOpts) -> Result<String> {
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|err| anyhow::anyhow!("tokenize failed: {err}"))?;
        let mut tokens = encoding.get_ids().to_vec();
        let mut output_tokens = Vec::new();
        let mut logits_processor = LogitsProcessor::new(42, Some(opts.temperature as f64), None);

        for index in 0..opts.max_tokens {
            let context = if index == 0 {
                tokens.as_slice()
            } else {
                &tokens[tokens.len() - 1..]
            };
            let input = Tensor::new(context, &self.device)?.unsqueeze(0)?;
            let logits = self
                .model
                .forward(&input, tokens.len() - context.len())
                .map_err(|err| anyhow::anyhow!("embedded inference failed: {err}"))?;
            let logits = logits.squeeze(0)?;
            let next_token = logits_processor
                .sample(&logits)
                .map_err(|err| anyhow::anyhow!("sampling failed: {err}"))?;

            if Some(next_token) == self.eos_token_id {
                break;
            }

            tokens.push(next_token);
            output_tokens.push(next_token);

            let partial = self
                .tokenizer
                .decode(&output_tokens, true)
                .map_err(|err| anyhow::anyhow!("decode failed: {err}"))?;
            if contains_stop(&partial, &opts.stop) {
                return Ok(truncate_at_stop(partial, &opts.stop));
            }
        }

        self.tokenizer
            .decode(&output_tokens, true)
            .map_err(|err| anyhow::anyhow!("decode failed: {err}"))
    }
}

fn ensure_tokenizer() -> Result<std::path::PathBuf> {
    let api = hf_hub::api::sync::Api::new()?;
    eprintln!(
        "falcon: ensuring embedded tokenizer {} ({})",
        TOKENIZER_MODEL_ID, TOKENIZER_FILE
    );
    api.model(TOKENIZER_MODEL_ID.to_string())
        .get(TOKENIZER_FILE)
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to resolve tokenizer file '{}' from '{}': {err}. \
Pre-download the file into the Hugging Face cache or configure network access.",
                TOKENIZER_FILE,
                TOKENIZER_MODEL_ID
            )
        })
}

fn contains_stop(text: &str, stops: &[String]) -> bool {
    stops
        .iter()
        .any(|stop| !stop.is_empty() && text.contains(stop))
}

fn truncate_at_stop(mut text: String, stops: &[String]) -> String {
    if let Some(index) = stops
        .iter()
        .filter(|stop| !stop.is_empty())
        .filter_map(|stop| text.find(stop))
        .min()
    {
        text.truncate(index);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::local::completer::GenOpts;

    #[test]
    fn stop_detection_ignores_empty_stops() {
        let stops = vec![String::new(), "\n\n".to_string()];

        assert!(contains_stop("one\n\ntwo", &stops));
        assert!(!contains_stop("one\ntwo", &stops));
    }

    #[test]
    fn truncates_at_earliest_stop() {
        let stops = vec!["END".to_string(), "\n\n".to_string()];

        assert_eq!(
            truncate_at_stop("keep\n\nremove END".to_string(), &stops),
            "keep"
        );
    }

    #[test]
    #[ignore = "downloads the embedded model/tokenizer and runs local inference"]
    fn loads_and_emits_text() {
        let cfg = EmbeddedModelConfig::default();
        let mut engine = LocalEngine::load(&cfg).expect("load model");
        let output = engine
            .complete(
                "Reply with only this JSON: {\"is_real\": true, \"confidence\": 50, \"rationale\": \"ok\"}",
                &GenOpts::default(),
            )
            .expect("complete");

        assert!(!output.trim().is_empty());
    }
}
