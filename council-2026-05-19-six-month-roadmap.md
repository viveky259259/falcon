# Falcon Council — Six-Month Roadmap

**Date:** 2026-05-19
**Window:** May 2026 → November 2026
**Goal:** make Falcon the default choice for Flutter tooling, analysis, scripting, and automations.

**Council members (simulated):**
- **PM** — Flutter SDK Product Manager (Google)
- **SDK Eng** — Flutter SDK Engineer (`package:analyzer`, analysis_server)
- **VSCode Dev** — VS Code dev-tooling engineer (Dart-Code-side experience)
- **App Dev** — working Flutter app developer / engineering lead

---

## Round 1 — Opening positions

### PM (opening)

The wedge is not "more rules" and not "AI safety net" — that's a tagline, not a wedge. The wedge is **becoming the pre-commit gate for any Flutter repo where an LLM touched the code**, which by end of 2026 is every Flutter repo. `dart analyze` runs on human-authored code with human-grade assumptions (the developer read what they wrote). Falcon's job is to be the layer that assumes the author didn't, and to ship as a *git hook + CI check + MCP server* triple that Cursor, Claude Code, and Copilot Workspace all call into automatically. The CLI is the delivery mechanism; the wedge is "the thing your coding agent runs before it hands code back to you." Everything else is distraction.

Three non-obvious risks: (1) Falcon's surface area is its own ceiling — 80 subcommands means no single one is the verb a developer types reflexively, and "what does Falcon do" has no one-sentence answer, which kills word-of-mouth. (2) Coding-agent vendors (Cursor, Anthropic, GitHub) will ship their own framework-aware linters within 12 months because verification is the bottleneck on agent autonomy, and Falcon becomes a commodity unless it's already the upstream they integrate. (3) Rust is a recruiting/contribution moat in the wrong direction — the Flutter community writes Dart, every drive-by contributor bounces off `cargo`, and "Rust-powered" reads as engineer vanity to the Flutter core team you eventually need to befriend.

Five bets, priority order:
1. **Own the MCP seat** in Claude Code, Cursor, and Cline by August — cheapest distribution on earth and nobody else is fighting for it.
2. **Ship `npx falcon` and a Homebrew tap** before September; `cargo install --git` is a conversion-rate disaster.
3. **Public "AI Code Quality Score" leaderboard** for the top 500 pub.dev packages — turns the score from a feature into a *standard* others cite.
4. **Land one high-trust design-partner case study** (Wolt, Reflectly, BMW, Alibaba) with a hard number like "caught 14 prod bugs from Copilot PRs in Q3"; one line replaces a year of marketing.
5. **Pick a fight with `very_good_analysis`** on the rules they don't cover (state-mgmt-correctness, async-misuse, golden-drift); position Falcon as the layer *above* the linter, not a competitor.

Predictions/disagreements with the other three: SDK Eng will push tree-sitter/analyzer parity — disagree, parity is a 3-year project, ship 80% correctness with great UX. VSCode Dev will push first-class extension with squigglies/quick-fixes — deprioritize under MCP, the agent is the new IDE. App Dev will want fewer false positives + `falcon doctor` that fixes — right on signal, wrong on scope; ship `falcon fix --safe`, tune precision on top 10 rules.

**One thing to kill:** the HTTP `api` server. Third protocol surface competing with MCP and LSP, no clear user.

### SDK Eng (opening)

Tree-sitter gives a concrete syntax tree and nothing else. Falcon cannot resolve `foo.bar` to a `PropertyAccessElement`, cannot tell whether `bar` is a getter, field, extension member, or `noSuchMethod` forward. No type inference: every `var x = something()` is opaque. No generic substitution, so `List<T>` flow-through is guesswork. No constant evaluator, so `@Deprecated`, `assert(const ...)`, sealed-type exhaustiveness, and `switch` completeness checks are out of reach. No element model means import resolution stops at the URI string — cannot follow `package:foo/foo.dart` to its actual library, which kills any cross-file rule that needs to know what's exported. Macros (3.5+) and augmentations are the killer: by spec they *rewrite the program before analysis*, and a CST-only tool sees the pre-macro source. Be honest in the README or get torched on r/FlutterDev the first time someone files "false positive on macro-generated code."

Where the architecture genuinely wins: (a) **Cold start** — `dart analyze` is 8–15s of SDK summary + driver warmup before the first diagnostic; tree-sitter parses from a `.so` in <50ms, so `falcon analyze --changed` against a git diff is genuinely sub-second. (b) **Embarrassingly parallel multi-package walks** — the analyzer's `AnalysisDriver` is single-isolate; rayon lets us shard a melos monorepo with linear scaling because tree-sitter parsers are `Send`. (c) **Sub-100ms MCP queries** — agents doing "show me all `setState` calls inside `build()`" need a syntactic query interface; tree-sitter queries return in microseconds and are *exactly* what agentic loops want. The analyzer's API was built for IDEs; ours can be built for agents.

Six-month engineering bets: (i) `--semantic` mode that shells out to `dart analyze --format=json` and merges results — type-aware rules without re-implementing CFE; tree-sitter stays the hot path. (ii) Stable query DSL over tree-sitter-dart exposed via MCP — this is the moat, not the rules. (iii) Persistent daemon (`falcon serve`) with mmap'd CST cache keyed by content hash. (iv) Macro/augmentation: parse the *expanded* output from `.dart_tool/` when present, fall back with a visible degraded-mode flag. (v) Publish a `falcon_analyzer_plugin` to pub.dev so we ride the existing `analysis_options.yaml` rail.

Predictions: PM wants a flashy AI-native rebrand and pub.dev parity with DCM — disagree on rebrand timing; ship semantic mode first or demos lie. VSCode Dev wants LSP parity (hover, go-to-def, rename) — wrong target; we'll lose to the official Dart extension; be the *diagnostics provider* that augments. App Dev wants "make CI green faster" + zero false positives — will resist any rule requiring `pub get`, so semantic mode must be opt-in or it dies.

**The one architectural mistake** Falcon must not make: trying to reimplement the element model in Rust. Five-year project; chase a moving target through macros and augmentations. Shell out, cache aggressively, stay in our lane.

### VSCode Dev (opening)

Open VS Code on any Flutter repo and Dart-Code has already grabbed the analysis_server socket, owns "Go to Definition," and streams diagnostics from `source: dart`. If Falcon-LSP also registers as a language server for `dart` files and emits unscoped diagnostics: two squiggles per line (stacked, hover tooltip becomes a wall), two duplicate Problems-panel entries, two competing Quick Fix lightbulbs where Falcon's code actions get buried, and `editor.codeActionsOnSave` running both providers fighting over import ordering. I have shipped this bug. Users uninstall within 48 hours and the 1-star review says "conflicts with Dart, broke my saves." Every Falcon diagnostic must carry `source: "falcon"` and a `code` prefixed `falcon/<rule-id>`, and the extension must ship with a documented escape hatch.

Three moments Falcon owns: (1) the half-second after Copilot/Cursor inserts 40 lines — Dart-Code is still re-parsing; Falcon's ai-score code lens ticks 82→67 with "3 new smells from AI insert." Dart-Code structurally cannot show this. (2) Renaming a Riverpod provider — Dart-Code does the textual rename; Falcon's CodeLens above the declaration shows "32 dependents, 4 with stale `ref.watch`, 1 circular" *before* the user clicks. (3) Opening a PR branch — a Falcon status-bar item shows "score -8 vs main, 2 new vuln, 1 golden drift" and clicking opens a diff webview. Dart-Code has no notion of "vs main."

Six-month IDE bets: (a) `falcon-vscode` on the Marketplace as a thin client to `falcon-lsp`, namespaced diagnostics, zero overlap with Dart-Code's rule IDs — nothing else matters if this isn't day-one. (b) File-header CodeLens for ai-score with delta-on-save and a one-click "explain regression" — the screenshot that sells the extension. (c) Dedicated Falcon panel in the activity bar so we never compete with the Problems panel. (d) Inline code actions sourced from `ai fix` exposed as a *separate* "Falcon: Quick Fix" command palette entry, not mixed into the default lightbulb until trust is earned. (e) A "Falcon Chat" participant (`@falcon` in Copilot Chat) that answers "why did my score drop?" using local analysis JSON.

Predictions: PM will want a flashy webview + marketplace install count as north star — disagree, beautiful dashboard alongside duplicate squiggles dies in reviews. SDK Eng will push deep `dart analyze` integration and `analysis_options.yaml` respect — agree on YAML respect, disagree if they want us to *replace* analyzer diagnostics rather than augment. App Dev will beg for "fast and don't nag me" + one-line install — agree completely; will fight PM for a "Quiet Mode" toggle.

**The one extension-shaped thing** that must ship in 6 months or adoption stays <5%: a published, signed VS Code Marketplace extension with a working LSP client, namespaced diagnostics, and onboarding that runs on first open without a `settings.json` edit. `cargo install --git` is a non-starter for 95% of Flutter devs.

### App Dev (opening)

Three Cursor PRs landed overnight. First one "fixed" a rebuild loop by wrapping half a widget tree in `setState` inside a `Future.then` — passes `dart analyze`, passes tests, will silently double-fire a network call in production. Second migrated a `Provider` to Riverpod and left two stale `ChangeNotifier` subscriptions no static tool flags. Third "improved" a list screen by introducing an unbounded `ListView` where a `ListView.builder` used to be — TestFlight will catch it in three days. None of this is novel. Same five shapes of AI slop every week: lifecycle misuse, async-in-build, mis-keyed widgets, Riverpod/Bloc scope leaks, and l10n strings hardcoded back in because the model didn't have the ARB file in context. My seniors spend 40% of review time being a human linter. `dart analyze` doesn't see semantics. `flutter_lints` is a style guide. `very_good_analysis` is stricter style. Custom_lint exists but writing one is a weekend and nobody on my team will. CI runs analyze + test + golden + a homegrown grep for `print(` and `TODO`. It does not understand that an AI added a second `Navigator.of(context).push` inside a `build`. The actual missing layer is *behavioral* review of AI diffs at PR time, and right now that layer is me on Slack at 11pm.

Trust preconditions: `cargo install --git` is dead on arrival — Homebrew tap, `pub global activate` shim, or a notarized binary in a GitHub release. Pin a version in `falcon.yaml` and have it self-verify checksum. It must *layer on top of* `dart analyze`, never disagree on the same line — if analyzer is silent and Falcon is silent, CI is green; if both speak, analyzer wins on style, Falcon wins on semantics, never both on the same line. 80 subcommands is a smell. I want `falcon check`, `falcon review`, `falcon fix`, `falcon ci`. The other 76 live behind `falcon x <thing>` and I never have to know they exist. Deterministic. Same SHA, same output. No LLM call in the default path. AI is opt-in. SARIF output GitHub already understands, and `--baseline` so I can adopt without 4,000 warnings on day one.

Five workflows: (a) `falcon review <branch>` → a markdown block I paste into the PR, listing the five semantic risks the AI introduced, ranked. (b) Pre-commit that blocks on a tight set — async-in-build, unbounded scroll, stale subscriptions, hardcoded user-facing strings — zero config, sub-second on changed files. (c) `falcon ci --baseline` that fails the build only on *new* issues vs main. (d) MCP/LSP path so Cursor itself sees Falcon findings while the AI is still typing, and self-corrects. (e) `falcon deps --explain` that tells me which of last week's `pub upgrade` bumps actually changed runtime behavior.

Predictions: PM will push glossy dashboard and "developer productivity" numbers — wrong; want fewer findings, not prettier ones. SDK Eng will push deep analyzer-element parity — wrong direction; ends with Falcon being a slower `dart analyze`. VSCode Dev will push Marketplace extension — wrong audience; my team lives in Cursor and JetBrains; squiggle is the *least* valuable surface. LSP yes, bespoke extension no.

**The one thing:** `falcon review` on a branch of AI-generated code that produces, in under 10 seconds, a PR-ready summary naming the three real semantic risks a human reviewer would have caught — file:line, one-sentence why, suggested diff. If that lands and it's right 80% of the time, I will install it on every machine I touch and put it in the onboarding doc on Monday.

---

## Round 2 — Cross-reactions

### SDK Eng (round 2)

Concessions. First, `falcon_analyzer_plugin` on pub.dev as a near-term bet was wrong. Shipping an analyzer plugin means inheriting the analyzer's lifecycle, IPC contract, and diagnostic taxonomy — and re-litigating "why does Falcon say X when dart analyze says Y" *inside the user's own IDE*. The app dev is right: the trust gate is *don't fight the analyzer on the same line*; the plugin makes that contract harder, not easier. Kill it for now; revisit year two. Second, oversold the persistent daemon as Q1. With the council converging on `falcon review` + pre-commit, the hot path is "cold-start, scan changed files, exit" — not "long-lived index serving an IDE." Daemon slips to month 4–5, behind MCP and Homebrew.

Pushback. PM's "analyzer parity is 3 years, ship 80%" is half right and half dangerous. 80% on *style* rules is fine. 80% on *flow-sensitive nullability, generic substitution, and extension resolution* is how Falcon ends up flagging code the analyzer already proved safe — exactly the same-line collision the app dev says will kill us. We don't need parity; we need a **typed name resolution layer** over tree-sitter (imports, exports, part-of, prefix, extension methods) so semantic rules can refuse to fire when they can't resolve a symbol. That's 2–3 months, not three years, and it's the load-bearing piece under `falcon review`'s "3 real semantic risks" promise. Without it, the 80% accuracy target is a coin flip on real Flutter codebases leaning on `freezed`, `riverpod_generator`, and part files.

Revised sequencing for the SDK layer, 6 months: (1) tree-sitter-Dart grammar fork pinned in-tree with patches for 3.x patterns, records, sealed classes, augmentations-as-parse-only; (2) name & import resolution pass; (3) `dart analyze` co-pilot mode — ingest its JSON, suppress any Falcon diagnostic on a line the analyzer already owns; (4) changed-file scan path optimized for `falcon review`, target p95 < 10s on 200-file diff; (5) MCP tool surface stabilized around `review`/`check`/`explain`/`fix-preview`; (6) macro/augmentation: parse-only, list affected symbols, refuse to lint generated regions — do not attempt expansion. Cuts: HTTP api, analyzer plugin, daemon (deferred), public leaderboard pipeline (not my layer), VS Code-specific affordances.

**Q for PM:** when `falcon review` and `dart analyze` disagree on the same line in a design-partner repo, whose verdict ships to the PR comment — and are you willing to suppress Falcon by default?

### VSCode Dev (round 2)

JetBrains concession. "VS Code marketplace" is misleading — what matters is the *LSP+extension surface* that Cursor, Windsurf, and VS Code all share. That covers maybe 70% of Flutter devs writing AI-assisted code today; my install graphs at previous jobs showed Cursor adoption among AI-forward teams runs 3–4× ahead of pure VS Code. JetBrains is the real gap: separate Kotlin/Java codebase, separate marketplace, separate review cycle, ~25% of serious Flutter shops. Concede JetBrains parity is a 2027 problem — thin IntelliJ plugin that shells out to `falcon-lsp` in Q1 2027 once the LSP server stabilizes. For this 6-month window, "VS Code-compatible" (Cursor + Windsurf + VS Code via one VSIX) is the wedge, not a Marketplace vanity number.

To PM. MCP fires when the agent decides to call a tool; the extension fires on every keystroke regardless of whether an agent is in the loop, which is the only way we catch the developer hand-editing AI output after the agent stops. The extension is also where the ai-score delta lens lives — MCP can return a score, but only an editor can render "this diff dropped your score from 82 to 71, here's the offending line" as a gutter annotation the human actually sees. And `@falcon` chat participant is the cheapest way to get the brand into Copilot Chat's UI for free. If you cut the extension, you cut the only surface where a non-agentic developer ever sees the word "Falcon" — that's a brand and retention problem.

To SDK Eng. Conceded — not building a language server; building a *diagnostics provider* that piggybacks on Dart-Code's existing LSP session. Falcon namespaces every diagnostic `falcon:` and suppresses anything `dart analyze` already flagged on the same line. No hover, no go-to-def, no rename — that was never the pitch.

Revised sequencing. Months 1–2: nothing IDE-side, full stop — Homebrew, `falcon review`, MCP registry, SARIF baseline ship first. Month 3: minimal VSIX wrapping `falcon-lsp` with namespaced diagnostics and one quick-fix category (AI-smell rules, our differentiator). Month 4: ai-score delta lens on save. Month 5: `@falcon` chat participant. Month 6: activity bar panel + Cursor/Windsurf compat testing. Postponed to 2027: JetBrains plugin, separated quick-fix categories, inline explain UI, hover cards.

**Q for PM:** if Cursor's MCP registry lands in August and a Flutter dev installs Falcon via Homebrew the same week, what's the first surface where they see "Falcon" as a brand rather than as an anonymous tool the agent silently invoked?

### App Dev (round 2)

PM scope pushback. "Fewer false positives + safe-fix" is table stakes; what I asked for is **behavioral** lint — rules that catch agent failure modes like silently swallowed exceptions, `setState` after dispose, unawaited futures in build, fake `mounted` checks, and Riverpod providers that leak because the agent didn't read the lifecycle. Style nits I already get from `dart analyze` and `very_good_analysis`; I need Falcon to flag the patterns that compile, pass tests, ship, and page me at 2am. Top-10 precision on the wrong 10 rules is worse than nothing.

MCP integration. Two preconditions: register against Cursor's MCP config and Claude Code's `.mcp.json` with the *same binary* and zero per-IDE flags, and the agent-facing tool surface ≤5 calls (`lint_file`, `lint_diff`, `explain`, `fix_safe`, `score`) — anything more, the agent picks the wrong tool. JetBrains AI Assistant doesn't speak MCP cleanly yet; stdio fallback acceptable. **Disqualifier:** if `falcon-mcp` needs a long-lived daemon, a port, or auth setup, dead. Launch per call, return in <300ms on a warm cache, or my team turns it off in week two.

`falcon review` spec. Input: `falcon review <base-ref>` (default `origin/main`), changed files in the diff only. Output: grouped findings per file with severity, rule id, one-line fix suggestion — plain text by default, `--format json` for CI, `--format gh` for PR comments. Exit 0 clean, 1 on `error`-severity, 2 on internal failure; warnings never fail. Does NOT: run on unchanged files, require a network call, post to GitHub itself, or invoke an LLM unless `--explain`. README example is literally that one command.

Concession on semantic mode. Pre-push hook and `falcon review` in CI — both already pay the `pub get` cost on a warm `.dart_tool/`. Opt-in via `--semantic`; auto-enabled in `review` if `.dart_tool/package_config.json` exists. Editor-save and pre-commit stay syntactic-only, non-negotiable.

**To PM, don't water down:** kill every subcommand that isn't `review`, `check`, `fix`, or `score` before September, even the ones you're proud of.

---

## Round 3 — PM final verdict & locked roadmap

### 1. Verdict

By November 2026, the sentence a Flutter dev says to a teammate is: *"Just run `falcon review` before you push — it's what my Cursor agent runs anyway, and it catches the AI slop `dart analyze` misses."* That's the whole positioning. Not "linter." Not "code quality platform." The pre-push gate that both humans and coding agents already trust because it catches the behavioral failures (silent catch, `setState` after dispose, unawaited futures in build, fake `mounted`, Riverpod scope leaks) that compile clean and ship broken. Everything else in the surface area dies or hides.

To the SDK engineer's question — whose verdict ships when `falcon review` and `dart analyze` disagree on the same line: **`dart analyze` wins, Falcon suppresses itself by default.** Co-pilot mode is the only honest posture for an 18-month-old tool sitting next to Google's analyzer. We ingest analyzer JSON, hash by `(file, line, rule-class)`, and if analyzer owns the line we shut up unless `--no-defer` is passed. One duplicate diagnostic in a design-partner PR comment and we're dead. We earn override on specific rule classes (behavioral, AI-failure-mode) where analyzer doesn't play — never on style, never on unused-import, never on type errors.

To the VS Code dev's question — first surface where a Cursor user sees "Falcon" as a brand the same week they Homebrew-install in August: **the PR comment.** Not the IDE. `falcon review --format gh` posts a single grouped comment signed "Falcon" with a one-line fix per finding and a footer linking to the rule docs. That's the brand moment. The VSIX in month 3 is retention; the PR comment in month 4 is acquisition. We are not racing to put a logo in the activity bar in August.

### 2. The four verbs

Conceded — the app dev is right and I was wrong to flatten his ask. Behavioral lint is the wedge, not "fewer false positives." Four verbs survive to top-level: **`review`, `check`, `fix`, `score`**. Everything else moves behind `falcon x <thing>` or dies. Disposition of every current top-level surface:

| Current surface | Fate |
|---|---|
| `analyze` | **Killed.** Overlaps `check`; the name invites a comparison to `dart analyze` we will lose. |
| `smells`, `metrics` | **Merged into `score`.** Signals feeding it, not user-facing. |
| `ai-score` | **Renamed to `score`.** "AI" in the name dates the product to 2024. |
| `manage`, `flutter`, `fvm`, `devtools` | **Killed.** Out of scope. We are a linter, not a Flutter toolchain manager. |
| `live`, `watch` | **Killed (pre-month-6).** Daemon in month 6 covers live; standalone watchers are a maintenance tax. |
| `asset-audit`, `theme-audit`, `l10n-coverage`, `deeplink-validate`, `animation-audit`, `golden-gen` | **Aliased to `falcon x`.** Real value, not the wedge. |
| `agents` | **Killed.** Naming collision with coding agents themselves. |
| `baseline` | **Kept as flags on `review` and `check`** (`--baseline`, `--update-baseline`). Not a verb. |
| `dep-graph`, `workspace`, `docs` | **Aliased to `falcon x`.** |
| `ai` | **Killed as a verb; folded into `--explain` flag** on `review`/`check`. |
| `fix` | **Kept.** Renamed internal `fix-preview` MCP tool to match. |
| `vuln-scan`, `refactor-sim`, `test-gen` | **Aliased to `falcon x`.** Promising, not core. |
| `pr-comment` | **Killed as a verb; becomes `review --format gh`.** |
| `api` (HTTP) | **Killed.** MCP is the API. |

September 1 cutover. Aliased commands print a deprecation hint pointing at `falcon x`. The `--help` output shows four verbs.

### 3. Month-by-month plan

**Month 1 — May 2026 — Foundations & truce.**
- Milestone: **`falcon review v0`** ships, changed-files-only, syntactic, exits per spec (0/1/2; warnings never fail).
- Supporting: (a) `dart analyze` co-pilot mode — ingest analyzer JSON, suppress overlapping diagnostics; (b) tree-sitter grammar fork w/ Dart 3.x patches; (c) Cargo release pipeline producing static binaries for macOS-arm64/x64 + Linux.
- Non-goal: any IDE work.

**Month 2 — June 2026 — MCP frozen, review real.**
- Milestone: **MCP tool surface locked at exactly 5 calls**: `lint_file`, `lint_diff`, `review`, `explain`, `fix_safe`. Stdio only, per-call launch, <300ms warm cache benchmark in CI.
- Supporting: (a) `falcon review` p95 <10s on 200-file diff measured against 3 OSS Flutter repos; (b) `--format gh` PR comment renderer with grouping + one-line fixes; (c) design-partner signed (one mid-size Flutter team, weekly feedback).
- Non-goal: behavioral rules beyond the existing pack — shape first.

**Month 3 — July 2026 — Resolution & the rules that matter.**
- Milestone: **Name & import resolution pass** over tree-sitter (extension resolution, prefixes, parts, exports) — unlocks the rules below.
- Supporting: (a) behavioral rule pack v1 — `setState_after_dispose`, `unawaited_future_in_build`, `fake_mounted_check`, `silent_catch`, `riverpod_scope_leak`, `dispose_not_called`; (b) minimal VSIX wrapping `falcon-lsp`, namespaced diagnostics under `falcon:`, one quick-fix category (`fix_safe`); (c) `--semantic` flag auto-on in `review` when `package_config.json` present.
- Non-goal: hover, go-to-def, rename. Dart-Code owns those.

**Month 4 — August 2026 — Distribution & the brand moment.**
- Milestone: **MCP registry seats in Cursor + Claude Code + Cline** simultaneously, same binary, zero per-IDE flags.
- Supporting: (a) design-partner case study published with specific numbers on AI-PR rejection rate before/after; (b) ai-score delta lens in VSIX (on save, show score change); (c) Homebrew tap live (formal GA September).
- Non-goal: JetBrains. Confirmed 2027.

**Month 5 — September 2026 — GA & the cutover.**
- Milestone: **`npx falcon` + `brew install falcon` GA, four-verb CLI cutover ships**, deprecation warnings on aliased commands.
- Supporting: (a) SARIF output on `review` for GitHub code-scanning; (b) `--baseline`/`--update-baseline` flags stable; (c) leaderboard for top 500 pub.dev packages — static, regenerated weekly, no live pipeline.
- Non-goal: a marketing site beyond the docs site.

**Month 6 — October–early November 2026 — Editor surface & 1.0.**
- Milestone: **Falcon 1.0 launches** with `@falcon` chat participant in VSIX + persistent daemon (editor-only, opt-in, never required for CLI/MCP/CI).
- Supporting: (a) activity-bar panel showing score + recent findings; (b) Cursor/Windsurf compat verified end-to-end; (c) 1.0 stability commitment on the 5-tool MCP surface and 4-verb CLI.
- Non-goal: any new rule category — month 6 is hardening.

### 4. Kill list

1. **HTTP `api` server** — MCP is the protocol; an HTTP surface is a second integration burden.
2. **`manage`, `flutter`, `fvm`, `devtools` subcommands** — out of scope; we are a linter.
3. **`live` / `watch` (pre-month-6)** — editor handles live feedback; standalone watchers are tax.
4. **`agents` subcommand** — name collides with coding agents.
5. **Persistent daemon (months 1–5)** — app dev's disqualifier; per-call launch with warm cache covers 95%.
6. **Pub.dev analyzer plugin** — SDK engineer conceded; don't ship into an ecosystem we don't yet understand at the resolution layer.
7. **`analyze` subcommand** — we lose any direct comparison with `dart analyze`; co-pilot, don't compete.
8. **Live AI-score leaderboard pipeline** — static weekly regen is enough for the credibility play.

### 5. Success metrics

**North-star** — *Percentage of `falcon review` invocations originating from a coding agent's MCP call vs. human CLI invocation, where the verdict was accepted* (no override flag, no `--no-defer`, exit 0 or fixes applied) — **target 60% by Nov 2026**. This measures "default" — agents calling us unprompted, humans not fighting the output. Install count and stars are vanity.

Supporting:
1. **MCP registry presence**: listed and discoverable in Cursor + Claude Code + Cline by **Aug 31, 2026**. Binary: 3/3 or fail.
2. **False-positive rate on the behavioral rule pack**: **≤5%** measured against the design-partner repo's triaged finding log over a rolling 4-week window in November.
3. **`falcon review` p95 latency**: **<10s** on a 200-file diff, semantic mode on, cold cache; **<3s** warm. Measured nightly in CI against 3 reference repos.
4. **Design-partner retention + one case study**: August design partner still running Falcon in CI in November (binary), and the case study reports a specific reduction in AI-generated PR revert rate.

### 6. The bet

Coding agents will become the primary author of Flutter code by 2027, and the agent's pre-handoff gate is the most defensible piece of real estate in the toolchain — more defensible than the IDE plugin, more defensible than the CI integration, more defensible than the analyzer itself. We win by being the four-verb CLI that an agent calls over MCP and a human also runs before push, with the same binary, the same output, and the same verdict. What makes it work: we concede every line `dart analyze` already owns, we ship exactly the behavioral rules that catch AI slop and nothing else, and we get into three MCP registries the same month a Homebrew tap goes live. What kills it: a single quarter spent on IDE feature parity with Dart-Code, a duplicate-diagnostic incident in a design-partner PR that destroys the trust gate, or a behavioral rule pack with >10% false-positive rate that gets us disabled in week two.

**Locked.**

---

## Appendix — Decisions delta vs. existing `ROADMAP.md`

The current `ROADMAP.md` runs v0.x → v3.0 as a feature laundry list. The council's roadmap replaces it with a positioning/adoption arc anchored on a single user verb (`falcon review`) and a single distribution surface (MCP + Homebrew). Concrete deltas:

- **Scope contraction.** ~28 top-level subcommands → 4 verbs + `falcon x` namespace by Sept 1.
- **Posture change.** Falcon stops competing with `dart analyze` and explicitly defers on same-line collisions; this is a hard gate, not a preference.
- **Distribution change.** `cargo install --git` deprecated in month 5; primary channels become Homebrew, `npx`, and GitHub Releases (notarized binary).
- **API contraction.** HTTP `api` killed; MCP frozen at 5 tools; LSP narrows to a diagnostics-provider role (no hover/go-to-def/rename).
- **Rule-pack focus.** Behavioral rules (AI failure modes) become the explicit differentiator; style rules deprioritized.
- **IDE pacing.** No IDE work months 1–2; minimal VSIX in month 3; chat participant + activity-bar panel in month 6. JetBrains explicitly 2027.

The original `ROADMAP.md` should be edited (or superseded) to reflect these decisions before any month-1 commits land.
