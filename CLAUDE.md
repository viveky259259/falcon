# Falcon

Rust-powered static analysis CLI for Flutter/Dart, designed for AI-generated code.

## Build & Test

```
cargo build            # debug build
cargo build --release  # release (LTO, stripped)
cargo test             # 719 tests
cargo test <name>      # single test
cargo run -- <args>    # run falcon CLI
cargo build --features ai-local                      # build with embedded SLM triage
cargo run --features ai-local -- ai triage <path>    # triage findings (real vs false-positive)
cargo fmt              # format
cargo clippy           # lint
```

Binaries: `falcon` (CLI), `falcon-lsp` (LSP server), `falcon-mcp` (MCP server).

## Architecture

- `src/main.rs` — CLI entry + command dispatch (`run()`); next refactor target is splitting `run()` into per-group handlers
- `src/cli_args.rs` — binary-local clap definitions (`Cli`, `Commands`, sub-action enums)
- `src/lib.rs` — library root
- `src/rules/` — lint rules (common/, flutter/, security/, ai/)
- `src/analysis/` — multi-file analysis passes (vuln_radar, refactor_sim, etc.)
- `src/ai/` — AI features (explain, fix, confidence)
- `src/ai_score/` — AI Code Quality Score (6 dimensions)
- `src/manage/` — app health, deps, architect, maintenance
- `src/doctor/` — environment diagnosis and repair (`falcon doctor`): checks, fixers, Flutter installer
- `src/lsp/`, `src/mcp/`, `src/api/` — protocol servers
- `src/dashboard/`, `src/ci/` — reporting/CI integrations
- `falcon_dart/`, `falcon-dart/` — Dart-side companion package
- `tests/` — integration tests

Tree-sitter parses Dart; rayon parallelizes file walks.

## Code Patterns

- Errors: `anyhow::Result<T>` at boundaries; `?` for propagation. Avoid panics in library code.
- Serde for all config/report I/O (JSON + YAML).
- New rules go in `src/rules/<category>/` and are registered in the rule registry.
- Public lib API lives in `src/lib.rs`; binaries stay thin.
- Prefer `colored` for terminal output, `serde_json` for machine output (gated by `--format`).

## Conventions

- Rust 2021 edition, formatted with `rustfmt`, linted with `clippy`.
- Keep files under 500 lines and functions under 80 lines where practical.
- Tests live alongside code (`#[cfg(test)] mod tests`) or in `tests/` for integration.
