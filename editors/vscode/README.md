# Falcon for VS Code

Rust-powered static analysis for Flutter/Dart projects.
Real-time diagnostics, quick fixes, and AI-powered code quality.

## Features

- **Real-time analysis** — sub-second diagnostics as you type
- **Inline diagnostics** — squiggly underlines for issues
- **Quick fixes** — one-click fixes for common issues
- **Auto-fix on save** — opt-in automatic fixing
- **43 lint rules** — Dart, Flutter, Provider/Riverpod, BLoC, Equatable
- **19 metrics** — complexity, coupling, cohesion, and more
- **Status bar** — issue counts at a glance

## Requirements

Install the Falcon CLI:

```bash
cargo install falcon
```

## Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `falcon.enable` | `true` | Enable Falcon analysis |
| `falcon.autoFixOnSave` | `false` | Apply fixes on save |
| `falcon.executablePath` | `""` | Path to falcon-lsp binary |
| `falcon.showStatusBar` | `true` | Show issue counts |

## Commands

- **Falcon: Analyze Workspace** — re-analyze all open files
- **Falcon: Fix All** — apply all auto-fixable fixes
- **Falcon: Restart Language Server** — restart the LSP server
- **Falcon: Show Output** — open the Falcon output channel
