use anyhow::Result;

/// Generation options for a single completion call.
#[derive(Debug, Clone)]
pub struct GenOpts {
    pub max_tokens: usize,
    pub temperature: f32,
    pub stop: Vec<String>,
}

impl Default for GenOpts {
    fn default() -> Self {
        Self { max_tokens: 128, temperature: 0.0, stop: Vec::new() }
    }
}

/// The inference seam. Anything that can turn a prompt into text.
pub trait Completer {
    fn complete(&mut self, prompt: &str, opts: &GenOpts) -> Result<String>;
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A fake used across triage tests: returns canned responses in order,
    /// and an Err once the queue is exhausted if `fail_when_empty` is set.
    pub struct FakeCompleter {
        pub responses: Vec<String>,
        pub calls: usize,
        pub fail_when_empty: bool,
    }

    impl FakeCompleter {
        pub fn new(responses: Vec<&str>) -> Self {
            Self { responses: responses.into_iter().map(String::from).collect(), calls: 0, fail_when_empty: false }
        }
    }

    impl Completer for FakeCompleter {
        fn complete(&mut self, _prompt: &str, _opts: &GenOpts) -> Result<String> {
            let idx = self.calls;
            self.calls += 1;
            match self.responses.get(idx) {
                Some(r) => Ok(r.clone()),
                None if self.fail_when_empty => anyhow::bail!("no more canned responses"),
                None => Ok(String::new()),
            }
        }
    }

    #[test]
    fn fake_completer_returns_canned_then_counts_calls() {
        let mut f = FakeCompleter::new(vec!["a", "b"]);
        assert_eq!(f.complete("p", &GenOpts::default()).unwrap(), "a");
        assert_eq!(f.complete("p", &GenOpts::default()).unwrap(), "b");
        assert_eq!(f.calls, 2);
    }
}
