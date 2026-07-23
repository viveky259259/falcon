# Falcon Backlog — May → Nov 2026

Derived from `council-2026-05-19-six-month-roadmap.md`. One epic per month + a cross-cutting kill/deprecation epic. Each story has acceptance criteria; each task is a concrete imperative work item. T-shirt sizes: **S** (≤1d), **M** (2–4d), **L** (1–2w), **XL** (2–4w).

**Legend:** `[EPIC-N]` → epic, `[N.M]` → story, `→` → task. Owner roles: `cli`, `lsp`, `mcp`, `rules`, `core` (parser/resolution), `ci`, `pkg` (distribution), `vsix`, `docs`.

---

## EPIC 0 — Kill list & deprecations (cross-cutting; lands by Sept 1)

**Outcome:** `falcon --help` shows four verbs by Sept 1, 2026. Deprecated commands print a one-line redirect and exit 0 for one minor version, then are removed.

### [0.1] Verb cutover plan in code (M)
**AC:**
- `clap` command tree refactored to four top-level verbs: `review`, `check`, `fix`, `score`.
- All other current top-level commands move under `falcon x <name>` namespace OR are removed (per disposition table).
- `--help` output lists 4 verbs at the top; `falcon x --help` lists the deprecated/experimental set.

**Tasks:**
- → `cli`: introduce `falcon x` subcommand group in `src/main.rs`.
- → `cli`: move `asset-audit`, `theme-audit`, `l10n-coverage`, `deeplink-validate`, `animation-audit`, `golden-gen`, `dep-graph`, `workspace`, `docs`, `vuln-scan`, `refactor-sim`, `test-gen` under `x`.
- → `cli`: keep `review`, `check`, `fix`, `score` at top-level with stable arg shape.
- → `cli`: rename `ai-score` → `score`; preserve old name as alias with deprecation hint.
- → `cli`: fold `pr-comment` into `review --format gh`; delete the subcommand module.
- → `docs`: update `README.md` and `docs/cli-reference.md` to the four-verb story.

### [0.2] Removals (M)
**AC:** the following are deleted from the binary surface:

**Tasks:**
- → `cli`: delete `analyze` top-level (rename callers to `check`).
- → `cli`: delete `manage`, `flutter`, `fvm`, `devtools` top-level (and their modules from `src/manage/`, `src/flutter_run/`, etc., or feature-flag them off).
- → `cli`: delete `agents` top-level.
- → `cli`: delete `live` and `watch` top-level for now (re-added month 6 inside daemon).
- → `cli`: delete the `api` HTTP server (`src/api/`) and its `mcp` route table overlap.
- → `cli`: fold the `ai` subcommand into `--explain` flag on `review`/`check`; delete the `ai` namespace.
- → `core`: drop the `falcon_analyzer_plugin` pub.dev plan (no work).

### [0.3] Deprecation UX (S)
**AC:** every removed/aliased command name still resolves for one minor version, prints a stderr deprecation hint, exits the parent verb's exit code.

**Tasks:**
- → `cli`: implement `DeprecationShim` that maps old name → new invocation + one-line message.
- → `ci`: snapshot test for deprecation output so renames are reviewable in diffs.

---

## EPIC 1 — Foundations & Truce (May 2026)

**Milestone:** `falcon review v0` ships; `dart analyze` same-line truce in place; static binaries on three platforms.

### [1.1] `falcon review v0` — changed-files-only syntactic pass (L)
**As a** Flutter dev, **I want** to run `falcon review` on a branch and get a grouped finding list, **so that** I can paste it into a PR or read it in CI.

**AC:**
- `falcon review [<base-ref>]` defaults base to `origin/main`.
- Runs only on files in `git diff --name-only <base>...HEAD` (added/modified).
- Plain-text output by default; grouped by file; `severity rule_id file:line — one-line message`.
- Exit codes: `0` clean (no error-severity), `1` if any finding ≥ error, `2` on internal failure. Warnings never fail.
- Zero network calls in the default path; no LLM invocation unless `--explain` is passed (stub for now).
- Runs against the existing rule set without behavioral pack; correctness on existing rules unchanged.

**Tasks:**
- → `cli`: add `Review { base_ref, format, baseline, explain, no_defer }` to the command enum.
- → `core`: implement `git_diff::changed_files(base) -> Vec<PathBuf>` (shell out to `git`, parse `--name-only --diff-filter=ACMR`).
- → `core`: route changed-file list into the existing analyze pipeline, skip the global walk.
- → `cli`: implement grouped text formatter (`format::text::review_grouped`).
- → `cli`: implement exit-code rule per AC.
- → `tests`: integration tests using `tempfile` for three diff shapes (rename, add, modify-only).

### [1.2] `dart analyze` co-pilot mode (L)
**AC:**
- `falcon review` and `falcon check` ingest `dart analyze --format=json` output when `.dart_tool/package_config.json` exists.
- Suppress any Falcon diagnostic whose `(file, line, class)` collides with an analyzer diagnostic.
- `--no-defer` flag bypasses suppression.
- Suppression is logged at `--verbose` so users can audit.
- Performance budget: analyzer shellout adds ≤ 2× the syntactic run on a 200-file diff; behind `--semantic` (auto-on in `review`).

**Tasks:**
- → `core`: add `analyzer_bridge` module: spawn `dart analyze --format=json --no-fatal-warnings`, parse, normalize.
- → `core`: define `RuleClass` enum (style, type, unused, behavioral, security) on every Falcon rule.
- → `core`: implement `suppression::defer_to_analyzer(falcon: &[Finding], analyzer: &[Finding]) -> Vec<Finding>` keyed on `(file, line, class)`.
- → `cli`: add `--semantic` / `--no-defer` flags; wire to `review` and `check`.
- → `tests`: fixture project with overlapping rules; assert Falcon stays silent on analyzer-owned lines.
- → `docs`: document the truce contract (one paragraph, prominent in README).

### [1.3] Tree-sitter grammar fork with Dart 3.x patches (M)
**AC:**
- `tree-sitter-dart` pinned to a Falcon-maintained fork in-tree.
- Patches for: records, patterns, sealed classes, augmentations-as-parse-only (do not attempt expansion).
- `cargo test` covers a corpus of Dart 3.x snippets without panics.

**Tasks:**
- → `core`: fork `tree-sitter-dart`, host under `falcon-lint/tree-sitter-dart`.
- → `core`: add grammar patches for records + patterns; add failing tests, then green.
- → `core`: parse-only support for `augment` declarations; round-trip test.
- → `ci`: pin the fork by commit hash in `Cargo.toml`.

### [1.4] Static binary release pipeline (M)
**AC:**
- GitHub Actions builds notarized macOS-arm64, macOS-x64, linux-x64 binaries on every tag.
- Release artifacts contain `falcon`, `falcon-lsp`, `falcon-mcp`, plus a `SHA256SUMS` file.
- A `falcon --version` matches the release tag.

**Tasks:**
- → `ci`: add `.github/workflows/release.yml` with three runners.
- → `ci`: macOS notarization via Apple ID secrets (request secrets from maintainer in plan).
- → `pkg`: artifact naming `falcon-<version>-<os>-<arch>.tar.gz`.
- → `pkg`: publish a `latest` redirect on GitHub Releases.

### [1.5] Non-goal guardrail (S)
**AC:** explicit non-goal banner in `CONTRIBUTING.md`: "no IDE work in May."

**Tasks:**
- → `docs`: add the banner; link to the council artifact.

---

## EPIC 2 — MCP frozen, review real (June 2026)

**Milestone:** MCP tool surface locked at 5 calls; `falcon review` p95 < 10s on 200-file diff; `--format gh` PR comments; one design partner signed.

### [2.1] Lock MCP surface at five tools (L)
**AC:**
- `falcon-mcp` exposes exactly these tools: `lint_file`, `lint_diff`, `review`, `explain`, `fix_safe`.
- Stdio transport only. No port. No long-lived daemon. Per-call launch.
- Schema for each tool is documented in `docs/mcp.md` with input/output JSON examples.
- Warm-cache benchmark: median MCP roundtrip < 300ms on 200-file repo.

**Tasks:**
- → `mcp`: audit `src/mcp/` for unused tool handlers; delete everything not in the five.
- → `mcp`: define stable JSON schemas (serde structs in `src/mcp/schema.rs`).
- → `mcp`: implement a content-hash CST cache at `${TMPDIR}/falcon-mcp/` (file mtime + sha256).
- → `ci`: add bench `benches/mcp_warm_cache.rs` asserting <300ms p50.
- → `docs`: write `docs/mcp.md` with copy-paste config for Cursor and Claude Code.

### [2.2] `falcon review` performance target (M)
**AC:**
- p95 latency < 10s on a 200-file diff, cold cache, on 3 reference OSS Flutter repos.
- p95 < 3s warm cache.
- Latency measured nightly in CI; regressions fail the build.

**Tasks:**
- → `core`: parallelize the diff pipeline with `rayon` (it already exists for the global walk — reuse).
- → `ci`: pick reference repos (e.g. `flutter/samples`, `Solido/awesome-flutter`, one mid-size monorepo); commit a `.github/perf-corpus.yml`.
- → `ci`: nightly perf job; regression budget 10%.
- → `core`: optional flame-graph dump on `--perf-trace` for debugging.

### [2.3] `--format gh` PR comment renderer (M)
**AC:**
- `falcon review --format gh` outputs a single markdown block suitable for `gh pr comment --body-file`.
- Findings grouped per file; severity icon prefix; one-line fix suggestion; footer links to rule docs.
- Falcon signature line in the footer: "_Posted by Falcon — see docs._"
- Truncates gracefully over 60 findings (collapses lowest-severity into a `<details>`).

**Tasks:**
- → `cli`: add `format::gh::render(findings)` returning a `String`.
- → `cli`: add `--format` enum (`text`, `json`, `gh`, `sarif`) on `review`; `sarif` stubbed until month 5.
- → `tests`: golden-file tests for the GH renderer with 5 fixture finding-sets.
- → `docs`: add the README example `falcon review --format gh | gh pr comment --body-file -`.

### [2.4] Sign one design partner (M)
**AC:**
- One mid-size Flutter team (5+ engineers, AI-assisted authoring, active CI) signs an evaluation MoU.
- Weekly feedback channel established (Slack/Discord/email).
- A private `partner-feedback.md` log starts capturing triaged findings.

**Tasks:**
- → `pm` (maintainer): outreach list, pitch deck (1 slide), MoU draft.
- → `pm`: kick-off call; install `falcon review` in their CI on a non-blocking job.
- → `pm`: weekly 30-min sync slot.

### [2.5] Non-goal guardrail (S)
**AC:** no new behavioral rules ship in June. Issue label `defer-to-july` is enforced.

**Tasks:**
- → `docs`: update CONTRIBUTING note.
- → `ci`: lint that fails PRs adding files under `src/rules/behavioral/` in June (a date-gated CI check).

---

## EPIC 3 — Resolution & behavioral rules (July 2026)

**Milestone:** Name & import resolution pass lands; behavioral rule pack v1 ships (6 rules); minimal VSIX ships.

### [3.1] Name & import resolution layer over tree-sitter (XL)
**AC:**
- New module `src/resolver/` resolves: imports, exports, `part`/`part of`, prefixes, extension members, class hierarchy (single-file scope), top-level symbol table per library.
- Rules can request `Resolver::resolve(node)` and get back `ResolvedSymbol { kind, declaring_library, type_hint }`.
- When resolution fails, rules can call `Resolver::is_ambiguous(node) -> bool` and bail.
- 100% of behavioral rules in [3.2] use the resolver and refuse to fire on `is_ambiguous`.

**Tasks:**
- → `core`: design `Resolver` API; doc the limits (no type inference, no generic substitution).
- → `core`: implement import URI → file path mapping using `.dart_tool/package_config.json`.
- → `core`: implement library-level symbol table per Dart file.
- → `core`: implement extension-member resolution (single import shadow rules).
- → `core`: implement class hierarchy resolution for single-library cases; mark cross-library as unresolved.
- → `tests`: corpus tests on a `freezed` + `riverpod_generator` fixture project.

### [3.2] Behavioral rule pack v1 — six rules (L)
**AC:** these rules ship in `src/rules/behavioral/`, each gated behind the resolver, each with golden-file tests on a real Flutter snippet:
1. `setState_after_dispose`
2. `unawaited_future_in_build`
3. `fake_mounted_check`
4. `silent_catch` (catch with no rethrow, no log, no specific exception type)
5. `riverpod_scope_leak` (provider holding a subscription not auto-disposed)
6. `dispose_not_called` (State subclass overrides initState but not dispose, holding a disposable)

**AC continued:**
- Each rule has documented severity (default `error` or `warning`).
- Each rule documents its preconditions (resolver result needed) and explicitly states when it bails.
- False-positive rate measured on the design-partner repo: ≤ 8% by end of July (goal of ≤ 5% by November).

**Tasks per rule (template):**
- → `rules`: implement detector using `Resolver`.
- → `rules`: write 5+ positive fixtures, 5+ negative fixtures.
- → `rules`: write doc page at `docs/rules/<rule_id>.md` with example + fix.
- → `pm`: run rule against design-partner repo; triage findings into TP/FP.

### [3.3] Minimal VSIX (M)
**AC:**
- Marketplace listing `falcon-vscode` published (unlisted/preview channel ok).
- Extension wraps `falcon-lsp` via `vscode-languageclient`.
- All diagnostics carry `source: "falcon"` and `code: falcon/<rule-id>`.
- Suppresses any diagnostic if Dart-Code already flagged the same `(uri, line)` (via `vscode.languages.getDiagnostics`).
- One quick-fix category exposed: `fix_safe` rule-fixes. No hover, no go-to-def, no rename.

**Tasks:**
- → `vsix`: scaffold `editors/vscode/` package (`package.json`, `extension.ts`, `tsconfig.json`).
- → `vsix`: LSP client config; `falcon-lsp` resolved from PATH first, then bundled binary.
- → `vsix`: collision-suppression filter on incoming diagnostics.
- → `vsix`: `Falcon: Quick Fix` command palette entry sourced from `fix_safe` code actions.
- → `vsix`: publisher account + first release to Marketplace (preview).
- → `docs`: `docs/vsix-install.md`.

### [3.4] `--semantic` auto-on in `review` (S)
**AC:**
- `falcon review` auto-enables `--semantic` if `.dart_tool/package_config.json` exists.
- `falcon check` does not auto-enable; must be passed explicitly.
- Pre-commit hook stays syntactic-only by default (documented).

**Tasks:**
- → `cli`: detection helper `paths::has_package_config(root) -> bool`.
- → `cli`: wire auto-on in `review` command setup.
- → `docs`: state the non-negotiable: editor-save + pre-commit = syntactic.

### [3.5] Non-goal guardrail (S)
**AC:** no IDE feature beyond namespaced diagnostics + one quick-fix in July.

---

## EPIC 4 — Distribution & brand (August 2026)

**Milestone:** MCP registry seats in three agents; design-partner case study published; ai-score delta lens in VSIX; Homebrew tap live.

### [4.1] Cursor MCP registry seat (M)
**AC:**
- Falcon listed in Cursor's MCP registry (or equivalent discovery surface).
- One-line install instruction documented and tested on a fresh Cursor install.
- `cursor.json` config snippet uses `stdio` transport and the 5-tool surface.

**Tasks:**
- → `pm`: contact Cursor partnerships (or open PR to their registry repo).
- → `mcp`: ensure the binary works on Cursor's bundled environment.
- → `docs`: `docs/install/cursor.md`.

### [4.2] Claude Code MCP registry seat (M)
**AC:**
- Falcon listed in the Claude Code marketplace / plugin catalog.
- `.mcp.json` snippet documented; works zero-config in a Flutter repo.

**Tasks:**
- → `pm`: submit listing via Anthropic's documented process.
- → `mcp`: validate stdio + working directory handling matches Claude Code's spec.
- → `docs`: `docs/install/claude-code.md`.

### [4.3] Cline MCP registry seat (M)
**AC:**
- Falcon discoverable in Cline's MCP registry.
- Install instruction tested on a fresh Cline install.

**Tasks:**
- → `pm`: outreach + listing submission.
- → `docs`: `docs/install/cline.md`.

### [4.4] Design-partner case study (M)
**AC:**
- Published blog post (or `docs/case-studies/<partner>.md`) with: weeks of usage, count of behavioral findings TP/FP, AI-PR revert rate before/after, one direct quote.
- Numbers verifiable; partner approves the post.

**Tasks:**
- → `pm`: collect metrics from design partner's CI logs.
- → `docs`: draft + partner review + publish.
- → `pm`: tweet/LinkedIn pre-cleared with partner.

### [4.5] AI-score delta lens in VSIX (M)
**AC:**
- File-header CodeLens shows current `score` and the delta from previous save.
- Tooltip lists which rules contributed to a delta.
- Toggleable in extension settings (default on, but `Falcon: Quiet Mode` setting suppresses).

**Tasks:**
- → `vsix`: implement `CodeLensProvider` calling `falcon score --format json` on the active file (debounced).
- → `vsix`: maintain a per-file score history in memory + workspace state.
- → `vsix`: settings: `falcon.quietMode`, `falcon.scoreLens.enabled`.
- → `tests`: extension integration test using `@vscode/test-electron`.

### [4.6] Homebrew tap (live, not GA) (S)
**AC:**
- `brew tap falcon-lint/tap && brew install falcon` works on macOS-arm64 + macOS-x64.
- Formula points at the GitHub Releases artifacts from [1.4].
- Auto-bumped via Actions on each tagged release.

**Tasks:**
- → `pkg`: create `homebrew-tap` repo with `falcon.rb` formula.
- → `ci`: action to update SHA256 + version on new release.
- → `docs`: install instructions.

### [4.7] Non-goal guardrail (S)
**AC:** no JetBrains work. No general marketing site work.

---

## EPIC 5 — GA & cutover (September 2026)

**Milestone:** `npx falcon` + `brew install falcon` GA; four-verb CLI cutover live (Sept 1); SARIF; baseline flags stable; static pub.dev leaderboard.

### [5.1] `npx falcon` (M)
**AC:**
- `npx falcon@latest review` works without a global install.
- npm package wraps the native binary, downloaded once on first run from GitHub Releases (checksum verified).
- Works on macOS-arm64, macOS-x64, linux-x64; gracefully errors on unsupported platforms.

**Tasks:**
- → `pkg`: create `npm/falcon/` package with a postinstall download script.
- → `pkg`: checksum verification against `SHA256SUMS` from the release.
- → `ci`: publish on tag.
- → `docs`: install snippet.

### [5.2] Homebrew GA + signed binaries (S)
**AC:**
- macOS binaries notarized and stapled. `xattr -d com.apple.quarantine` is not required.
- Formula promoted out of preview.

**Tasks:**
- → `ci`: enable notarization in [1.4] pipeline (Apple Developer secrets).
- → `pkg`: update tap to point at notarized artifacts.

### [5.3] Four-verb CLI cutover (S)
**AC:**
- On Sept 1, the default `--help` shows only `review`, `check`, `fix`, `score`, `x`.
- Aliased commands print deprecation hint with one-line redirect (from EPIC 0).
- `CHANGELOG.md` documents the breaking change with migration table.

**Tasks:**
- → `cli`: ship the cutover behind a `--legacy-help` for one release.
- → `docs`: migration guide page.

### [5.4] SARIF output (M)
**AC:**
- `falcon review --format sarif` emits SARIF 2.1.0 conforming JSON.
- Validates against the SARIF schema in CI.
- GitHub code-scanning upload works end-to-end on the design-partner repo.

**Tasks:**
- → `cli`: implement `format::sarif::render`.
- → `tests`: SARIF schema validation test (use `serde_json` + checked-in schema).
- → `docs`: GitHub code-scanning recipe.

### [5.5] Baseline flags stable (M)
**AC:**
- `falcon review --baseline .falcon-baseline.json` only reports *new* findings vs the baseline.
- `falcon review --update-baseline` rewrites the baseline.
- Baseline format documented and stable (schema version field).
- Works with rename detection (git move heuristic).

**Tasks:**
- → `core`: design `Baseline` struct + JSON schema with `schema_version: 1`.
- → `core`: implement diff against baseline (key by `rule_id + file + nearest-anchor-hash`).
- → `cli`: wire flags on `review` and `check`.
- → `tests`: rename test fixture; assert no false-new findings.

### [5.6] Static pub.dev leaderboard (M)
**AC:**
- `site/leaderboard/` rebuilt weekly via Actions; top 500 packages by pub points scored with Falcon.
- Sortable static HTML; no live API.
- Each package row links to a Falcon score report page (also static).

**Tasks:**
- → `ci`: weekly job that `pub get`s each package and runs `falcon score --format json`.
- → `site`: static generator (single Rust binary or `mdbook`-style) producing the HTML.
- → `docs`: explanation of methodology + opt-out instructions.

### [5.7] Non-goal guardrail (S)
**AC:** no new marketing site work beyond docs.

---

## EPIC 6 — Editor surface & 1.0 (October–early November 2026)

**Milestone:** Falcon 1.0 launches with `@falcon` chat participant + persistent daemon (opt-in, editor only) + activity-bar panel; Cursor/Windsurf compat verified.

### [6.1] `@falcon` chat participant (M)
**AC:**
- VS Code chat participant registered: typing `@falcon` in chat lets the user ask "why did my score drop?" or "explain rule X."
- Backed by local analysis JSON; no LLM call unless the user has Copilot/another LLM and the participant proxies.
- Documented in `docs/vsix.md` with a screenshot.

**Tasks:**
- → `vsix`: implement chat participant via VS Code Chat API.
- → `vsix`: query layer that maps natural-language intents → MCP tool calls.
- → `docs`: screenshot + walkthrough.

### [6.2] Persistent daemon — editor only, opt-in (L)
**AC:**
- `falcon serve` runs as a long-lived process; mmaps a CST cache; never required for CLI/MCP/CI.
- VSIX prefers daemon when present (falls back to per-invocation MCP).
- Daemon disables itself on idle (>15min) by default.
- Documented as "experimental" for 1.0.

**Tasks:**
- → `core`: implement `falcon serve` with Unix-socket transport.
- → `core`: mmap CST cache keyed by `(path, sha256)`.
- → `vsix`: discover daemon socket; auto-start if missing.
- → `core`: idle shutdown logic.
- → `docs`: experimental flag callout.

### [6.3] Activity-bar panel (M)
**AC:**
- Falcon tree view in VS Code activity bar: Score, Recent Findings, Rule Docs links.
- Refreshes on save and on daemon push (if running).
- Empty state with one-line "Run `falcon review` to populate."

**Tasks:**
- → `vsix`: tree-view provider + activity-bar contribution.
- → `vsix`: data feed via daemon socket or MCP call.
- → `vsix`: theming for dark/light.

### [6.4] Cursor & Windsurf compat verification (S)
**AC:**
- Manual end-to-end test on Cursor and Windsurf (same VSIX, no re-package).
- Documented compat matrix in `docs/editors.md`.

**Tasks:**
- → `vsix`: smoke-test checklist in `editors/vscode/COMPAT.md`.
- → `docs`: compat matrix.

### [6.5] Falcon 1.0 release (M)
**AC:**
- Stability commitment documented: 5-tool MCP surface and 4-verb CLI frozen through 1.x.
- Semver policy in `CONTRIBUTING.md`.
- Launch blog post + Hacker News + r/FlutterDev + design-partner co-tweet ready.
- `CHANGELOG.md` v1.0 entry.

**Tasks:**
- → `pm`: launch checklist (artifacts, copy, scheduled posts).
- → `docs`: 1.0 release notes.
- → `ci`: tag `v1.0.0`; release pipeline runs.

### [6.6] Non-goal guardrail (S)
**AC:** no new rule categories in October; hardening only.

---

## Cross-cutting tracks

### [X.1] North-star instrumentation (M, runs across months 2–6)
**AC:** by November 2026, we can report the north-star metric — `%` of `falcon review` invocations from an MCP agent call that were accepted (exit 0 or fixes applied, no override flags).

**Tasks:**
- → `mcp`: emit anonymous local telemetry events on each invocation (opt-in, off by default).
- → `mcp`: invocation source classification (`mcp:<agent_id>` vs `cli:<tty>` vs `cli:ci`).
- → `core`: opt-in upload endpoint with clear `--telemetry-off` and `FALCON_TELEMETRY=0`.
- → `docs`: privacy doc detailing exactly what is sent.

### [X.2] False-positive rate measurement (S, monthly)
**AC:** monthly TP/FP triage report on the design-partner repo, starting July, with rolling 4-week average ≤ 5% by November.

**Tasks:**
- → `pm`: triage spreadsheet template.
- → `pm`: monthly review meeting.

### [X.3] Perf regression CI (S)
**AC:** the nightly perf job from [2.2] runs through November; regressions block release.

---

## Summary by month

| Month | Epic | One-line outcome |
|---|---|---|
| May | EPIC 1 | `falcon review v0` + analyzer truce + static binaries |
| June | EPIC 2 | MCP frozen at 5 tools + perf + GH PR comments + design partner |
| July | EPIC 3 | Resolver + 6 behavioral rules + minimal VSIX |
| August | EPIC 4 | 3 MCP registry seats + case study + score lens + Homebrew live |
| September | EPIC 5 | 4-verb cutover + `npx`/`brew` GA + SARIF + baseline + leaderboard |
| Oct–Nov | EPIC 6 | `@falcon` chat + daemon + activity panel + Falcon 1.0 |

EPIC 0 (kill list) runs across May–September with the hard cutover gate on Sept 1.
