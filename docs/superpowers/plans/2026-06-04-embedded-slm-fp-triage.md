# Embedded SLM False-Positive Triage — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an in-process small language model (candle) that triages each rule finding as real-issue vs. false-positive, emitting `{is_real, confidence, rationale}`, exposed as `falcon x ai triage <path>`.

**Architecture:** A `Completer` trait is the seam between orchestration and inference. All pure logic (prompt building, verdict parsing, code-window extraction, cache-path resolution, triage orchestration) compiles in the **default build** and is unit-tested with a fake `Completer` — no model download. Only the candle-backed `LocalEngine` and the `hf-hub` weight download are gated behind the `ai-local` Cargo feature. The CLI command exists in both builds; without `ai-local` it prints a "rebuild with `--features ai-local`" message.

**Tech Stack:** Rust 2021, candle (`candle-core`, `candle-transformers`), `tokenizers`, `hf-hub`, `dirs`, `serde`/`serde_json`, `anyhow`. Default model: `Qwen2.5-0.5B-Instruct` Q4 GGUF.

**Spec:** `docs/superpowers/specs/2026-06-04-embedded-slm-fp-triage-design.md`

---

## File Structure

| File | Responsibility | Gated? |
|---|---|---|
| `Cargo.toml` | `ai-local` feature + optional deps | n/a |
| `src/ai/config.rs` (modify) | `AiProvider::Embedded`, `EmbeddedModelConfig` | no |
| `src/ai/local/mod.rs` (create) | module root, re-exports | no (engine re-export gated) |
| `src/ai/local/completer.rs` (create) | `Completer` trait, `GenOpts` | no |
| `src/ai/local/prompt.rs` (create) | `build_triage_prompt` | no |
| `src/ai/local/triage.rs` (create) | `TriageVerdict`, code window, parse, `triage_issues` | no |
| `src/ai/local/model_cache.rs` (create) | cache-path resolution (pure) + download (gated) | mixed |
| `src/ai/local/engine.rs` (create) | candle `LocalEngine` impl `Completer` | yes (`ai-local`) |
| `src/ai/mod.rs` (modify) | `pub mod local;` | no |
| `src/main.rs` (modify) | `AiAction::Triage` + dispatch (cfg stub) | mixed |

---

## Task 1: Add the `ai-local` Cargo feature and optional deps

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add optional dependencies and the feature**

In `Cargo.toml`, under `[dependencies]` add (pin exact versions to the latest compatible candle release at implementation time — these are known-good starting points):

```toml
candle-core = { version = "0.8", optional = true }
candle-transformers = { version = "0.8", optional = true }
tokenizers = { version = "0.20", optional = true }
hf-hub = { version = "0.3", optional = true }
dirs = { version = "5", optional = true }
```

Add a new section (after `[dependencies]`):

```toml
[features]
default = []
ai-local = ["dep:candle-core", "dep:candle-transformers", "dep:tokenizers", "dep:hf-hub", "dep:dirs"]
```

- [ ] **Step 2: Verify the default build is unchanged**

Run: `cargo build`
Expected: PASS, and no candle/tokenizers/hf-hub crates compiled (they are opt-in).

- [ ] **Step 3: Verify the feature build resolves**

Run: `cargo build --features ai-local`
Expected: PASS (downloads + compiles candle etc. on first run). If a pinned version fails to resolve, bump to the latest minor and retry.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add opt-in ai-local feature (candle, tokenizers, hf-hub)"
```

---

## Task 2: Config — `AiProvider::Embedded` and `EmbeddedModelConfig`

**Files:**
- Modify: `src/ai/config.rs`
- Test: `src/ai/config.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Add to the bottom of `src/ai/config.rs`:

```rust
#[cfg(test)]
mod embedded_tests {
    use super::*;

    #[test]
    fn embedded_config_has_sane_defaults() {
        let c = EmbeddedModelConfig::default();
        assert_eq!(c.model_id, "Qwen/Qwen2.5-0.5B-Instruct-GGUF");
        assert!(c.model_file.ends_with(".gguf"));
        assert_eq!(c.max_tokens, 128);
        assert_eq!(c.context_lines, 12);
        assert_eq!(c.max_issues, 100);
    }

    #[test]
    fn provider_embedded_parses_from_alias() {
        let p: AiProvider = serde_yaml::from_str("embedded").unwrap();
        assert_eq!(p, AiProvider::Embedded);
    }

    #[test]
    fn effective_model_for_embedded_uses_embedded_model_id() {
        let mut cfg = AiConfig::default();
        cfg.provider = AiProvider::Embedded;
        cfg.embedded = Some(EmbeddedModelConfig::default());
        assert_eq!(cfg.effective_model(), "Qwen/Qwen2.5-0.5B-Instruct-GGUF");
    }

    #[test]
    fn embedded_is_available_when_enabled_with_config() {
        let mut cfg = AiConfig::default();
        cfg.enabled = true;
        cfg.provider = AiProvider::Embedded;
        cfg.embedded = Some(EmbeddedModelConfig::default());
        assert!(cfg.is_available());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib embedded_tests`
Expected: FAIL — `EmbeddedModelConfig` and `AiProvider::Embedded` do not exist.

- [ ] **Step 3: Implement the config additions**

In `src/ai/config.rs`, add the `Embedded` variant to the enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[serde(alias = "openai")]
    OpenAi,
    Anthropic,
    Gemini,
    Local,
    #[serde(alias = "embedded")]
    Embedded,
    None,
}
```

Add the `embedded` field to `AiConfig` (next to `local`):

```rust
    #[serde(default)]
    pub embedded: Option<EmbeddedModelConfig>,
```

Add `embedded: None` to the `Default for AiConfig` impl.

Add the new struct (after `LocalModelConfig`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedModelConfig {
    #[serde(default = "default_embedded_model_id")]
    pub model_id: String,
    #[serde(default = "default_embedded_model_file")]
    pub model_file: String,
    #[serde(default = "default_embedded_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_embedded_context_lines")]
    pub context_lines: usize,
    #[serde(default = "default_embedded_max_issues")]
    pub max_issues: usize,
}

impl Default for EmbeddedModelConfig {
    fn default() -> Self {
        Self {
            model_id: default_embedded_model_id(),
            model_file: default_embedded_model_file(),
            max_tokens: default_embedded_max_tokens(),
            context_lines: default_embedded_context_lines(),
            max_issues: default_embedded_max_issues(),
        }
    }
}

fn default_embedded_model_id() -> String { "Qwen/Qwen2.5-0.5B-Instruct-GGUF".to_string() }
fn default_embedded_model_file() -> String { "qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string() }
fn default_embedded_max_tokens() -> usize { 128 }
fn default_embedded_context_lines() -> usize { 12 }
fn default_embedded_max_issues() -> usize { 100 }
```

Update `effective_model()` — add an arm before the closing brace of the match:

```rust
            AiProvider::Embedded => self
                .embedded
                .as_ref()
                .map(|e| e.model_id.clone())
                .unwrap_or_else(default_embedded_model_id),
```

Update `is_available()` — add an arm to its match:

```rust
            AiProvider::Embedded => self.embedded.is_some(),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib embedded_tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/ai/config.rs
git commit -m "feat(ai): add Embedded provider + EmbeddedModelConfig"
```

---

## Task 3: `Completer` trait, `GenOpts`, and a test fake

**Files:**
- Create: `src/ai/local/mod.rs`
- Create: `src/ai/local/completer.rs`
- Modify: `src/ai/mod.rs`

- [ ] **Step 1: Register the module**

In `src/ai/mod.rs` add:

```rust
pub mod local;
```

Create `src/ai/local/mod.rs`:

```rust
pub mod completer;
pub mod prompt;
pub mod triage;
pub mod model_cache;

#[cfg(feature = "ai-local")]
pub mod engine;
```

- [ ] **Step 2: Write the failing test**

Create `src/ai/local/completer.rs`:

```rust
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
```

Note: `mod tests` is declared `pub` so Task 7's tests (in `triage.rs`) can reuse `FakeCompleter` via `use crate::ai::local::completer::tests::FakeCompleter;`.

- [ ] **Step 3: Create empty stub files so the module compiles**

`mod.rs` references `prompt`, `triage`, `model_cache` which are filled in later tasks. Create empty stubs now:

```bash
printf '' > src/ai/local/prompt.rs
printf '' > src/ai/local/triage.rs
printf '' > src/ai/local/model_cache.rs
```

Run: `cargo test --lib completer`
Expected: PASS (1 test).

- [ ] **Step 4: Commit**

```bash
git add src/ai/mod.rs src/ai/local/
git commit -m "feat(ai): add Completer trait + GenOpts with test fake"
```

---

## Task 4: `build_triage_prompt`

**Files:**
- Modify: `src/ai/local/prompt.rs`
- Test: `src/ai/local/prompt.rs`

- [ ] **Step 1: Write the failing test**

Replace the empty `src/ai/local/prompt.rs` with:

```rust
use crate::reporters::Issue;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reporters::Severity;
    use std::path::PathBuf;

    fn sample_issue() -> Issue {
        Issue {
            rule: "unused-code".to_string(),
            message: "'foo' appears to be unused".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("lib/foo.dart"),
            line: 10,
            column: 3,
        }
    }

    #[test]
    fn prompt_contains_rule_message_location_and_code() {
        let code = "void bar() {}\nvoid foo() {}\n";
        let p = build_triage_prompt(&sample_issue(), code);
        assert!(p.contains("unused-code"));
        assert!(p.contains("appears to be unused"));
        assert!(p.contains("lib/foo.dart"));
        assert!(p.contains("void foo()"));
        // Must instruct JSON-only output with the three required keys.
        assert!(p.contains("is_real"));
        assert!(p.contains("confidence"));
        assert!(p.contains("rationale"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib prompt`
Expected: FAIL — `build_triage_prompt` not found.

- [ ] **Step 3: Implement `build_triage_prompt`**

Add above the `#[cfg(test)]` block in `src/ai/local/prompt.rs`:

```rust
/// Build the FP-triage prompt for one finding plus its surrounding code.
pub fn build_triage_prompt(issue: &Issue, code_window: &str) -> String {
    format!(
        "You are a senior Flutter/Dart static-analysis reviewer. A linter flagged a \
finding. Decide whether it is a REAL problem worth a developer's attention, or a \
FALSE POSITIVE given the surrounding code.\n\n\
Respond with ONE JSON object and nothing else:\n\
{{\"is_real\": true|false, \"confidence\": 0-100, \"rationale\": \"one short sentence\"}}\n\n\
Few-shot example:\n\
Finding: rule=unused-code at a.dart:1 — '_internal' appears to be unused\n\
Code:\nclass A {{ void _internal() {{}} }}\n\
Answer: {{\"is_real\": true, \"confidence\": 80, \"rationale\": \"Private method has no \
references in this file.\"}}\n\n\
Now the actual finding:\n\
Finding: rule={rule} at {file}:{line} — {message}\n\
Code:\n{code}\n\
Answer:",
        rule = issue.rule,
        file = issue.file.display(),
        line = issue.line,
        message = issue.message,
        code = code_window,
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib prompt`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add src/ai/local/prompt.rs
git commit -m "feat(ai): add build_triage_prompt for FP triage"
```

---

## Task 5: Code-window extraction

**Files:**
- Modify: `src/ai/local/triage.rs`
- Test: `src/ai/local/triage.rs`

- [ ] **Step 1: Write the implementation + tests together**

Put this in `src/ai/local/triage.rs` (currently empty):

```rust
/// Extract a window of source lines centered on `line` (1-based), with `radius`
/// lines of context on each side. Clamps to file bounds. Returns the window text.
pub fn extract_code_window(source: &str, line: usize, radius: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let center = line.saturating_sub(1).min(lines.len() - 1);
    let start = center.saturating_sub(radius);
    let end = (center + radius).min(lines.len() - 1);
    lines[start..=end].join("\n")
}

#[cfg(test)]
mod window_tests {
    use super::*;

    const SRC: &str = "l1\nl2\nl3\nl4\nl5";

    #[test]
    fn window_centered_in_middle() {
        assert_eq!(extract_code_window(SRC, 3, 1), "l2\nl3\nl4");
    }

    #[test]
    fn window_clamps_at_start() {
        assert_eq!(extract_code_window(SRC, 1, 2), "l1\nl2\nl3");
    }

    #[test]
    fn window_clamps_at_end() {
        assert_eq!(extract_code_window(SRC, 5, 2), "l3\nl4\nl5");
    }

    #[test]
    fn empty_source_yields_empty() {
        assert_eq!(extract_code_window("", 1, 3), "");
    }

    #[test]
    fn line_beyond_eof_clamps_to_last() {
        assert_eq!(extract_code_window(SRC, 999, 1), "l4\nl5");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib window_tests`
Expected: PASS (5 tests).

- [ ] **Step 3: Commit**

```bash
git add src/ai/local/triage.rs
git commit -m "feat(ai): add code-window extraction with boundary clamping"
```

---

## Task 6: Verdict parsing

**Files:**
- Modify: `src/ai/local/triage.rs`
- Test: `src/ai/local/triage.rs`

- [ ] **Step 1: Write the implementation + tests**

Append to `src/ai/local/triage.rs`:

```rust
use serde::Deserialize;

/// Raw shape the model is asked to emit.
#[derive(Debug, Deserialize)]
struct RawVerdict {
    is_real: bool,
    confidence: u8,
    rationale: String,
}

/// Parsed outcome of triaging one finding. `degraded` means we could not parse a
/// model verdict and fell back to "uncertain".
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVerdict {
    pub is_real: bool,
    pub confidence: u8,
    pub rationale: String,
    pub degraded: bool,
}

/// Parse a model completion into a verdict. Extracts the first balanced JSON
/// object even if surrounded by prose. On any failure, returns an "uncertain"
/// degraded verdict (never errors).
pub fn parse_verdict(raw: &str) -> ParsedVerdict {
    if let Some(json) = first_json_object(raw) {
        if let Ok(v) = serde_json::from_str::<RawVerdict>(&json) {
            return ParsedVerdict {
                is_real: v.is_real,
                confidence: v.confidence.min(100),
                rationale: v.rationale,
                degraded: false,
            };
        }
    }
    ParsedVerdict {
        is_real: true, // fail safe: do not silently hide a finding
        confidence: 0,
        rationale: "Model output could not be parsed; treated as uncertain.".to_string(),
        degraded: true,
    }
}

/// Return the first balanced `{...}` substring, or None.
fn first_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let mut depth = 0usize;
    for (i, ch) in s[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(s[start..start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn parses_clean_json() {
        let v = parse_verdict(r#"{"is_real": false, "confidence": 72, "rationale": "barrel re-export"}"#);
        assert!(!v.is_real);
        assert_eq!(v.confidence, 72);
        assert_eq!(v.rationale, "barrel re-export");
        assert!(!v.degraded);
    }

    #[test]
    fn parses_json_wrapped_in_prose() {
        let v = parse_verdict("Sure! Here is my answer:\n{\"is_real\": true, \"confidence\": 90, \"rationale\": \"x\"} Hope that helps.");
        assert!(v.is_real);
        assert_eq!(v.confidence, 90);
        assert!(!v.degraded);
    }

    #[test]
    fn clamps_confidence_over_100() {
        let v = parse_verdict(r#"{"is_real": true, "confidence": 250, "rationale": "x"}"#);
        assert_eq!(v.confidence, 100);
    }

    #[test]
    fn malformed_output_is_degraded_uncertain() {
        let v = parse_verdict("I think this is probably fine, no JSON here");
        assert!(v.degraded);
        assert_eq!(v.confidence, 0);
        assert!(v.is_real, "fail-safe keeps finding visible");
    }

    #[test]
    fn empty_output_is_degraded() {
        let v = parse_verdict("");
        assert!(v.degraded);
    }
}
```

Note: `serde_json::from_str::<u8>` rejects values > 255, so `"confidence": 250` parses (then clamps to 100); values > 255 fall through to the degraded path, which is acceptable.

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib parse_tests`
Expected: PASS (5 tests).

- [ ] **Step 3: Commit**

```bash
git add src/ai/local/triage.rs
git commit -m "feat(ai): robust verdict parsing with degraded fallback"
```

---

## Task 7: `triage_issues` orchestration

**Files:**
- Modify: `src/ai/local/triage.rs`
- Test: `src/ai/local/triage.rs`

- [ ] **Step 1: Write the implementation + tests**

Append to `src/ai/local/triage.rs`:

```rust
use crate::ai::config::EmbeddedModelConfig;
use crate::ai::local::completer::{Completer, GenOpts};
use crate::ai::local::prompt::build_triage_prompt;
use crate::reporters::Issue;
use std::path::Path;

/// Final triage result for one finding.
#[derive(Debug, Clone)]
pub struct TriageVerdict {
    pub issue: Issue,
    pub is_real: bool,
    pub confidence: u8,
    pub rationale: String,
    pub degraded: bool,
}

/// Outcome of a triage run: verdicts plus whether the issue list was truncated.
pub struct TriageRun {
    pub verdicts: Vec<TriageVerdict>,
    pub truncated_from: Option<usize>, // Some(original_len) if capped
}

/// Triage findings sequentially through `engine`. Reads code windows relative to
/// `project_root`. Applies `cfg.max_issues` cap (recording truncation). A failing
/// completion degrades that single verdict to "uncertain" and continues.
pub fn triage_issues(
    issues: &[Issue],
    project_root: &Path,
    engine: &mut dyn Completer,
    cfg: &EmbeddedModelConfig,
) -> TriageRun {
    let original_len = issues.len();
    let truncated_from = if original_len > cfg.max_issues { Some(original_len) } else { None };
    let opts = GenOpts { max_tokens: cfg.max_tokens, temperature: 0.0, stop: vec!["\n\n".to_string()] };

    let verdicts = issues
        .iter()
        .take(cfg.max_issues)
        .map(|issue| {
            let abs = project_root.join(&issue.file);
            let source = std::fs::read_to_string(&abs)
                .or_else(|_| std::fs::read_to_string(&issue.file))
                .unwrap_or_default();
            let window = extract_code_window(&source, issue.line, cfg.context_lines);
            let prompt = build_triage_prompt(issue, &window);
            let parsed = match engine.complete(&prompt, &opts) {
                Ok(text) => parse_verdict(&text),
                Err(_) => ParsedVerdict {
                    is_real: true,
                    confidence: 0,
                    rationale: "Inference failed; treated as uncertain.".to_string(),
                    degraded: true,
                },
            };
            TriageVerdict {
                issue: issue.clone(),
                is_real: parsed.is_real,
                confidence: parsed.confidence,
                rationale: parsed.rationale,
                degraded: parsed.degraded,
            }
        })
        .collect();

    TriageRun { verdicts, truncated_from }
}

#[cfg(test)]
mod orchestration_tests {
    use super::*;
    use crate::ai::local::completer::tests::FakeCompleter;
    use crate::reporters::Severity;
    use std::path::PathBuf;

    fn issue(line: usize) -> Issue {
        Issue {
            rule: "unused-code".to_string(),
            message: "'x' appears to be unused".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("does_not_exist.dart"),
            line,
            column: 1,
        }
    }

    #[test]
    fn maps_each_issue_to_a_verdict() {
        let issues = vec![issue(1), issue(2)];
        let mut fake = FakeCompleter::new(vec![
            r#"{"is_real": true, "confidence": 80, "rationale": "real"}"#,
            r#"{"is_real": false, "confidence": 60, "rationale": "noise"}"#,
        ]);
        let run = triage_issues(&issues, Path::new("."), &mut fake, &EmbeddedModelConfig::default());
        assert_eq!(run.verdicts.len(), 2);
        assert!(run.verdicts[0].is_real);
        assert!(!run.verdicts[1].is_real);
        assert!(run.truncated_from.is_none());
    }

    #[test]
    fn inference_error_degrades_single_verdict() {
        let issues = vec![issue(1)];
        let mut fake = FakeCompleter { responses: vec![], calls: 0, fail_when_empty: true };
        let run = triage_issues(&issues, Path::new("."), &mut fake, &EmbeddedModelConfig::default());
        assert_eq!(run.verdicts.len(), 1);
        assert!(run.verdicts[0].degraded);
    }

    #[test]
    fn applies_max_issues_cap_and_records_truncation() {
        let issues: Vec<Issue> = (1..=5).map(issue).collect();
        let cfg = EmbeddedModelConfig { max_issues: 2, ..EmbeddedModelConfig::default() };
        let mut fake = FakeCompleter::new(vec![
            r#"{"is_real": true, "confidence": 1, "rationale": "a"}"#,
            r#"{"is_real": true, "confidence": 1, "rationale": "b"}"#,
        ]);
        let run = triage_issues(&issues, Path::new("."), &mut fake, &cfg);
        assert_eq!(run.verdicts.len(), 2);
        assert_eq!(run.truncated_from, Some(5));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib orchestration_tests`
Expected: PASS (3 tests).

- [ ] **Step 3: Run the full pure-logic suite**

Run: `cargo test --lib local`
Expected: PASS (completer + prompt + window + parse + orchestration tests).

- [ ] **Step 4: Commit**

```bash
git add src/ai/local/triage.rs
git commit -m "feat(ai): triage_issues orchestration with cap + degrade path"
```

---

## Task 8: Cache-path resolution (pure) + gated download

**Files:**
- Modify: `src/ai/local/model_cache.rs`
- Test: `src/ai/local/model_cache.rs`

- [ ] **Step 1: Write the pure path logic + tests (always compiled)**

Replace empty `src/ai/local/model_cache.rs` with:

```rust
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
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib model_cache`
Expected: PASS (3 tests).

- [ ] **Step 3: Add the gated download helper**

Append to `src/ai/local/model_cache.rs`:

```rust
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
```

Note: GGUF files embed their own tokenizer metadata; the candle quantized loader in Task 9 fetches `tokenizer.json` separately from the base instruct repo. If a non-GGUF model is configured later, extend this to also fetch the tokenizer here.

- [ ] **Step 4: Verify both builds compile**

Run: `cargo build` then `cargo build --features ai-local`
Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ai/local/model_cache.rs
git commit -m "feat(ai): cache-path resolution + gated hf-hub model download"
```

---

## Task 9: `LocalEngine` (candle, gated)

**Files:**
- Create: `src/ai/local/engine.rs`
- Test: `src/ai/local/engine.rs` (`#[ignore]` real-inference test)

> This task only compiles under `--features ai-local`. The candle API surface
> shifts between releases; treat the code below as a close-to-correct skeleton and
> adjust import paths/method names to the pinned candle version. The public
> contract (`LocalEngine::load` + `impl Completer`) must not change.

- [ ] **Step 1: Create the engine**

Create `src/ai/local/engine.rs`:

```rust
use crate::ai::config::EmbeddedModelConfig;
use crate::ai::local::completer::{Completer, GenOpts};
use crate::ai::local::model_cache::ensure_model;
use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;

pub struct LocalEngine {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
}

impl LocalEngine {
    /// Load the configured quantized model from cache (downloading on first use).
    pub fn load(cfg: &EmbeddedModelConfig) -> Result<Self> {
        let model_path = ensure_model(cfg)?;
        let device = Device::Cpu; // Metal/CUDA can be added behind further cfg later.

        let mut file = std::fs::File::open(&model_path)?;
        let content = candle_core::quantized::gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow::anyhow!("failed to read GGUF '{}': {e}", model_path.display()))?;
        let model = ModelWeights::from_gguf(content, &mut file, &device)?;

        // The q4 GGUF repo may not ship tokenizer.json; fetch it from the base
        // instruct repo. Adjust the repo id if the GGUF repo includes it.
        let tok_path = hf_hub::api::sync::Api::new()?
            .model("Qwen/Qwen2.5-0.5B-Instruct".to_string())
            .get("tokenizer.json")?;
        let tokenizer = Tokenizer::from_file(tok_path)
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {e}"))?;

        Ok(Self { model, tokenizer, device })
    }
}

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
            let context = if index == 0 { &tokens[..] } else { &tokens[tokens.len() - 1..] };
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
    use super::*;

    #[test]
    #[ignore = "downloads ~350MB model; run manually with --features ai-local -- --ignored"]
    fn loads_and_emits_json_like_text() {
        let cfg = EmbeddedModelConfig::default();
        let mut engine = LocalEngine::load(&cfg).expect("load model");
        let out = engine
            .complete("Reply with JSON: {\"is_real\": true, \"confidence\": 50, \"rationale\": \"x\"}", &GenOpts::default())
            .expect("complete");
        assert!(out.contains('{'));
    }
}
```

- [ ] **Step 2: Build under the feature**

Run: `cargo build --features ai-local`
Expected: PASS. If candle import paths differ, fix them (e.g. `quantized_qwen2` module name or `from_gguf` signature) until it compiles. Do not change the `Completer` signature.

- [ ] **Step 3: Smoke-test inference manually (optional, networked)**

Run: `cargo test --features ai-local --lib engine -- --ignored`
Expected: downloads the model once, then PASS.

- [ ] **Step 4: Commit**

```bash
git add src/ai/local/engine.rs
git commit -m "feat(ai): candle LocalEngine implementing Completer (ai-local)"
```

---

## Task 10: CLI — `ai triage` command + output rendering

**Files:**
- Modify: `src/ai/local/triage.rs` (add renderers)
- Modify: `src/main.rs` (`AiAction` enum; dispatch in `Commands::Ai { action } => match action`)

- [ ] **Step 1: Add renderers + test**

Append to `src/ai/local/triage.rs`:

```rust
use colored::Colorize;

/// Render a triage run to stdout (human-readable).
pub fn print_triage_run(run: &TriageRun) {
    if let Some(orig) = run.truncated_from {
        eprintln!(
            "{} triaged first {} of {} findings (raise embedded.max_issues to cover all)",
            "note:".yellow().bold(),
            run.verdicts.len(),
            orig
        );
    }
    for v in &run.verdicts {
        let verdict = if v.degraded {
            "uncertain".bright_black()
        } else if v.is_real {
            "real".red()
        } else {
            "false-positive".green()
        };
        println!(
            "  {} {}:{}  [{}]  {}% — {}",
            v.issue.rule.bright_cyan(),
            v.issue.file.display(),
            v.issue.line,
            verdict,
            v.confidence,
            v.rationale
        );
    }
}

/// Serialize a triage run to a pretty JSON string (machine output).
pub fn triage_run_to_json(run: &TriageRun) -> String {
    let items: Vec<serde_json::Value> = run
        .verdicts
        .iter()
        .map(|v| {
            serde_json::json!({
                "rule": v.issue.rule,
                "file": v.issue.file.display().to_string(),
                "line": v.issue.line,
                "is_real": v.is_real,
                "confidence": v.confidence,
                "rationale": v.rationale,
                "degraded": v.degraded,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "truncated_from": run.truncated_from,
        "verdicts": items,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::reporters::Severity;
    use std::path::PathBuf;

    #[test]
    fn json_contains_expected_fields() {
        let run = TriageRun {
            truncated_from: Some(3),
            verdicts: vec![TriageVerdict {
                issue: Issue {
                    rule: "unused-code".into(),
                    message: "m".into(),
                    severity: Severity::Warning,
                    file: PathBuf::from("a.dart"),
                    line: 7,
                    column: 1,
                },
                is_real: false,
                confidence: 65,
                rationale: "r".into(),
                degraded: false,
            }],
        };
        let json = triage_run_to_json(&run);
        assert!(json.contains("\"truncated_from\": 3"));
        assert!(json.contains("\"rule\": \"unused-code\""));
        assert!(json.contains("\"confidence\": 65"));
        assert!(json.contains("\"is_real\": false"));
    }
}
```

- [ ] **Step 2: Run the renderer test**

Run: `cargo test --lib render_tests`
Expected: PASS (1 test).

- [ ] **Step 3: Add the `Triage` subcommand variant**

In `src/main.rs`, inside `enum AiAction { ... }`, add a variant after `Status`:

```rust
    /// Triage findings as real vs. false-positive using the embedded SLM
    Triage {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
```

- [ ] **Step 4: Add dispatch with cfg stub**

In `src/main.rs`, inside `Commands::Ai { action } => match action { ... }`, add an arm after `AiAction::Status { .. } => { ... }`:

```rust
            AiAction::Triage { path, format } => {
                #[cfg(not(feature = "ai-local"))]
                {
                    let _ = (&path, &format);
                    println!("{} embedded AI is not compiled in.", "✗".red().bold());
                    println!("  Rebuild with: cargo build --release --features ai-local");
                }
                #[cfg(feature = "ai-local")]
                {
                    use falcon::ai::config::EmbeddedModelConfig;
                    use falcon::ai::local::engine::LocalEngine;
                    use falcon::ai::local::triage::{print_triage_run, triage_issues, triage_run_to_json};

                    let falcon_config = FalconConfig::load(&path)?;
                    let embedded_cfg = falcon_config
                        .ai
                        .embedded
                        .clone()
                        .unwrap_or_default();
                    let falcon = Falcon::new(falcon_config)?;
                    let report = falcon.analyze(&path)?;

                    let mut engine = LocalEngine::load(&embedded_cfg)?;
                    let run = triage_issues(&report.issues, &path, &mut engine, &embedded_cfg);

                    if format == "json" {
                        println!("{}", triage_run_to_json(&run));
                    } else {
                        print_triage_run(&run);
                    }
                }
            }
```

- [ ] **Step 5: Verify both builds compile**

Run: `cargo build` then `cargo build --features ai-local`
Expected: both PASS.

- [ ] **Step 6: Verify the feature-off command prints the rebuild hint**

Run: `cargo run -- x ai triage .`
Expected: prints "embedded AI is not compiled in" + the rebuild command.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/ai/local/triage.rs
git commit -m "feat(ai): wire `falcon x ai triage` command (cfg-gated engine)"
```

---

## Task 11: `ai status` shows the embedded provider + docs

**Files:**
- Modify: `src/main.rs` (the `AiAction::Setup` help line + `AiAction::Status` rendering)
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the Setup help line**

In `src/main.rs`, in `AiAction::Setup`, change the providers line:

```rust
                println!("  Supported providers: openai, anthropic, gemini, local (Ollama), embedded (in-process SLM)");
```

- [ ] **Step 2: Add an embedded line to Status**

In `AiAction::Status`, after the existing feature-toggle prints, add:

```rust
                if let Some(ref e) = ai.embedded {
                    println!();
                    println!("  Embedded model:");
                    println!("    Model id:   {}", e.model_id);
                    println!("    Max issues: {}", e.max_issues);
                    #[cfg(feature = "ai-local")]
                    println!("    Engine:     compiled in (ai-local)");
                    #[cfg(not(feature = "ai-local"))]
                    println!("    Engine:     NOT compiled (rebuild with --features ai-local)");
                }
```

- [ ] **Step 3: Document the feature in CLAUDE.md**

In `CLAUDE.md`, under `## Build & Test`, add after the `cargo run -- <args>` line:

```
cargo build --features ai-local                      # build with embedded SLM triage
cargo run --features ai-local -- x ai triage <path>    # triage findings (real vs false-positive)
```

- [ ] **Step 4: Verify**

Run: `cargo build && cargo run -- ai status`
Expected: PASS; status prints (embedded section only when configured).

- [ ] **Step 5: Commit**

```bash
git add src/main.rs CLAUDE.md
git commit -m "feat(ai): surface embedded provider in ai status + docs"
```

---

## Task 12: Final verification

- [ ] **Step 1: Full default test suite**

Run: `cargo test`
Expected: all existing tests + the new pure-logic tests PASS (no model download).

- [ ] **Step 2: Lint and format**

Run: `cargo clippy --all-targets && cargo clippy --all-targets --features ai-local && cargo fmt --check`
Expected: no warnings; formatting clean. Fix any findings, then re-run.

- [ ] **Step 3: Feature build + ignored inference test (networked, optional)**

Run: `cargo test --features ai-local -- --ignored`
Expected: downloads model once, engine test PASSES.

- [ ] **Step 4: Final commit (if fmt/clippy changed anything)**

```bash
git add -A
git commit -m "chore(ai): clippy + fmt for embedded triage slice"
```

---

## Self-Review Notes

- **Spec coverage:** engine (T9), model cache + download (T8), prompt (T4), triage orchestration + window + parse (T5–T7), config `Embedded`/`EmbeddedModelConfig` (T2), Cargo feature gate (T1), CLI command + feature-off stub (T10), output text+JSON (T10), `ai status`/docs (T11), per-issue error-degrade path (T6/T7), truncation warning (T7/T10) — all covered.
- **Refinement vs. spec:** the spec said the whole `local/` module is gated; the plan gates only `engine.rs` + the download fn so unit tests run in the default build. This honors the spec's "tests run with no model download" requirement and is the intended reading. The cache layout helper (`model_cache_dir`) is informational since `hf-hub` owns its own cache; noted in T8.
- **Type consistency:** `Completer::complete(&mut self, &str, &GenOpts)`, `GenOpts{max_tokens,temperature,stop}`, `ParsedVerdict{is_real,confidence,rationale,degraded}`, `TriageVerdict{issue,is_real,confidence,rationale,degraded}`, `TriageRun{verdicts,truncated_from}`, `EmbeddedModelConfig{model_id,model_file,max_tokens,context_lines,max_issues}` — used identically across T2–T11.
- **No placeholders:** every code step contains complete, runnable code; candle in T9 is flagged as version-sensitive with an explicit "adjust imports, keep the contract" instruction rather than a TODO.
