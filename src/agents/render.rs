//! AGENTS.md content generators.

use crate::agents::features::{Feature, FeatureSource};
use std::path::Path;

/// AGENTS.md for the falcon repo itself. Distillation of CLAUDE.md aimed at
/// non-Claude agents (Codex, Cursor, etc.) so they follow the same rules.
pub fn render_root_falcon_agents_md() -> String {
    String::from(
        r#"# AGENTS.md — falcon

This file tells AI agents (Claude Code, Codex, Cursor, Aider, …) how to work in
this repo. It complements `CLAUDE.md`.

## What this is
Falcon is a Rust-powered static analysis CLI for Flutter/Dart, designed to catch
issues in AI-generated code. It ships three binaries: `falcon`, `falcon-lsp`,
`falcon-mcp`.

## Build / test loop
```
cargo build              # debug
cargo build --release    # release
cargo test               # full suite
cargo test <name>        # single test
cargo fmt                # required before commits
cargo clippy             # required before commits
```
Run `cargo test` after every non-trivial change. Tests live alongside code
(`#[cfg(test)] mod tests`) or under `tests/` for integration.

## Where things go
- `src/rules/<category>/` — single-file lint rules (common, flutter, security, ai)
- `src/analysis/` — multi-file analysis passes
- `src/smells/` — categorized smell reporting (dead code / code smell / security)
- `src/ai/` — AI-assisted explain/fix
- `src/ai_score/` — AI Code Quality Score (6 dimensions)
- `src/manage/`, `src/runtime/` — app health, devtools bridges
- `src/lsp/`, `src/mcp/`, `src/api/` — protocol servers
- `src/agents/` — generators for AGENTS.md (this file)
- `tests/` — integration tests

When adding a new rule: drop a file under `src/rules/<category>/`, register it
in the rule registry (`src/rules/mod.rs`), add tests, and add a classifier
entry in `src/smells/mod.rs::classify` if the rule should be bucketed.

## Conventions
- Rust 2021 edition.
- Errors: `anyhow::Result<T>` at boundaries; `?` to propagate; never panic in
  library code.
- Serde for all config/report I/O (JSON + YAML).
- Public lib API lives in `src/lib.rs`; binaries (`src/main.rs`) stay thin.
- Keep files <500 lines and functions <80 lines where practical.
- Write tests *before* the implementation — falcon expects the TDD loop.

## Things not to do
- Do not introduce new dependencies without checking conflicts in `Cargo.toml`.
- Do not skip `cargo fmt` / `cargo clippy`.
- Do not add backwards-compat shims for code you are removing — delete it.
- Do not write speculative abstractions; three similar lines beat a premature
  helper.
- Do not commit secrets — secrets live in the private `viveky259259/secrets`
  repo, never in this one.

## Useful falcon-on-falcon commands
```
falcon analyze .
falcon x smells .
falcon agents init           # regenerate this file
```
"#,
    )
}

/// AGENTS.md for the *root* of a Flutter app.
pub fn render_root_flutter_agents_md(project_root: &Path) -> String {
    let project_name = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "this app".to_string());

    format!(
        r#"# AGENTS.md — {name}

This file tells AI agents how to work in this Flutter project. Per-feature
AGENTS.md files under `lib/features/` (or your feature dirs) hold rules
scoped to a single feature.

## Build / test loop
```
flutter pub get
flutter analyze
flutter test
flutter run -d chrome           # this app targets web
flutter build web --release
```

## Static analysis
This project uses **falcon** for AI-aware static analysis on top of
`flutter analyze`. Run before submitting:
```
falcon analyze .
falcon x smells .               # categorized: dead code / code smells / security smells
falcon check-unused-files .
falcon check-dead-code .
```
A non-zero `security smells` count blocks merge. Code smells are advisory.

## Where things go
- `lib/` — application code
- `lib/features/<name>/` (or your equivalent) — one folder per feature, each
  with its own AGENTS.md
- `test/` — unit + widget tests
- `integration_test/` — driver tests
- `web/` — web entry point and `index.html`

## Conventions
- Dart 3, null-safety on. Prefer `final`/`const` everywhere it compiles.
- Avoid `dynamic` — falcon flags it as a code smell.
- No hardcoded credentials. Use environment variables or secure storage.
  falcon's `avoid-hardcoded-credentials` rule fails CI on this.
- Keep functions <50 lines and `build` methods <80 lines.
- Use `flutter_lints` plus the project `analysis_options.yaml`.

## Things not to do
- Do not commit `build/`, `.dart_tool/`, `pubspec.lock` for libraries
  (apps may commit it).
- Do not import across feature boundaries except through the feature's public
  API (typically `lib/features/<name>/<name>.dart`).
- Do not run `flutter pub upgrade` casually — pin versions and update
  intentionally.

## Useful falcon commands
```
falcon agents init .            # (re)generate this file and per-feature files
falcon flutter <args>           # passthrough to flutter
falcon fvm <args>               # passthrough to fvm
falcon devtools memory --attach <ws://…>   # runtime memory snapshot
```
"#,
        name = project_name
    )
}

/// AGENTS.md scoped to a single feature directory inside a Flutter app.
pub fn render_feature_agents_md(feat: &Feature) -> String {
    let layout_note = match feat.source {
        FeatureSource::LibFeatures => {
            "This feature lives under `lib/features/`. Sibling features are at the same level."
        }
        FeatureSource::LibSrcFeatures => {
            "This feature lives under `lib/src/features/`. Sibling features are at the same level."
        }
        FeatureSource::LibTopLevel => {
            "This feature is a top-level `lib/<name>/` directory. Treat it as the public seam of the feature."
        }
    };

    let entry_files = if feat.top_level_dart_files.is_empty() {
        "_(no top-level .dart files yet)_".to_string()
    } else {
        feat.top_level_dart_files
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .map(|n| format!("- `{n}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"# AGENTS.md — feature `{name}`

{layout_note}

## What this feature owns
Files at the root of this folder are the feature's public seam. Don't import
private files (anything inside subfolders, especially under `_` prefixes) from
outside this feature.

Top-level files in this feature:
{entry_files}

## How to work here
- Run `falcon x smells {dir}` after changes — it reports dead code, code smells,
  and security smells scoped to this folder.
- Run `flutter test test/{name}/` if a matching test folder exists.
- New widgets: prefer `StatelessWidget` + `const` constructors; falcon flags
  excessive `setState` and unnecessary rebuilds.
- New async flows: ensure futures are `await`-ed or explicitly `unawaited(…)`;
  falcon flags `avoid-unawaited-futures`.

## Boundaries
- This feature MAY depend on: shared `lib/widgets/`, `lib/utils/`, `lib/theme/`,
  third-party packages from `pubspec.yaml`.
- This feature MAY NOT depend on: other features (`lib/features/<other>/…`)
  except via their public file. Cross-feature reach-arounds break encapsulation.
- Changes to public files in this feature should be reflected in tests under
  `test/{name}/` (create the folder if missing).

## Things not to do
- Do not add `// ignore: …` lint suppressions without a `// justification:` line.
- Do not move shared helpers into this feature — promote them to `lib/shared/`
  or `lib/utils/`.
- Do not introduce a new state-management library here without project-wide
  agreement; match what other features already use.

## Quick checks before submitting
```
falcon x smells {dir}
flutter analyze {dir}
flutter test
```
"#,
        name = feat.name,
        dir = feat
            .dir
            .display()
            .to_string()
            .replace(std::path::MAIN_SEPARATOR, "/"),
        entry_files = entry_files,
        layout_note = layout_note,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn root_falcon_md_mentions_falcon_and_cargo() {
        let s = render_root_falcon_agents_md();
        assert!(s.contains("falcon"));
        assert!(s.contains("cargo test"));
        assert!(s.contains("AGENTS.md"));
    }

    #[test]
    fn root_flutter_md_uses_project_name() {
        let s = render_root_flutter_agents_md(Path::new("/tmp/my_app"));
        assert!(s.contains("my_app"));
        assert!(s.contains("flutter"));
    }

    #[test]
    fn feature_md_uses_feature_name_and_dir() {
        let feat = Feature {
            dir: PathBuf::from("lib/features/auth"),
            name: "auth".to_string(),
            source: FeatureSource::LibFeatures,
            top_level_dart_files: vec![PathBuf::from("lib/features/auth/login.dart")],
        };
        let s = render_feature_agents_md(&feat);
        assert!(s.contains("feature `auth`"));
        assert!(s.contains("lib/features/auth"));
        assert!(s.contains("login.dart"));
    }

    #[test]
    fn feature_md_handles_zero_dart_files() {
        let feat = Feature {
            dir: PathBuf::from("lib/features/empty"),
            name: "empty".to_string(),
            source: FeatureSource::LibFeatures,
            top_level_dart_files: vec![],
        };
        let s = render_feature_agents_md(&feat);
        assert!(s.contains("no top-level .dart files yet"));
    }
}
