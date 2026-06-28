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
- **Score lens** — file-header Falcon score with delta from previous save
- **Chat participant** — ask `@falcon` about score drops and rule explanations

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
| `falcon.deferToAnalyzer` | `true` | Suppress Falcon diagnostics on lines the Dart analyzer already flagged |
| `falcon.quietMode` | `false` | Suppress Falcon info- and hint-level diagnostics |
| `falcon.scoreLens.enabled` | `true` | Show file-header score and save-to-save delta |

## Coexistence with Dart-Code

Falcon is designed to live alongside the official [Dart-Code](https://marketplace.visualstudio.com/items?itemName=Dart-Code.dart-code) extension without producing duplicate squiggles. Every Falcon diagnostic is namespaced with `source: "falcon"` and `code: "falcon/<rule-id>"`, and by default Falcon **defers to the Dart analyzer**: if Dart-Code has already published a diagnostic on a given line, Falcon will suppress its own diagnostic on that same line. To disable this and see every Falcon finding regardless of analyzer overlap, set `falcon.deferToAnalyzer` to `false` in your settings. Use `falcon.quietMode` to additionally hide Falcon's info- and hint-level diagnostics.

On the very first publish for a file, Dart diagnostics may not be visible yet, so overlap suppression can lag briefly before self-correcting on subsequent publishes.

## Commands

- **Falcon: Analyze Workspace** — re-analyze all open files
- **Falcon: Fix All** — apply all auto-fixable fixes
- **Falcon: Restart Language Server** — restart the LSP server
- **Falcon: Show Output** — open the Falcon output channel
