## 0.5.1

- Fresh release after the v0.5.0 tag was created before master finished syncing
- Includes the Falcon 0.5.0 navigation graph release plus latest CLI dispatch and preflight integrations
- 61 lint rules, 90+ commands

## 0.5.0

- Updated to Falcon 0.5.0
- Added the Flutter navigation graph analysis API
- 61 lint rules, 90+ commands

## 0.4.0

- Updated to Falcon 0.4.0
- 55 lint rules, 93 commands

## 0.3.0

- Updated to Falcon 0.3.0
- 55 lint rules, 93 commands

## 0.2.0

- Enterprise HTML reports with dashboard layout, charts, and dark/light theme
- Project properties from pubspec.yaml in report header
- Level of Concern breakdown across 8 categories
- Test coverage section mapping source to test files
- Branch comparison: `falcon x compare-branches --base main --branch dev`
- Report history with auto-save after every analysis
- `falcon x history` to view stored runs
- `falcon x compare-reports --run1 N --run2 M` for run comparison
- Self-update: `falcon x update` / `falcon x update --version X`
- Rich console icons for improved UX
- 61+ lint rules, 470+ tests

## 0.1.0

- Initial release
- Dart wrapper for the Falcon Rust-powered CLI
- Delegates to `falcon` binary on PATH or in `~/.cargo/bin/`
- Supports all Falcon commands
