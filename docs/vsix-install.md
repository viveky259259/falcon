# VS Code Extension Install

Falcon's VS Code extension is packaged from `editors/vscode/`.

## Local Package

Build a local VSIX with runtime dependencies included:

```bash
cd editors/vscode
npm install
npm run compile
npx vsce package --out /private/tmp/falcon-vscode.vsix
```

The VSIX must include:

- `extension/package.json`
- `extension/README.md`
- `extension/LICENSE.txt`
- `extension/icon.png`
- `extension/out/**`
- `extension/node_modules/vscode-languageclient/**`

## Install Smoke Test

Install into VS Code:

```bash
code --install-extension /private/tmp/falcon-vscode.vsix
```

Open a Flutter or Dart workspace and confirm:

- The Falcon output channel says the LSP server is running.
- Falcon diagnostics show `source: falcon`.
- Quick fixes appear for supported Falcon diagnostics.
- The file-header score lens appears unless `falcon.quietMode` is enabled.
- VS Code Chat accepts `@falcon why did my score drop?`.

## LSP Binary Resolution

The extension starts `falcon-lsp` in this order:

1. `falcon.executablePath`, when configured.
2. `falcon-lsp` found on `PATH`.
3. A bundled binary at `bin/<platform>-<arch>/falcon-lsp`.

Release packaging may stage platform binaries such as:

```text
editors/vscode/bin/darwin-arm64/falcon-lsp
editors/vscode/bin/darwin-x64/falcon-lsp
editors/vscode/bin/linux-x64/falcon-lsp
```

Bundled macOS and Linux binaries must keep executable bits before the VSIX is
created.
