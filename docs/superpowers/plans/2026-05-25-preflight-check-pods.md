# Preflight: `falcon check-pods` — Implementation Plan (PR 3 of 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Ship `falcon check-pods`: verify `ios/Podfile`'s `platform :ios, 'X.Y'` is ≥ every transitive plugin podspec's `s.ios.deployment_target`. Includes the shared `preflight::pub_cache` module that PR4 (`check-platform-deps`) will reuse.

**Tech Stack:** Rust 2021, regex parsing (Podfile + podspec), `walkdir` (already in deps), `tempfile` (dev-dep), `pubspec.lock` YAML reading via existing `serde_yaml`.

**Spec:** `docs/superpowers/specs/2026-05-25-preflight-checks-design.md` v2 (§check-pods + §Source-of-truth strategy).

---

## File Structure

**Created:**
- `src/preflight/pub_cache.rs` — locate the pub-cache root + enumerate installed plugins from `pubspec.lock`.
- `src/check_pods/mod.rs` — `pub fn run(root, format, config, refresh) -> anyhow::Result<i32>`.
- `src/check_pods/podfile.rs` — parse Podfile's `platform :ios, 'X.Y'` / `platform :osx, 'X.Y'`.
- `src/check_pods/podspec.rs` — extract `s.ios.deployment_target` / `s.osx.deployment_target` from a podspec file.
- `tests/preflight_pods_tests.rs` — integration test.

**Modified:**
- `src/preflight/mod.rs` — register `pub mod pub_cache;`.
- `src/lib.rs` — register `pub mod check_pods;`.
- `src/main.rs` — `Commands::CheckPods` variant + dispatch (with `--refresh`).

---

## Task 1: Shared `preflight::pub_cache` module

**Files:**
- Create: `src/preflight/pub_cache.rs`
- Modify: `src/preflight/mod.rs` (add `pub mod pub_cache;`)

- [ ] **Step 1: Create `src/preflight/pub_cache.rs`**

```rust
//! Locate the pub-cache root and enumerate installed plugins from
//! `pubspec.lock`. Shared by `check_pods` and `check_platform_deps`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One plugin entry resolved from `pubspec.lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPlugin {
    /// Plugin name as used in `pubspec.yaml`.
    pub name: String,
    /// Version string (e.g. "8.0.0").
    pub version: String,
    /// Source kind: "hosted", "git", "path", "sdk".
    pub source: String,
    /// Absolute path to the plugin's directory in the pub-cache, when source
    /// is `hosted`. None for git/path/sdk sources (Falcon can't infer them
    /// without more lockfile parsing).
    pub root: Option<PathBuf>,
}

/// Return the pub-cache root the user is using.
/// Precedence: `FALCON_PUB_CACHE` env > `PUB_CACHE` env > platform default.
pub fn locate_pub_cache() -> PathBuf {
    if let Ok(p) = std::env::var("FALCON_PUB_CACHE") {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("PUB_CACHE") {
        return PathBuf::from(p);
    }
    if cfg!(windows) {
        if let Ok(p) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(p).join("Pub").join("Cache");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".pub-cache");
    }
    PathBuf::from(".pub-cache")
}

#[derive(Debug, Deserialize)]
struct PubspecLock {
    #[serde(default)]
    packages: BTreeMap<String, LockedPackage>,
}

#[derive(Debug, Deserialize)]
struct LockedPackage {
    #[serde(default)]
    source: String,
    #[serde(default)]
    version: String,
}

/// Read `<project>/pubspec.lock` and return one `InstalledPlugin` per package.
/// Hosted (pub.dev) packages get their `root` resolved against the given
/// `pub_cache` root. Non-pub.dev sources keep `root = None`.
pub fn iter_installed_plugins(project_root: &Path, pub_cache: &Path) -> Result<Vec<InstalledPlugin>> {
    let lock_path = project_root.join("pubspec.lock");
    if !lock_path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&lock_path)
        .with_context(|| format!("Reading {}", lock_path.display()))?;
    let lock: PubspecLock = serde_yaml::from_str(&text).context("Parsing pubspec.lock")?;

    let mut out = Vec::with_capacity(lock.packages.len());
    for (name, pkg) in lock.packages {
        // Skip the dummy "flutter" / "sky_engine" SDK entries — they aren't
        // plugins we can introspect.
        if pkg.source == "sdk" {
            continue;
        }
        let root = if pkg.source == "hosted" {
            Some(
                pub_cache
                    .join("hosted")
                    .join("pub.dev")
                    .join(format!("{name}-{}", pkg.version)),
            )
        } else {
            None
        };
        out.push(InstalledPlugin {
            name,
            version: pkg.version,
            source: pkg.source,
            root,
        });
    }
    Ok(out)
}

/// Convenience: filter the plugins to only those that have an iOS source folder
/// (under `ios/` or `darwin/`).
pub fn plugins_with_ios(plugins: &[InstalledPlugin]) -> Vec<&InstalledPlugin> {
    plugins
        .iter()
        .filter(|p| match &p.root {
            Some(root) => root.join("ios").is_dir() || root.join("darwin").is_dir(),
            None => false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn locate_pub_cache_honors_falcon_env_first() {
        std::env::set_var("FALCON_PUB_CACHE", "/tmp/fake-falcon-cache");
        let p = locate_pub_cache();
        assert_eq!(p, PathBuf::from("/tmp/fake-falcon-cache"));
        std::env::remove_var("FALCON_PUB_CACHE");
    }

    #[test]
    fn iter_installed_plugins_empty_when_no_lock() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn iter_installed_plugins_parses_hosted_packages() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        write(
            &tmp.path().join("pubspec.lock"),
            r#"packages:
  location:
    source: hosted
    version: "8.0.0"
  http:
    source: hosted
    version: "1.2.0"
sdks:
  dart: ">=3.0.0"
"#,
        );
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert_eq!(plugins.len(), 2);
        let loc = plugins.iter().find(|p| p.name == "location").unwrap();
        assert_eq!(loc.version, "8.0.0");
        assert_eq!(loc.source, "hosted");
        assert_eq!(
            loc.root,
            Some(pub_cache.path().join("hosted/pub.dev/location-8.0.0"))
        );
    }

    #[test]
    fn iter_installed_plugins_skips_sdk_entries() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        write(
            &tmp.path().join("pubspec.lock"),
            r#"packages:
  flutter:
    source: sdk
    version: "0.0.0"
  location:
    source: hosted
    version: "8.0.0"
"#,
        );
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "location");
    }

    #[test]
    fn iter_installed_plugins_keeps_non_pub_sources_without_root() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        write(
            &tmp.path().join("pubspec.lock"),
            r#"packages:
  my_git_plugin:
    source: git
    version: "0.0.0"
  my_path_plugin:
    source: path
    version: "0.0.0"
"#,
        );
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert_eq!(plugins.len(), 2);
        for p in &plugins {
            assert!(p.root.is_none(), "non-hosted should have root = None: {p:?}");
        }
    }

    #[test]
    fn plugins_with_ios_filters_by_directory() {
        let pub_cache = TempDir::new().unwrap();
        let plugins = vec![
            InstalledPlugin {
                name: "with_ios".into(),
                version: "1.0.0".into(),
                source: "hosted".into(),
                root: Some(pub_cache.path().join("with_ios-1.0.0")),
            },
            InstalledPlugin {
                name: "no_ios".into(),
                version: "1.0.0".into(),
                source: "hosted".into(),
                root: Some(pub_cache.path().join("no_ios-1.0.0")),
            },
            InstalledPlugin {
                name: "git_plug".into(),
                version: "0.0.0".into(),
                source: "git".into(),
                root: None,
            },
        ];
        std::fs::create_dir_all(pub_cache.path().join("with_ios-1.0.0/ios")).unwrap();
        std::fs::create_dir_all(pub_cache.path().join("no_ios-1.0.0/lib")).unwrap();
        let filtered = plugins_with_ios(&plugins);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "with_ios");
    }
}
```

- [ ] **Step 2: Register the submodule**

Edit `src/preflight/mod.rs` to add `pub mod pub_cache;` after `pub mod reporter;`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib preflight::pub_cache::tests 2>&1 | tail -10`
Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/preflight/pub_cache.rs src/preflight/mod.rs
git commit -m "feat(preflight): shared pub_cache module (locate + enumerate plugins)

Adds locate_pub_cache() honoring FALCON_PUB_CACHE > PUB_CACHE > platform
default, iter_installed_plugins() reading pubspec.lock and resolving
hosted/pub.dev entries to their pub-cache directory, and
plugins_with_ios() filter. Used by check-pods and check-platform-deps.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Podfile parser

**Files:** Create `src/check_pods/podfile.rs`, `src/check_pods/mod.rs` (stub), modify `src/lib.rs`.

- [ ] **Step 1: Create `src/check_pods/podfile.rs`**

```rust
//! Parse `ios/Podfile` (and `macos/Podfile`) for the `platform` directive.

/// Result of scanning a Podfile for its platform directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodfilePlatform {
    /// The major.minor version string declared (e.g. "13.0"). None when absent.
    pub version: Option<String>,
}

/// Parse the contents of a `Podfile` and return the iOS platform version.
///
/// Looks for `platform :ios, 'X.Y'` (or double-quoted equivalent) on any
/// non-comment line. Returns None if absent.
pub fn parse_ios_platform(podfile: &str) -> PodfilePlatform {
    parse_platform(podfile, "ios")
}

/// Same for macOS Podfile.
pub fn parse_osx_platform(podfile: &str) -> PodfilePlatform {
    parse_platform(podfile, "osx")
}

fn parse_platform(podfile: &str, kind: &str) -> PodfilePlatform {
    let needle_lower = format!("platform :{kind}");
    for raw in podfile.lines() {
        let line = strip_comment(raw);
        let trimmed = line.trim();
        if !trimmed.contains(&needle_lower) {
            continue;
        }
        // Extract the first quoted string on the line.
        if let Some(version) = extract_first_quoted(trimmed) {
            return PodfilePlatform { version: Some(version) };
        }
    }
    PodfilePlatform { version: None }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn extract_first_quoted(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\'' || c == b'"' {
            let quote = c;
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j > start {
                return std::str::from_utf8(&bytes[start..j]).ok().map(|s| s.to_string());
            }
        }
        i += 1;
    }
    None
}

/// Compare two `MAJOR.MINOR` version strings. Returns negative if `a < b`,
/// zero if equal, positive if `a > b`. Unparseable segments compare as 0.
pub fn cmp_version(a: &str, b: &str) -> i32 {
    let parse = |v: &str| -> (u32, u32) {
        let mut it = v.split('.').filter_map(|s| s.parse::<u32>().ok());
        (it.next().unwrap_or(0), it.next().unwrap_or(0))
    };
    let (am, an) = parse(a);
    let (bm, bn) = parse(b);
    if am != bm {
        am as i32 - bm as i32
    } else {
        an as i32 - bn as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ios_platform_basic() {
        let pf = "platform :ios, '13.0'\n";
        assert_eq!(parse_ios_platform(pf).version, Some("13.0".to_string()));
    }

    #[test]
    fn parse_ios_platform_double_quoted() {
        let pf = "platform :ios, \"14.5\"\n";
        assert_eq!(parse_ios_platform(pf).version, Some("14.5".to_string()));
    }

    #[test]
    fn parse_ios_platform_absent() {
        let pf = "target 'Runner' do\n  use_frameworks!\nend\n";
        assert_eq!(parse_ios_platform(pf).version, None);
    }

    #[test]
    fn parse_ios_platform_commented_line_ignored() {
        let pf = "# platform :ios, '14.0'\n";
        assert_eq!(parse_ios_platform(pf).version, None);
    }

    #[test]
    fn parse_ios_platform_inline_comment_ignored_after_directive() {
        let pf = "platform :ios, '12.0' # legacy\n";
        assert_eq!(parse_ios_platform(pf).version, Some("12.0".to_string()));
    }

    #[test]
    fn parse_osx_platform_basic() {
        let pf = "platform :osx, '10.15'\n";
        assert_eq!(parse_osx_platform(pf).version, Some("10.15".to_string()));
    }

    #[test]
    fn cmp_version_ordering() {
        assert!(cmp_version("12.0", "13.0") < 0);
        assert!(cmp_version("13.0", "12.0") > 0);
        assert_eq!(cmp_version("13.0", "13.0"), 0);
        assert!(cmp_version("13.5", "13.0") > 0);
        assert!(cmp_version("13.0", "13.5") < 0);
        // Unparseable components default to 0
        assert_eq!(cmp_version("foo.bar", "0.0"), 0);
    }
}
```

- [ ] **Step 2: Create `src/check_pods/mod.rs` (stub)**

```rust
//! `falcon check-pods` — verify iOS Podfile deployment target.

pub mod podfile;
```

- [ ] **Step 3: Register module in `src/lib.rs`**

Add `pub mod check_pods;` near `pub mod check_a11y;`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib check_pods::podfile::tests 2>&1 | tail -10`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/check_pods/mod.rs src/check_pods/podfile.rs src/lib.rs
git commit -m "feat(check-pods): scaffold module + Podfile parser

Adds parse_ios_platform() / parse_osx_platform() returning the
quoted version string from \`platform :ios|:osx, 'X.Y'\` declarations,
plus cmp_version() for MAJOR.MINOR comparison.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Podspec scanner

**File:** Create `src/check_pods/podspec.rs`, modify `src/check_pods/mod.rs`.

- [ ] **Step 1: Create `src/check_pods/podspec.rs`**

```rust
//! Scan a plugin's podspecs for `s.ios.deployment_target` (and macOS sibling).

use std::path::{Path, PathBuf};

/// One podspec's relevant deployment-target info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodspecInfo {
    /// Path to the .podspec file.
    pub podspec_path: PathBuf,
    /// Plugin (parent directory) name.
    pub plugin: String,
    /// Declared iOS deployment target (e.g. "13.0"). None when absent → caller
    /// applies the CocoaPods default (9.0 for iOS, 10.10 for macOS).
    pub ios_target: Option<String>,
    /// Declared macOS deployment target (e.g. "10.15"). None when absent.
    pub osx_target: Option<String>,
}

/// Walk `<plugin_root>/ios/*.podspec` and `<plugin_root>/darwin/*.podspec`
/// and return one PodspecInfo per podspec found.
pub fn scan_plugin_podspecs(plugin_name: &str, plugin_root: &Path) -> Vec<PodspecInfo> {
    let mut out = Vec::new();
    for sub in ["ios", "darwin"] {
        let dir = plugin_root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("podspec") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            out.push(PodspecInfo {
                podspec_path: path.to_path_buf(),
                plugin: plugin_name.to_string(),
                ios_target: extract_deployment_target(&text, "ios"),
                osx_target: extract_deployment_target(&text, "osx"),
            });
        }
    }
    out
}

/// Extract `s.<platform>.deployment_target = '<X.Y>'` from a podspec body.
/// Handles single/double quotes and arbitrary whitespace. Returns None when
/// the directive is absent.
pub fn extract_deployment_target(podspec: &str, platform: &str) -> Option<String> {
    let needle = format!(".{platform}.deployment_target");
    for line in podspec.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if !trimmed.contains(&needle) {
            continue;
        }
        if let Some(version) = first_quoted(trimmed) {
            return Some(version);
        }
    }
    None
}

fn first_quoted(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\'' || c == b'"' {
            let quote = c;
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j > start {
                return std::str::from_utf8(&bytes[start..j]).ok().map(|s| s.to_string());
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_deployment_target_basic() {
        let s = "Pod::Spec.new do |s|\n  s.ios.deployment_target = '13.0'\nend\n";
        assert_eq!(extract_deployment_target(s, "ios"), Some("13.0".to_string()));
    }

    #[test]
    fn extract_deployment_target_double_quoted() {
        let s = "s.ios.deployment_target = \"14.5\"\n";
        assert_eq!(extract_deployment_target(s, "ios"), Some("14.5".to_string()));
    }

    #[test]
    fn extract_deployment_target_absent() {
        let s = "Pod::Spec.new do |s|\nend\n";
        assert_eq!(extract_deployment_target(s, "ios"), None);
    }

    #[test]
    fn extract_deployment_target_commented_out() {
        let s = "# s.ios.deployment_target = '13.0'\n";
        assert_eq!(extract_deployment_target(s, "ios"), None);
    }

    #[test]
    fn extract_deployment_target_osx_separate() {
        let s = "s.osx.deployment_target = '10.15'\n";
        assert_eq!(extract_deployment_target(s, "osx"), Some("10.15".to_string()));
        assert_eq!(extract_deployment_target(s, "ios"), None);
    }

    #[test]
    fn scan_plugin_podspecs_finds_ios_podspec() {
        let tmp = TempDir::new().unwrap();
        let plugin_root = tmp.path();
        std::fs::create_dir_all(plugin_root.join("ios")).unwrap();
        std::fs::write(
            plugin_root.join("ios/location.podspec"),
            "s.ios.deployment_target = '13.0'\n",
        )
        .unwrap();
        let result = scan_plugin_podspecs("location", plugin_root);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ios_target, Some("13.0".to_string()));
        assert_eq!(result[0].plugin, "location");
    }

    #[test]
    fn scan_plugin_podspecs_finds_multiple_podspecs() {
        let tmp = TempDir::new().unwrap();
        let plugin_root = tmp.path();
        std::fs::create_dir_all(plugin_root.join("ios")).unwrap();
        std::fs::write(plugin_root.join("ios/image_picker.podspec"), "s.ios.deployment_target = '12.0'\n").unwrap();
        std::fs::write(plugin_root.join("ios/image_picker_ios.podspec"), "s.ios.deployment_target = '14.0'\n").unwrap();
        let result = scan_plugin_podspecs("image_picker", plugin_root);
        assert_eq!(result.len(), 2);
        let max = result.iter().filter_map(|p| p.ios_target.as_deref()).max();
        assert_eq!(max, Some("14.0"));
    }

    #[test]
    fn scan_plugin_podspecs_walks_darwin_folder_too() {
        let tmp = TempDir::new().unwrap();
        let plugin_root = tmp.path();
        std::fs::create_dir_all(plugin_root.join("darwin")).unwrap();
        std::fs::write(
            plugin_root.join("darwin/path_provider.podspec"),
            "s.ios.deployment_target = '11.0'\n",
        )
        .unwrap();
        let result = scan_plugin_podspecs("path_provider", plugin_root);
        assert_eq!(result.len(), 1);
        assert!(result[0].podspec_path.to_string_lossy().contains("/darwin/"));
    }

    #[test]
    fn scan_plugin_podspecs_no_ios_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let result = scan_plugin_podspecs("pure_dart", tmp.path());
        assert!(result.is_empty());
    }
}
```

- [ ] **Step 2: Register submodule**

Edit `src/check_pods/mod.rs` to add `pub mod podspec;` after `pub mod podfile;`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib check_pods::podspec::tests 2>&1 | tail -10`
Expected: 9 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/check_pods/podspec.rs src/check_pods/mod.rs
git commit -m "feat(check-pods): podspec scanner

Adds scan_plugin_podspecs() walking ios/ and darwin/ for *.podspec files
and extract_deployment_target() pulling the platform's deployment target
out of each. Handles multiple podspecs per plugin (image_picker ships
two — root + ios suffix variant).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `check_pods::run` orchestrator

**File:** Modify `src/check_pods/mod.rs` (replace contents).

- [ ] **Step 1: Replace `src/check_pods/mod.rs` with:**

```rust
//! `falcon check-pods` — verify iOS Podfile deployment target.

pub mod podfile;
pub mod podspec;

use crate::config::{FalconConfig, Severity};
use crate::preflight::pub_cache::{iter_installed_plugins, locate_pub_cache};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue};
use anyhow::Result;
use std::path::{Path, PathBuf};

const RULE_ID_PODFILE_MISSING: &str = "pods/podfile-not-found";
const RULE_ID_DEPLOYMENT_TARGET_LOW: &str = "pods/deployment-target-too-low";
const RULE_ID_PLATFORM_NOT_SET: &str = "pods/platform-not-set";

/// Default iOS deployment target CocoaPods applies when the directive is absent.
const DEFAULT_IOS_PODFILE_TARGET: &str = "12.0";
/// Default iOS deployment target CocoaPods applies when a podspec omits it.
const DEFAULT_IOS_PODSPEC_TARGET: &str = "9.0";

/// Run the check. Returns exit code: 0 clean, 1 warnings only, 2 errors.
///
/// `_refresh` is reserved for future cache invalidation; check-pods does not
/// yet write `.falcon/plugin-requirements.yaml` itself — that lands with
/// check-platform-deps.
pub fn run(
    root: &Path,
    format: OutputFormat,
    config: &FalconConfig,
    _refresh: bool,
) -> Result<i32> {
    let mut issues: Vec<PreflightIssue> = Vec::new();

    let podfile_path = root.join("ios").join("Podfile");
    let Ok(podfile_text) = std::fs::read_to_string(&podfile_path) else {
        issues.push(PreflightIssue {
            rule_id: RULE_ID_PODFILE_MISSING.into(),
            severity: Severity::Info,
            title: "ios/Podfile not found".into(),
            file: Some(PathBuf::from("ios/Podfile")),
            line: None,
            plugin: None,
            message: "No ios/Podfile to check. Skipping pod deployment-target check.".into(),
            suggestion: None,
        });
        issues.retain(|issue| !is_suppressed(issue, config));
        let exit = exit_code_for_issues(&issues);
        print!("{}", reporter::render(&issues, format));
        return Ok(exit);
    };

    let podfile_platform = podfile::parse_ios_platform(&podfile_text);
    let podfile_target_str = podfile_platform
        .version
        .clone()
        .unwrap_or_else(|| DEFAULT_IOS_PODFILE_TARGET.to_string());

    if podfile_platform.version.is_none() {
        issues.push(PreflightIssue {
            rule_id: RULE_ID_PLATFORM_NOT_SET.into(),
            severity: Severity::Warning,
            title: "ios/Podfile does not set platform :ios".into(),
            file: Some(PathBuf::from("ios/Podfile")),
            line: None,
            plugin: None,
            message: format!(
                "No `platform :ios, 'X.Y'` directive found in ios/Podfile. \
CocoaPods will default to {DEFAULT_IOS_PODFILE_TARGET}, which may be lower than your plugins need."
            ),
            suggestion: Some(format!(
                "Add `platform :ios, '{DEFAULT_IOS_PODFILE_TARGET}'` (or a higher version) near the top of ios/Podfile."
            )),
        });
    }

    // Scan every hosted plugin's podspec(s) for ios deployment_target.
    let pub_cache = locate_pub_cache();
    let plugins = iter_installed_plugins(root, &pub_cache).unwrap_or_default();
    let mut max_target = podfile_target_str.clone();
    let mut binding_plugins: Vec<(String, String)> = Vec::new();

    for plugin in &plugins {
        let Some(plugin_root) = &plugin.root else { continue };
        if !plugin_root.is_dir() {
            continue;
        }
        let podspecs = podspec::scan_plugin_podspecs(&plugin.name, plugin_root);
        for ps in &podspecs {
            let target = ps
                .ios_target
                .clone()
                .unwrap_or_else(|| DEFAULT_IOS_PODSPEC_TARGET.to_string());
            if podfile::cmp_version(&target, &podfile_target_str) > 0 {
                binding_plugins.push((plugin.name.clone(), target.clone()));
                if podfile::cmp_version(&target, &max_target) > 0 {
                    max_target = target;
                }
            }
        }
    }

    if !binding_plugins.is_empty() {
        binding_plugins.sort();
        binding_plugins.dedup();
        let names: Vec<String> = binding_plugins
            .iter()
            .map(|(name, ver)| format!("{name} (requires {ver})"))
            .collect();
        issues.push(PreflightIssue {
            rule_id: RULE_ID_DEPLOYMENT_TARGET_LOW.into(),
            severity: Severity::Error,
            title: "iOS deployment target too low".into(),
            file: Some(PathBuf::from("ios/Podfile")),
            line: None,
            plugin: None,
            message: format!(
                "Podfile platform :ios = {podfile_target_str} but plugins require ≥ {max_target}: {}",
                names.join(", ")
            ),
            suggestion: Some(format!(
                "Set `platform :ios, '{max_target}'` near the top of ios/Podfile, then run `pod install`."
            )),
        });
    }

    issues.retain(|issue| !is_suppressed(issue, config));
    let exit = exit_code_for_issues(&issues);
    print!("{}", reporter::render(&issues, format));
    Ok(exit)
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

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    /// Build a faux project + faux pub-cache. Returns (project_tmp, pub_cache_tmp).
    fn fixture() -> (TempDir, TempDir) {
        let project = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        std::env::set_var("FALCON_PUB_CACHE", pub_cache.path());
        (project, pub_cache)
    }

    fn write_podfile(project: &Path, version: Option<&str>) {
        let body = match version {
            Some(v) => format!("platform :ios, '{v}'\ntarget 'Runner' do\nend\n"),
            None => "target 'Runner' do\nend\n".to_string(),
        };
        write(&project.join("ios/Podfile"), &body);
    }

    fn write_plugin(
        pub_cache: &Path,
        name: &str,
        version: &str,
        ios_deployment: Option<&str>,
    ) {
        let plugin_root = pub_cache
            .join("hosted/pub.dev")
            .join(format!("{name}-{version}"));
        std::fs::create_dir_all(plugin_root.join("ios")).unwrap();
        let podspec = match ios_deployment {
            Some(d) => format!("Pod::Spec.new do |s|\n  s.ios.deployment_target = '{d}'\nend\n"),
            None => "Pod::Spec.new do |s|\nend\n".to_string(),
        };
        std::fs::write(plugin_root.join("ios").join(format!("{name}.podspec")), podspec).unwrap();
    }

    fn write_lock(project: &Path, packages: &[(&str, &str)]) {
        let mut lock = String::from("packages:\n");
        for (name, version) in packages {
            lock.push_str(&format!("  {name}:\n    source: hosted\n    version: \"{version}\"\n"));
        }
        write(&project.join("pubspec.lock"), &lock);
    }

    #[test]
    fn no_podfile_emits_info_exit_zero() {
        let (project, _cache) = fixture();
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn missing_platform_directive_emits_warning() {
        let (project, _cache) = fixture();
        write_podfile(project.path(), None);
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        // No plugins → no Error. Just the Warning for missing directive.
        assert_eq!(code, 1);
    }

    #[test]
    fn plugin_requires_higher_than_podfile_emits_error() {
        let (project, cache) = fixture();
        write_podfile(project.path(), Some("12.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("13.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn plugin_equal_to_podfile_is_clean() {
        let (project, cache) = fixture();
        write_podfile(project.path(), Some("13.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("13.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn plugin_lower_than_podfile_is_clean() {
        let (project, cache) = fixture();
        write_podfile(project.path(), Some("14.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("11.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn multiple_plugins_max_target_named_in_error() {
        let (project, cache) = fixture();
        write_podfile(project.path(), Some("12.0"));
        write_lock(project.path(), &[("a", "1.0.0"), ("b", "2.0.0"), ("c", "3.0.0")]);
        write_plugin(cache.path(), "a", "1.0.0", Some("13.0"));
        write_plugin(cache.path(), "b", "2.0.0", Some("14.0"));
        write_plugin(cache.path(), "c", "3.0.0", Some("12.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn plugin_without_podspec_target_uses_cocoapods_default() {
        let (project, cache) = fixture();
        write_podfile(project.path(), Some("10.0"));
        write_lock(project.path(), &[("x", "1.0.0")]);
        write_plugin(cache.path(), "x", "1.0.0", None);
        // Default podspec target is 9.0 → 9.0 <= 10.0 → clean.
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn suppression_silences_low_target_error() {
        let (project, cache) = fixture();
        write_podfile(project.path(), Some("12.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("13.0"));
        let cfg = FalconConfig {
            preflight: PreflightConfig {
                suppress: vec![PreflightSuppression {
                    rule_id: RULE_ID_DEPLOYMENT_TARGET_LOW.into(),
                    plugin: None,
                    key: None,
                    reason: "Bumping deferred to release branch.".into(),
                }],
                config: Default::default(),
            },
            ..FalconConfig::default()
        };
        let code = run(project.path(), OutputFormat::Text, &cfg, false).unwrap();
        assert_eq!(code, 0);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib check_pods::tests 2>&1 | tail -15`
Expected: 8 tests pass.

NOTE: Tests set `FALCON_PUB_CACHE` env var. They may conflict if run in parallel — Rust's test runner uses threads by default. If you see flaky failures, run with `cargo test --lib check_pods::tests -- --test-threads=1`. If a stable serial approach is needed, also accept that as a workaround.

- [ ] **Step 3: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add src/check_pods/mod.rs
git commit -m "feat(check-pods): implement run() orchestrator

Wires Podfile parsing, podspec scanning per plugin in pub-cache,
deployment-target comparison, suppression. Errors when any plugin
requires a higher iOS deployment_target than the Podfile sets.
Warnings when the Podfile has no explicit platform directive.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Wire `Commands::CheckPods` into the CLI

**File:** Modify `src/main.rs`.

- [ ] **Step 1: Add the command variant** (near other `Commands::Check*`):

```rust
    /// Verify ios/Podfile deployment target is ≥ every plugin's required minimum.
    CheckPods {
        /// Path to the Flutter project.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Re-scan plugin podspecs even if cached results would be reused.
        #[arg(long)]
        refresh: bool,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: PreflightOutputFormat,
    },
```

- [ ] **Step 2: Add the dispatch arm:**

```rust
        Commands::CheckPods { path, refresh, format } => {
            let config = falcon::config::FalconConfig::load(&path).unwrap_or_default();
            let code = falcon::check_pods::run(&path, format, &config, refresh)?;
            process::exit(code);
        }
```

- [ ] **Step 3: Build + smoke test**

Run: `cargo build 2>&1 | tail -5` — expect clean.
Run: `cargo run --bin falcon -- check-pods --help 2>&1 | head -15` — expect help text showing `--refresh` and `--format`.

- [ ] **Step 4: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): wire Commands::CheckPods

Adds the \`falcon check-pods [path] [--refresh] [--format text|json|sarif]\`
subcommand that delegates to falcon::check_pods::run().

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Integration test

**File:** Create `tests/preflight_pods_tests.rs`.

- [ ] **Step 1: Create the file:**

```rust
//! End-to-end test for `falcon check-pods`.

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

#[test]
fn check_pods_exits_zero_with_no_podfile() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("check-pods")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn check_pods_exits_two_when_plugin_requires_higher() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    write(&tmp.path().join("ios/Podfile"), "platform :ios, '12.0'\n");
    write(
        &tmp.path().join("pubspec.lock"),
        "packages:\n  location:\n    source: hosted\n    version: \"8.0.0\"\n",
    );
    std::fs::create_dir_all(cache.path().join("hosted/pub.dev/location-8.0.0/ios")).unwrap();
    write(
        &cache
            .path()
            .join("hosted/pub.dev/location-8.0.0/ios/location.podspec"),
        "s.ios.deployment_target = '13.0'\n",
    );
    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("check-pods")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("location"), "stdout: {stdout}");
    assert!(stdout.contains("13.0"), "stdout: {stdout}");
}

#[test]
fn check_pods_json_format() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    write(&tmp.path().join("ios/Podfile"), "platform :ios, '12.0'\n");
    write(
        &tmp.path().join("pubspec.lock"),
        "packages:\n  location:\n    source: hosted\n    version: \"8.0.0\"\n",
    );
    std::fs::create_dir_all(cache.path().join("hosted/pub.dev/location-8.0.0/ios")).unwrap();
    write(
        &cache.path().join("hosted/pub.dev/location-8.0.0/ios/location.podspec"),
        "s.ios.deployment_target = '13.0'\n",
    );
    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("check-pods")
        .arg(tmp.path())
        .args(["--format", "json"])
        .output()
        .expect("failed to execute falcon");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["schema_version"], serde_json::json!(1));
    assert!(parsed["issues"].as_array().unwrap().iter().any(|i| {
        i["rule_id"] == "pods/deployment-target-too-low"
    }));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test preflight_pods_tests 2>&1 | tail -8`
Expected: 3 tests pass.

- [ ] **Step 3: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add tests/preflight_pods_tests.rs
git commit -m "test(check-pods): integration test against the CLI binary

Three e2e tests: no-podfile (exit 0), plugin-requires-higher (exit 2 +
stdout names plugin + version), and --format json validity.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Finalize

- [ ] **Step 1: Smoke test on falcon_dart (won't have ios/ but should not crash)**

Run: `cargo run --quiet --bin falcon -- check-pods falcon_dart 2>&1 | tail -10`
Expected: exit 0 with Info "ios/Podfile not found".

- [ ] **Step 2: Working tree clean**

Run: `git status -sb` — expect clean.

- [ ] **Step 3: Push**

Run: `git push 2>&1 | tail -3`

- [ ] **Step 4: Print summary**

Echo:

```
PR-3 (check-pods) complete + pushed.
Files added:    src/preflight/pub_cache.rs, src/check_pods/{mod,podfile,podspec}.rs,
                tests/preflight_pods_tests.rs
Files modified: src/preflight/mod.rs, src/lib.rs, src/main.rs
Tests added:    6 (pub_cache) + 7 (podfile) + 9 (podspec) + 8 (orchestrator) + 3 (integration) = 33
Next PR:        check-platform-deps per docs/superpowers/specs/2026-05-25-preflight-checks-design.md
```
