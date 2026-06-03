# Falcon Embedded AI — Slice 1: SLM False-Positive Triage

**Date:** 2026-06-04
**Status:** Approved design, pending implementation plan
**Author:** brainstormed with Claude Code

## Context

Falcon's current "AI" surface is heuristic, not model-backed:

- `src/ai/explain.rs`, `src/ai/fix.rs` — hardcoded explanation DB and pattern-matched fixes.
- `src/ai/confidence.rs` — heuristic confidence scoring for unused-code findings.
- `src/ai/config.rs` — real provider *config* scaffolding (OpenAI/Anthropic/Gemini/Ollama) but **no inference engine is wired up**: no `reqwest`, no embeddings, no local-model crate in `Cargo.toml`.

The broader goal ("make Falcon much better with AI, maybe RAG or SLM") is platform-sized. It has been decomposed into layered sub-projects, each with its own spec → plan → build cycle:

- **L1 — Inference layer** (talk to a model)
- **L2 — Retrieval/RAG layer** (embed codebase + rules + findings)
- **Features** — smarter fixes, false-positive triage, semantic Q&A, static+runtime-aware analysis

**This spec covers the first vertical slice only.** It builds just enough embedded-inference foundation to ship one high-value feature end-to-end: false-positive triage.

## Decisions (locked during brainstorming)

| Decision | Choice | Rationale |
|---|---|---|
| Sequencing | Vertical slice first | Prove value end-to-end before generalizing the foundation |
| Inference substrate | Embedded SLM, in-process | No external server dependency; private; offline after first download |
| First feature | False-positive triage | Bounded, classification-like — best fit for a small model; directly raises signal |
| Weight distribution | Download on first run | Keeps binary small; one-time network fetch to cache |
| Engine | `candle` (pure Rust) | No C++ toolchain; matches Falcon's clean single-binary, easy-cross-compile ethos |
| Compile gating | Cargo feature `ai-local` | Default builds stay lean and fast; AI is opt-in at compile time |

## Goal & Scope

One feature: an in-process small language model that reads each rule finding **plus the surrounding code** and judges **real issue vs. false positive**, emitting `{is_real, confidence, rationale}`. It **augments** the existing heuristic `confidence.rs` (which stays) via a new command — it does not replace it.

### Explicitly out of scope for this slice
RAG / embeddings, smarter fixes, semantic Q&A, runtime-state analysis, multi-threaded/batched inference, GPU performance tuning. These are later slices built on the same foundation.

## Architecture

New module `src/ai/local/`, entirely behind `#[cfg(feature = "ai-local")]`.

```
src/ai/local/
  mod.rs          # module root, public surface, cfg-gated re-exports
  completer.rs    # trait Completer { fn complete(&mut self, prompt, opts) -> Result<String> }  <-- test seam
  engine.rs       # LocalEngine: candle model + tokenizer; implements Completer
  model_cache.rs  # cache-dir resolution + first-run weight/tokenizer download (hf-hub)
  prompt.rs       # system prompt + few-shot FP-triage template; prompt builder
  triage.rs       # orchestration: code window -> prompt -> complete -> parse verdict
```

### Components

- **`completer.rs`** — defines `trait Completer { fn complete(&mut self, prompt: &str, opts: &GenOpts) -> anyhow::Result<String>; }` and `GenOpts { max_tokens, temperature, stop }`. **This is the testability boundary**: all triage/prompt/parse logic depends on `&mut dyn Completer`, so it is fully testable with a fake and zero model download.

- **`engine.rs`** — `LocalEngine` wraps candle (`candle-core`, `candle-transformers`) + `tokenizers`. `LocalEngine::load(spec: &EmbeddedModelConfig) -> Result<Self>` loads weights + `tokenizer.json` from the cache. Implements `Completer` via greedy/low-temp sampling with stop tokens and a `max_tokens` cap. Device: CPU by default; best-effort Metal on macOS (`cfg(target_os = "macos")`). The engine is **loaded once** and reused across all findings in a run.

- **`model_cache.rs`** — resolves the cache dir (`~/.cache/falcon/models/<model-id>/`, honoring `XDG_CACHE_HOME`, via `dirs`). On first run, downloads model weights + `tokenizer.json` (via `hf-hub`) idempotently, printing a one-line progress indicator. Verifies file presence before returning a ready path.

- **`prompt.rs`** — the FP-triage system prompt and a small few-shot template. `build_triage_prompt(issue, code_window) -> String`. Instructs the model to return a single JSON object `{"is_real": bool, "confidence": 0-100, "rationale": "<one line>"}`.

- **`triage.rs`** — orchestration. For each `Issue`: extract a code window of ±`context_lines` around `issue.line` (clamped to file bounds), build the prompt, call `complete`, and parse the JSON verdict robustly into `TriageVerdict`. Public entry: `triage_issues(issues, project_root, engine: &mut dyn Completer, cfg) -> Vec<TriageVerdict>`.

```rust
pub struct TriageVerdict {
    pub issue: Issue,        // from crate::reporters::Issue
    pub is_real: bool,
    pub confidence: u8,      // 0-100
    pub rationale: String,
    pub degraded: bool,      // true when parsing/inference failed -> "uncertain" fallback
}
```

### Feature-off stub
When the binary is built **without** `ai-local`, the `ai triage` command still exists (so `--help` is honest) but prints an actionable message: rebuild with `--features ai-local`. Implemented via a `cfg`-gated stub so `main.rs` compiles in both configurations.

## Model & Configuration

- **Default model:** `Qwen2.5-0.5B-Instruct` (Q4 GGUF, ~350 MB). Fast on CPU, adequate for binary triage with few-shot. Fully configurable.
- **Config additions** in `src/ai/config.rs`:
  - New `AiProvider::Embedded` variant (serde alias `"embedded"`), distinct from the existing Ollama `Local`.
  - New struct on `AiConfig`:
    ```rust
    pub struct EmbeddedModelConfig {
        pub model_id: String,      // hf repo id; default "Qwen/Qwen2.5-0.5B-Instruct-GGUF"
        pub model_file: String,    // specific GGUF file within the repo
        pub max_tokens: usize,     // default 128
        pub context_lines: usize,  // code window radius, default 12
        pub max_issues: usize,     // per-run cap, default 100
    }
    ```
  - `effective_model()` / `is_available()` updated to handle `Embedded`.

## Data Flow

1. User runs `falcon ai triage <path>` (feature-gated).
2. Falcon runs normal analysis → `Vec<Issue>`.
3. Issues are capped at `max_issues` (configurable). **If truncated, a warning is printed** — no silent caps.
4. `LocalEngine` is loaded once (downloading weights on first ever run).
5. Per issue: read ±`context_lines` around `issue.line`, build prompt, `complete`, parse → `TriageVerdict`.
6. Output: human-readable table (rule, location, verdict, confidence, rationale) or machine JSON via `--format json` (using `serde_json`, consistent with existing reporters).
7. (Optional, follow-up) verdicts can demote/flag low-confidence findings in the main report.

Inference is **sequential** for this slice (single model instance; rayon parallelism is out of scope here).

## Error Handling

- `anyhow::Result<T>` at boundaries; `?` for propagation; **no panics in library code**.
- Weight download failure → clear error with the cache path and manual-download instructions.
- Model-load failure → clean error, no panic.
- **Per-issue inference or parse failure degrades to an `uncertain` verdict (`degraded: true`) and continues** — one bad finding never aborts the run.
- Context-window and `max_issues` caps are configurable and announced when hit.

## Testing (TDD)

Write tests before implementation. The following run in **default CI with no model download**:

- **Code-window extraction** — correct radius; clamps at file start/end; single-line files; issue on line 1 / last line.
- **Prompt construction** — `build_triage_prompt` includes rule, message, location, and the code window; stable structure.
- **Verdict parsing** — clean JSON; JSON embedded in surrounding prose; extra fields ignored; malformed/empty output → `uncertain` with `degraded: true`.
- **Triage orchestration** — driven by a fake `Completer` (canned responses, including a failing one) to assert end-to-end behavior and the degrade path, fully offline.
- **Cache-path resolution** — honors `XDG_CACHE_HOME`; correct `<id>` subpath.

Real-inference tests are marked `#[ignore]` / gated behind `ai-local` (they require a download). Target: behavioral coverage of every public function in `triage.rs`, `prompt.rs`, `model_cache.rs` path logic.

## Cargo

```toml
[features]
ai-local = ["dep:candle-core", "dep:candle-transformers", "dep:tokenizers", "dep:hf-hub", "dep:dirs"]

[dependencies]
candle-core = { version = "...", optional = true }
candle-transformers = { version = "...", optional = true }
tokenizers = { version = "...", optional = true }
hf-hub = { version = "...", optional = true }
dirs = { version = "...", optional = true }
```

Default build pulls none of these — compile time and binary size unchanged unless opted in. (Exact versions pinned during implementation against the current candle release.)

## Integration Points (existing code)

- `crate::reporters::Issue { rule, message, severity, file, line, column }` — input to triage.
- `src/ai/confidence.rs` — heuristic scorer stays; triage is a parallel, additive surface.
- `src/ai/mod.rs` — add `#[cfg(feature = "ai-local")] pub mod local;`.
- `src/main.rs` — register `ai triage` subcommand (with cfg-gated stub for the feature-off build); mirror the dispatch style of the existing `check-unused-confidence` command.
- `src/ai/config.rs` — `AiProvider::Embedded` + `EmbeddedModelConfig`.

## Future Slices (not this spec)

1. RAG/embeddings layer (bge-small via candle) reusing `LocalEngine` infra.
2. Smarter fixes (larger generative model, same engine abstraction).
3. Semantic Q&A over the embedded index.
4. Static + runtime-aware analysis (devtools bridge + model).
