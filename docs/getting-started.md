# Getting Started with Falcon

Falcon is a Rust-powered static analysis tool for Flutter and Dart. It catches issues common in AI-generated code, computes a health score, and provides enterprise-grade reporting — all at 30,000+ lines/sec.

## Installation

### Install script on macOS/Linux (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/viveky259259/falcon/master/scripts/install.sh | sh
```

Downloads the release archive for your platform from GitHub Releases, verifies
it against `SHA256SUMS`, and installs `falcon`, `falcon-lsp`, and `falcon-mcp`
to `~/.local/bin` — no Rust toolchain required. It also offers to add a short
`ff` alias for `falcon` (so `falcon run` becomes `ff run`); pass `--no-shortcut`
to skip that, or `--install-dir DIR` to install elsewhere. See
`scripts/install.sh --help` for all options.

### With Rust installed

```bash
cargo binstall falcon-flutter   # fetches the prebuilt binary — seconds
cargo install falcon-flutter    # builds from source — a few minutes
```

### Without a global install

```bash
npx falcon-flutter@latest review
```

The npm package downloads the native binary from GitHub Releases once, verifies
it with `SHA256SUMS`, and reuses it on later runs.

Each of these installs three binaries:
- `falcon` — the main CLI
- `falcon-lsp` — Language Server Protocol for VS Code
- `falcon-mcp` — MCP server for AI tool integration

### Homebrew

Not published yet — the `falcon-lint/homebrew-tap` repository doesn't exist.
See [docs/install/homebrew.md](install/homebrew.md) for status.

### Verify installation

```bash
falcon --version
falcon --help
```

### Updating

```bash
falcon x update              # Update to latest release
falcon x update --version X  # Install specific version
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
falcon x init
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
falcon x compare-branches . --base main --branch feature/my-feature
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
falcon x history .
```

Compare any two stored runs:

```bash
falcon x compare-reports . --run1 1 --run2 3 --output delta.html
```

## Codebase Intelligence

Get a bird's-eye view of your project:

```bash
falcon x codebase-intel .
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
        run: cargo install falcon-flutter
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
