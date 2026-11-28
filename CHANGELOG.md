# Changelog

## 0.7.0 (2026-11-28)

The running-app MCP tools are new in this version. Falcon 0.6.0 does not have them. They have been run end to end on macOS desktop and the iOS simulator only ([docs/loop-evidence](docs/loop-evidence/README.md)); Android, physical iOS devices, Windows, Linux and Flutter web are untested, and `screenshot` does not support Flutter web.

### Removed

- The pub.dev leaderboard: the static generator (`src/leaderboard.rs`), the weekly `leaderboard.yml` workflow, its scraping script, and its docs and opt-out list. Its `falcon x leaderboard` command had already been lost in a merge before v0.5.0, and the weekly job never completed a run.

### Fixed (release blockers)

- `falcon-mcp` no longer answers JSON-RPC notifications such as `notifications/initialized`. JSON-RPC 2.0 forbids replies to messages without an `id`, and strict MCP clients reject them.
- `falcon x enterprise check` exits 1 only when a `Block` policy fails. `Warn` and `Audit` failures are still reported, marked `(warn)` or `(audit)`, but no longer fail CI.
- Tag releases now run `cargo test --locked` before building, and build release binaries with `--locked`.
- GitHub Pages: deploys no longer nest Markdown docs under `docs/docs/`. `scripts/build_site.sh` copies them next to the committed HTML docs and fails if those go missing.
- GitHub App: `npm start` now runs `probot run ./index.js` (it previously exited without starting a server). PRs are fetched from the base repository with the installation token, so private repositories and PRs from deleted forks work, and `falcon review` diffs the whole PR against its base branch instead of failing on a one-commit clone. Commands run asynchronously, `.dart_tool/` is stripped from untrusted checkouts so a PR cannot enable `dart analyze`, and the unused `stripe` dependency is removed.

### Changed

- The six behavioral rules (`dispose-not-called`, `set-state-after-dispose`, `unawaited-future-in-build`, `fake-mounted-check`, `silent-catch`, `riverpod-scope-leak`) are now enabled by default at warning severity, and at error in the `ai-generated` and `strict` presets. Projects without their own `rules:` list may see new warnings; `--fail-on error` pipelines are unaffected.

### Fixed

- `interact`: an exception thrown by the app after a tap, long press or typed text (an `onPressed` that throws, for example) is now in `logs` as Flutter reports it. The events were sent from a closure inside the evaluated expression, and Flutter cannot print an error whose stack has such a frame, so `logs` showed a `stack_frame.dart` assertion and the real exception was lost.
- `logs` with `errors_only` returns the whole `══╡ EXCEPTION CAUGHT … ╞══` block on macOS and iOS. It returned only the header there, because every app line starts with `flutter: `.
- `interact`: `tap` and `long_press` accept `index` without `find`, meaning the element `widget_tree` lists at that index. They failed with "needs `find`".
- `screenshot`: the inspector capture (the path used under Impeller) is cut to the screen. A widget that overflowed made the image continue below the screen. The capture is taken at the screen's own resolution; when the painted area is larger than 4096 pixels it is taken smaller.
- `falcon score` no longer prints "Production Ready — qualifies for Falcon Certified badge" for a project with a hardcoded credential. The label is capped at "Needs Improvement" and the score webhook's `production_ready` is false, whatever the number.
- `avoid-hardcoded-credentials` flags AWS access keys (`AKIA…`, `ASIA…`), Stripe live keys, GitHub tokens, Slack tokens and private key blocks by their format. It only matched variable names such as `apiKey`, so `awsId = 'AKIA…'` passed. Projects with such keys in source will see new findings.
- The MCP `score` tool (`falcon_ai_score`) reports `production_ready: false` for a project with a hardcoded credential, as `falcon score` does, and returns `hardcoded_credentials`. It decided from the number alone.
- Frame statistics (`x devtools trace`, `runtime-check`, MCP `app_metrics`) come from the app's `Flutter.Frame` events. They asked for a `getRenderStats` extension that Flutter does not have, and the frame history replayed on connect is no longer counted.
- Rebuild counts (`x devtools rebuilds`, MCP `app_metrics`) turn on Flutter's rebuild tracking and count `Flutter.RebuiltWidgets` events, named by source location. They were always empty. `x devtools rebuilds` gains `--duration`.
- Hot reload and hot restart go through the services `flutter run` registers, so they behave like pressing `r` and `R`. A restart without `flutter run` says why it cannot work, and MCP `reload` returns an error when the reload fails instead of a success with a message.
- `screenshot` falls back to `ext.flutter.inspector.screenshot` when the engine refuses (it does under Impeller), and `--window` on macOS reads the window bounds correctly.
- `interact` waits for the current frame to finish before acting, and taps a point of the widget that a hit test reaches, so a widget partly covered by another is still tapped and a covered one is refused.
- `interact` with `index` alone refuses an element that has no position, instead of tapping the top-left corner of the screen.
- `x run` writes each "Another exception was thrown" line as its own error.
- `avoid-long-functions` measures a top-level function from its signature to the end of its body. `check-widgets` only looks at `build` methods and ignores `setState` inside closures. `upgrade-check` ignores strings and comments and matches whole identifiers. `explain` covers every registered rule.
- Network requests that end without a status (connection refused) count as failed, and `verify --target app_test.dart` finds the file in `integration_test/`.
- `falcon x api` no longer resets connections it rejects (408, 413, 400) before the client reads the response.
- `falcon devtools inspector`, `falcon devtools tree-diff` and `falcon journey` read the widget tree again. The VM Service wraps extension replies in a `{"type": "_extensionType", "result": …}` envelope, which was taken for the root node, so the tree came back as one node and journeys labelled every screen `_extensionType`.
- Four preset entries named rules that do not exist (`avoid-unnecessary-set-state`, `avoid-throw-in-catch`, `always-override-equals-hashcode`, `avoid-collection-methods-unrelated-types`), so those rules were silently skipped. They now use the real names, and a test checks every preset and default entry against the rule registry.
- A `falcon.yaml` without a `rules:` key (for example, one that only tunes `metrics:`) ran no lint rules at all. A missing key now means the default rule set; an explicit `rules: []` still disables rules. Projects with such a config will start seeing findings.
- The AI Code Quality Score looked up `avoid-throw-in-catch` instead of `avoid-throw-in-catch-block`, so throw-in-catch findings never lowered the Error Handling dimension. Scores for projects with those findings may drop slightly.
- The README no longer suggests `npx falcon-flutter`, which is not published on npm.

### Added

- `falcon-mcp` can run and drive the app, so an agent can check its own change. New tools: `run_app` and `stop_app` keep one `flutter run` alive across calls; `reload` hot reloads through it and reports compile errors; `screenshot` returns the screen as an image; `widget_tree` lists on-screen text, buttons and fields with positions; `interact` taps, long presses, types, scrolls (paging through lazy lists), goes back and waits, refusing ambiguous matches and covered widgets; `logs` returns app output with whole Flutter error blocks; `tree_diff`, `app_metrics` (memory, network, performance, profiler, rebuilds, trace, routes), `journey` and `verify` expose the existing runtime commands. All of them are refused over the HTTP bridge.
- `falcon-mcp` also advertises `score`, `conventions` and `generate_tests` (golden or unit, preview by default); `falcon_ai_score` and `falcon_conventions` are now aliases of the first two. The surface is 20 tools.
- `falcon-mcp` negotiates the MCP protocol version (2025-06-18, 2025-03-26, 2024-11-05), sends progress notifications for long calls, and serves prompts (`dev_loop`, `review_ai_code`) and resources (`falcon://rules`, `falcon://rules/{name}`, `falcon://session`).
- `docs/loop-evidence`: the commands and output of one recorded agent run through `run_app`, `interact`, `logs`, `reload`, `screenshot` and `verify` on macOS desktop and the iOS simulator, with what the run does not show.
- `LICENSE` (MIT) at the repository root and in the published crate, and a `SECURITY.md` describing how to report vulnerabilities privately.

### Security

- `falcon x api` now requires `Authorization: Bearer <token>` on every endpoint except `GET /health`. A random token is generated and printed at startup; set `FALCON_API_TOKEN` (at least 16 characters) to pin one. POST bodies must be `Content-Type: application/json`, and responses no longer send `Access-Control-Allow-Origin: *`, so web pages can no longer drive or read the API from a browser.
- `falcon x api` refuses to bind a non-loopback host unless `--allow-remote` is passed.
- `falcon x update` verifies the downloaded archive against the release's `SHA256SUMS` before installing and refuses releases that publish none. Downloads are HTTPS-only, fail on HTTP errors, and no longer send a GitHub token on the curl command line.

## 0.6.0 (2026-10-10)

### Added

- Project source graphs with stable identities, source hashes, spans and observed/inferred relationship provenance. Directory `falcon check` writes `.falcon-data/project-graph.json`; `falcon graph` exports JSON. Use `--no-graph` to skip generation or `--graph-output` to select a path.
- Additive graph generation APIs for Falcon and the SDK, with explicit unresolved relationships and coverage limits.
- `falcon doctor` — diagnoses Flutter, Dart, CocoaPods, Android SDK and Xcode for the current project, and installs or repairs what it can. Reads the Flutter release manifest to offer the version your project pins alongside the latest on your channel; verifies checksums; prints (never edits) the PATH line.
- `doctor` MCP tool — two-phase: call it with a path to get every check's status and the questions each fix needs, then again with `execute: true` and `decisions` to perform the install. Execution is refused over the HTTP bridge.

### Fixed

- Automatic fixes preserve unrelated strings and comments; editor Fix All applies syntax edits and uses UTF-16 positions.
- Incremental checks include dependent files for relative roots, and source overlays replace disk declarations.
- Explicit configuration paths, missing analysis roots, incomplete source reads and report write failures now produce correct results or errors.
- Preflight findings stay within one structured JSON report.
- HTTP requests use bounded framing, connections and total read deadlines.
- GitHub webhook commands avoid shell interpolation, and the Action accepts an explicit baseline filename.

## 0.5.3 (2026-09-12)

### Fixed

- `cargo install falcon` references (VS Code extension docs, GitLab/Bitbucket CI templates, the `falcon-lint/action` GitHub Action) updated to `falcon-flutter` — these installed an unrelated crate after the package rename in 0.5.2
- npm package renamed from the squatted `falcon` (an unrelated abandoned package) to `falcon-flutter`, so `npx falcon-flutter@latest` actually installs this project
- `scripts/release.sh` now keeps `npm/falcon/package.json`'s version in sync with `Cargo.toml`

### Added

- `scripts/install.sh` — a `curl | sh` installer that downloads, checksum-verifies, and installs the release binaries with no Rust/npm/Homebrew required, with an optional `ff` shortcut alias for `falcon`
- `cargo binstall` support via `[package.metadata.binstall]` in `Cargo.toml`

## 0.5.2 (2026-09-09)

### Added

- `falcon screenshot` for one-command Flutter screenshots
- `--window` for native macOS app-window capture when a VM-service screenshot is unavailable

### Changed

- Moved screenshot capture out of the nested `falcon devtools` command group while preserving VM-service capture options

## 0.5.1 (2026-07-23)

### Changed

- Fresh release after the v0.5.0 tag was created before master finished syncing
- Includes the v0.5.0 navigation graph release plus the latest CLI dispatch and release preflight integrations from origin/master

### Stats

- 61 lint rules
- 2,435 test definitions
- 90+ CLI commands
- 263 source files (78,413 lines of Rust)

## 0.5.0 (2026-07-23)

### Added

- Versioned, JSON-serializable navigation graph API for Flutter applications
- GoRouter and FlutterFlow route, screen, and navigation-edge extraction
- Contract checks for orphan routes, dangling references, and unguarded sensitive screens
- Navigation graph diffs with broken-edge detection
- Scoped Mermaid diagrams and Markdown pull-request summaries
- 28 contract tests covering extraction, checks, diffs, rendering, and schema stability

### Stats

- 61 lint rules
- 2,435 test definitions
- 90+ CLI commands
- 263 source files (78,413 lines of Rust)

## 0.4.0 (2026-05-05)

### Stats
- 55 lint rules
- 460 tests passing
- 93 CLI commands
- 214 source files (45480 lines of Rust)

### Changes
- Began the four-verb CLI cutover: default `falcon --help` now highlights
  `review`, `check`, `fix`, `score`, and `x`, while `falcon --legacy-help`
  preserves the historical command list for one release.
- Added `falcon check` as the stable project-checking verb. `falcon analyze`
  remains available during the migration window.
- Added `docs/cli-migration.md` with the command migration table for v1.0.

### CLI Migration

| Before | Now |
|---|---|
| `falcon analyze .` | `falcon check .` |
| `falcon ai-score .` | `falcon score .` |
| `falcon pr-comment . --base-ref origin/main` | `falcon review . --base-ref origin/main --format gh` |
| `falcon asset-audit .` | `falcon x asset-audit .` |
| `falcon theme-audit .` | `falcon x theme-audit .` |
| `falcon l10n-coverage .` | `falcon x l10n-coverage .` |
| `falcon deeplink-validate .` | `falcon x deeplink-validate .` |
| `falcon animation-audit .` | `falcon x animation-audit .` |
| `falcon golden-gen .` | `falcon x golden-gen .` |
| `falcon dep-graph .` | `falcon x dep-graph .` |
| `falcon workspace .` | `falcon x workspace .` |
| `falcon docs docs/` | `falcon x docs docs/` |
| `falcon vuln-scan .` | `falcon x vuln-scan .` |
| `falcon refactor-sim . --scenario migrate-to-riverpod` | `falcon x refactor-sim . --scenario migrate-to-riverpod` |
| `falcon test-gen .` | `falcon x test-gen .` |

# Changelog

## 0.3.0 (2026-05-05)

### Stats
- 55 lint rules
- 460 tests passing
- 93 CLI commands
- 214 source files (45480 lines of Rust)

### Changes
- 306ae3e chore: gitignore .claude/ (per-user editor config)
- 3f49c02 chore: cargo fmt sweep across rules, analysis, ai_score, tests
- 0bd69dd feat(cli): wire smells, agents, live, flutter, fvm subcommands
- 4900b29 feat(flutter_run): VM service URI capture for flutter run -d chrome
- 99906eb feat(runtime): add live mode and tighten memory leak scoring
- fb1cd62 feat(agents): generate AGENTS.md for Codex/Cursor/Aider parity with Claude
- 0bc752c feat(smells): categorize findings into dead code, code smells, security smells
- 7a8d835 Add DevTools runtime commands
- 7512051 feat: add 7 new Flutter-quality analysis commands
- 7f2258c chore: update gstack solve state for branch comparison feature
- e451e86 feat: organize CLI commands into semantic groups
- 5ebfe75 docs: comprehensive v0.2.0 developer documentation update
- 11684c3 feat: update falcon_cli Dart package to v0.2.0 for pub.dev
- 9ceb967 fix: self-update uses gh CLI auth for private repos

# Changelog

All notable changes to Falcon are documented in this file.

## [0.2.0] - 2026-03-20

### Added

- **Enterprise HTML Reports**: Complete dashboard redesign with sidebar navigation, dark/light theme toggle, SVG charts, and interactive tabs (Overview, Issues, Metrics)
- **Project Properties**: Parses `pubspec.yaml` and displays project name, version, SDK constraints, and dependencies in the report header
- **Level of Concern**: Visual breakdown of issues across 8 categories (Security, Error Handling, Type Safety, Complexity, Performance, Resource Safety, Code Smells, Conventions) with severity gauges
- **Test Coverage Section**: Maps source files to test files and shows coverage percentages with ring gauges and stacked bar visualization
- **Branch Comparison** (`falcon x compare-branches`): Analyzes two git branches side-by-side with auto stash/restore safety, shows delta with color-coded indicators, and generates HTML comparison reports
- **Report History**: Auto-saves analysis snapshots after every `falcon check` run to `.falcon-data/history.json`
- **History Viewer** (`falcon x history`): Lists all stored analysis runs with timestamp, branch, health score, and issue counts
- **Report Comparison** (`falcon x compare-reports`): Compares any two stored runs with `--run1 N --run2 M` flags, supports HTML output
- **Self-Update** (`falcon x update`): Updates Falcon from GitHub releases, supports `--version X.Y.Z` for specific versions and `--list` for available versions
- **Rich Console Icons**: Icons throughout CLI output for branches, files, health, issues, errors, metrics, and more

### Fixed

- **Floating-point delta display**: Fixed `-0` showing as "Declined" when health score is unchanged (e.g., 81.3 → 81.0 now correctly shows "Unchanged")

### Changed

- `AnalysisReport` struct now includes `project_path` field for project-aware reporting
- HTML report title now shows project name instead of generic "Falcon Analysis Report"

## [0.1.0] - 2026-03-18

### Added

- Initial release
- 61+ lint rules for Flutter/Dart (flutter, BLoC, Riverpod, accessibility, equatable)
- Code metrics: cyclomatic complexity, maintainability index, Halstead volume, class metrics (WMC, CBO, DIT, RFC, LCOM, TCC, WOC)
- Unused code detection (files, declarations, parameters, dead code, localization keys)
- AI Code Quality Score (0-100) with 6-dimension breakdown
- Multiple output formats: console, JSON, HTML, SARIF, CodeClimate, Checkstyle, SonarQube
- MCP server for AI tool integration (Cursor, Windsurf)
- LSP server for VS Code
- GitHub PR comments
- Webhook notifications
- Performance benchmark
- Rule presets (recommended, strict, flutter, riverpod, bloc, performance, ai-generated)
- Codebase intelligence (health score, god files, hotspots, tech debt)
- Cognitive complexity analysis
- Widget rebuild detection
- Async anti-pattern detection
- Cyclic dependency detection
- Convention detection and drift analysis
- Vulnerability scanning
- Flutter upgrade compatibility checks
- Clean architecture layer enforcement
- Baseline support for incremental adoption
- Watch mode for continuous analysis
- 470+ tests
