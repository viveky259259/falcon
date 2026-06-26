use crate::ai::local::completer::{Completer, GenOpts};
use anyhow::Result;

pub struct LocalEngine;

impl Completer for LocalEngine {
    fn complete(&mut self, _prompt: &str, _opts: &GenOpts) -> Result<String> {
        anyhow::bail!("embedded local inference engine is not implemented yet")
    }
}
