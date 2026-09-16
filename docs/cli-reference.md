# CLI Reference

Falcon is migrating to a compact top-level CLI: `review`, `check`, `fix`,
`score`, and `x`. See [CLI Migration](cli-migration.md) for the v1.0 command
mapping. The full historical help remains available for one release with
`falcon --legacy-help`.

## Core Analysis

| Command | Description |
|---|---|
| `falcon check [path]` | Full analysis (metrics + rules + unused detection) |
| `falcon x smells [path]` | Categorized dead code, code smell, and security smell report |
| `falcon x metrics [path]` | Calculate code metrics only |
| `falcon score [path]` | AI Code Quality Score (0-100) with 6-dimension breakdown |
| `falcon x ai-report [path]` | Full "State of AI-Generated Flutter Code" report |

### falcon check

```bash
falcon check . --format console --preset ai-generated --fail-on error
```

| Flag | Description | Default |
|---|---|---|
| `--format` | Output format: console, json, html, sarif, codeclimate, checkstyle, sonar | console |
| `--output` | Output file path | falcon-report.html |
| `--config` | Path to falcon.yaml | auto-detect |
| `--since` | Only analyze files changed since git ref | none |
| `--baseline` | Only report new violations | false |
| `--fail-on` | Minimum severity to fail: error, warning, info | error |
| `--preset` | Rule preset: recommended, strict, flutter, ai-generated | none |

#### HTML Report (v0.2.0)

```bash
falcon check . --format html --output report.html
```

Generates an enterprise-grade HTML dashboard with:
- Project properties (name, version, SDK, dependencies from pubspec.yaml)
- Health score ring gauge
- Level of Concern breakdown (Security, Error Handling, Type Safety, etc.)
- Severity distribution donut chart
- Top rules bar chart
- Complexity hotspots table
- Test coverage analysis (source-to-test file mapping)
- Issues grouped by file with search and severity filters
- Function and class metrics tables
- Dark/light theme toggle

### falcon score

```bash
falcon score . --badge --json
```

`falcon ai-score` remains available as a deprecated alias until v1.0.

| Flag | Description |
|---|---|
| `--badge` | Print shields.io badge markdown |
| `--json` | Output as JSON |

## Code Checks

| Command | Description |
|---|---|
| `falcon x check-unused-code` | Unused code declarations |
| `falcon x check-unused-files` | Unused Dart files |
| `falcon x check-dependencies` | Unused pubspec.yaml dependencies |
| `falcon x check-cycles` | Cyclic import dependencies |
| `falcon x check-unused-params` | Unused function parameters |
| `falcon x check-dead-code` | Unreachable code after return/throw |
| `falcon x check-unused-l10n` | Unused localization keys |
| `falcon x check-promoted-deps` | Over/under-promoted dependencies |
| `falcon x check-platform` | Kotlin/Swift platform channel issues |
| `falcon x check-codegen` | Code generation quality (.g.dart, .freezed.dart) |
| `falcon x check-perf` | DevTools-style performance analysis |
| `falcon x check-unused-confidence` | Score unused code findings by confidence |
| `falcon x check-layers` | Clean architecture layer enforcement |
| `falcon x check-imports` | Import restriction rules |
| `falcon x check-widgets` | Widget rebuild issues and build method complexity |
| `falcon x check-async` | Async/await anti-patterns (async void, unawaited futures) |
| `falcon x cognitive-complexity` | Function cognitive complexity |
| `falcon x codebase-intel` | Health score, god files, hotspots, tech debt estimate |

## Comparison & History (v0.2.0)

### falcon x history

List all stored analysis runs for a project.

```bash
falcon x history /path/to/project
```

Shows a table with timestamp, branch, file count, health score, issue count, and commit hash. Snapshots are automatically saved after every `falcon check` and `falcon x compare-branches` run.

### falcon x compare-reports

Compare two stored analysis runs from history.

```bash
falcon x compare-reports /path/to/project --run1 1 --run2 3
falcon x compare-reports /path/to/project --run1 1 --run2 3 --output comparison.html
```

| Flag | Description | Default |
|---|---|---|
| `--run1` | Run number for baseline (1-based) | second-to-last |
| `--run2` | Run number for comparison | latest |
| `--output` | Generate HTML comparison report | none |

### falcon x compare-branches

Compare analysis results between two git branches. Checks out each branch, runs full analysis, then shows the delta.

```bash
falcon x compare-branches /path/to/project --base main --branch feature/my-feature
falcon x compare-branches . --base main --branch dev --output comparison.html
```

| Flag | Description | Default |
|---|---|---|
| `--base` | Base branch name (required) | — |
| `--branch` | Branch to compare (required) | — |
| `--output` | HTML comparison report path | `falcon-branch-comparison.html` |
| `--config` | Path to falcon.yaml | auto-detect |

The comparison shows:
- Health score delta with direction indicator
- File count, LOC, and total issues delta
- Error/warning/info breakdown
- Metrics delta (cyclomatic complexity, maintainability, god files)
- Top 10 rule changes with counts
- New rules detected and rules resolved

Both branch snapshots are saved to history for later re-comparison.

## Self-Update (v0.2.0)

### falcon x update

Update Falcon to the latest or a specific version.

```bash
falcon x update                    # Update to latest
falcon x update --version 0.2.0   # Install specific version
falcon x update --list             # Show version info and available releases
```

| Flag | Description |
|---|---|
| `--version` | Target version (e.g. 0.2.0). Omit for latest. |
| `--list` | List current version info, platform, and available releases |

The update mechanism downloads pre-built binaries from GitHub releases. If no binary is available for your platform, it provides `cargo install` fallback instructions.

## AI Features

| Command | Description |
|---|---|
| `falcon x provenance [path]` | Detect AI-generated vs human-written files |
| `falcon x conventions [path]` | Auto-detect team naming/architecture patterns |
| `falcon x drift [path]` | Detect convention drift in new code |
| `falcon x predict [path]` | Predict production risks from code patterns |
| `falcon x discover-rules [path]` | Propose new rules from observed patterns |
| `falcon x ai triage [path] --format text\|json` | Embedded AI false-positive triage entry point |
| `falcon x refactor-sim --scenario <s>` | Simulate refactoring impact |
| `falcon x test-gen [path]` | Generate test stubs from code analysis |
| `falcon x vuln-scan [path]` | Security vulnerability radar |
| `falcon x upgrade-check [path]` | Flutter upgrade compatibility |

### falcon x ai triage

```bash
falcon x ai triage . --format text
falcon x ai triage . --format json
cargo run --features ai-local -- x ai triage .
```

`falcon x ai triage` is the embedded AI false-positive review entry point. In
the default build it exits successfully and reports that embedded triage
requires rebuilding with `--features ai-local`. With `ai-local` enabled, the
command analyzes findings, loads the configured Qwen2.5 GGUF model once, and
returns triage-shaped text or JSON. If per-issue inference or verdict parsing
fails, that issue degrades safely to `is_real: true`, `confidence: 0`. First
use downloads the model and tokenizer into Falcon's model cache under
`XDG_CACHE_HOME` or `~/.cache/falcon/models`. This command is an explicit
embedded-triage action: it uses `ai.embedded` when present, otherwise embedded
defaults, even if the general `ai.enabled` flag is off or another provider is
selected for other AI surfaces.

| Flag | Description | Default |
|---|---|---|
| `[path]` | Path to analyze | `.` |
| `--format` | Output format: `text` or `json` | `text` |

## CI/CD

| Command | Description |
|---|---|
| `falcon review [path] --format gh` | Print PR-ready markdown for changed Dart files |
| `falcon review [path] --format json` | Print JSON findings for changed Dart files |
| `falcon review [path] --format sarif` | Print SARIF findings for changed Dart files |
| `falcon x webhook --url <url>` | Send webhook notification |
| `falcon fix [path]` | Auto-fix lint issues |
| `falcon x benchmark` | Run performance benchmark |
| `falcon x compare` | Compare Falcon vs dart analyze |

`falcon review` defaults its base ref to `origin/main` and uses
`git diff --name-only --diff-filter=ACMR <base-ref>...HEAD` to analyze only
changed `.dart` files. Use `--base-ref <ref>` or the legacy `--diff <ref>`
alias to compare against a different ref.

Use `--strictness quick` to report only error-level findings, `standard` for
the default lint findings plus standard review observations, and `thorough` for
the extra review observations such as missing-test checks.

For Dart projects with `.dart_tool/package_config.json`, review auto-enables
semantic mode. It runs `dart analyze --format=json` and suppresses Falcon
findings on the same file, line, and rule class as analyzer diagnostics.
Style analyzer diagnostics can suppress style findings, for example, but not
Falcon behavioral or security findings on the same line. `--semantic` keeps this
behavior explicit, and `--no-defer-to-analyzer` (`--no-defer`) keeps Falcon
findings even when the analyzer reports the same file, line, and class.

`falcon check` stays syntactic unless `--semantic` is passed. Keep editor-save
and pre-commit hooks syntactic by default; use semantic mode in review or CI
where the analyzer shellout is expected.

### Baselines

Use baselines to suppress findings that are already known while keeping new
findings visible:

```bash
falcon review . --base-ref origin/main --format json --update-baseline .falcon-baseline.json
falcon review . --base-ref origin/main --format json --baseline .falcon-baseline.json
falcon check . --format json --baseline .falcon-baseline.json
```

Baseline files are JSON with `schema_version: 1`, Falcon's package `version`,
`created_at`, and `entries`. Each entry records the rule, project-relative file,
line, and a message hash. `--update-baseline` rewrites the file with the current
unfiltered findings; `--baseline` reports only findings not present in that
file.

### GitHub Code Scanning

Use SARIF output with GitHub's Code Scanning upload action to surface Falcon
findings in the repository Security tab. The workflow needs
`security-events: write` permission.

```yaml
name: Falcon Code Scanning

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read
  security-events: write

jobs:
  falcon:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: actions-rust-lang/setup-rust-toolchain@v1

      - run: cargo install falcon-flutter

      - run: falcon review . --base-ref origin/main --format sarif > falcon-results.sarif

      - uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: falcon-results.sarif
          category: falcon
```

The bundled GitHub Action can do the same upload for full-project analysis:

```yaml
- uses: viveky259259/falcon/action@main
  with:
    path: .
    sarif-upload: "true"
```

## Toolchain

| Command | Description |
|---|---|
| `falcon doctor [path]` | Diagnose the project's toolchain (Flutter, Dart, CocoaPods, Android SDK, Xcode) and offer to fix what's missing |

### falcon doctor

```bash
falcon doctor --fix --yes
```

| Flag | Description | Default |
|---|---|---|
| `--fix` | Apply fixes without asking to confirm each one | false |
| `--yes` | Accept the recommended answer to every decision question and never prompt | false |
| `--dry-run` | Print the plan without executing anything | false |
| `--only` | Only run these checks (comma-separated: flutter, dart, cocoapods, android, xcode) | all |
| `--skip` | Skip these checks | none |
| `--channel` | Flutter channel to install (stable, beta, master) | stable |
| `--flutter-version` | Flutter version to install (x.y.z, `latest`, or `project`) | latest |
| `--dir` | Directory to install the SDK into | `~/development/flutter` |
| `--format` | Output format: text, json | text |

Falcon never runs `sudo` and never accepts a licence agreement on your
behalf — those steps print as instructions for you to run yourself. See the
"`falcon doctor` — fix your toolchain" section in [README.md](../README.md)
for the full behavior.

## Dashboard

| Command | Description |
|---|---|
| `falcon x dashboard snapshot` | Capture and save analysis snapshot |
| `falcon x dashboard history` | View snapshot history |
| `falcon x dashboard serve` | Start web dashboard server |
| `falcon x trends [--last N]` | Show quality trends from history |
| `falcon x rule-impact` | Analyze rule impact and auto-tune |

## Platform

| Command | Description |
|---|---|
| `falcon x cloud init --team <name>` | Initialize team cloud config |
| `falcon x cloud dashboard` | Team dashboard |
| `falcon x enterprise init` | Initialize enterprise policies |
| `falcon x enterprise check` | Check policies |
| `falcon x enterprise compliance` | Generate compliance report |
| `falcon x certify [path]` | Evaluate for Falcon certification |

## Infrastructure

| Command | Description |
|---|---|
| `falcon x mcp` | Start MCP server (stdio) for AI tool integration |
| `falcon x api --port 8090` | Start HTTP API server |
| `falcon x init` | Generate default falcon.yaml |
| `falcon x explain <rule>` | Explain a rule with examples |
| `falcon x validate` | Validate falcon.yaml config |
| `falcon x watch` | Watch for file changes and re-analyze |
| `falcon x dep-graph` | Show file dependency graph |
| `falcon x workspace` | Analyze all packages in a monorepo |
