# Preflight: `falcon check-assets` — Implementation Plan (PR 1 of 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `falcon check-assets` end-to-end: verify every entry in `pubspec.yaml`'s `flutter.assets:` list exists on disk, with `--format text|json`, suppression via `falcon.yaml`, and the shared `preflight` module that subsequent commands will reuse.

**Architecture:** New `src/preflight/` shared module (types + reporter), new `src/check_assets/` module (pubspec parsing + orchestrator), new `Commands::CheckAssets` variant in `src/main.rs`, and a `preflight: PreflightConfig` field added to `FalconConfig`.

**Tech Stack:** Rust 2021, `serde_yaml` (existing), `serde_json` (existing), `tempfile` (dev-dep, existing), `anyhow` (existing). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-05-25-preflight-checks-design.md` v2 — authoritative on types, output shapes, exit codes, suppression schema.

---

## File Structure

**Created:**
- `src/preflight/mod.rs` — shared types: `PreflightIssue`, `OutputFormat`, `TargetPlatform`, exit-code helper.
- `src/preflight/reporter.rs` — text + JSON emitters for `&[PreflightIssue]`.
- `src/check_assets/mod.rs` — `pub fn run(root, format, config) -> anyhow::Result<i32>` orchestrator.
- `src/check_assets/pubspec.rs` — parse `pubspec.yaml` `flutter.assets:` with line numbers.
- `tests/preflight_assets_tests.rs` — integration test exercising the CLI binary.

**Modified:**
- `src/lib.rs` — register `preflight` and `check_assets` modules.
- `src/main.rs` — add `Commands::CheckAssets` variant + dispatch.
- `src/config/mod.rs` — add `preflight: PreflightConfig` field to `FalconConfig`.

The shared `src/preflight/` module lands in this PR so the next four commands (a11y, pods, platform-deps, analyze rollup) can reuse it. It contains only what's needed for check-assets in this PR; `pub_cache.rs` lands with check-pods.

---

## Task 1: Add shared `preflight` module skeleton with types

**Files:**
- Create: `src/preflight/mod.rs`

- [ ] **Step 1: Write the failing test**

Append to `src/preflight/mod.rs` (file is being created — put the tests at the bottom in a `#[cfg(test)] mod tests` block):

```rust
//! Shared types for Falcon's pre-flight checks (check-assets, check-a11y,
//! check-pods, check-platform-deps).

use crate::config::Severity;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single finding from any pre-flight check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightIssue {
    pub rule_id: String,
    pub severity: Severity,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Output format selector for any `falcon check-*` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
}

/// Platform selector for commands that scope to one platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TargetPlatform {
    Ios,
    Android,
    Both,
}

/// Compute the exit code from a slice of issues.
/// Returns 2 if any Error, 1 if any Warning, 0 otherwise.
pub fn exit_code_for_issues(issues: &[PreflightIssue]) -> i32 {
    let mut code = 0;
    for issue in issues {
        match issue.severity {
            Severity::Error => return 2,
            Severity::Warning => code = code.max(1),
            Severity::Info => {}
        }
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(severity: Severity) -> PreflightIssue {
        PreflightIssue {
            rule_id: "x/y".into(),
            severity,
            title: "t".into(),
            file: None,
            line: None,
            plugin: None,
            message: "m".into(),
            suggestion: None,
        }
    }

    #[test]
    fn exit_code_empty_is_zero() {
        assert_eq!(exit_code_for_issues(&[]), 0);
    }

    #[test]
    fn exit_code_info_only_is_zero() {
        assert_eq!(exit_code_for_issues(&[issue(Severity::Info)]), 0);
    }

    #[test]
    fn exit_code_warning_is_one() {
        assert_eq!(
            exit_code_for_issues(&[issue(Severity::Warning), issue(Severity::Info)]),
            1
        );
    }

    #[test]
    fn exit_code_error_short_circuits_to_two() {
        assert_eq!(
            exit_code_for_issues(&[
                issue(Severity::Warning),
                issue(Severity::Error),
                issue(Severity::Info)
            ]),
            2
        );
    }

    #[test]
    fn preflight_issue_serializes_omitting_none_fields() {
        let issue = PreflightIssue {
            rule_id: "assets/missing-file".into(),
            severity: Severity::Error,
            title: "Missing asset".into(),
            file: None,
            line: None,
            plugin: None,
            message: ".env not found".into(),
            suggestion: None,
        };
        let json = serde_json::to_value(&issue).unwrap();
        assert!(json.get("file").is_none(), "file should be omitted: {json}");
        assert!(json.get("line").is_none(), "line should be omitted: {json}");
        assert!(json.get("plugin").is_none(), "plugin should be omitted: {json}");
        assert!(json.get("suggestion").is_none(), "suggestion should be omitted: {json}");
    }
}
```

- [ ] **Step 2: Wire the module into the lib crate**

Open `src/lib.rs` and add (preserving alphabetical order; place near other `pub mod` entries):

```rust
pub mod preflight;
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test --lib preflight::tests -- --nocapture`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/preflight/mod.rs src/lib.rs
git commit -m "feat(preflight): shared types + exit-code helper

Adds src/preflight/mod.rs with PreflightIssue / OutputFormat /
TargetPlatform / exit_code_for_issues(), and registers the module in
src/lib.rs. PreflightIssue serializes with None fields omitted so JSON
output stays compact.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Add text reporter for `PreflightIssue`

**Files:**
- Create: `src/preflight/reporter.rs`
- Modify: `src/preflight/mod.rs` (add `pub mod reporter;`)

- [ ] **Step 1: Write the failing test**

Create `src/preflight/reporter.rs` with this initial content:

```rust
//! Output emitters for pre-flight issues — text (ANSI) and JSON.
//!
//! SARIF is intentionally deferred to the analyze-rollup PR.

use super::{OutputFormat, PreflightIssue};
use crate::config::Severity;
use colored::Colorize;

/// Render a slice of issues as ANSI-colored text. Returns the rendered string;
/// the caller decides whether to print to stdout/stderr.
pub fn render_text(issues: &[PreflightIssue]) -> String {
    if issues.is_empty() {
        return format!("  {} No pre-flight issues found.\n", "✓".green().bold());
    }
    let mut out = String::new();
    for issue in issues {
        let badge = match issue.severity {
            Severity::Error => "Error".red().bold(),
            Severity::Warning => "Warning".yellow().bold(),
            Severity::Info => "Info".blue().bold(),
        };
        out.push_str(&format!("{} {}\n", badge, issue.title.bold()));
        if let Some(plugin) = &issue.plugin {
            out.push_str(&format!("  Plugin: {}\n", plugin));
        }
        if let Some(file) = &issue.file {
            match issue.line {
                Some(line) => out.push_str(&format!("  File: {}:{}\n", file.display(), line)),
                None => out.push_str(&format!("  File: {}\n", file.display())),
            }
        }
        out.push_str(&format!("  Rule: {}\n", issue.rule_id));
        out.push_str(&format!("  {}\n", issue.message));
        if let Some(suggestion) = &issue.suggestion {
            out.push_str(&format!("  {} {}\n", "→".bright_cyan(), suggestion));
        }
        out.push('\n');
    }
    out
}

/// Render as `{"schema_version": 1, "issues": [...]}` JSON.
pub fn render_json(issues: &[PreflightIssue]) -> String {
    let doc = serde_json::json!({
        "schema_version": 1,
        "issues": issues,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{\"issues\":[]}".into())
}

/// Render with the format the caller picked.
pub fn render(issues: &[PreflightIssue], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => render_text(issues),
        OutputFormat::Json => render_json(issues),
        OutputFormat::Sarif => {
            // Deferred to the analyze-rollup PR; for now fall back to JSON to keep
            // CI pipelines from blowing up if they pass --format sarif early.
            render_json(issues)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn disable_colors() {
        INIT.call_once(|| {
            colored::control::set_override(false);
        });
    }

    fn sample_error() -> PreflightIssue {
        PreflightIssue {
            rule_id: "assets/missing-file".into(),
            severity: Severity::Error,
            title: "Missing Asset".into(),
            file: Some(PathBuf::from("pubspec.yaml")),
            line: Some(109),
            plugin: None,
            message: "The asset `.env` is declared in pubspec.yaml but not on disk.".into(),
            suggestion: Some("Create the file or remove the declaration.".into()),
        }
    }

    #[test]
    fn render_text_empty_returns_ok_line() {
        disable_colors();
        let out = render_text(&[]);
        assert!(out.contains("No pre-flight issues found"), "got: {out}");
    }

    #[test]
    fn render_text_includes_title_file_line_rule_message_suggestion() {
        disable_colors();
        let out = render_text(&[sample_error()]);
        assert!(out.contains("Missing Asset"));
        assert!(out.contains("pubspec.yaml:109"));
        assert!(out.contains("assets/missing-file"));
        assert!(out.contains("The asset `.env` is declared"));
        assert!(out.contains("Create the file or remove the declaration."));
    }

    #[test]
    fn render_text_severity_labels() {
        disable_colors();
        let mut iss = sample_error();

        iss.severity = Severity::Error;
        assert!(render_text(&[iss.clone()]).contains("Error"));

        iss.severity = Severity::Warning;
        assert!(render_text(&[iss.clone()]).contains("Warning"));

        iss.severity = Severity::Info;
        assert!(render_text(&[iss.clone()]).contains("Info"));
    }

    #[test]
    fn render_json_has_schema_version_and_issues_array() {
        let json = render_json(&[sample_error()]);
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["schema_version"], serde_json::json!(1));
        assert_eq!(val["issues"].as_array().unwrap().len(), 1);
        assert_eq!(val["issues"][0]["rule_id"], "assets/missing-file");
        assert_eq!(val["issues"][0]["line"], 109);
    }

    #[test]
    fn render_json_empty_returns_empty_issues_array() {
        let json = render_json(&[]);
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["schema_version"], serde_json::json!(1));
        assert_eq!(val["issues"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn render_sarif_falls_back_to_json_for_now() {
        let sarif = render(&[sample_error()], OutputFormat::Sarif);
        let val: serde_json::Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(val["schema_version"], serde_json::json!(1));
    }
}
```

- [ ] **Step 2: Register the submodule**

Append to `src/preflight/mod.rs` (after the existing items, before the `#[cfg(test)] mod tests` block):

```rust
pub mod reporter;
```

- [ ] **Step 3: Run the test**

Run: `cargo test --lib preflight::reporter::tests`
Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/preflight/reporter.rs src/preflight/mod.rs
git commit -m "feat(preflight): text + json reporters for PreflightIssue

Adds src/preflight/reporter.rs with render_text (ANSI-aware), render_json
(schema_version: 1 wrapper), and render() dispatcher. SARIF is deferred
to the analyze-rollup PR; for now it falls back to JSON so passing
--format sarif does not crash callers.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Add `PreflightConfig` to `FalconConfig`

**Files:**
- Modify: `src/config/mod.rs` (add `PreflightConfig` struct + field on `FalconConfig`)

- [ ] **Step 1: Write the failing test**

Append to `src/config/mod.rs` inside its existing test block (find `#[cfg(test)] mod tests` near the bottom; if none exists, add one). Use this test:

```rust
#[test]
fn preflight_config_default_is_empty() {
    let cfg = FalconConfig::default();
    assert!(cfg.preflight.suppress.is_empty());
    assert!(cfg.preflight.config.is_empty());
}

#[test]
fn preflight_config_parses_from_yaml() {
    let yaml = r#"
preflight:
  suppress:
    - rule_id: assets/missing-file
      reason: "Bootstrap step documented in README."
  config:
    check-assets:
      warn_on_empty_directory: false
"#;
    let cfg: FalconConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.preflight.suppress.len(), 1);
    assert_eq!(cfg.preflight.suppress[0].rule_id, "assets/missing-file");
    assert!(cfg.preflight.config.contains_key("check-assets"));
}
```

- [ ] **Step 2: Add the types**

Insert these struct definitions in `src/config/mod.rs` (place them near `UnusedConfig`, around line 183):

```rust
/// Configuration for pre-flight checks (check-assets, check-a11y, check-pods,
/// check-platform-deps).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PreflightConfig {
    /// Issues to skip entirely.
    #[serde(default)]
    pub suppress: Vec<PreflightSuppression>,

    /// Tuning knobs per check, free-form so each command can read its own keys.
    #[serde(default)]
    pub config: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightSuppression {
    pub rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub reason: String,
}
```

- [ ] **Step 3: Add the field to `FalconConfig`**

Edit `src/config/mod.rs` around line 27 (inside `pub struct FalconConfig { ... }`):

Insert before the closing brace:

```rust
    #[serde(default)]
    pub preflight: PreflightConfig,
```

Edit `impl Default for FalconConfig` around line 30: add `preflight: PreflightConfig::default(),` inside the `Self { ... }` literal.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib config::tests::preflight`
Expected: 2 tests pass.

Run: `cargo build`
Expected: succeeds (no other modules break — the new field has `#[serde(default)]`).

- [ ] **Step 5: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat(config): add preflight.suppress + preflight.config to FalconConfig

Wires the schema from spec §Suppression. PreflightConfig is opt-in via
#[serde(default)] so existing falcon.yaml files keep working unchanged.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Parse `flutter.assets:` from `pubspec.yaml` with line tracking

**Files:**
- Create: `src/check_assets/pubspec.rs`

- [ ] **Step 1: Write the failing tests**

Create `src/check_assets/pubspec.rs` with this initial content:

```rust
//! Parse `flutter.assets:` from `pubspec.yaml`, capturing the original line
//! number of each declaration so errors can point users at the exact spot.

use anyhow::{Context, Result};

/// One declared asset entry from `pubspec.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDecl {
    /// The path string exactly as the user wrote it.
    pub path: String,
    /// 1-based line number in `pubspec.yaml`.
    pub line: usize,
}

/// Parse the `flutter: { assets: [...] }` list from a `pubspec.yaml` string.
///
/// Returns an empty vec if there is no `flutter.assets:` section. Returns an
/// error only if the YAML itself is unparseable.
pub fn parse_assets(yaml: &str) -> Result<Vec<AssetDecl>> {
    // Use serde_yaml for structural parse; track line numbers via a second pass
    // over the raw text. serde_yaml does not expose line numbers, but
    // pubspec.yaml's `flutter.assets:` block is always a list of strings, so
    // matching each value back to its line is unambiguous.
    let doc: serde_yaml::Value =
        serde_yaml::from_str(yaml).context("Failed to parse pubspec.yaml as YAML")?;
    let assets = doc
        .get("flutter")
        .and_then(|f| f.get("assets"))
        .and_then(|a| a.as_sequence());
    let Some(seq) = assets else {
        return Ok(Vec::new());
    };

    // Collect declared paths in order.
    let paths: Vec<String> = seq
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    // Second pass: scan the raw text for the line-number annotation. We look for
    // any line under the `assets:` key that has the form `  - <value>` and the
    // value matches one of `paths` (in order). This handles both quoted and
    // unquoted strings.
    let mut lines: Vec<usize> = Vec::with_capacity(paths.len());
    let mut in_assets_block = false;
    let mut idx = 0;
    for (lineno, line) in yaml.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("assets:") {
            in_assets_block = true;
            continue;
        }
        if in_assets_block {
            // Any line that's not indented as a list item under assets ends the block.
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                break;
            }
            if let Some(item) = trimmed.strip_prefix("- ") {
                let item = item.trim().trim_matches(|c| c == '"' || c == '\'');
                if idx < paths.len() && paths[idx] == item {
                    lines.push(lineno + 1);
                    idx += 1;
                }
            }
        }
    }

    // If we somehow under-counted, fall back to line 0 (unknown). This is rare.
    while lines.len() < paths.len() {
        lines.push(0);
    }

    Ok(paths
        .into_iter()
        .zip(lines)
        .map(|(path, line)| AssetDecl { path, line })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assets_empty_when_no_flutter_section() {
        let yaml = "name: foo\nversion: 1.0.0\n";
        assert!(parse_assets(yaml).unwrap().is_empty());
    }

    #[test]
    fn parse_assets_empty_when_flutter_section_has_no_assets() {
        let yaml = "name: foo\nflutter:\n  uses-material-design: true\n";
        assert!(parse_assets(yaml).unwrap().is_empty());
    }

    #[test]
    fn parse_assets_single_entry_with_line_number() {
        let yaml = "name: foo\nflutter:\n  assets:\n    - assets/logo.png\n";
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "assets/logo.png");
        assert_eq!(result[0].line, 4);
    }

    #[test]
    fn parse_assets_multiple_entries_track_distinct_lines() {
        let yaml = r#"name: foo
flutter:
  assets:
    - assets/a.png
    - assets/b.png
    - assets/c.png
"#;
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], AssetDecl { path: "assets/a.png".into(), line: 4 });
        assert_eq!(result[1], AssetDecl { path: "assets/b.png".into(), line: 5 });
        assert_eq!(result[2], AssetDecl { path: "assets/c.png".into(), line: 6 });
    }

    #[test]
    fn parse_assets_handles_quoted_strings() {
        let yaml = "flutter:\n  assets:\n    - \".env.development\"\n    - '.env.production'\n";
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path, ".env.development");
        assert_eq!(result[1].path, ".env.production");
    }

    #[test]
    fn parse_assets_dir_entry_kept_with_trailing_slash() {
        let yaml = "flutter:\n  assets:\n    - assets/icons/\n";
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result[0].path, "assets/icons/");
    }

    #[test]
    fn parse_assets_invalid_yaml_returns_err() {
        let yaml = "flutter:\n  assets:\n  - [unclosed\n";
        assert!(parse_assets(yaml).is_err());
    }

    #[test]
    fn parse_assets_top_level_assets_ignored() {
        // Flutter only recognises `flutter: { assets: ... }`, never top-level
        // `assets:`. We mirror that.
        let yaml = "assets:\n  - top/level.png\nname: foo\n";
        assert!(parse_assets(yaml).unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Stub a minimal `src/check_assets/mod.rs` that re-exports the parser**

Create `src/check_assets/mod.rs`:

```rust
//! `falcon check-assets` — verify every asset declared in pubspec.yaml exists.

pub mod pubspec;
```

- [ ] **Step 3: Register the module in `src/lib.rs`**

Add (alphabetical):

```rust
pub mod check_assets;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib check_assets::pubspec::tests`
Expected: 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/check_assets/mod.rs src/check_assets/pubspec.rs src/lib.rs
git commit -m "feat(check-assets): parse pubspec.yaml flutter.assets with line numbers

Adds parse_assets() returning Vec<AssetDecl> with each entry's 1-based
line number from the original pubspec.yaml. Handles empty/missing
flutter sections, quoted strings, directory entries (trailing slash),
and rejects invalid YAML.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Implement the `check_assets::run` orchestrator

**Files:**
- Modify: `src/check_assets/mod.rs`

- [ ] **Step 1: Write the failing tests**

Replace `src/check_assets/mod.rs` with:

```rust
//! `falcon check-assets` — verify every asset declared in pubspec.yaml exists.

pub mod pubspec;

use crate::config::{FalconConfig, Severity};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const RULE_ID_MISSING_FILE: &str = "assets/missing-file";
const RULE_ID_MISSING_DIR: &str = "assets/missing-directory";
const RULE_ID_EMPTY_DIR: &str = "assets/empty-directory";
const RULE_ID_GLOB_PATTERN: &str = "assets/glob-pattern";

/// Run the check. Returns exit code: 0 clean, 1 warnings only, 2 errors.
pub fn run(root: &Path, format: OutputFormat, config: &FalconConfig) -> Result<i32> {
    let pubspec_path = root.join("pubspec.yaml");
    if !pubspec_path.exists() {
        anyhow::bail!(
            "No pubspec.yaml found at {}. Is this a Flutter project?",
            root.display()
        );
    }
    let pubspec_text = std::fs::read_to_string(&pubspec_path)
        .with_context(|| format!("Reading {}", pubspec_path.display()))?;
    let decls = pubspec::parse_assets(&pubspec_text)?;

    let warn_on_empty_dir = config
        .preflight
        .config
        .get("check-assets")
        .and_then(|c| c.get("warn_on_empty_directory"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut issues: Vec<PreflightIssue> = Vec::new();
    for decl in &decls {
        let issue = inspect_decl(root, decl, warn_on_empty_dir);
        if let Some(issue) = issue {
            if !is_suppressed(&issue, config) {
                issues.push(issue);
            }
        }
    }

    let exit = exit_code_for_issues(&issues);
    let rendered = reporter::render(&issues, format);
    print!("{rendered}");
    Ok(exit)
}

fn inspect_decl(root: &Path, decl: &pubspec::AssetDecl, warn_on_empty_dir: bool) -> Option<PreflightIssue> {
    let pubspec_path = PathBuf::from("pubspec.yaml");

    // Reject glob patterns up front — Flutter's pubspec doesn't expand them.
    if decl.path.chars().any(|c| c == '*' || c == '?' || c == '[') {
        return Some(PreflightIssue {
            rule_id: RULE_ID_GLOB_PATTERN.into(),
            severity: Severity::Warning,
            title: "Glob Pattern in pubspec Assets".into(),
            file: Some(pubspec_path),
            line: Some(decl.line),
            plugin: None,
            message: format!(
                "Asset entry `{}` contains a glob metacharacter. \
                Flutter does not expand globs in pubspec.yaml — list each file or use a directory entry ending in `/`.",
                decl.path
            ),
            suggestion: Some(
                "Replace with the directory form (e.g. `assets/icons/`) or list files individually."
                    .into(),
            ),
        });
    }

    let on_disk = root.join(&decl.path);

    if decl.path.ends_with('/') {
        // Directory entry.
        if !on_disk.exists() {
            return Some(PreflightIssue {
                rule_id: RULE_ID_MISSING_DIR.into(),
                severity: Severity::Error,
                title: "Missing Asset Directory".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!(
                    "Directory `{}` is declared in pubspec.yaml but does not exist on disk.",
                    decl.path
                ),
                suggestion: Some(
                    "Create the directory and add files to it, or remove the declaration."
                        .into(),
                ),
            });
        }
        if !on_disk.is_dir() {
            return Some(PreflightIssue {
                rule_id: RULE_ID_MISSING_DIR.into(),
                severity: Severity::Error,
                title: "Asset Directory Is Not a Directory".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!(
                    "Path `{}` is declared as a directory (trailing `/`) but is a file on disk.",
                    decl.path
                ),
                suggestion: Some("Remove the trailing slash, or replace with a directory.".into()),
            });
        }
        let empty = std::fs::read_dir(&on_disk)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if empty && warn_on_empty_dir {
            return Some(PreflightIssue {
                rule_id: RULE_ID_EMPTY_DIR.into(),
                severity: Severity::Warning,
                title: "Empty Asset Directory".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!("Directory `{}` exists but contains no files.", decl.path),
                suggestion: Some(
                    "Add files to the directory, or remove the declaration if unused.".into(),
                ),
            });
        }
        None
    } else {
        // File entry.
        if !on_disk.exists() {
            return Some(PreflightIssue {
                rule_id: RULE_ID_MISSING_FILE.into(),
                severity: Severity::Error,
                title: "Missing Asset".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!(
                    "Asset `{}` is declared in pubspec.yaml but does not exist on disk.",
                    decl.path
                ),
                suggestion: Some(
                    "Create the file or remove the declaration. If it is gitignored, document the bootstrap step in README."
                        .into(),
                ),
            });
        }
        None
    }
}

fn is_suppressed(issue: &PreflightIssue, config: &FalconConfig) -> bool {
    config
        .preflight
        .suppress
        .iter()
        .any(|s| s.rule_id == issue.rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PreflightConfig, PreflightSuppression};
    use tempfile::TempDir;

    fn write_pubspec(dir: &Path, assets: &[&str]) {
        let mut yaml = String::from("name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n");
        for a in assets {
            yaml.push_str(&format!("    - {a}\n"));
        }
        std::fs::write(dir.join("pubspec.yaml"), yaml).unwrap();
    }

    fn touch(dir: &Path, rel: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "").unwrap();
    }

    #[test]
    fn no_pubspec_yaml_returns_error() {
        let tmp = TempDir::new().unwrap();
        let result = run(tmp.path(), OutputFormat::Text, &FalconConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn no_assets_declared_exits_zero() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pubspec.yaml"), "name: testapp\n").unwrap();
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn all_assets_present_exits_zero() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/a.png", "assets/b.png"]);
        touch(tmp.path(), "assets/a.png");
        touch(tmp.path(), "assets/b.png");
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn missing_file_exits_two() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/missing.png"]);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn missing_directory_exits_two() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/"]);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn empty_directory_exits_one_warning() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/"]);
        std::fs::create_dir_all(tmp.path().join("assets/icons")).unwrap();
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn empty_directory_silenced_by_config() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/"]);
        std::fs::create_dir_all(tmp.path().join("assets/icons")).unwrap();
        let mut cfg = FalconConfig::default();
        let mut tunings = std::collections::HashMap::new();
        let yaml: serde_yaml::Value =
            serde_yaml::from_str("warn_on_empty_directory: false").unwrap();
        tunings.insert("check-assets".to_string(), yaml);
        cfg.preflight = PreflightConfig { suppress: vec![], config: tunings };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn glob_pattern_emits_warning() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/*.png"]);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn suppression_silences_missing_file() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/missing.png"]);
        let cfg = FalconConfig {
            preflight: PreflightConfig {
                suppress: vec![PreflightSuppression {
                    rule_id: RULE_ID_MISSING_FILE.into(),
                    plugin: None,
                    key: None,
                    reason: "Test fixture.".into(),
                }],
                config: Default::default(),
            },
            ..FalconConfig::default()
        };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn mixed_missing_and_present_reports_only_missing() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/present.png", "assets/missing.png"]);
        touch(tmp.path(), "assets/present.png");
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --lib check_assets::tests`
Expected: 10 tests pass.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed (all previous tests still green).

- [ ] **Step 4: Commit**

```bash
git add src/check_assets/mod.rs
git commit -m "feat(check-assets): implement run() orchestrator with TempDir tests

Wires pubspec parsing, file/directory existence checks, glob warning,
empty-directory warning (toggleable via preflight.config.check-assets.
warn_on_empty_directory), and suppression via preflight.suppress.

Exit codes: 0 clean, 1 warning, 2 error.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Wire `Commands::CheckAssets` into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add the import**

Near the top of `src/main.rs`, with the other module imports, add:

```rust
use falcon::preflight::OutputFormat;
```

- [ ] **Step 2: Add the command variant**

Inside `enum Commands { ... }` in `src/main.rs`, insert a new variant. Place it alphabetically near `CheckUnusedCode`:

```rust
    /// Verify every asset declared in pubspec.yaml exists on disk (pre-flight check).
    CheckAssets {
        /// Path to the Flutter project.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
```

- [ ] **Step 3: Add the dispatch arm**

In the `match cli.command { ... }` block in `run()`, add a new arm near the other Check* commands:

```rust
        Commands::CheckAssets { path, format } => {
            let config = falcon::config::FalconConfig::load(&path).unwrap_or_default();
            let code = falcon::check_assets::run(&path, format, &config)?;
            process::exit(code);
        }
```

- [ ] **Step 4: Build to verify**

Run: `cargo build`
Expected: succeeds with no new warnings.

- [ ] **Step 5: Smoke-test the binary**

Run: `cargo run -- check-assets --help`
Expected: prints the help text for the new subcommand including `--format <FORMAT>`.

Run: `cargo run -- check-assets nonexistent-path 2>&1 | head -3`
Expected: error message about missing `pubspec.yaml`.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): wire Commands::CheckAssets

Adds the `falcon check-assets [path] [--format text|json|sarif]`
subcommand that delegates to falcon::check_assets::run() and exits with
the returned code (0/1/2).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Add an integration test exercising the CLI binary

**Files:**
- Create: `tests/preflight_assets_tests.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/preflight_assets_tests.rs`:

```rust
//! End-to-end test for `falcon check-assets`.
//!
//! Builds and invokes the actual CLI binary against a TempDir fixture, asserts
//! exit code + presence of substrings in stdout.

use std::process::Command;
use tempfile::TempDir;

fn falcon_bin() -> std::path::PathBuf {
    // CARGO_BIN_EXE_<name> is set by cargo for integration tests.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_falcon"))
}

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn check_assets_exits_zero_when_all_present() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/a.png\n",
    );
    write(&tmp.path().join("assets/a.png"), "");

    let output = Command::new(falcon_bin())
        .arg("check-assets")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn check_assets_exits_two_on_missing_file() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/missing.png\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-assets")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Missing Asset"), "stdout: {stdout}");
    assert!(stdout.contains("assets/missing.png"), "stdout: {stdout}");
}

#[test]
fn check_assets_json_format_is_valid_json() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/missing.png\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-assets")
        .arg(tmp.path())
        .args(["--format", "json"])
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(parsed["schema_version"], serde_json::json!(1));
    assert_eq!(parsed["issues"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["issues"][0]["rule_id"], "assets/missing-file");
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test --test preflight_assets_tests -- --nocapture`
Expected: 3 tests pass.

- [ ] **Step 3: Run the full suite once more**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add tests/preflight_assets_tests.rs
git commit -m "test(check-assets): integration test against the CLI binary

Exercises all-present (exit 0), missing-file (exit 2 + stdout substring),
and --format json (valid JSON, schema_version = 1) against a real
TempDir fixture invoking the falcon binary built by cargo.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Verify exit state and finalize

- [ ] **Step 1: Clean build + clippy + tests**

Run: `cargo build` — expect clean.
Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'` — expect `0 failed`.

- [ ] **Step 2: Smoke test in the Falcon repo itself**

Falcon's own pubspec is at `falcon_dart/pubspec.yaml` and `falcon-dart/pubspec.yaml`. Try one:

Run: `cargo run -- check-assets falcon_dart 2>&1 | tail -10`
Expected: either exit 0 (clean) or exit 2 with a specific missing-asset message that reflects the actual state of that directory. Either is a successful smoke test — we're verifying the binary works end-to-end on a real Flutter project layout.

- [ ] **Step 3: Confirm clean working tree**

Run: `git status -sb` — expect clean tree.

- [ ] **Step 4: Print a summary line**

Echo:

```
PR-1 (check-assets) complete.
Files added:    src/preflight/{mod,reporter}.rs, src/check_assets/{mod,pubspec}.rs,
                tests/preflight_assets_tests.rs
Files modified: src/lib.rs, src/main.rs, src/config/mod.rs
Tests added:    ~30 unit + 3 integration
Next PR:        check-a11y per docs/superpowers/specs/2026-05-25-preflight-checks-design.md
```

- [ ] **Step 5: No additional commit (all previous tasks committed individually).**

---

## Self-review checks before handoff

1. **Spec coverage:** Every requirement in the v2 spec applicable to check-assets has a task above — pubspec parsing with line numbers (Task 4), file/directory checks (Task 5), glob warning (Task 5), suppression via `falcon.yaml` (Task 3+5), `--format text|json` (Tasks 2, 6), exit codes 0/1/2 (Task 1's helper + Task 5).
2. **Placeholder scan:** No `TBD`, `TODO`, or `implement later` in any step. All code blocks are complete.
3. **Type consistency:** `PreflightIssue` shape stable across tasks; `OutputFormat` enum referenced consistently; `RULE_ID_*` constants used identically in implementation and tests; `FalconConfig::preflight` referenced with the same field path.

## What this plan does NOT cover (deferred to later PRs)

- SARIF output (falls back to JSON in this PR's reporter; full SARIF lands with the analyze-rollup PR).
- `src/preflight/pub_cache.rs` (lands with PR 3 — check-pods).
- Apple-API map (lands with PR 4 — check-platform-deps).
- `falcon analyze` integration (lands with PR 5 — analyze rollup).
- The other three pre-flight commands (own PRs each).
