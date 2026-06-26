# CLI Reference

## Core Analysis

| Command | Description |
|---|---|
| `falcon analyze [path]` | Full analysis (metrics + rules + unused detection) |
| `falcon metrics [path]` | Calculate code metrics only |
| `falcon score [path]` | AI Code Quality Score (0-100) with 6-dimension breakdown |
| `falcon ai-report [path]` | Full "State of AI-Generated Flutter Code" report |

### falcon analyze

```bash
falcon analyze . --format console --preset ai-generated --fail-on error
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
falcon analyze . --format html --output report.html
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
| `falcon check-unused-code` | Unused code declarations |
| `falcon check-unused-files` | Unused Dart files |
| `falcon check-dependencies` | Unused pubspec.yaml dependencies |
| `falcon check-cycles` | Cyclic import dependencies |
| `falcon check-unused-params` | Unused function parameters |
| `falcon check-dead-code` | Unreachable code after return/throw |
| `falcon check-unused-l10n` | Unused localization keys |
| `falcon check-promoted-deps` | Over/under-promoted dependencies |
| `falcon check-layers` | Clean architecture layer enforcement |
| `falcon check-imports` | Import restriction rules |
| `falcon check-widgets` | Widget rebuild issues and build method complexity |
| `falcon check-async` | Async/await anti-patterns (async void, unawaited futures) |
| `falcon check-platform` | Kotlin/Swift platform channel issues |
| `falcon check-codegen` | Code generation quality (.g.dart, .freezed.dart) |
| `falcon check-perf` | DevTools-style performance analysis |
| `falcon cognitive-complexity` | Function cognitive complexity |
| `falcon codebase-intel` | Health score, god files, hotspots, tech debt estimate |

## Comparison & History (v0.2.0)

### falcon history

List all stored analysis runs for a project.

```bash
falcon history /path/to/project
```

Shows a table with timestamp, branch, file count, health score, issue count, and commit hash. Snapshots are automatically saved after every `falcon analyze` and `falcon compare-branches` run.

### falcon compare-reports

Compare two stored analysis runs from history.

```bash
falcon compare-reports /path/to/project --run1 1 --run2 3
falcon compare-reports /path/to/project --run1 1 --run2 3 --output comparison.html
```

| Flag | Description | Default |
|---|---|---|
| `--run1` | Run number for baseline (1-based) | second-to-last |
| `--run2` | Run number for comparison | latest |
| `--output` | Generate HTML comparison report | none |

### falcon compare-branches

Compare analysis results between two git branches. Checks out each branch, runs full analysis, then shows the delta.

```bash
falcon compare-branches /path/to/project --base main --branch feature/my-feature
falcon compare-branches . --base main --branch dev --output comparison.html
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

### falcon update

Update Falcon to the latest or a specific version.

```bash
falcon update                    # Update to latest
falcon update --version 0.2.0   # Install specific version
falcon update --list             # Show version info and available releases
```

| Flag | Description |
|---|---|
| `--version` | Target version (e.g. 0.2.0). Omit for latest. |
| `--list` | List current version info, platform, and available releases |

The update mechanism downloads pre-built binaries from GitHub releases. If no binary is available for your platform, it provides `cargo install` fallback instructions.

## AI Features

| Command | Description |
|---|---|
| `falcon provenance [path]` | Detect AI-generated vs human-written files |
| `falcon conventions [path]` | Auto-detect team naming/architecture patterns |
| `falcon drift [path]` | Detect convention drift in new code |
| `falcon predict [path]` | Predict production risks from code patterns |
| `falcon discover-rules [path]` | Propose new rules from observed patterns |
| `falcon ai triage [path] --format text\|json` | Embedded AI false-positive triage entry point |
| `falcon refactor-sim --scenario <s>` | Simulate refactoring impact |
| `falcon test-gen [path]` | Generate test stubs from code analysis |
| `falcon vuln-scan [path]` | Security vulnerability radar |
| `falcon upgrade-check [path]` | Flutter upgrade compatibility |

### falcon ai triage

```bash
falcon ai triage . --format text
falcon ai triage . --format json
cargo run --features ai-local -- ai triage .
```

`falcon ai triage` is the embedded AI false-positive review entry point. In
the default build it exits successfully and reports that embedded triage
requires rebuilding with `--features ai-local`. With `ai-local` enabled, the
command is compiled in but still reports unavailable until `LocalEngine`
inference is wired.

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
| `falcon pr-comment [path]` | Post full-project analysis results as GitHub PR comment |
| `falcon webhook --url <url>` | Send webhook notification |
| `falcon fix [path]` | Auto-fix lint issues |
| `falcon benchmark` | Run performance benchmark |
| `falcon compare` | Compare Falcon vs dart analyze |

`falcon review` defaults its base ref to `origin/main` and uses
`git diff --name-only --diff-filter=ACMR <base-ref>...HEAD` to analyze only
changed `.dart` files. Use `--base-ref <ref>` or the legacy `--diff <ref>`
alias to compare against a different ref.

Use `--strictness quick` to report only error-level findings, `standard` for
the default lint findings plus standard review observations, and `thorough` for
the extra review observations such as missing-test checks.

For Dart projects with `.dart_tool/package_config.json`, review runs
`dart analyze --format=json` as an analyzer co-pilot and suppresses Falcon
findings on the same file, line, and rule class as analyzer diagnostics.
Style analyzer diagnostics can suppress style findings, for example, but not
Falcon behavioral or security findings on the same line. `--analyzer-copilot`
keeps this behavior explicit, and `--no-defer-to-analyzer` keeps Falcon findings
even when the analyzer reports the same file, line, and class.

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

      - run: cargo install --git https://github.com/viveky259259/falcon

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

## Dashboard

| Command | Description |
|---|---|
| `falcon dashboard snapshot` | Capture and save analysis snapshot |
| `falcon dashboard history` | View snapshot history |
| `falcon dashboard serve` | Start web dashboard server |
| `falcon trends [--last N]` | Show quality trends from history |
| `falcon rule-impact` | Analyze rule impact and auto-tune |

## Platform

| Command | Description |
|---|---|
| `falcon cloud init --team <name>` | Initialize team cloud config |
| `falcon cloud dashboard` | Team dashboard |
| `falcon enterprise init` | Initialize enterprise policies |
| `falcon enterprise check` | Check policies |
| `falcon enterprise compliance` | Generate compliance report |
| `falcon certify [path]` | Evaluate for Falcon certification |

## Infrastructure

| Command | Description |
|---|---|
| `falcon mcp` | Start MCP server (stdio) for AI tool integration |
| `falcon api --port 8090` | Start HTTP API server |
| `falcon init` | Generate default falcon.yaml |
| `falcon explain <rule>` | Explain a rule with examples |
| `falcon validate` | Validate falcon.yaml config |
| `falcon watch` | Watch for file changes and re-analyze |
| `falcon dep-graph` | Show file dependency graph |
| `falcon workspace` | Analyze all packages in a monorepo |
