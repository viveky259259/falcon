# Contributing to Falcon

Falcon is currently executing the May-November 2026 council roadmap in
`backlog-2026-05-19-six-month.md`. Keep changes aligned with the active monthly
milestone and avoid widening the product surface while the core review and MCP
flows are stabilizing.

## Build and Test

Run the smallest focused test while iterating, then run the full gate before a
non-trivial commit:

```bash
cargo test <name>
cargo test
cargo fmt
cargo clippy
```

## Roadmap Guardrails

### May 2026: No IDE Work

May's non-goal is explicit: no IDE work. Do not add new VS Code, Cursor,
JetBrains, or editor-specific features during this milestone. Keep editor plans
in docs or issues until the roadmap reaches the minimal VSIX phase.

### June 2026: Behavioral Rule Freeze

June's non-goal is also explicit: no new behavioral rules ship in June. New
files under `src/rules/behavioral/` must be deferred with the `defer-to-july`
label and should not merge until the July behavioral-rule milestone.

Modifying existing behavioral rules, tests, and documentation is allowed when it
supports the June MCP/review milestone and does not add a new rule surface.

The CI guardrail in `.github/workflows/behavioral-rule-freeze.yml` enforces the
June freeze by blocking newly added files under `src/rules/behavioral/`.
