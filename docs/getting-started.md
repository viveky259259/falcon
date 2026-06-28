# Getting Started with Falcon

Falcon is a Rust-powered static analysis tool for Flutter and Dart. It catches issues common in AI-generated code, computes a health score, and provides enterprise-grade reporting — all at 30,000+ lines/sec.

## Installation

### Homebrew on macOS (recommended)

```bash
brew tap falcon-lint/tap
brew install falcon
```

The preview Homebrew tap installs the release archives from GitHub Releases and
includes all three binaries: `falcon`, `falcon-lsp`, and `falcon-mcp`.

### From source

```bash
cargo install --git https://github.com/viveky259259/falcon
```

### Without a global install

```bash
npx falcon@latest review
```

The npm package downloads the native binary from GitHub Releases once, verifies
it with `SHA256SUMS`, and reuses it on later runs.

This installs three binaries:
- `falcon` — the main CLI
- `falcon-lsp` — Language Server Protocol for VS Code
- `falcon-mcp` — MCP server for AI tool integration

### Verify installation

```bash
falcon --version
falcon --help
```

### Updating

```bash
falcon update              # Update to latest release
falcon update --version X  # Install specific version
```

## Quick Start

### 1. Run full analysis

```bash
cd /path/to/your/flutter/app
falcon check .
```

Runs 61+ lint rules, code metrics, and unused code detection. Results are automatically saved to history.

### 2. Generate HTML report

```bash
falcon check . --format html --output report.html
```

Opens an enterprise-grade dashboard with:
- Project properties from pubspec.yaml
- Health score gauge and KPI cards
- Level of Concern breakdown
- Severity distribution charts
- Test coverage analysis
- Complexity hotspots
- Issues grouped by file with search and filter

### 3. Score your project

```bash
falcon score .
```

Get a 0-100 AI Code Quality Score with a 6-dimension breakdown.

### 4. Initialize config

```bash
falcon init
```

Creates a `falcon.yaml` config file with default settings.

## Configuration

Falcon uses a `falcon.yaml` file for configuration:

```yaml
metrics:
  cyclomatic_complexity: 20
  lines_of_code: 300
  number_of_parameters: 6
  maximum_nesting_level: 5

rules:
  - avoid-dynamic
  - avoid-empty-catch
  - ensure-dispose-lifecycle
  - prefer-const-constructors

unused:
  enabled: true

exclude:
  - "**/*.g.dart"
  - "**/*.freezed.dart"
```

## Branch Comparison

Compare code quality between two git branches:

```bash
falcon compare-branches . --base main --branch feature/my-feature
```

This checks out each branch, runs analysis, and shows the delta:

```
  🦅 falcon Branch Comparison
  🌿 main vs feature/my-feature

  📉 ── Overview ──
    💚 Health Score           90 → 89        ▼ -1
    📁 Files                  82 → 268       ▲ +186
    ⚡ Total Issues         1510 → 6681      ▲ +5171

  📋 ── Rule Changes (top 10) ──
    ⬆  +1687 no-magic-numbers
    ⬆  +1127 prefer-trailing-comma
```

Add `--output comparison.html` for an HTML comparison report.

## Analysis History

Every `falcon check` run is automatically saved. View your history:

```bash
falcon history .
```

Compare any two stored runs:

```bash
falcon compare-reports . --run1 1 --run2 3 --output delta.html
```

## Codebase Intelligence

Get a bird's-eye view of your project:

```bash
falcon codebase-intel .
```

Shows health score, god files, complexity hotspots, and tech debt estimate.

## CI/CD Integration

### GitHub Actions

```yaml
name: Falcon Analysis
on: [pull_request]

jobs:
  falcon:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Falcon
        run: cargo install --git https://github.com/viveky259259/falcon
      - name: Run Analysis
        run: falcon check . --fail-on error
      - name: Generate Report
        run: falcon check . --format html --output falcon-report.html
      - name: Upload Report
        uses: actions/upload-artifact@v4
        with:
          name: falcon-report
          path: falcon-report.html
```

### AI Tool Integration (MCP)

Add to your Cursor/Windsurf MCP config:

```json
{
  "mcpServers": {
    "falcon": {
      "command": "falcon-mcp"
    }
  }
}
```

Agent-specific setup:

- [Homebrew](install/homebrew.md)
- [Cursor](install/cursor.md)
- [Claude Code](install/claude-code.md)
- [Cline](install/cline.md)

## What's Next

- [CLI Reference](cli-reference.md) — all commands and flags
- [Rule Catalog](rule-catalog.md) — all 61+ rules with examples
