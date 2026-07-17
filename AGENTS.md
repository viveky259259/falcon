# AGENTS.md — falcon

This file tells AI agents (Claude Code, Codex, Cursor, Aider, …) how to work in
this repo. It complements `CLAUDE.md`.

## What this is
Falcon is a Rust-powered static analysis CLI for Flutter/Dart, designed to catch
issues in AI-generated code. It ships three binaries: `falcon`, `falcon-lsp`,
`falcon-mcp`.

## Build / test loop
```
cargo build              # debug
cargo build --release    # release
cargo test               # full suite
cargo test <name>        # single test
cargo fmt                # required before commits
cargo clippy             # required before commits
```
Run `cargo test` after every non-trivial change. Tests live alongside code
(`#[cfg(test)] mod tests`) or under `tests/` for integration.

## Where things go
- `src/rules/<category>/` — single-file lint rules (common, flutter, security, ai)
- `src/analysis/` — multi-file analysis passes
- `src/smells/` — categorized smell reporting (dead code / code smell / security)
- `src/ai/` — AI-assisted explain/fix
- `src/ai_score/` — AI Code Quality Score (6 dimensions)
- `src/manage/`, `src/runtime/` — app health, devtools bridges
- `src/lsp/`, `src/mcp/`, `src/api/` — protocol servers
- `src/agents/` — generators for AGENTS.md (this file)
- `tests/` — integration tests

When adding a new rule: drop a file under `src/rules/<category>/`, register it
in the rule registry (`src/rules/mod.rs`), add tests, and add a classifier
entry in `src/smells/mod.rs::classify` if the rule should be bucketed.

## Conventions
- Rust 2021 edition.
- Errors: `anyhow::Result<T>` at boundaries; `?` to propagate; never panic in
  library code.
- Serde for all config/report I/O (JSON + YAML).
- Public lib API lives in `src/lib.rs`; binaries (`src/main.rs`) stay thin.
- Keep files <500 lines and functions <80 lines where practical.
- Write tests *before* the implementation — falcon expects the TDD loop.

## Things not to do
- Do not introduce new dependencies without checking conflicts in `Cargo.toml`.
- Do not skip `cargo fmt` / `cargo clippy`.
- Do not add backwards-compat shims for code you are removing — delete it.
- Do not write speculative abstractions; three similar lines beat a premature
  helper.
- Do not commit secrets — secrets live in the private `viveky259259/secrets`
  repo, never in this one.

## Useful falcon-on-falcon commands
```
falcon check .
falcon x smells .
falcon x agents init           # regenerate this file
```
