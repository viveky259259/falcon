# falcon_cli

Dart wrapper for [Falcon](https://github.com/viveky259259/falcon) — Rust-powered static analysis for Flutter & Dart.

30,000+ lines/sec | 61+ rules | AI Code Score | Enterprise HTML reports

## Install

```bash
dart pub global activate falcon_cli
```

> **Prerequisite**: Requires the Falcon Rust binary:
> ```bash
> cargo install falcon-flutter
> ```
> Or download from [GitHub Releases](https://github.com/viveky259259/falcon/releases).

## Quick Start

```bash
# Full analysis with HTML report
falcon check . --format html --output report.html

# AI Code Quality Score (0-100)
falcon score .

# Compare two branches
falcon x compare-branches . --base main --branch feature/my-feature

# View analysis history
falcon x history .

# Codebase intelligence
falcon x codebase-intel .

# Self-update
falcon x update
```

## What Falcon Detects

- **Security**: Hardcoded credentials, insecure storage, certificate bypass
- **Error Handling**: Empty catches, unawaited futures, async void
- **Type Safety**: Dynamic types, missing annotations
- **Complexity**: Long functions, deep nesting, god files
- **Performance**: Widget rebuild issues, missing const, await-in-loop
- **Flutter**: Missing dispose, BLoC anti-patterns, Riverpod issues

## Enterprise HTML Reports

```bash
falcon check . --format html --output report.html
```

Generates a full dashboard with:
- Health score gauge and KPI cards
- Project properties from pubspec.yaml
- Level of Concern breakdown (8 categories)
- Test coverage analysis
- Severity charts and top rules
- Complexity hotspots with risk badges
- Issues grouped by file with search and filter
- Dark/light theme toggle

## Branch Comparison

```bash
falcon x compare-branches . --base main --branch dev --output comparison.html
```

Analyzes both branches and shows delta: health score, issues, metrics, and rule changes.

## Report History

Every `falcon check` auto-saves a snapshot:

```bash
falcon x history .                            # List all runs
falcon x compare-reports . --run1 1 --run2 3  # Compare any two
```

## CI/CD

```yaml
# GitHub Actions
- run: cargo install falcon-flutter
- run: falcon check . --fail-on error
- run: falcon check . --format html --output report.html
```

## All Commands

| Category | Commands |
|----------|----------|
| **Analysis** | `check`, `x metrics`, `score`, `x ai-report` |
| **Checks** | `x check-unused-code`, `x check-cycles`, `x check-widgets`, `x check-async`, `x cognitive-complexity`, `x codebase-intel` |
| **Comparison** | `x compare-branches`, `x compare-reports`, `x history` |
| **AI** | `x provenance`, `x conventions`, `x drift`, `x predict`, `x vuln-scan` |
| **CI/CD** | `review --format gh`, `x webhook`, `fix`, `x benchmark` |
| **Update** | `x update`, `x update --version X`, `x update --list` |

See the [CLI Reference](https://github.com/viveky259259/falcon/blob/master/docs/cli-reference.md) for all flags and options.

## License

MIT
