# Changelog

## Unreleased

### Added

- `falcon doctor` — diagnoses Flutter, Dart, CocoaPods, Android SDK and Xcode for the current project, and installs or repairs what it can. Reads the Flutter release manifest to offer the version your project pins alongside the latest on your channel; verifies checksums; prints (never edits) the PATH line.
- `doctor` MCP tool — two-phase: call it with a path to get every check's status and the questions each fix needs, then again with `execute: true` and `decisions` to perform the install. Execution is refused over the HTTP bridge.

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
