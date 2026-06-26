use crate::ai::config::EmbeddedModelConfig;
use crate::ai::local::completer::{Completer, GenOpts};
use anyhow::Result;

pub struct LocalEngine;

impl LocalEngine {
    pub fn load(_cfg: &EmbeddedModelConfig) -> Result<Self> {
        Ok(Self)
    }
}

impl Completer for LocalEngine {
    fn complete(&mut self, _prompt: &str, _opts: &GenOpts) -> Result<String> {
        anyhow::bail!("embedded local inference engine is not implemented yet")
    }
}
