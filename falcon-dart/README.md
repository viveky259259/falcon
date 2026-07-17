# falcon_cli

Dart wrapper for [Falcon](https://github.com/viveky259259/falcon) — Rust-powered static analysis for Flutter & Dart.

30,000+ lines/sec | 61+ rules | AI Code Score | Enterprise HTML reports

## Install

```bash
dart pub global activate falcon_cli
```

> **Prerequisite**: Requires the Falcon Rust binary:
> ```bash
> cargo install --git https://github.com/viveky259259/falcon
> ```
> Or download from [GitHub Releases](https://github.com/viveky259259/falcon/releases).

## Quick Start

```bash
# Full analysis with HTML report
falcon analyze . --format html --output report.html

# AI Code Quality Score (0-100)
falcon ai-score .

# Compare two branches
falcon compare-branches . --base main --branch feature/my-feature

# View analysis history
falcon history .

# Codebase intelligence
falcon x codebase-intel .

# Self-update
falcon update
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
falcon analyze . --format html --output report.html
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
falcon compare-branches . --base main --branch dev --output comparison.html
```

Analyzes both branches and shows delta: health score, issues, metrics, and rule changes.

## Report History

Every `falcon analyze` auto-saves a snapshot:

```bash
falcon history .                              # List all runs
falcon compare-reports . --run1 1 --run2 3    # Compare any two
```

## CI/CD

```yaml
# GitHub Actions
- run: cargo install --git https://github.com/viveky259259/falcon
- run: falcon analyze . --fail-on error
- run: falcon analyze . --format html --output report.html
```

## All Commands

| Category | Commands |
|----------|----------|
| **Analysis** | `analyze`, `x metrics`, `score`, `x ai-report` |
| **Checks** | `x check-unused-code`, `x check-cycles`, `x check-widgets`, `x check-async`, `x cognitive-complexity`, `x codebase-intel` |
| **Comparison** | `compare-branches`, `compare-reports`, `history` |
| **AI** | `x provenance`, `x conventions`, `x drift`, `x predict`, `x vuln-scan` |
| **CI/CD** | `pr-comment`, `webhook`, `fix`, `benchmark` |
| **Update** | `update`, `update --version X`, `update --list` |

See the [CLI Reference](https://github.com/viveky259259/falcon/blob/master/docs/cli-reference.md) for all flags and options.

## License

MIT
