# Changelog

## 0.4.0 (2026-05-05)

### Stats
- 55 lint rules
- 460 tests passing
- 93 CLI commands
- 214 source files (45480 lines of Rust)

### Changes
- 

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
- **Branch Comparison** (`falcon compare-branches`): Analyzes two git branches side-by-side with auto stash/restore safety, shows delta with color-coded indicators, and generates HTML comparison reports
- **Report History**: Auto-saves analysis snapshots after every `falcon analyze` run to `.falcon-data/history.json`
- **History Viewer** (`falcon history`): Lists all stored analysis runs with timestamp, branch, health score, and issue counts
- **Report Comparison** (`falcon compare-reports`): Compares any two stored runs with `--run1 N --run2 M` flags, supports HTML output
- **Self-Update** (`falcon update`): Updates Falcon from GitHub releases, supports `--version X.Y.Z` for specific versions and `--list` for available versions
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
