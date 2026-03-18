# Getting Started with Falcon

## Installation

### From source (recommended)

```bash
cargo install --git https://github.com/viveky259259/falcon
```

This installs three binaries:
- `falcon` — the main CLI
- `falcon-lsp` — Language Server Protocol for VS Code
- `falcon-mcp` — MCP server for AI tool integration

### Verify installation

```bash
falcon --version
falcon --help
```

## Quick Start

### 1. Score your project

```bash
cd /path/to/your/flutter/app
falcon ai-score .
```

This gives you a 0-100 AI Code Quality Score with a 6-dimension breakdown.

### 2. Run full analysis

```bash
falcon analyze .
```

Runs all 58+ lint rules, metrics, and unused code detection.

### 3. Initialize config

```bash
falcon init
```

Creates a `falcon.yaml` config file in your project root with default settings.

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
        run: falcon analyze . --fail-on error
      - name: Post PR Comment
        run: falcon pr-comment --dry-run
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
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

Falcon will analyze Flutter code in real-time during generation.

## What's Next

- [CLI Reference](cli-reference.md) — all 74 commands
- [Rule Catalog](rule-catalog.md) — all 58+ rules
- [AI Code Score](ai-code-score.md) — understanding your score
