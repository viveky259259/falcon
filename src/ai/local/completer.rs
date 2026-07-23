use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct GenOpts {
    pub max_tokens: usize,
    pub temperature: f32,
    pub stop: Vec<String>,
}

impl Default for GenOpts {
    fn default() -> Self {
        Self {
            max_tokens: 128,
            temperature: 0.0,
            stop: Vec::new(),
        }
    }
}

pub trait Completer {
    fn complete(&mut self, prompt: &str, opts: &GenOpts) -> Result<String>;
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct FakeCompleter {
        pub responses: Vec<String>,
        pub calls: usize,
        pub fail_when_empty: bool,
    }

    impl FakeCompleter {
        pub fn new(responses: Vec<&str>) -> Self {
            Self {
                responses: responses.into_iter().map(String::from).collect(),
                calls: 0,
                fail_when_empty: false,
            }
        }
    }

    impl Completer for FakeCompleter {
        fn complete(&mut self, _prompt: &str, _opts: &GenOpts) -> Result<String> {
            let idx = self.calls;
            self.calls += 1;
            match self.responses.get(idx) {
                Some(response) => Ok(response.clone()),
                None if self.fail_when_empty => anyhow::bail!("no more canned responses"),
                None => Ok(String::new()),
            }
        }
    }

    #[test]
    fn fake_completer_returns_canned_then_counts_calls() {
        let mut fake = FakeCompleter::new(vec!["a", "b"]);
        assert_eq!(fake.complete("prompt", &GenOpts::default()).unwrap(), "a");
        assert_eq!(fake.complete("prompt", &GenOpts::default()).unwrap(), "b");
        assert_eq!(fake.calls, 2);
    }
}
