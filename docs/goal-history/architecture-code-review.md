# Goal: Review Falcon architecture and code

## Starting point
- Original goal: Review architecture and code using staff-engineer.
- Scope: Read-only code review; only this required progress log is written.
- Starting state: Repository is in `falcon/` beneath the supplied workspace; tracked working tree clean. Rust library and CLI, LSP, MCP binaries plus editor integrations.
- Success criteria: Evidence-backed architecture assessment, prioritized actionable findings with source locations, and relevant verification results. No fixes or commits requested.
- History begins: 2026-10-09 22:39:30 Asia/Kolkata, after initial repository discovery. Earlier elapsed time unmeasured.

## Current state
- As of: 2026-10-10 02:45:21 Asia/Kolkata.
- Status: All twelve review findings and the observed Clippy blocker are fixed; final verification passed. User's later remediation instruction supersedes the original read-only scope.
- Verified progress: Full default-feature tests: 2,657 passed, 0 failed, 3 ignored. Formatting, Clippy with denied warnings, default all-targets build and optional ai-local build pass. Mocked webhook and Action regressions, npm installer fixtures and extension TypeScript checks pass. HTTP and LSP regression tests exercise live child processes.
- Limits: Local verification uses Rust 1.101 nightly on macOS; remote stable/Linux CI, release downloads and marketplace installation were not run. Webhook external calls are mocked.
- Additional assessment: Compared the user's linked loops/graphs post with the completed development workflow and Falcon's implementation. Verified loops and durable regression constraints are present; a persistent task scheduler and feedback into task planning are absent from inspected paths.
- Graph status: Authorized project graph generation implemented. Directory checks persist the full graph; explicit graph export and library/SDK methods are available. Final tests: 2,676 passed, 3 ignored; format, Clippy and default/ai-local builds pass. Source graph inference limits are documented.
- Next milestone: Local atomic commit of graph generation and delivery. Unrelated in-progress site files are excluded from staging; no push authorized.
- Reproduction scripts and logs remain under ignored `target/`; task scratch was excluded from staging and removed after completion.

## Chronological history
Timezone: Asia/Kolkata (UTC+05:30). Durations are elapsed wall-clock intervals.

| # | Time (start → end) | Duration | Action / investigation | Evidence / result | Gaps | Decision and reason | Next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-10-09 22:39:30 | Not measured | Locate repository and read instructions/configuration | Nested Rust repo; clean tracked tree; Cargo manifests and AGENTS.md inspected | Review coverage and checks pending | Review core execution and external boundaries first | Inspect analysis, servers, caches, and CI |
| 2 | 2026-10-09 22:39:30 → 2026-10-09 22:43:55 | 4m 25s | Trace main workflows and verify suspected defects | `cargo test --locked`: exit 0, 2,638 passed; `cargo fmt --all -- --check`: exit 0; `cargo clippy --locked --all-targets -- -D warnings`: exit 101, needless_bool; focused Python and mocked Node probes exit 0 | Optional feature and remaining integrations outside verified scope | Report defects without source edits; targeted probes show gaps missed by existing tests | Deliver prioritized findings and architecture assessment |

| 3 | 2026-10-09 23:08:16 → 2026-10-09 23:11:14 | 2m 58s | Resume under developer tooling lens; validate packaging checks and public contracts | npm test and extension tsc --noEmit exit 0; four CLI probes and two live LSP probes confirm additional defects | Real release downloads, marketplace installation and remote Actions not exercised | Add six evidence-backed tooling findings; source remains unchanged | Deliver supplemental review and remediation priorities |

| 4 | 2026-10-09 23:28:35 → 2026-10-09 23:48:00 | 19m 25s | Implement all authorized review fixes with regression-first verification | Initial CLI suite failed 11 of 12 cases; HTTP, LSP and adapter reproductions failed before fixes. Final cargo test --locked: 2,657 passed, 3 ignored, exit 0; fmt, Clippy, both builds, Node contracts, npm and tsc pass | Remote stable/Linux and real release/editor installation unverified | Share parsed syntax edits across CLI/LSP; preserve existing public signatures with additive fallible APIs; add adapter CI checks | Local atomic commit and delivery |

| 5 | 2026-10-09 23:49:00 | Not measured | Commit the task changes locally | Conventional Commit `fix: enforce analysis and developer tooling contracts` succeeded; 39 task files staged, scratch excluded; no push | Remote CI remains outside authorized scope | Mark local remediation complete; refresh history in the same local commit | Deliver verification and commit reference |

| 6 | 2026-10-09 23:55:07 → 2026-10-09 23:56:13 | 1m 6s | Compare linked loops/graphs concept with our workflow and Falcon code; initial fetch attempts occurred before this measured interval | X direct fetch returned 403; public FxTwitter JSON exposed the post and quoted guide. Inspected Rayon analysis, import graph, fix tracking, tuning, AGENTS generators and previous verified task evidence | Video and attributed Karpathy quotation not verified; no runtime orchestrator demonstrated | Distinguish source dependency graphs from work orchestration; stored history and printed recommendations do not establish automatic planning feedback | Deliver evidence-backed present/partial/missing assessment; no implementation authorized |

| 7 | 2026-10-10 00:03:37 | Not measured | Locate Ocean Assurance graph proofs after user clarification that Falcon is a tool | Local repository has Code-Graph-RAG evaluation record, own repository graph assembly, and a PostgreSQL continuous-work-graph experiment documenting 16 passing scenarios | Historical counts and experiment results inspected, not rerun; production qualification remains outstanding | Reuse source-graph provenance and delta patterns as design references; distinguish continuous-work orchestration from Falcon analysis needs | Explain prior POCs and their relevance; no implementation authorized |

## Verified findings and evidence

1. **P1 — Shell injection in GitHub webhook:** `github-app/index.js:27` interpolates a PR-controlled branch into `execSync`. `git check-ref-format --branch` accepts the harmless substitution-bearing branch used by `target/review-github-shell.js`; a mocked Git shell function received its expanded value. No real clone, external comment, or network action was performed. Use executable plus argument arrays without a shell.
2. **P1 — Automatic fixes change unrelated string literals:** `src/ai/fix.rs:103-105`, duplicated in `src/lsp/actions.rs:62-64`, replaces every `[0]` on the line. Real CLI fix changed `label = '[0]'` to `label = '.first'` alongside `xs[0]`. Use exact syntax spans and preservation assertions.
3. **P2 — Relative incremental paths skip dependents:** `src/main.rs:3134` joins Git output without normalizing; the dependency graph normalizes keys at `src/incremental/dep_graph.rs:29`. Same fixture: relative root analyzes 1 file and reports 0 errors; absolute root analyzes 2 and reports the dependent's dispose error. Normalize both paths consistently.
4. **P2 — Existing-file source overlays suppress resolver findings:** `src/sdk.rs:194-199` appends supplied classes to disk classes. Duplicate same-file declarations make `resolve_class_name` return None. MCP lint_file with source/project_root returns no issue while the file exists; deleting the disk file and sending identical source reports dispose-not-called. Replace classes for the supplied file.
5. **P2 — HTTP framing and connection starvation:** `src/api/server.rs:63` reads only once and ignores Content-Length; `:45-48` handles clients synchronously with no read timeout. A valid POST split across headers/body returns 400 before the body arrives; an idle connection blocks the next /health request beyond its timeout. Use proper framing, bounded connections and deadlines.
6. **P2 — Missing analysis roots return success:** `src/lib.rs:311-316` drops traversal errors. Real CLI check on an absent path exits 0 with a clean zero-file JSON report. Validate roots and report incomplete analysis explicitly.

Probe sources: `target/review-probes.py`, `target/review-github-shell.js`. Results: `target/review-probes.log`, `target/review-tests.log`, `target/review-clippy.log` (ignored build artifacts).

## Architecture assessment

The Rule trait, resolver context, report types, and Rayon file execution provide useful seams. The main weakness is divergent analysis and fix implementations across Falcon, SDK, MCP and LSP, with inconsistent path/error/overlay behavior. Prefer one reusable per-file analysis pipeline and one syntax-span fix representation. CLI dispatch has been grouped into modules, but most implementations remain in the 3,336-line main.rs; move command implementation into those groups incrementally. Retain the current single library and binaries; the observed problems do not require new frameworks or service boundaries.

## Supplemental developer tooling review

User requested this additional review after the architecture/code review. The interval between reviews is excluded from work duration.

1. **P1 — Explicit configuration file silently ignored:** `src/main.rs:612-613` passes the documented --config file path to `FalconConfig::load`; `src/config/mod.rs:51` appends falcon.yaml. A custom error-severity rule returned warning severity and exit 0 with the file argument, versus error severity and exit 1 with its containing directory. Separate loading an explicit file from project config discovery; explicit missing paths must fail.
2. **P2 — Preflight invalidates JSON output:** `src/main.rs:713-716` prints textual preflight reports after the main JSON document. Opt-in preflight plus --format json produced JSONDecodeError (Extra data). Keep one structured document per invocation and put human progress on stderr.
3. **P2 — GitHub Action baseline argument is obsolete:** `action/action.yml:77-78` supplies bare --baseline, but the CLI requires --baseline FILE. The exact resulting argument form exits 2. Accept and forward a concrete baseline path.
4. **P2 — Report write failures return success:** `src/reporters/sarif.rs:124` prints the write error without propagating it; the Reporter trait returns unit. A clean project with a nonexistent output directory exits 0 and creates no report. Make reporting return Result and propagate IO failures.
5. **P2 — Editor Fix All command does no work:** `src/lsp/server.rs:197-200` logs success only. A live LSP session advertised a preferred quick fix for the open document; executing falcon.fixAll returned success with only window/logMessage and no workspace/applyEdit. Implement a verified non-overlapping edit set or withdraw the advertised command until supported.
6. **P2 — Unicode diagnostic positions use bytes:** `src/lsp/diagnostics.rs:44` forwards tree-sitter byte columns as character positions, with no alternative encoding negotiated during initialization. Live probe after an emoji returned column 36 instead of UTF-16 column 34. Microsoft LSP 3.17 position definition confirms default UTF-16: https://raw.githubusercontent.com/microsoft/language-server-protocol/gh-pages/_specifications/lsp/3.17/types/position.md . Convert offsets at the protocol boundary; check diagnostic and edit ranges.

Evidence: ignored `target/devtool-review-probes.py` and `.log`, `target/devtool-lsp-probe.py` and `.log`. npm/falcon npm test: exit 0; editors/vscode existing tsc -p . --noEmit: exit 0. No new full Rust suite run because source is unchanged; earlier default-suite evidence remains recorded separately.

Developer tooling assessment: Public flags, stdout documents, exit statuses, report files and editor actions are contracts. Add release-contract tests around built binaries and shipped adapters, with configurable severity, preflight + JSON, write failures, baseline paths, fix edits and Unicode fixtures. Keep platform/download and editor installation claims distinct from fixture/type-check evidence. Packaging concerns inspected but no additional unverified installer defect is claimed.

## Remediation authorization

User requested “Fix all of them” after both reviews. Scope now includes fixing all twelve verified findings and the observed Clippy blocker. Original read-only review scope and evidence above remain preserved. Work resumed at 2026-10-09 23:28:35 Asia/Kolkata; time between reviews and remediation is excluded. Current milestone: failing regression coverage before targeted implementation. Implementation plan: `.tmp/TASK.md` (not staged).

## Remediation outcome and evidence

- Webhook execution uses executable/argument arrays and isolated temporary checkouts with cleanup. Action inputs use Bash arrays, including an explicit baseline filename. Both adapters have hermetic Node tests and a CI job.
- CLI and LSP share parsed syntax edits, preserving strings/comments and combining non-overlapping fixes. Fix All now sends workspace edits; protocol diagnostics and edits use UTF-16 positions.
- Incremental paths use canonical identities; edited-source overlays replace disk declarations. Missing roots and incomplete source reads now return errors.
- HTTP reads bounded Content-Length requests with a total deadline, a bounded queue and worker pool. Trickle tests initially exposed a per-read timeout gap (4.83 seconds); switching to a total deadline passes the live regression.
- Explicit config files are loaded directly and missing explicit configs fail. Preflight findings join the structured report. File report IO errors propagate through additive fallible Reporter methods, preserving existing public methods.
- Existing tests expecting nonexistent sources to appear clean, and double-literal tests expecting the reverse of the rule's documented behavior, were updated to the corrected contracts. Cases were retained; malformed syntax fixtures were made parseable. A further regression verifies existing const Spacer transformations.
- Final Rust suite log: ignored `target/remediation-final-tests.log`. Focused red evidence: `target/remediation-red.log`, `target/http-red.log`, `target/lsp-red.log`, `target/github-red.log`, `target/action-red.log`. Commands: `cargo test --locked`, `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked --all-targets`, `cargo build --locked --features ai-local`, Node adapter tests, `npm test` in npm/falcon, and existing extension `tsc -p . --noEmit`; all final executions exited 0.

## Loops and graphs comparison

Source: [user's post](https://x.com/callanxai/status/2108285248223986139), quoting [Hanako's guide](https://x.com/hanakoxbt/status/2091515787366306154). Text was retrieved from the public FxTwitter JSON mirror after X blocked direct access. Attribution to Karpathy and the embedded video remain unverified.

Our development workflow already reproduces failures, corrects scoped changes, checks deterministic tests/lints/builds, and retains regression tests and a durable goal history. Independent verification calls are batched where appropriate. This is an evidence-backed manual workflow, not a persisted graph scheduler.

Falcon provides deterministic rule/metric workers in `src/lib.rs:83`, parallel file execution at line 90, and a source import/dependent graph in `src/incremental/dep_graph.rs:8`. The import graph controls re-analysis scope, not agent jobs. `src/agents/mod.rs` generates instructions rather than scheduling agents. `src/ai_score/fix_tracking.rs:74` persists explicitly recorded outcomes. `src/ai_score/self_tune.rs:76` records trigger/suppression counts; its acceptance/rejection fields are initialized to zero and are not wired to fix-history consumption. `src/main.rs:2408` prints tuning recommendations rather than applying constraints to future task planning.

Missing capability in inspected paths: persisted work-unit IDs, declared task dependencies, resumable state, targeted bounded retries, and accepted evidence converted into constraints consumed by the next task planner. Regression tests are already durable executable constraints. A small future orchestration layer could connect them to planning without replacing Falcon's analysis engine or adding a graph database.

## Ocean Assurance reference and scope correction

User clarified Falcon is a tool and asked whether Ocean Assurance already has a graph POC. Inspected `/Users/advait/Documents/Codex/from-daily/other-folders/Documents/ocean/repo/ocean-assurance`: `docs/decisions/agent-conversation-decisions.md` records an opt-in Code-Graph-RAG 0.0.945 trial (15,185 nodes; 58,696 relationships); `repository-analysis/src/graph.ts` implements snapshot-pinned repository/file/symbol graphs with observed/inferred edges and provenance, with graph deltas. Separately, `experiments/continuous-work-graph/README.md` reports 16 PostgreSQL scenarios passing on 2026-10-01, with production qualification still outstanding. These are different graph purposes. Earlier suggestion to add agent-task orchestration was a possible extension, not a requirement; for Falcon the relevant reference is the source relationship graph and evidence identity. No Ocean files changed; no experiment rerun.

## Project graph implementation authorization

User requested automatic graph generation when Falcon is used on a project. Work began with repository/skill inspection; measured history starts 2026-10-10 02:30:39 Asia/Kolkata (UTC+05:30). Starting tracked change is this task-owned history from prior discussions. Success criteria: deterministic source graph with provenance and unresolved facts, project check persistence, explicit JSON export, preserved library APIs, targeted regressions and existing checks. Scope is code relationships, not agent orchestration.

| # | Time (start → end) | Duration | Action / investigation | Evidence / result | Gaps | Decision and reason | Next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| G1 | 2026-10-10 02:30:39 → 2026-10-10 02:45:21 | 14m 42s | Inspect existing analysis/parser/resolver, define additive source graph and CLI contracts, write integration regressions before production | Eight initial contract tests failed before production implementation (target/graph-red.log); scope regressions exposed prefix/parameter/part visibility, repeated show, type parameter and export/import distinctions, then passed after corrections. Final 18 graph contracts plus atomic-temp preservation regression pass; full suite 2,676 passed, 3 ignored; fmt, Clippy, both builds pass. Real companion export: 23 nodes, 25 valid-endpoint edges | External SDK/packages and receiver dispatch remain unresolved; graph is disk-content observation rather than atomic filesystem/Git snapshot | Existing tree-sitter/sha2, versioned deterministic relative identities, provenance, explicit ambiguity; default check persistence with no-graph opt-out; preserve old public API/result structs | Local task-only commit and delivery |

## Project graph outcome

Implemented `src/project_graph/{types,extract,resolve,mod}.rs`, public `Falcon::generate_project_graph` and SDK method; `falcon graph` exports JSON and directory `check` persists `.falcon-data/project-graph.json` atomically. Full graph generation also occurs for incremental checks; single-file checks preserve the existing artifact. `--graph-output` selects an output path; `--no-graph` opts out. Existing source edits and report stdout are unchanged. Types, source/identity contracts, coverage and performance limits are documented in `docs/project-graph.md`; no external dependency added.

Verification: final `cargo test --locked` exits 0 (2,676 passed, 3 ignored); `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked --all-targets`, `cargo build --locked --features ai-local` exit 0. Final suite log is ignored `target/graph-final-tests.log`. Red evidence is `target/graph-red.log`, `target/graph-scope-red.log`, `target/graph-visibility-red.log`, `target/graph-library-red.log`, `target/graph-write-red.log`. Actual CLI export of falcon_dart to ignored `target/falcon-dart-project-graph.json` passes ID uniqueness and edge endpoint checks (23 nodes/25 edges). Remote CI was not run. Other work produced untracked site files during verification; those are not graph task files and remain untouched.
