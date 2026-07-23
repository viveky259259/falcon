# Preflight: `falcon analyze` Rollup — Implementation Plan (PR 5 of 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Wire the four `check-*` commands into `falcon analyze` so they run after the existing analyze flow, take `max(exit_codes)`, and respect a `falcon.yaml` `analyze.preflight.{enabled,skip}` config. Skips are opt-out (default: enabled).

**Spec:** `docs/superpowers/specs/2026-05-25-preflight-checks-design.md` v2 (§Rollup in falcon analyze).

---

## File Structure

**Modified:**
- `src/config/mod.rs` — add `AnalyzeConfig { preflight: AnalyzePreflightConfig }` to `FalconConfig`.
- `src/main.rs` — call each `check_*::run()` after the existing analyze branch returns, take max exit code.

No new files. This PR is small; it's just wiring + a test.

---

## Task 1: Add `AnalyzeConfig` + `AnalyzePreflightConfig` to FalconConfig

**File:** Modify `src/config/mod.rs`.

- [ ] **Step 1: Add the structs near `PreflightConfig` (already present from PR1):**

```rust
/// Configuration controlling how `falcon analyze` invokes the four pre-flight checks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyzeConfig {
    #[serde(default)]
    pub preflight: AnalyzePreflightConfig,
}

/// Tuning for the analyze-rollup of pre-flight checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzePreflightConfig {
    /// Whether to run the four pre-flight checks during `falcon analyze`. Default: true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// List of check names to skip. Valid values: "check-assets", "check-a11y",
    /// "check-pods", "check-platform-deps".
    #[serde(default)]
    pub skip: Vec<String>,
}

impl Default for AnalyzePreflightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            skip: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}
```

- [ ] **Step 2: Add the field to `FalconConfig`:**

```rust
    #[serde(default)]
    pub analyze: AnalyzeConfig,
```

(Place it near the existing `preflight: PreflightConfig` field.)

Add `analyze: AnalyzeConfig::default(),` to `impl Default for FalconConfig`.

- [ ] **Step 3: Add tests inside the existing `mod tests` block in `src/config/mod.rs`:**

```rust
#[test]
fn analyze_preflight_default_is_enabled_no_skips() {
    let cfg = FalconConfig::default();
    assert!(cfg.analyze.preflight.enabled);
    assert!(cfg.analyze.preflight.skip.is_empty());
}

#[test]
fn analyze_preflight_parses_disabled() {
    let yaml = "analyze:\n  preflight:\n    enabled: false\n";
    let cfg: FalconConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(!cfg.analyze.preflight.enabled);
}

#[test]
fn analyze_preflight_parses_skip_list() {
    let yaml = "analyze:\n  preflight:\n    skip: [check-pods, check-platform-deps]\n";
    let cfg: FalconConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(cfg.analyze.preflight.enabled); // still default-true
    assert_eq!(cfg.analyze.preflight.skip, vec!["check-pods", "check-platform-deps"]);
}
```

- [ ] **Step 4: Run tests + full suite**

Run: `cargo test --lib config::tests::analyze_preflight 2>&1 | tail -8`
Expected: 3 tests pass.

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat(config): add analyze.preflight {enabled, skip} to FalconConfig

Lets users opt out of the four pre-flight checks during \`falcon
analyze\` via:
  analyze:
    preflight:
      enabled: false
or selectively skip via skip: [check-pods, check-platform-deps].
Default is enabled with no skips.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Wire the four checks into `Commands::Analyze`

**File:** Modify `src/main.rs`.

Find the existing `Commands::Analyze { ... } => { ... }` dispatch arm. After its existing body runs (capturing its exit code in a local variable), run each of the four pre-flight checks (gated by `config.analyze.preflight.enabled` and the skip list), and exit with `max(existing_exit, ...preflight_exits)`.

- [ ] **Step 1: Locate the existing Analyze arm**

Search `src/main.rs` for `Commands::Analyze {` and inspect what it does today. Today it likely calls `falcon::Falcon::new(config).analyze(...)`, prints the report, and either `process::exit(code)` or returns Ok.

Refactor approach (minimal): replace the trailing `process::exit(existing_code)` with a manually-summed max across the existing exit code + four pre-flight runs. If the arm uses `process::exit(0)` unconditionally, change it to use the actual exit code from the analyze flow.

- [ ] **Step 2: Replace the Analyze arm with this body**

The exact replacement depends on what's there now. The pattern to apply:

```rust
        Commands::Analyze { path, format, /* other existing flags */ } => {
            // ... existing analyze flow ...
            let existing_exit = /* whatever exit code the existing flow computes */;

            let preflight_exit = run_preflight_rollup(&path, &config)?;
            let final_exit = existing_exit.max(preflight_exit);
            process::exit(final_exit);
        }
```

Below the `Commands` match block (at module level inside `main.rs`), add this helper:

```rust
fn run_preflight_rollup(path: &PathBuf, config: &falcon::config::FalconConfig) -> anyhow::Result<i32> {
    if !config.analyze.preflight.enabled {
        return Ok(0);
    }
    let skip = &config.analyze.preflight.skip;
    let fmt = falcon::preflight::OutputFormat::Text;

    let mut worst = 0;

    if !skip.iter().any(|s| s == "check-assets") {
        println!("\n── pre-flight: check-assets ──");
        let code = falcon::check_assets::run(path, fmt, config)?;
        worst = worst.max(code);
    }
    if !skip.iter().any(|s| s == "check-a11y") {
        println!("\n── pre-flight: check-a11y ──");
        let code = falcon::check_a11y::run(path, fmt, config)?;
        worst = worst.max(code);
    }
    if !skip.iter().any(|s| s == "check-pods") {
        println!("\n── pre-flight: check-pods ──");
        let code = falcon::check_pods::run(path, fmt, config, false)?;
        worst = worst.max(code);
    }
    if !skip.iter().any(|s| s == "check-platform-deps") {
        println!("\n── pre-flight: check-platform-deps ──");
        let code = falcon::check_platform_deps::run(path, fmt, config, false, None)?;
        worst = worst.max(code);
    }
    Ok(worst)
}
```

NOTE about types: if `falcon::preflight::OutputFormat` is not imported in main.rs under that exact path, use `PreflightOutputFormat::Text` (the alias established in PR1).

- [ ] **Step 3: Build + sanity check**

Run: `cargo build 2>&1 | tail -5` — expect clean.
Run: `cargo run --bin falcon -- analyze --help 2>&1 | head -10` — expect unchanged help.

- [ ] **Step 4: Manual sanity**

Run: `cargo run --quiet --bin falcon -- analyze falcon_dart 2>&1 | tail -25`
Expected: existing falcon analyze output, THEN four `── pre-flight: ... ──` sections, then exits. (falcon_dart has no flutter app structure → all four should report Info-level only, exit 0.)

- [ ] **Step 5: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): roll the four pre-flight checks into falcon analyze

After the existing analyze flow finishes, runs check-assets, check-a11y,
check-pods, and check-platform-deps in sequence, prints a section
header per check, and exits with max(existing_exit, preflight_exits).
Each check can be individually skipped via
\`analyze.preflight.skip: [check-pods]\` in falcon.yaml, and the
entire rollup disabled via \`analyze.preflight.enabled: false\`.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Integration test — analyze rollup runs all four checks

**File:** Create `tests/preflight_analyze_rollup_tests.rs`.

- [ ] **Step 1: Create the file:**

```rust
//! End-to-end test for the `falcon analyze` pre-flight rollup.

use std::process::Command;
use tempfile::TempDir;

fn falcon_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_falcon"))
}

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

/// Build a Flutter-ish project that will trigger errors in multiple pre-flight
/// checks: a missing asset (check-assets), no ensureSemantics call
/// (check-a11y), and a missing Info.plist key for a fake "location" plugin
/// (check-platform-deps).
fn fixture_with_problems(tmp: &TempDir, cache: &TempDir) {
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/missing.png\n",
    );
    write(
        &tmp.path().join("pubspec.lock"),
        "packages:\n  location:\n    source: hosted\n    version: \"8.0.0\"\n",
    );
    write(&tmp.path().join("lib/main.dart"), "void main() { runApp(MyApp()); }\n");
    write(
        &cache
            .path()
            .join("hosted/pub.dev/location-8.0.0/ios/Classes/Loc.m"),
        "[CLLocationManager.shared requestWhenInUseAuthorization];",
    );
}

#[test]
fn analyze_rollup_runs_all_preflight_checks_and_takes_max_exit() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    fixture_with_problems(&tmp, &cache);

    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("analyze")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // All four pre-flight section headers should appear.
    assert!(stdout.contains("check-assets"), "stdout: {stdout}");
    assert!(stdout.contains("check-a11y"), "stdout: {stdout}");
    assert!(stdout.contains("check-pods"), "stdout: {stdout}");
    assert!(stdout.contains("check-platform-deps"), "stdout: {stdout}");
    // At least one Error should bubble exit code to 2.
    assert_eq!(output.status.code(), Some(2), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn analyze_rollup_skip_list_omits_named_checks() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    fixture_with_problems(&tmp, &cache);
    // Skip check-platform-deps and check-pods via falcon.yaml.
    write(
        &tmp.path().join("falcon.yaml"),
        "analyze:\n  preflight:\n    skip:\n      - check-pods\n      - check-platform-deps\n",
    );

    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("analyze")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skipped sections should NOT appear; included ones should.
    assert!(stdout.contains("check-assets"), "stdout: {stdout}");
    assert!(stdout.contains("check-a11y"), "stdout: {stdout}");
    assert!(
        !stdout.contains("── pre-flight: check-pods ──"),
        "stdout: {stdout}"
    );
    assert!(
        !stdout.contains("── pre-flight: check-platform-deps ──"),
        "stdout: {stdout}"
    );
}

#[test]
fn analyze_rollup_disabled_when_enabled_false() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    fixture_with_problems(&tmp, &cache);
    write(
        &tmp.path().join("falcon.yaml"),
        "analyze:\n  preflight:\n    enabled: false\n",
    );

    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("analyze")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("── pre-flight:"),
        "expected no pre-flight section headers when disabled, stdout: {stdout}"
    );
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test preflight_analyze_rollup_tests 2>&1 | tail -8`
Expected: 3 tests pass.

If they fail because the existing `Commands::Analyze` arm doesn't take a positional `[path]` argument the way other commands do, adjust the test args to match. Inspect the actual `Commands::Analyze` variant in `src/main.rs` if needed.

- [ ] **Step 3: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add tests/preflight_analyze_rollup_tests.rs
git commit -m "test(analyze): integration test for preflight rollup

Three e2e tests: default-on runs all four checks + takes max exit code,
analyze.preflight.skip list omits named checks, and
analyze.preflight.enabled=false fully disables the rollup.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Finalize, smoke test, push

- [ ] **Step 1: Smoke test**

Run: `cargo run --quiet --bin falcon -- analyze falcon_dart 2>&1 | tail -30`
Expected: existing analyze output PLUS four `── pre-flight: ... ──` sections. Exit 0 (no real errors on the falcon_dart Dart package).

- [ ] **Step 2: Print summary**

```
PR-5 (analyze rollup) complete.
Files modified: src/config/mod.rs, src/main.rs
Files added:    tests/preflight_analyze_rollup_tests.rs
Tests added:    3 (config) + 3 (integration) = 6
All four preflight commands now run inside `falcon analyze` and contribute to max(exit_codes).
```

- [ ] **Step 3: Push**

```bash
git push 2>&1 | tail -3
```
