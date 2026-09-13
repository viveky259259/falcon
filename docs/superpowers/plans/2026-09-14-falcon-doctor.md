# Falcon Doctor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `falcon doctor` — a command that diagnoses the Flutter toolchain for the current project and installs or repairs what it can, exposed to AI agents through a two-phase MCP tool.

**Architecture:** A `Check` probes the host and returns a `CheckResult`; a fixer turns user *decisions* into an ordered `Plan` of `Step`s; an executor runs the plan through injected `CommandRunner`/`Downloader` traits. The CLI and the MCP tool are thin adapters over that identical core, so interactive prompts and an agent's JSON decisions cannot drift apart.

**Tech Stack:** Rust 2021, clap 4 (derive), serde/serde_json/serde_yaml, sha2, `colored`. Network and archive work shells out to `curl`/`tar`/`unzip`, matching `src/self_update.rs`. No new runtime dependencies.

**Spec:** `docs/superpowers/specs/2026-09-14-falcon-doctor-design.md`

## Global Constraints

Every task's requirements implicitly include this section.

- **No new runtime dependencies.** No `reqwest`, no `dialoguer`, no `inquire`, no `semver` crate. `Cargo.toml` `[dependencies]` must be unchanged by this plan. Dev-dependencies already present (`tempfile`, `pretty_assertions`, `jsonschema`) may be used freely.
- **Falcon never runs `sudo`.** A step requiring root is always `Step::Handoff`.
- **Licences are never auto-accepted.** `flutter doctor --android-licenses` and `xcodebuild -license accept` are always `Step::Handoff`.
- **No shell rc file is ever modified.** PATH guidance is printed via `Step::PathHint`.
- **No test touches the network; no test writes outside a tempdir.** The executor is always driven through fake `CommandRunner`/`Downloader` in tests.
- **Home directory resolution** uses `std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))`, matching `src/main.rs:3103`. Do not use the `dirs` crate — it is gated behind the `ai-local` feature.
- **Flutter channels are `stable`, `beta`, `master`.** There is no "alpha" channel. The release manifest publishes archives for stable and beta only.
- **Files under 500 lines, functions under 80 lines** (project convention, `CLAUDE.md`).
- **Errors are `anyhow::Result<T>` at boundaries**; no panics in library code.
- Run `cargo fmt` and `cargo clippy` before every commit. Prefix cargo with `source ~/.cargo/env &&`.

## File Structure

**Created:**

| File | Responsibility |
|---|---|
| `src/version_util.rs` | Shared semantic-version comparison and pubspec range satisfaction |
| `src/doctor/mod.rs` | Orchestrator: `probe()`, `plan()`, `execute()`; re-exports |
| `src/doctor/types.rs` | `Status`, `FixKind`, `CheckResult`, `FixOffer`, `Question`, `Choice`, `Plan`, `Step`, `Probe`, `Outcome`, `Trust`, exit-code mapping |
| `src/doctor/host.rs` | OS/arch/home/shell-rc/package-manager detection as pure functions |
| `src/doctor/exec.rs` | `CommandRunner` + `Downloader` traits, real and fake impls, `execute_plan` |
| `src/doctor/report.rs` | Text and JSON rendering of a `Diagnosis` and of execution outcomes |
| `src/doctor/prompt.rs` | Numbered-choice stdin prompt over `Question`/`Choice` |
| `src/doctor/checks/mod.rs` | `Check` trait and the check registry |
| `src/doctor/checks/flutter.rs` | Flutter probe + fix offer |
| `src/doctor/checks/dart.rs` | Dart probe, deduped against the Flutter fixer |
| `src/doctor/checks/cocoapods.rs` | CocoaPods probe + fixer |
| `src/doctor/checks/android.rs` | Android SDK probe + Assisted fixer |
| `src/doctor/checks/xcode.rs` | Xcode probe + Assisted fixer |
| `src/doctor/flutter/releases.rs` | Google release-manifest model, channel/arch filtering |
| `src/doctor/flutter/version.rs` | Pin discovery ladder + pubspec constraint resolution |
| `src/doctor/flutter/install.rs` | Builds the install `Plan` from decisions |
| `src/command_dispatch/environment.rs` | `Commands::Doctor` handler |
| `tests/fixtures/releases_macos.json` | Trimmed real-shape release manifest |
| `tests/doctor_cli.rs` | End-to-end `--dry-run --format json` integration test |

**Modified:** `src/lib.rs` (module registration) · `src/cli_args.rs` (`Commands::Doctor`) · `src/command_dispatch.rs` (`CommandGroup::Environment`) · `src/self_update.rs:292` (extract `version_cmp`) · `src/mcp/schema.rs` · `src/mcp/tools.rs` · `src/mcp/server.rs` · `src/api/server.rs` · `tests/v20_tests.rs` · `tests/mcp_tool_surface.rs`

**Working software checkpoint:** after Task 9, `falcon doctor` diagnoses and installs Flutter end to end. Tasks 10-13 add the remaining fixers; Task 14 adds the MCP surface.

---

### Task 1: Shared version comparison

`self_update.rs` already has a private `version_cmp`. Extract it so the doctor's constraint solver does not become a second implementation.

**Files:**
- Create: `src/version_util.rs`
- Modify: `src/self_update.rs:292` (delete `version_cmp`, import the shared one), `src/lib.rs` (add `pub mod version_util;`)
- Test: inline `#[cfg(test)] mod tests` in `src/version_util.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering`, `pub fn satisfies(version: &str, constraint: &str) -> bool`.

- [ ] **Step 1: Read the existing implementation**

Run: `sed -n 285,320p src/self_update.rs`

Copy its body verbatim into the new file — this task must not change comparison behaviour, only its location.

- [ ] **Step 2: Write the failing tests**

Create `src/version_util.rs` with the tests but no implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn compares_numeric_segments_not_strings() {
        assert_eq!(version_cmp("3.10.0", "3.9.0"), Ordering::Greater);
    }

    #[test]
    fn equal_versions_compare_equal() {
        assert_eq!(version_cmp("3.24.5", "3.24.5"), Ordering::Equal);
    }

    #[test]
    fn caret_constraint_allows_same_major() {
        assert!(satisfies("3.24.5", "^3.22.0"));
        assert!(!satisfies("4.0.0", "^3.22.0"));
        assert!(!satisfies("3.21.0", "^3.22.0"));
    }

    #[test]
    fn range_constraint_respects_both_bounds() {
        assert!(satisfies("3.5.0", ">=3.4.0 <4.0.0"));
        assert!(!satisfies("4.0.0", ">=3.4.0 <4.0.0"));
        assert!(!satisfies("3.3.0", ">=3.4.0 <4.0.0"));
    }

    #[test]
    fn any_constraint_accepts_everything() {
        assert!(satisfies("1.0.0", "any"));
        assert!(satisfies("1.0.0", ""));
    }

    #[test]
    fn prerelease_suffix_is_ignored_for_ordering() {
        assert_eq!(version_cmp("3.24.0-1.2.pre", "3.24.0"), Ordering::Equal);
    }

    #[test]
    fn unparseable_constraint_is_not_satisfied() {
        assert!(!satisfies("3.24.5", "wat"));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib version_util`
Expected: FAIL — `cannot find function version_cmp`

- [ ] **Step 4: Implement**

```rust
//! Shared semantic-version comparison and pubspec constraint satisfaction.

use std::cmp::Ordering;

/// Compare two dotted version strings numerically. Any `-suffix` or
/// `+suffix` is ignored, so `3.24.0-1.2.pre` equals `3.24.0`.
pub fn version_cmp(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (pa, pb) = (parts(a), parts(b));
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// Does `version` satisfy a pubspec-style `constraint`?
///
/// Supports `any`, the empty string, `^x.y.z`, and space-separated
/// comparator lists such as `>=3.4.0 <4.0.0`. An unrecognised constraint
/// returns `false` — callers must treat that as "cannot determine".
pub fn satisfies(version: &str, constraint: &str) -> bool {
    let c = constraint.trim().trim_matches(['\'', '"']);
    if c.is_empty() || c == "any" {
        return true;
    }
    if let Some(base) = c.strip_prefix('^') {
        let base = base.trim();
        if version_cmp(version, base) == Ordering::Less {
            return false;
        }
        let major = base.split('.').next().unwrap_or("0");
        let next_major = major.parse::<u64>().map(|m| m + 1).unwrap_or(0);
        return version_cmp(version, &format!("{}.0.0", next_major)) == Ordering::Less;
    }
    let mut saw_comparator = false;
    for token in c.split_whitespace() {
        let (op, rhs) = split_comparator(token);
        let Some(op) = op else { return false };
        saw_comparator = true;
        let ord = version_cmp(version, rhs);
        let ok = match op {
            ">=" => ord != Ordering::Less,
            "<=" => ord != Ordering::Greater,
            ">" => ord == Ordering::Greater,
            "<" => ord == Ordering::Less,
            "=" => ord == Ordering::Equal,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    saw_comparator
}

fn split_comparator(token: &str) -> (Option<&str>, &str) {
    for op in [">=", "<=", ">", "<", "="] {
        if let Some(rest) = token.strip_prefix(op) {
            return (Some(op), rest.trim());
        }
    }
    (None, token)
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib version_util`
Expected: PASS, 7 tests

- [ ] **Step 6: Point `self_update` at the shared function**

Delete `fn version_cmp` from `src/self_update.rs` and add `use crate::version_util::version_cmp;` near the top of that file. Add `pub mod version_util;` to `src/lib.rs` in alphabetical position (between `pub mod unused;` and `pub mod workspace;`).

- [ ] **Step 7: Verify nothing regressed**

Run: `source ~/.cargo/env && cargo test`
Expected: PASS — the full existing suite, including `self_update`'s own tests

- [ ] **Step 8: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/version_util.rs src/self_update.rs src/lib.rs
git commit -m "refactor: extract version_cmp into shared version_util with constraint satisfaction"
```

---

### Task 2: Doctor core types

**Files:**
- Create: `src/doctor/types.rs`, `src/doctor/mod.rs`
- Modify: `src/lib.rs` (add `pub mod doctor;`)
- Test: inline in `src/doctor/types.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Status`, `FixKind`, `Choice`, `Question`, `StepSummary`, `FixOffer`, `CheckResult`, `Diagnosis`, `Probe`, `Step`, `Plan`, `Outcome`, `Trust`, and `pub fn exit_code(checks: &[CheckResult], outcomes: &[Outcome]) -> i32`.

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/types.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &'static str, status: Status) -> CheckResult {
        CheckResult { id: id.to_string(), status, required_by: vec![], fix: None }
    }

    #[test]
    fn healthy_environment_exits_zero() {
        let checks = vec![check("flutter", Status::Ok { version: "3.24.5".into() })];
        assert_eq!(exit_code(&checks, &[]), 0);
    }

    #[test]
    fn outdated_is_a_warning() {
        let checks = vec![check("flutter", Status::Outdated {
            found: "3.19.0".into(),
            needed: "3.22.0".into(),
        })];
        assert_eq!(exit_code(&checks, &[]), 1);
    }

    #[test]
    fn missing_is_a_failure() {
        let checks = vec![check("flutter", Status::Missing)];
        assert_eq!(exit_code(&checks, &[]), 2);
    }

    #[test]
    fn skipped_checks_do_not_affect_exit_code() {
        let checks = vec![check("xcode", Status::Skipped { because: "no ios/ directory".into() })];
        assert_eq!(exit_code(&checks, &[]), 0);
    }

    #[test]
    fn awaiting_manual_outranks_failure() {
        let checks = vec![check("android", Status::Missing)];
        let outcomes = vec![Outcome::AwaitingManual { command: "flutter doctor --android-licenses".into() }];
        assert_eq!(exit_code(&checks, &outcomes), 3);
    }

    #[test]
    fn failed_outcome_is_exit_two() {
        let checks = vec![check("flutter", Status::Ok { version: "3.24.5".into() })];
        let outcomes = vec![Outcome::Failed { error: "checksum mismatch".into() }];
        assert_eq!(exit_code(&checks, &outcomes), 2);
    }

    #[test]
    fn status_serializes_with_a_state_tag() {
        let json = serde_json::to_value(Status::Missing).unwrap();
        assert_eq!(json, serde_json::json!({ "state": "missing" }));
    }

    #[test]
    fn choice_round_trips_through_json() {
        let c = Choice {
            value: "stable".into(),
            label: "stable".into(),
            rationale: "latest on stable".into(),
            recommended: true,
        };
        let back: Choice = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::types`
Expected: FAIL — `file not found for module doctor`, then missing types

- [ ] **Step 3: Implement the types**

Prepend to `src/doctor/types.rs`:

```rust
//! Core types shared by every doctor check, fixer and adapter.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The health of one toolchain component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Ok { version: String },
    Missing,
    Outdated { found: String, needed: String },
    Broken { reason: String },
    /// Not needed by this project — reported, never failed.
    Skipped { because: String },
}

/// How much of a fix Falcon can perform unattended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixKind {
    /// Falcon completes it start to finish.
    Automatic,
    /// Falcon completes part; at least one `Step::Handoff` remains.
    Assisted,
    /// Falcon can only describe the fix.
    Manual,
}

/// One option for a decision, carrying the reason it is suggested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
    pub rationale: String,
    pub recommended: bool,
}

/// A decision only the user (or the driving agent) can make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub prompt: String,
    pub options: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSummary {
    pub id: String,
    pub describe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixOffer {
    pub kind: FixKind,
    pub questions: Vec<Question>,
    pub steps: Vec<StepSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub status: Status,
    /// Why this project needs the component, e.g. "android/ directory present".
    pub required_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub host: crate::doctor::host::HostInfo,
    pub checks: Vec<CheckResult>,
}

/// A command run purely to confirm a step worked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    Download { id: String, url: String, sha256: String, dest: PathBuf },
    Extract { id: String, archive: PathBuf, dest: PathBuf },
    Run { id: String, program: String, args: Vec<String>, cwd: Option<PathBuf> },
    Verify { id: String, probe: Probe },
    /// Print the export line. Falcon never edits rc files.
    PathHint { id: String, dir: PathBuf, rc_file: Option<PathBuf> },
    /// A step Falcon will not perform: root, Apple ID auth, or a licence.
    Handoff { id: String, reason: String, command: String, docs_url: String, verify: Probe },
}

impl Step {
    pub fn id(&self) -> &str {
        match self {
            Step::Download { id, .. }
            | Step::Extract { id, .. }
            | Step::Run { id, .. }
            | Step::Verify { id, .. }
            | Step::PathHint { id, .. }
            | Step::Handoff { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub check_id: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Done { step_id: String },
    Skipped { step_id: String },
    AwaitingManual { command: String },
    Failed { error: String },
}

/// Where a request came from. `Remote` may never execute a fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Local,
    Remote,
}

/// 0 healthy, 1 warnings, 2 a check or fix failed, 3 manual action required.
pub fn exit_code(checks: &[CheckResult], outcomes: &[Outcome]) -> i32 {
    if outcomes.iter().any(|o| matches!(o, Outcome::AwaitingManual { .. })) {
        return 3;
    }
    if outcomes.iter().any(|o| matches!(o, Outcome::Failed { .. })) {
        return 2;
    }
    let mut code = 0;
    for c in checks {
        match c.status {
            Status::Missing | Status::Broken { .. } => return 2,
            Status::Outdated { .. } => code = code.max(1),
            Status::Ok { .. } | Status::Skipped { .. } => {}
        }
    }
    code
}
```

Create `src/doctor/mod.rs`:

```rust
//! `falcon doctor` — diagnose the project's toolchain and repair what we can.

pub mod host;
pub mod types;

pub use types::*;
```

Add `pub mod doctor;` to `src/lib.rs` between `pub mod docs;` and `pub mod flutter_run;`.

- [ ] **Step 4: Stub the host module so this compiles**

Task 3 fills this in. For now create `src/doctor/host.rs`:

```rust
//! Host platform detection.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub home: Option<PathBuf>,
    pub shell_rc: Option<PathBuf>,
    pub package_managers: Vec<String>,
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::types`
Expected: PASS, 8 tests

- [ ] **Step 6: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/ src/lib.rs
git commit -m "feat(doctor): core types for checks, fixes, plans and exit codes"
```

---

### Task 3: Host detection

**Files:**
- Modify: `src/doctor/host.rs` (replace the Task 2 stub)
- Test: inline in `src/doctor/host.rs`

**Interfaces:**
- Consumes: `HostInfo` from Task 2.
- Produces: `pub enum Os { MacOs, Linux, Windows }`, `pub enum Arch { Arm64, X64 }`, `pub fn current_os() -> Os`, `pub fn current_arch() -> Arch`, `pub fn home_from(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf>`, `pub fn shell_rc_for(shell: &str, home: &Path) -> Option<PathBuf>`, `pub fn detect() -> HostInfo`. `Os` gains `pub fn manifest_name(&self) -> &'static str` returning `"macos" | "linux" | "windows"`.

- [ ] **Step 1: Write the failing tests**

Append to `src/doctor/host.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn home_prefers_home_over_userprofile() {
        let got = home_from(|k| match k {
            "HOME" => Some("/Users/ada".into()),
            "USERPROFILE" => Some("C:\\Users\\ada".into()),
            _ => None,
        });
        assert_eq!(got, Some(PathBuf::from("/Users/ada")));
    }

    #[test]
    fn home_falls_back_to_userprofile() {
        let got = home_from(|k| (k == "USERPROFILE").then(|| "C:\\Users\\ada".to_string()));
        assert_eq!(got, Some(PathBuf::from("C:\\Users\\ada")));
    }

    #[test]
    fn home_is_none_when_neither_is_set() {
        assert_eq!(home_from(|_| None), None);
    }

    #[test]
    fn shell_rc_maps_known_shells() {
        let home = Path::new("/Users/ada");
        assert_eq!(shell_rc_for("/bin/zsh", home), Some(home.join(".zshrc")));
        assert_eq!(shell_rc_for("/bin/bash", home), Some(home.join(".bashrc")));
        assert_eq!(
            shell_rc_for("/opt/homebrew/bin/fish", home),
            Some(home.join(".config/fish/config.fish"))
        );
    }

    #[test]
    fn shell_rc_is_none_for_unknown_shell() {
        assert_eq!(shell_rc_for("/bin/nonsuch", Path::new("/Users/ada")), None);
    }

    #[test]
    fn manifest_name_matches_google_naming() {
        assert_eq!(Os::MacOs.manifest_name(), "macos");
        assert_eq!(Os::Linux.manifest_name(), "linux");
        assert_eq!(Os::Windows.manifest_name(), "windows");
    }

    #[test]
    fn detect_populates_os_and_arch() {
        let info = detect();
        assert!(!info.os.is_empty());
        assert!(info.arch == "arm64" || info.arch == "x64");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::host`
Expected: FAIL — `cannot find function home_from`

- [ ] **Step 3: Implement**

Replace the body of `src/doctor/host.rs` above the test module with:

```rust
//! Host platform detection, written as pure functions so tests never read
//! the real environment.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
    Windows,
}

impl Os {
    /// The name Google uses in `releases_<name>.json`.
    pub fn manifest_name(&self) -> &'static str {
        match self {
            Os::MacOs => "macos",
            Os::Linux => "linux",
            Os::Windows => "windows",
        }
    }
}

/// Matches the `dart_sdk_arch` field in the release manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    Arm64,
    X64,
}

impl Arch {
    pub fn manifest_name(&self) -> &'static str {
        match self {
            Arch::Arm64 => "arm64",
            Arch::X64 => "x64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub home: Option<PathBuf>,
    pub shell_rc: Option<PathBuf>,
    pub package_managers: Vec<String>,
}

pub fn current_os() -> Os {
    if cfg!(target_os = "macos") {
        Os::MacOs
    } else if cfg!(target_os = "windows") {
        Os::Windows
    } else {
        Os::Linux
    }
}

pub fn current_arch() -> Arch {
    if cfg!(target_arch = "aarch64") {
        Arch::Arm64
    } else {
        Arch::X64
    }
}

/// Resolve the home directory, mirroring `src/main.rs:3103`.
pub fn home_from(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    get("HOME")
        .or_else(|| get("USERPROFILE"))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// The rc file Falcon will *name* (never edit) for a given `$SHELL`.
pub fn shell_rc_for(shell: &str, home: &Path) -> Option<PathBuf> {
    let name = Path::new(shell).file_name()?.to_str()?;
    match name {
        "zsh" => Some(home.join(".zshrc")),
        "bash" => Some(home.join(".bashrc")),
        "fish" => Some(home.join(".config/fish/config.fish")),
        _ => None,
    }
}

/// Which package managers are on PATH — used to pick between `brew` and `gem`.
pub fn package_managers(is_on_path: impl Fn(&str) -> bool) -> Vec<String> {
    ["brew", "apt-get", "dnf", "pacman", "choco", "winget", "gem", "xcodes"]
        .iter()
        .filter(|p| is_on_path(p))
        .map(|p| p.to_string())
        .collect()
}

/// Is `program` resolvable on PATH? Uses `which`/`where`, no new dependency.
pub fn on_path(program: &str) -> bool {
    let finder = if cfg!(target_os = "windows") { "where" } else { "which" };
    std::process::Command::new(finder)
        .arg(program)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn detect() -> HostInfo {
    let home = home_from(|k| std::env::var(k).ok());
    let shell_rc = home
        .as_ref()
        .and_then(|h| std::env::var("SHELL").ok().and_then(|s| shell_rc_for(&s, h)));
    HostInfo {
        os: current_os().manifest_name().to_string(),
        arch: current_arch().manifest_name().to_string(),
        home,
        shell_rc,
        package_managers: package_managers(on_path),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::host`
Expected: PASS, 7 tests

- [ ] **Step 5: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/host.rs
git commit -m "feat(doctor): host os/arch/home/shell detection as pure functions"
```

---

### Task 4: Executor with injected runner and downloader

This is the task that makes every later task testable without a network.

**Files:**
- Create: `src/doctor/exec.rs`
- Modify: `src/doctor/mod.rs` (add `pub mod exec;`)
- Test: inline in `src/doctor/exec.rs`

**Interfaces:**
- Consumes: `Step`, `Plan`, `Outcome`, `Probe` from Task 2.
- Produces:
  - `pub trait CommandRunner { fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> Result<CommandOutput, String>; }`
  - `pub trait Downloader { fn fetch(&self, url: &str, dest: &Path) -> Result<(), String>; fn sha256(&self, path: &Path) -> Result<String, String>; }`
  - `pub struct CommandOutput { pub status: i32, pub stdout: String, pub stderr: String }`
  - `pub struct RealRunner;` `pub struct CurlDownloader;`
  - `pub struct FakeRunner { … }` and `pub struct FakeDownloader { … }` under `#[cfg(any(test, feature = "testing"))]` — plain `pub` is fine, they are test helpers in the same crate
  - `pub enum HandoffPolicy { Prompt, Report }`
  - `pub fn execute_plan(plan: &Plan, runner: &dyn CommandRunner, dl: &dyn Downloader, policy: HandoffPolicy, dry_run: bool) -> Vec<Outcome>`

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/exec.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::types::{Plan, Probe, Step};
    use std::path::PathBuf;

    fn download_plan(sha: &str) -> Plan {
        Plan {
            check_id: "flutter".into(),
            steps: vec![
                Step::Download {
                    id: "download".into(),
                    url: "https://example.test/flutter.tar.xz".into(),
                    sha256: sha.into(),
                    dest: PathBuf::from("/tmp/scratch/flutter.tar.xz"),
                },
                Step::Run {
                    id: "verify-runs".into(),
                    program: "flutter".into(),
                    args: vec!["--version".into()],
                    cwd: None,
                },
            ],
        }
    }

    #[test]
    fn dry_run_executes_nothing_and_reports_every_step_skipped() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("abc123");
        let out = execute_plan(&download_plan("abc123"), &runner, &dl, HandoffPolicy::Report, true);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|o| matches!(o, Outcome::Skipped { .. })));
        assert!(runner.calls().is_empty(), "dry run must not run commands");
        assert_eq!(dl.fetches(), 0, "dry run must not download");
    }

    #[test]
    fn checksum_mismatch_fails_and_stops_the_plan() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("deadbeef");
        let out = execute_plan(&download_plan("abc123"), &runner, &dl, HandoffPolicy::Report, false);
        assert!(matches!(out.last(), Some(Outcome::Failed { .. })));
        assert_eq!(out.len(), 1, "plan must stop at the failed step");
        assert!(runner.calls().is_empty(), "must not proceed after a bad checksum");
    }

    #[test]
    fn successful_plan_runs_each_step_in_order() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("abc123");
        let out = execute_plan(&download_plan("abc123"), &runner, &dl, HandoffPolicy::Report, false);
        assert!(out.iter().all(|o| matches!(o, Outcome::Done { .. })), "{:?}", out);
        assert_eq!(runner.calls(), vec!["flutter --version".to_string()]);
    }

    #[test]
    fn handoff_under_report_policy_awaits_manual_action() {
        let plan = Plan {
            check_id: "android".into(),
            steps: vec![Step::Handoff {
                id: "licenses".into(),
                reason: "Licence agreements must be accepted by you".into(),
                command: "flutter doctor --android-licenses".into(),
                docs_url: "https://docs.flutter.dev/get-started/install".into(),
                verify: Probe { program: "flutter".into(), args: vec!["doctor".into()] },
            }],
        };
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("");
        let out = execute_plan(&plan, &runner, &dl, HandoffPolicy::Report, false);
        assert_eq!(
            out,
            vec![Outcome::AwaitingManual { command: "flutter doctor --android-licenses".into() }]
        );
        assert!(runner.calls().is_empty(), "report policy must not run the verify probe");
    }

    #[test]
    fn failing_command_stops_the_plan() {
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let dl = FakeDownloader::new("abc123");
        let out = execute_plan(&download_plan("abc123"), &runner, &dl, HandoffPolicy::Report, false);
        assert!(matches!(out.last(), Some(Outcome::Failed { .. })));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn path_hint_is_never_a_command() {
        let plan = Plan {
            check_id: "flutter".into(),
            steps: vec![Step::PathHint {
                id: "path".into(),
                dir: PathBuf::from("/Users/ada/development/flutter"),
                rc_file: Some(PathBuf::from("/Users/ada/.zshrc")),
            }],
        };
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("");
        let out = execute_plan(&plan, &runner, &dl, HandoffPolicy::Report, false);
        assert_eq!(out, vec![Outcome::Done { step_id: "path".into() }]);
        assert!(runner.calls().is_empty(), "PathHint must never touch the shell");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::exec`
Expected: FAIL — `cannot find type FakeRunner`

- [ ] **Step 3: Implement**

Prepend to `src/doctor/exec.rs`:

```rust
//! Plan execution behind injectable IO, so tests assert the plan without
//! running commands or touching the network.

use crate::doctor::types::{Outcome, Plan, Step};
use std::path::Path;
use std::sync::Mutex;

pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait CommandRunner {
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>)
        -> Result<CommandOutput, String>;
}

pub trait Downloader {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), String>;
    fn sha256(&self, path: &Path) -> Result<String, String>;
}

/// What to do when a plan reaches a step Falcon will not perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffPolicy {
    /// Interactive: ask the user to run it, then re-verify. Used by the CLI on a TTY.
    Prompt,
    /// Report it and stop. Used by `--yes`, CI, and MCP.
    Report,
}

pub struct RealRunner;

impl CommandRunner for RealRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
    ) -> Result<CommandOutput, String> {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("failed to run `{}`: {}", program, e))?;
        Ok(CommandOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }
}

/// Shells out to `curl`, matching the pattern in `src/self_update.rs:49`.
pub struct CurlDownloader;

impl Downloader for CurlDownloader {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), String> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let status = std::process::Command::new("curl")
            .args([
                "-fL",
                "--proto",
                "=https",
                "--tlsv1.2",
                "--retry",
                "2",
                "-o",
            ])
            .arg(dest)
            .arg(url)
            .status()
            .map_err(|e| format!("failed to run curl: {}", e))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("curl failed downloading {}", url))
        }
    }

    fn sha256(&self, path: &Path) -> Result<String, String> {
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[derive(Default)]
pub struct FakeRunner {
    calls: Mutex<Vec<String>>,
    failing: Mutex<Vec<String>>,
}

impl FakeRunner {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub fn fail_on(&mut self, invocation: &str) {
        self.failing.lock().unwrap().push(invocation.to_string());
    }
}

impl CommandRunner for FakeRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        _cwd: Option<&Path>,
    ) -> Result<CommandOutput, String> {
        let invocation = if args.is_empty() {
            program.to_string()
        } else {
            format!("{} {}", program, args.join(" "))
        };
        self.calls.lock().unwrap().push(invocation.clone());
        if self.failing.lock().unwrap().contains(&invocation) {
            return Ok(CommandOutput {
                status: 1,
                stdout: String::new(),
                stderr: "boom".into(),
            });
        }
        Ok(CommandOutput { status: 0, stdout: String::new(), stderr: String::new() })
    }
}

pub struct FakeDownloader {
    digest: String,
    fetches: Mutex<usize>,
}

impl FakeDownloader {
    pub fn new(digest: &str) -> Self {
        Self { digest: digest.to_string(), fetches: Mutex::new(0) }
    }

    pub fn fetches(&self) -> usize {
        *self.fetches.lock().unwrap()
    }
}

impl Downloader for FakeDownloader {
    fn fetch(&self, _url: &str, _dest: &Path) -> Result<(), String> {
        *self.fetches.lock().unwrap() += 1;
        Ok(())
    }

    fn sha256(&self, _path: &Path) -> Result<String, String> {
        Ok(self.digest.clone())
    }
}

/// Run a plan step by step, stopping at the first failure or handoff.
pub fn execute_plan(
    plan: &Plan,
    runner: &dyn CommandRunner,
    dl: &dyn Downloader,
    policy: HandoffPolicy,
    dry_run: bool,
) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    for step in &plan.steps {
        if dry_run {
            outcomes.push(Outcome::Skipped { step_id: step.id().to_string() });
            continue;
        }
        let outcome = run_step(step, runner, dl, policy);
        let stop = matches!(outcome, Outcome::Failed { .. } | Outcome::AwaitingManual { .. });
        outcomes.push(outcome);
        if stop {
            break;
        }
    }
    outcomes
}

fn run_step(
    step: &Step,
    runner: &dyn CommandRunner,
    dl: &dyn Downloader,
    policy: HandoffPolicy,
) -> Outcome {
    match step {
        Step::Download { id, url, sha256, dest } => {
            if let Err(e) = dl.fetch(url, dest) {
                return Outcome::Failed { error: e };
            }
            match dl.sha256(dest) {
                Ok(actual) if &actual == sha256 => Outcome::Done { step_id: id.clone() },
                Ok(actual) => {
                    let _ = std::fs::remove_file(dest);
                    Outcome::Failed {
                        error: format!(
                            "checksum mismatch for {} (expected {}, got {}); download deleted",
                            url, sha256, actual
                        ),
                    }
                }
                Err(e) => Outcome::Failed { error: e },
            }
        }
        Step::Extract { id, archive, dest } => {
            let archive_s = archive.to_string_lossy().to_string();
            let dest_s = dest.to_string_lossy().to_string();
            let (program, args) = if archive_s.ends_with(".zip") {
                ("unzip", vec!["-q".to_string(), archive_s, "-d".to_string(), dest_s])
            } else {
                ("tar", vec!["-xf".to_string(), archive_s, "-C".to_string(), dest_s])
            };
            finish(runner.run(program, &args, None), id)
        }
        Step::Run { id, program, args, cwd } => finish(runner.run(program, args, cwd.as_deref()), id),
        Step::Verify { id, probe } => finish(runner.run(&probe.program, &probe.args, None), id),
        Step::PathHint { id, .. } => Outcome::Done { step_id: id.clone() },
        Step::Handoff { command, .. } => match policy {
            HandoffPolicy::Report => Outcome::AwaitingManual { command: command.clone() },
            HandoffPolicy::Prompt => Outcome::AwaitingManual { command: command.clone() },
        },
    }
}

fn finish(result: Result<CommandOutput, String>, id: &str) -> Outcome {
    match result {
        Ok(out) if out.status == 0 => Outcome::Done { step_id: id.to_string() },
        Ok(out) => Outcome::Failed {
            error: format!("step `{}` exited {}: {}", id, out.status, out.stderr.trim()),
        },
        Err(e) => Outcome::Failed { error: e },
    }
}
```

`HandoffPolicy::Prompt` is wired to the same result here deliberately — the interactive retry loop lives in the CLI adapter (Task 9), where stdin is available, not in the executor.

Add `pub mod exec;` to `src/doctor/mod.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::exec`
Expected: PASS, 6 tests

- [ ] **Step 5: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/exec.rs src/doctor/mod.rs
git commit -m "feat(doctor): plan executor behind injectable runner and downloader"
```

---

### Task 5: Flutter release manifest client

**Files:**
- Create: `src/doctor/flutter/mod.rs`, `src/doctor/flutter/releases.rs`, `tests/fixtures/releases_macos.json`
- Modify: `src/doctor/mod.rs` (add `pub mod flutter;`)
- Test: inline in `src/doctor/flutter/releases.rs`, reading the fixture with `include_str!`

**Interfaces:**
- Consumes: `Arch`, `Os` from Task 3.
- Produces: `pub struct Release { hash, channel, version, dart_sdk_version: Option<String>, dart_sdk_arch: Option<String>, archive, sha256 }`, `pub struct ReleaseManifest { base_url, current_release: BTreeMap<String, String>, releases: Vec<Release> }` with methods `parse(&str) -> Result<Self, String>`, `archive_url(&Release) -> String`, `for_channel(&str, Arch) -> Vec<&Release>` (newest first), `latest(&str, Arch) -> Option<&Release>`, `find(&str, &str, Arch) -> Option<&Release>`; plus `pub fn manifest_url(os: Os) -> String` and `pub fn channel_has_archives(channel: &str) -> bool`.

- [ ] **Step 1: Create the fixture**

Create `tests/fixtures/releases_macos.json` — real shape, trimmed, deliberately containing one arm64 entry, one x64 entry of the same version, one beta, and one legacy entry with no `dart_sdk_arch` field:

```json
{
  "base_url": "https://storage.googleapis.com/flutter_infra_release/releases",
  "current_release": {
    "beta": "b2222222222222222222222222222222222222222",
    "stable": "a1111111111111111111111111111111111111111"
  },
  "releases": [
    {
      "hash": "a1111111111111111111111111111111111111111",
      "channel": "stable",
      "version": "3.24.5",
      "dart_sdk_version": "3.5.4",
      "dart_sdk_arch": "arm64",
      "archive": "stable/macos/flutter_macos_arm64_3.24.5-stable.zip",
      "sha256": "1111111111111111111111111111111111111111111111111111111111111111"
    },
    {
      "hash": "a1111111111111111111111111111111111111111",
      "channel": "stable",
      "version": "3.24.5",
      "dart_sdk_version": "3.5.4",
      "dart_sdk_arch": "x64",
      "archive": "stable/macos/flutter_macos_3.24.5-stable.zip",
      "sha256": "2222222222222222222222222222222222222222222222222222222222222222"
    },
    {
      "hash": "b2222222222222222222222222222222222222222",
      "channel": "beta",
      "version": "3.27.0",
      "dart_sdk_version": "3.6.0",
      "dart_sdk_arch": "arm64",
      "archive": "beta/macos/flutter_macos_arm64_3.27.0-beta.zip",
      "sha256": "3333333333333333333333333333333333333333333333333333333333333333"
    },
    {
      "hash": "c3333333333333333333333333333333333333333",
      "channel": "stable",
      "version": "3.19.0",
      "dart_sdk_version": "3.3.0",
      "archive": "stable/macos/flutter_macos_3.19.0-stable.zip",
      "sha256": "4444444444444444444444444444444444444444444444444444444444444444"
    }
  ]
}
```

- [ ] **Step 2: Write the failing tests**

Create `src/doctor/flutter/releases.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::host::{Arch, Os};

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn manifest() -> ReleaseManifest {
        ReleaseManifest::parse(FIXTURE).expect("fixture must parse")
    }

    #[test]
    fn manifest_url_uses_googles_os_naming() {
        assert_eq!(
            manifest_url(Os::MacOs),
            "https://storage.googleapis.com/flutter_infra_release/releases/releases_macos.json"
        );
        assert!(manifest_url(Os::Linux).ends_with("releases_linux.json"));
    }

    #[test]
    fn for_channel_filters_by_architecture() {
        let m = manifest();
        let arm = m.for_channel("stable", Arch::Arm64);
        assert_eq!(arm.len(), 1, "only one stable arm64 entry in the fixture");
        assert!(arm[0].archive.contains("arm64"));
    }

    #[test]
    fn missing_dart_sdk_arch_is_treated_as_x64() {
        let m = manifest();
        let x64: Vec<_> = m.for_channel("stable", Arch::X64).iter().map(|r| r.version.clone()).collect();
        assert!(x64.contains(&"3.19.0".to_string()), "legacy entry must count as x64: {:?}", x64);
    }

    #[test]
    fn for_channel_returns_newest_first() {
        let m = manifest();
        let versions: Vec<_> = m.for_channel("stable", Arch::X64).iter().map(|r| r.version.clone()).collect();
        assert_eq!(versions, vec!["3.24.5".to_string(), "3.19.0".to_string()]);
    }

    #[test]
    fn latest_uses_current_release_hash() {
        let m = manifest();
        assert_eq!(m.latest("stable", Arch::Arm64).unwrap().version, "3.24.5");
        assert_eq!(m.latest("beta", Arch::Arm64).unwrap().version, "3.27.0");
    }

    #[test]
    fn find_matches_channel_version_and_arch() {
        let m = manifest();
        let r = m.find("stable", "3.24.5", Arch::X64).unwrap();
        assert_eq!(r.sha256, "2".repeat(64));
        assert!(m.find("stable", "9.9.9", Arch::X64).is_none());
    }

    #[test]
    fn archive_url_joins_base_and_path() {
        let m = manifest();
        let r = m.find("stable", "3.24.5", Arch::Arm64).unwrap();
        assert_eq!(
            m.archive_url(r),
            "https://storage.googleapis.com/flutter_infra_release/releases/stable/macos/flutter_macos_arm64_3.24.5-stable.zip"
        );
    }

    #[test]
    fn master_has_no_published_archives() {
        assert!(channel_has_archives("stable"));
        assert!(channel_has_archives("beta"));
        assert!(!channel_has_archives("master"));
    }

    #[test]
    fn unknown_fields_do_not_break_parsing() {
        let json = r#"{"base_url":"https://x","current_release":{},"releases":[],"surprise":1}"#;
        assert!(ReleaseManifest::parse(json).is_ok());
    }

    #[test]
    fn malformed_json_returns_an_error_not_a_panic() {
        assert!(ReleaseManifest::parse("not json").is_err());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::flutter::releases`
Expected: FAIL — `cannot find type ReleaseManifest`

- [ ] **Step 4: Implement**

Prepend to `src/doctor/flutter/releases.rs`:

```rust
//! Client for Google's Flutter release manifest — the same source
//! flutter.dev's install page is generated from.

use crate::doctor::host::{Arch, Os};
use serde::Deserialize;
use std::collections::BTreeMap;

const RELEASES_BASE: &str = "https://storage.googleapis.com/flutter_infra_release/releases";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub hash: String,
    pub channel: String,
    pub version: String,
    #[serde(default)]
    pub dart_sdk_version: Option<String>,
    /// Absent on older entries, which are x64.
    #[serde(default)]
    pub dart_sdk_arch: Option<String>,
    pub archive: String,
    pub sha256: String,
}

impl Release {
    pub fn arch(&self) -> Arch {
        match self.dart_sdk_arch.as_deref() {
            Some("arm64") => Arch::Arm64,
            _ => Arch::X64,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseManifest {
    pub base_url: String,
    #[serde(default)]
    pub current_release: BTreeMap<String, String>,
    pub releases: Vec<Release>,
}

impl ReleaseManifest {
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("malformed Flutter release manifest: {}", e))
    }

    pub fn archive_url(&self, release: &Release) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), release.archive)
    }

    /// Every release on `channel` for `arch`, newest version first.
    pub fn for_channel(&self, channel: &str, arch: Arch) -> Vec<&Release> {
        let mut out: Vec<&Release> = self
            .releases
            .iter()
            .filter(|r| r.channel == channel && r.arch() == arch)
            .collect();
        out.sort_by(|a, b| crate::version_util::version_cmp(&b.version, &a.version));
        out
    }

    /// The channel head, resolved through `current_release` and narrowed to `arch`.
    pub fn latest(&self, channel: &str, arch: Arch) -> Option<&Release> {
        let hash = self.current_release.get(channel)?;
        self.releases
            .iter()
            .find(|r| &r.hash == hash && r.arch() == arch)
            .or_else(|| self.for_channel(channel, arch).into_iter().next())
    }

    pub fn find(&self, channel: &str, version: &str, arch: Arch) -> Option<&Release> {
        self.releases
            .iter()
            .find(|r| r.channel == channel && r.version == version && r.arch() == arch)
    }
}

pub fn manifest_url(os: Os) -> String {
    format!("{}/releases_{}.json", RELEASES_BASE, os.manifest_name())
}

/// Only stable and beta ship prebuilt archives; master must be cloned.
pub fn channel_has_archives(channel: &str) -> bool {
    matches!(channel, "stable" | "beta")
}

pub const CHANNELS: [&str; 3] = ["stable", "beta", "master"];
```

Create `src/doctor/flutter/mod.rs`:

```rust
//! Flutter SDK resolution and installation.

pub mod releases;
```

Add `pub mod flutter;` to `src/doctor/mod.rs`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::flutter::releases`
Expected: PASS, 10 tests

- [ ] **Step 6: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/flutter/ src/doctor/mod.rs tests/fixtures/releases_macos.json
git commit -m "feat(doctor): Flutter release manifest client with arch-aware filtering"
```

---

### Task 6: Flutter version resolution ladder

**Files:**
- Create: `src/doctor/flutter/version.rs`
- Modify: `src/doctor/flutter/mod.rs` (add `pub mod version;`)
- Test: inline in `src/doctor/flutter/version.rs`, using `tempfile`

**Interfaces:**
- Consumes: `ReleaseManifest`, `Release` (Task 5); `crate::version_util::satisfies` (Task 1); `Arch` (Task 3).
- Produces:
  - `pub enum PinSource { Fvm, Asdf, CiWorkflow, Pubspec, ChannelHead }` with `pub fn rationale_prefix(&self) -> &'static str`
  - `pub struct VersionCandidate { pub version: String, pub channel: Option<String>, pub source: PinSource, pub rationale: String }`
  - `pub fn discover_pin(root: &Path) -> Option<VersionCandidate>`
  - `pub struct SdkConstraints { pub flutter: Option<String>, pub dart: Option<String> }`
  - `pub fn read_pubspec_constraints(root: &Path) -> SdkConstraints`
  - `pub fn resolve_from_constraints(m: &ReleaseManifest, channel: &str, arch: Arch, c: &SdkConstraints) -> Option<VersionCandidate>`
  - `pub fn candidates(root: &Path, m: &ReleaseManifest, channel: &str, arch: Arch) -> Vec<VersionCandidate>`

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/flutter/version.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::flutter::releases::ReleaseManifest;
    use crate::doctor::host::Arch;
    use std::fs;
    use tempfile::TempDir;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn manifest() -> ReleaseManifest {
        ReleaseManifest::parse(FIXTURE).unwrap()
    }

    fn project() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn fvmrc_is_the_highest_priority_pin() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.24.5"}"#).unwrap();
        fs::write(dir.path().join(".tool-versions"), "flutter 3.19.0-stable\n").unwrap();
        let pin = discover_pin(dir.path()).unwrap();
        assert_eq!(pin.version, "3.24.5");
        assert_eq!(pin.source, PinSource::Fvm);
        assert!(pin.rationale.contains("FVM"), "rationale was {:?}", pin.rationale);
    }

    #[test]
    fn legacy_fvm_config_is_read_when_fvmrc_is_absent() {
        let dir = project();
        fs::create_dir_all(dir.path().join(".fvm")).unwrap();
        fs::write(
            dir.path().join(".fvm/fvm_config.json"),
            r#"{"flutterSdkVersion":"3.22.1"}"#,
        )
        .unwrap();
        assert_eq!(discover_pin(dir.path()).unwrap().version, "3.22.1");
    }

    #[test]
    fn asdf_pin_beats_ci_workflow() {
        let dir = project();
        fs::write(dir.path().join(".tool-versions"), "dart 3.5.0\nflutter 3.19.0-stable\n").unwrap();
        fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        fs::write(
            dir.path().join(".github/workflows/ci.yml"),
            "jobs:\n  t:\n    steps:\n      - uses: subosito/flutter-action@v2\n        with:\n          flutter-version: 3.13.0\n",
        )
        .unwrap();
        let pin = discover_pin(dir.path()).unwrap();
        assert_eq!(pin.version, "3.19.0");
        assert_eq!(pin.source, PinSource::Asdf);
        assert_eq!(pin.channel.as_deref(), Some("stable"));
    }

    #[test]
    fn ci_workflow_pin_is_found_when_nothing_else_pins() {
        let dir = project();
        fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        fs::write(
            dir.path().join(".github/workflows/ci.yml"),
            "      - uses: subosito/flutter-action@v2\n        with:\n          flutter-version: '3.13.0'\n",
        )
        .unwrap();
        let pin = discover_pin(dir.path()).unwrap();
        assert_eq!(pin.version, "3.13.0");
        assert_eq!(pin.source, PinSource::CiWorkflow);
    }

    #[test]
    fn no_pin_returns_none() {
        assert!(discover_pin(project().path()).is_none());
    }

    #[test]
    fn pubspec_constraints_are_read_from_the_environment_block() {
        let dir = project();
        fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\nenvironment:\n  sdk: '>=3.4.0 <4.0.0'\n  flutter: '>=3.22.0'\n",
        )
        .unwrap();
        let c = read_pubspec_constraints(dir.path());
        assert_eq!(c.dart.as_deref(), Some(">=3.4.0 <4.0.0"));
        assert_eq!(c.flutter.as_deref(), Some(">=3.22.0"));
    }

    #[test]
    fn missing_pubspec_yields_empty_constraints() {
        let c = read_pubspec_constraints(project().path());
        assert!(c.flutter.is_none() && c.dart.is_none());
    }

    #[test]
    fn resolve_picks_the_newest_release_satisfying_both_constraints() {
        let c = SdkConstraints {
            flutter: Some(">=3.20.0".into()),
            dart: Some(">=3.4.0 <4.0.0".into()),
        };
        let got = resolve_from_constraints(&manifest(), "stable", Arch::X64, &c).unwrap();
        assert_eq!(got.version, "3.24.5");
        assert_eq!(got.source, PinSource::Pubspec);
    }

    #[test]
    fn resolve_rejects_releases_whose_dart_version_is_too_old() {
        let c = SdkConstraints { flutter: None, dart: Some(">=3.6.0".into()) };
        assert!(resolve_from_constraints(&manifest(), "stable", Arch::X64, &c).is_none());
    }

    #[test]
    fn candidates_always_include_channel_head_alongside_the_pin() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let got = candidates(dir.path(), &manifest(), "stable", Arch::X64);
        let versions: Vec<_> = got.iter().map(|c| c.version.clone()).collect();
        assert!(versions.contains(&"3.19.0".to_string()), "pin missing: {:?}", versions);
        assert!(versions.contains(&"3.24.5".to_string()), "channel head missing: {:?}", versions);
        assert_eq!(got[0].source, PinSource::Fvm, "the pin must be offered first");
    }

    #[test]
    fn candidates_are_deduplicated_when_pin_equals_channel_head() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.24.5"}"#).unwrap();
        let got = candidates(dir.path(), &manifest(), "stable", Arch::X64);
        assert_eq!(got.iter().filter(|c| c.version == "3.24.5").count(), 1);
    }

    #[test]
    fn a_pin_conflicting_with_pubspec_is_still_offered_with_the_conflict_stated() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\nenvironment:\n  flutter: '>=3.22.0'\n",
        )
        .unwrap();
        let got = candidates(dir.path(), &manifest(), "stable", Arch::X64);
        let pin = got.iter().find(|c| c.version == "3.19.0").expect("pin must still be offered");
        assert!(
            pin.rationale.contains("conflict"),
            "conflict must be stated: {:?}",
            pin.rationale
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::flutter::version`
Expected: FAIL — `cannot find function discover_pin`

- [ ] **Step 3: Implement**

Prepend to `src/doctor/flutter/version.rs`:

```rust
//! Which Flutter version does this project want?
//!
//! Ladder, first hit wins: FVM pin, asdf pin, CI workflow pin, pubspec
//! constraints, channel head. The channel head is always offered too, so the
//! user sees the difference between "what the project declares" and "newest".

use super::releases::ReleaseManifest;
use crate::doctor::host::Arch;
use crate::version_util::satisfies;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinSource {
    Fvm,
    Asdf,
    CiWorkflow,
    Pubspec,
    ChannelHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCandidate {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub source: PinSource,
    pub rationale: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SdkConstraints {
    pub flutter: Option<String>,
    pub dart: Option<String>,
}

pub fn discover_pin(root: &Path) -> Option<VersionCandidate> {
    read_fvmrc(root)
        .or_else(|| read_fvm_config(root))
        .or_else(|| read_tool_versions(root))
        .or_else(|| read_ci_workflows(root))
}

fn read_fvmrc(root: &Path) -> Option<VersionCandidate> {
    let text = std::fs::read_to_string(root.join(".fvmrc")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let version = value.get("flutter")?.as_str()?.to_string();
    Some(VersionCandidate {
        version,
        channel: None,
        source: PinSource::Fvm,
        rationale: "pinned by FVM in this repo (.fvmrc)".into(),
    })
}

fn read_fvm_config(root: &Path) -> Option<VersionCandidate> {
    let text = std::fs::read_to_string(root.join(".fvm").join("fvm_config.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let version = value.get("flutterSdkVersion")?.as_str()?.to_string();
    Some(VersionCandidate {
        version,
        channel: None,
        source: PinSource::Fvm,
        rationale: "pinned by FVM in this repo (.fvm/fvm_config.json)".into(),
    })
}

fn read_tool_versions(root: &Path) -> Option<VersionCandidate> {
    let text = std::fs::read_to_string(root.join(".tool-versions")).ok()?;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("flutter") {
            continue;
        }
        let raw = parts.next()?;
        // asdf spells versions `3.19.0-stable`.
        let (version, channel) = match raw.split_once('-') {
            Some((v, c)) => (v.to_string(), Some(c.to_string())),
            None => (raw.to_string(), None),
        };
        return Some(VersionCandidate {
            version,
            channel,
            source: PinSource::Asdf,
            rationale: "pinned by asdf (.tool-versions)".into(),
        });
    }
    None
}

fn read_ci_workflows(root: &Path) -> Option<VersionCandidate> {
    let dir = root.join(".github").join("workflows");
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !matches!(path.extension().and_then(|e| e.to_str()), Some("yml" | "yaml")) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for line in text.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("flutter-version:") else { continue };
            let version = rest.trim().trim_matches(['\'', '"']).to_string();
            if version.is_empty() || version.contains("${{") {
                continue;
            }
            let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            return Some(VersionCandidate {
                version,
                channel: None,
                source: PinSource::CiWorkflow,
                rationale: format!("what CI builds with (.github/workflows/{})", file),
            });
        }
    }
    None
}

/// Read `environment.sdk` and `environment.flutter` from pubspec.yaml.
pub fn read_pubspec_constraints(root: &Path) -> SdkConstraints {
    let Ok(text) = std::fs::read_to_string(root.join("pubspec.yaml")) else {
        return SdkConstraints::default();
    };
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return SdkConstraints::default();
    };
    let env = doc.get("environment");
    let read = |key: &str| -> Option<String> {
        env?.get(key)?.as_str().map(|s| s.trim().to_string())
    };
    SdkConstraints { flutter: read("flutter"), dart: read("sdk") }
}

/// Newest release on `channel` satisfying every declared constraint.
pub fn resolve_from_constraints(
    manifest: &ReleaseManifest,
    channel: &str,
    arch: Arch,
    constraints: &SdkConstraints,
) -> Option<VersionCandidate> {
    let release = manifest.for_channel(channel, arch).into_iter().find(|r| {
        let flutter_ok = constraints
            .flutter
            .as_deref()
            .map(|c| satisfies(&r.version, c))
            .unwrap_or(true);
        let dart_ok = match (&constraints.dart, &r.dart_sdk_version) {
            (Some(c), Some(v)) => satisfies(v, c),
            (Some(_), None) => false,
            (None, _) => true,
        };
        flutter_ok && dart_ok
    })?;
    let mut parts = Vec::new();
    if let Some(f) = &constraints.flutter {
        parts.push(format!("Flutter {}", f));
    }
    if let Some(d) = &constraints.dart {
        parts.push(format!("Dart {}", d));
    }
    Some(VersionCandidate {
        version: release.version.clone(),
        channel: Some(channel.to_string()),
        source: PinSource::Pubspec,
        rationale: format!("newest {} release satisfying {}", channel, parts.join(" and ")),
    })
}

/// Every version worth offering, pin first, channel head always included.
pub fn candidates(
    root: &Path,
    manifest: &ReleaseManifest,
    channel: &str,
    arch: Arch,
) -> Vec<VersionCandidate> {
    let constraints = read_pubspec_constraints(root);
    let mut out: Vec<VersionCandidate> = Vec::new();

    if let Some(mut pin) = discover_pin(root) {
        if let Some(c) = &constraints.flutter {
            if !satisfies(&pin.version, c) {
                pin.rationale = format!(
                    "{} — conflict: pubspec requires Flutter {}",
                    pin.rationale, c
                );
            }
        }
        out.push(pin);
    }

    if let Some(from_pubspec) = resolve_from_constraints(manifest, channel, arch, &constraints) {
        out.push(from_pubspec);
    }

    if let Some(head) = manifest.latest(channel, arch) {
        out.push(VersionCandidate {
            version: head.version.clone(),
            channel: Some(channel.to_string()),
            source: PinSource::ChannelHead,
            rationale: format!("latest on {}", channel),
        });
    }

    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.version.clone()));
    out
}
```

Add `pub mod version;` to `src/doctor/flutter/mod.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::flutter::version`
Expected: PASS, 12 tests

- [ ] **Step 5: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/flutter/version.rs src/doctor/flutter/mod.rs
git commit -m "feat(doctor): Flutter version ladder — FVM/asdf/CI pins ahead of pubspec constraints"
```

---

### Task 7: Flutter install plan builder

**Files:**
- Create: `src/doctor/flutter/install.rs`
- Modify: `src/doctor/flutter/mod.rs` (add `pub mod install;`)
- Test: inline in `src/doctor/flutter/install.rs`

**Interfaces:**
- Consumes: `ReleaseManifest` (Task 5), `Plan`/`Step`/`Probe` (Task 2), `Arch`/`HostInfo` (Task 3), `channel_has_archives` (Task 5).
- Produces:
  - `pub struct InstallDecisions { pub channel: String, pub version: String, pub dir: PathBuf }`
  - `pub fn default_install_dir(home: Option<&Path>) -> PathBuf`
  - `pub fn dir_is_usable(dir: &Path) -> Result<(), String>`
  - `pub fn build_plan(manifest: &ReleaseManifest, host: &HostInfo, arch: Arch, d: &InstallDecisions, scratch: &Path) -> Result<Plan, String>`
  - `pub fn path_export_line(dir: &Path) -> String`

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/flutter/install.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::flutter::releases::ReleaseManifest;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::Step;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn manifest() -> ReleaseManifest {
        ReleaseManifest::parse(FIXTURE).unwrap()
    }

    fn host() -> HostInfo {
        HostInfo {
            os: "macos".into(),
            arch: "arm64".into(),
            home: Some(PathBuf::from("/Users/ada")),
            shell_rc: Some(PathBuf::from("/Users/ada/.zshrc")),
            package_managers: vec![],
        }
    }

    fn decisions(channel: &str, version: &str) -> InstallDecisions {
        InstallDecisions {
            channel: channel.into(),
            version: version.into(),
            dir: PathBuf::from("/Users/ada/development/flutter"),
        }
    }

    #[test]
    fn default_dir_is_under_home() {
        assert_eq!(
            default_install_dir(Some(Path::new("/Users/ada"))),
            PathBuf::from("/Users/ada/development/flutter")
        );
    }

    #[test]
    fn default_dir_without_home_is_relative_to_cwd() {
        assert_eq!(default_install_dir(None), PathBuf::from("flutter"));
    }

    #[test]
    fn missing_directory_is_usable() {
        let dir = TempDir::new().unwrap();
        assert!(dir_is_usable(&dir.path().join("nope")).is_ok());
    }

    #[test]
    fn empty_directory_is_usable() {
        let dir = TempDir::new().unwrap();
        assert!(dir_is_usable(dir.path()).is_ok());
    }

    #[test]
    fn non_empty_directory_is_refused() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("something"), "x").unwrap();
        let err = dir_is_usable(dir.path()).unwrap_err();
        assert!(err.contains("not empty"), "unhelpful error: {}", err);
    }

    #[test]
    fn plan_downloads_extracts_verifies_and_hints_path() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "3.24.5"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        let ids: Vec<_> = plan.steps.iter().map(|s| s.id().to_string()).collect();
        assert_eq!(
            ids,
            vec!["download", "extract", "prime", "verify", "path-hint"],
            "plan step order changed"
        );
        assert_eq!(plan.check_id, "flutter");
    }

    #[test]
    fn plan_uses_the_arch_specific_archive_and_its_checksum() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "3.24.5"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        match &plan.steps[0] {
            Step::Download { url, sha256, .. } => {
                assert!(url.contains("arm64"), "wrong architecture archive: {}", url);
                assert_eq!(sha256, &"1".repeat(64));
            }
            other => panic!("expected a download step, got {:?}", other),
        }
    }

    #[test]
    fn master_channel_falls_back_to_a_git_clone() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("master", "master"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        match &plan.steps[0] {
            Step::Run { program, args, .. } => {
                assert_eq!(program, "git");
                assert!(args.contains(&"master".to_string()), "args were {:?}", args);
            }
            other => panic!("expected a git clone, got {:?}", other),
        }
        assert!(
            !plan.steps.iter().any(|s| matches!(s, Step::Download { .. })),
            "master has no published archive"
        );
    }

    #[test]
    fn unknown_version_is_an_error_not_a_panic() {
        let err = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "9.9.9"),
            Path::new("/tmp/scratch"),
        )
        .unwrap_err();
        assert!(err.contains("9.9.9"), "error should name the version: {}", err);
    }

    #[test]
    fn plan_never_contains_sudo() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "3.24.5"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        for step in &plan.steps {
            if let Step::Run { program, args, .. } = step {
                assert_ne!(program, "sudo");
                assert!(!args.iter().any(|a| a == "sudo"));
            }
        }
    }

    #[test]
    fn export_line_points_at_the_sdk_bin_directory() {
        let line = path_export_line(Path::new("/Users/ada/development/flutter"));
        assert_eq!(line, r#"export PATH="$PATH:/Users/ada/development/flutter/bin""#);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::flutter::install`
Expected: FAIL — `cannot find function build_plan`

- [ ] **Step 3: Implement**

Prepend to `src/doctor/flutter/install.rs`:

```rust
//! Turns install decisions into an executable plan. Builds the plan only —
//! running it is the executor's job, which is what makes `--dry-run` free.

use super::releases::{channel_has_archives, ReleaseManifest};
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::{Plan, Probe, Step};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallDecisions {
    pub channel: String,
    pub version: String,
    pub dir: PathBuf,
}

pub fn default_install_dir(home: Option<&Path>) -> PathBuf {
    match home {
        Some(h) => h.join("development").join("flutter"),
        None => PathBuf::from("flutter"),
    }
}

/// Refuse to install over an existing non-empty directory.
pub fn dir_is_usable(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    if !dir.is_dir() {
        return Err(format!("{} exists and is not a directory", dir.display()));
    }
    let mut entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    if entries.next().is_some() {
        return Err(format!(
            "{} is not empty — remove it or pass --dir to install elsewhere",
            dir.display()
        ));
    }
    Ok(())
}

pub fn path_export_line(dir: &Path) -> String {
    format!(r#"export PATH="$PATH:{}/bin""#, dir.display())
}

pub fn build_plan(
    manifest: &ReleaseManifest,
    host: &HostInfo,
    arch: Arch,
    decisions: &InstallDecisions,
    scratch: &Path,
) -> Result<Plan, String> {
    let flutter_bin = decisions.dir.join("bin").join("flutter");
    let flutter_bin_s = flutter_bin.to_string_lossy().to_string();

    let mut steps: Vec<Step> = Vec::new();

    if channel_has_archives(&decisions.channel) {
        let release = manifest
            .find(&decisions.channel, &decisions.version, arch)
            .ok_or_else(|| {
                format!(
                    "no {} {} build for {} on the {} channel",
                    decisions.version,
                    arch.manifest_name(),
                    host.os,
                    decisions.channel
                )
            })?;
        let archive_name = Path::new(&release.archive)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "flutter-sdk-archive".to_string());
        let archive_path = scratch.join(archive_name);
        let parent = decisions
            .dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        steps.push(Step::Download {
            id: "download".into(),
            url: manifest.archive_url(release),
            sha256: release.sha256.clone(),
            dest: archive_path.clone(),
        });
        steps.push(Step::Extract {
            id: "extract".into(),
            archive: archive_path,
            dest: parent,
        });
    } else {
        // master ships no archive; clone it instead.
        steps.push(Step::Run {
            id: "clone".into(),
            program: "git".into(),
            args: vec![
                "clone".into(),
                "--depth".into(),
                "1".into(),
                "-b".into(),
                decisions.channel.clone(),
                "https://github.com/flutter/flutter.git".into(),
                decisions.dir.to_string_lossy().to_string(),
            ],
            cwd: None,
        });
    }

    // Priming downloads the bundled Dart SDK and proves the binary runs.
    steps.push(Step::Run {
        id: "prime".into(),
        program: flutter_bin_s.clone(),
        args: vec!["--version".into()],
        cwd: None,
    });
    steps.push(Step::Verify {
        id: "verify".into(),
        probe: Probe { program: flutter_bin_s, args: vec!["--version".into()] },
    });
    steps.push(Step::PathHint {
        id: "path-hint".into(),
        dir: decisions.dir.clone(),
        rc_file: host.shell_rc.clone(),
    });

    Ok(Plan { check_id: "flutter".into(), steps })
}
```

Add `pub mod install;` to `src/doctor/flutter/mod.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::flutter::install`
Expected: PASS, 11 tests

- [ ] **Step 5: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/flutter/install.rs src/doctor/flutter/mod.rs
git commit -m "feat(doctor): build Flutter install plans with checksum and PATH guidance"
```

---

### Task 8: The Flutter check

**Files:**
- Create: `src/doctor/checks/mod.rs`, `src/doctor/checks/flutter.rs`
- Modify: `src/doctor/mod.rs` (add `pub mod checks;`)
- Test: inline in `src/doctor/checks/flutter.rs`

**Interfaces:**
- Consumes: everything from Tasks 2-7.
- Produces:
  - `pub trait Check { fn id(&self) -> &'static str; fn probe(&self, ctx: &CheckContext) -> CheckResult; }`
  - `pub struct CheckContext { pub root: PathBuf, pub host: HostInfo, pub arch: Arch, pub manifest: Option<ReleaseManifest>, pub runner: Box<dyn CommandRunner> }`
  - `pub struct FlutterCheck;` implementing `Check`
  - `pub fn flutter_questions(root: &Path, manifest: &ReleaseManifest, arch: Arch, host: &HostInfo) -> Vec<Question>`
  - `pub fn parse_flutter_version(stdout: &str) -> Option<String>`

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/checks/flutter.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::flutter::releases::ReleaseManifest;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::Status;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn host() -> HostInfo {
        HostInfo {
            os: "macos".into(),
            arch: "arm64".into(),
            home: Some(PathBuf::from("/Users/ada")),
            shell_rc: None,
            package_managers: vec![],
        }
    }

    fn ctx(root: &Path, runner: FakeRunner) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: host(),
            arch: Arch::Arm64,
            manifest: Some(ReleaseManifest::parse(FIXTURE).unwrap()),
            runner: Box::new(runner),
        }
    }

    #[test]
    fn parses_the_version_out_of_flutter_version_output() {
        let out = "Flutter 3.24.5 • channel stable • https://github.com/flutter/flutter.git";
        assert_eq!(parse_flutter_version(out).as_deref(), Some("3.24.5"));
    }

    #[test]
    fn unparseable_version_output_is_none() {
        assert_eq!(parse_flutter_version("command not found"), None);
    }

    #[test]
    fn missing_flutter_offers_an_automatic_fix() {
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(result.status, Status::Missing);
        let fix = result.fix.expect("a missing SDK must offer a fix");
        assert_eq!(fix.kind, FixKind::Automatic);
        let ids: Vec<_> = fix.questions.iter().map(|q| q.id.clone()).collect();
        assert_eq!(
            ids,
            vec!["flutter.channel", "flutter.version", "flutter.dir"]
        );
    }

    #[test]
    fn channel_question_offers_stable_beta_and_master_with_stable_recommended() {
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        let q = &result.fix.unwrap().questions[0];
        let values: Vec<_> = q.options.iter().map(|o| o.value.clone()).collect();
        assert_eq!(values, vec!["stable", "beta", "master"]);
        assert!(q.options[0].recommended);
        assert!(
            q.options[2].rationale.contains("not recommended for production"),
            "master must be labelled: {:?}",
            q.options[2].rationale
        );
    }

    #[test]
    fn version_options_carry_a_rationale_for_every_choice() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        let q = &result.fix.unwrap().questions[1];
        assert!(q.options.len() >= 2);
        assert!(q.options.iter().all(|o| !o.rationale.is_empty()));
        assert_eq!(q.options.iter().filter(|o| o.recommended).count(), 1);
    }

    #[test]
    fn a_working_flutter_reports_ok_and_offers_no_fix() {
        let dir = TempDir::new().unwrap();
        let runner = FakeRunner::with_stdout(
            "flutter --version",
            "Flutter 3.24.5 • channel stable • https://github.com/flutter/flutter.git",
        );
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(result.status, Status::Ok { version: "3.24.5".into() });
        assert!(result.fix.is_none());
    }

    #[test]
    fn a_version_below_the_pubspec_constraint_is_outdated() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\nenvironment:\n  flutter: '>=3.22.0'\n",
        )
        .unwrap();
        let runner = FakeRunner::with_stdout(
            "flutter --version",
            "Flutter 3.19.0 • channel stable • https://github.com/flutter/flutter.git",
        );
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(
            result.status,
            Status::Outdated { found: "3.19.0".into(), needed: ">=3.22.0".into() }
        );
        assert!(result.fix.is_some(), "an outdated SDK must still offer an upgrade");
    }

    #[test]
    fn without_a_manifest_the_fix_degrades_to_manual() {
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let mut c = ctx(dir.path(), runner);
        c.manifest = None;
        let result = FlutterCheck.probe(&c);
        assert_eq!(result.status, Status::Missing);
        assert_eq!(result.fix.unwrap().kind, FixKind::Manual);
    }
}
```

- [ ] **Step 2: Extend `FakeRunner` with a stdout helper**

The tests above need canned stdout. Add to `src/doctor/exec.rs`, inside `impl FakeRunner`:

```rust
    /// Build a runner that answers one invocation with canned stdout.
    pub fn with_stdout(invocation: &str, stdout: &str) -> Self {
        let runner = Self::default();
        runner
            .stdout
            .lock()
            .unwrap()
            .insert(invocation.to_string(), stdout.to_string());
        runner
    }
```

Add the field to the struct and populate it in `run`:

```rust
#[derive(Default)]
pub struct FakeRunner {
    calls: Mutex<Vec<String>>,
    failing: Mutex<Vec<String>>,
    stdout: Mutex<std::collections::HashMap<String, String>>,
}
```

and in `CommandRunner::run` for `FakeRunner`, replace the success return with:

```rust
        let stdout = self
            .stdout
            .lock()
            .unwrap()
            .get(&invocation)
            .cloned()
            .unwrap_or_default();
        Ok(CommandOutput { status: 0, stdout, stderr: String::new() })
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::checks::flutter`
Expected: FAIL — `cannot find type FlutterCheck`

- [ ] **Step 4: Implement the check trait**

Create `src/doctor/checks/mod.rs`:

```rust
//! Toolchain checks. Each one probes the host and, when it can, offers a fix.

pub mod flutter;

use crate::doctor::exec::CommandRunner;
use crate::doctor::flutter::releases::ReleaseManifest;
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::CheckResult;
use std::path::PathBuf;

pub struct CheckContext {
    pub root: PathBuf,
    pub host: HostInfo,
    pub arch: Arch,
    /// `None` when the release manifest could not be fetched — fixes then
    /// degrade to `Manual` rather than guessing.
    pub manifest: Option<ReleaseManifest>,
    pub runner: Box<dyn CommandRunner>,
}

pub trait Check {
    fn id(&self) -> &'static str;
    fn probe(&self, ctx: &CheckContext) -> CheckResult;
}
```

- [ ] **Step 5: Implement the Flutter check**

Prepend to `src/doctor/checks/flutter.rs`:

```rust
//! Is a usable Flutter SDK present, and if not, what would installing one take?

use super::{Check, CheckContext};
use crate::doctor::flutter::install::default_install_dir;
use crate::doctor::flutter::releases::{ReleaseManifest, CHANNELS};
use crate::doctor::flutter::version::{candidates, read_pubspec_constraints};
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::{CheckResult, Choice, FixKind, FixOffer, Question, Status, StepSummary};
use crate::version_util::satisfies;
use std::path::Path;

pub struct FlutterCheck;

/// Pull `3.24.5` out of `Flutter 3.24.5 • channel stable • …`.
pub fn parse_flutter_version(stdout: &str) -> Option<String> {
    let token = stdout.split_whitespace().nth(1)?;
    token
        .chars()
        .next()
        .filter(|c| c.is_ascii_digit())
        .map(|_| token.to_string())
}

impl Check for FlutterCheck {
    fn id(&self) -> &'static str {
        "flutter"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        let required_by = vec!["every Falcon command that builds or runs the app".to_string()];
        let constraints = read_pubspec_constraints(&ctx.root);

        let found = ctx
            .runner
            .run("flutter", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .and_then(|o| parse_flutter_version(&o.stdout));

        let status = match &found {
            None => Status::Missing,
            Some(v) => match &constraints.flutter {
                Some(c) if !satisfies(v, c) => {
                    Status::Outdated { found: v.clone(), needed: c.clone() }
                }
                _ => Status::Ok { version: v.clone() },
            },
        };

        let fix = match status {
            Status::Ok { .. } => None,
            _ => Some(build_offer(&ctx.root, ctx.manifest.as_ref(), ctx.arch, &ctx.host)),
        };

        CheckResult { id: self.id().to_string(), status, required_by, fix }
    }
}

fn build_offer(
    root: &Path,
    manifest: Option<&ReleaseManifest>,
    arch: Arch,
    host: &HostInfo,
) -> FixOffer {
    let Some(manifest) = manifest else {
        return FixOffer {
            kind: FixKind::Manual,
            questions: vec![],
            steps: vec![StepSummary {
                id: "manual".into(),
                describe: "Could not reach the Flutter release manifest. \
                           Install manually from https://docs.flutter.dev/get-started/install"
                    .into(),
            }],
        };
    };

    FixOffer {
        kind: FixKind::Automatic,
        questions: flutter_questions(root, manifest, arch, host),
        steps: vec![
            StepSummary { id: "download".into(), describe: "Download the SDK archive".into() },
            StepSummary { id: "extract".into(), describe: "Verify its checksum and extract".into() },
            StepSummary { id: "prime".into(), describe: "Run flutter --version once".into() },
            StepSummary { id: "path-hint".into(), describe: "Print the PATH line to add".into() },
        ],
    }
}

pub fn flutter_questions(
    root: &Path,
    manifest: &ReleaseManifest,
    arch: Arch,
    host: &HostInfo,
) -> Vec<Question> {
    let channel_q = Question {
        id: "flutter.channel".into(),
        prompt: "Which Flutter channel?".into(),
        options: CHANNELS
            .iter()
            .map(|c| Choice {
                value: c.to_string(),
                label: c.to_string(),
                rationale: match *c {
                    "stable" => "production-ready; what almost every project wants".into(),
                    "beta" => "next release, broadly usable, occasional breakage".into(),
                    _ => "bleeding edge, built from source, not recommended for production".into(),
                },
                recommended: *c == "stable",
            })
            .collect(),
        default: Some("stable".into()),
    };

    // Known limitation: the version list is computed for stable, because the
    // channel question has not been answered yet when the offer is built. If
    // the caller picks beta and then a stable-only version, `build_plan`
    // refuses with an error naming both the version and the channel — the
    // failure is loud, not a wrong install. Widening this to per-channel
    // option groups is a follow-up, not part of this plan.
    let found = candidates(root, manifest, "stable", arch);
    let version_q = Question {
        id: "flutter.version".into(),
        prompt: "Which Flutter version?".into(),
        options: found
            .iter()
            .enumerate()
            .map(|(i, c)| Choice {
                value: c.version.clone(),
                label: c.version.clone(),
                rationale: c.rationale.clone(),
                recommended: i == 0,
            })
            .collect(),
        default: found.first().map(|c| c.version.clone()),
    };

    let dir = default_install_dir(host.home.as_deref());
    let dir_q = Question {
        id: "flutter.dir".into(),
        prompt: "Where should the SDK be installed?".into(),
        options: vec![Choice {
            value: dir.to_string_lossy().to_string(),
            label: dir.to_string_lossy().to_string(),
            rationale: "the location flutter.dev's own instructions use".into(),
            recommended: true,
        }],
        default: Some(dir.to_string_lossy().to_string()),
    };

    vec![channel_q, version_q, dir_q]
}
```

Add `pub mod checks;` to `src/doctor/mod.rs`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::checks::flutter`
Expected: PASS, 8 tests

- [ ] **Step 7: Run the whole suite**

Run: `source ~/.cargo/env && cargo test`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/checks/ src/doctor/mod.rs src/doctor/exec.rs
git commit -m "feat(doctor): Flutter check with channel and version questions"
```

---

### Task 9: CLI wiring — `falcon doctor` end to end

**This is the working-software checkpoint.** After this task `falcon doctor` diagnoses and installs Flutter for real.

**Files:**
- Create: `src/doctor/prompt.rs`, `src/doctor/report.rs`, `src/command_dispatch/environment.rs`
- Modify: `src/doctor/mod.rs` (orchestrator + new submodules), `src/cli_args.rs` (add `Commands::Doctor`), `src/command_dispatch.rs` (add `CommandGroup::Environment`)
- Test: inline in `src/doctor/prompt.rs` and `src/doctor/report.rs`

**Interfaces:**
- Consumes: `Check`/`CheckContext`/`FlutterCheck` (Task 8), `execute_plan`/`HandoffPolicy`/`RealRunner`/`CurlDownloader` (Task 4), `build_plan`/`InstallDecisions`/`dir_is_usable` (Task 7), `manifest_url` (Task 5), `exit_code` (Task 2).
- Produces:
  - `pub struct DoctorOptions { pub root: PathBuf, pub fix: bool, pub yes: bool, pub dry_run: bool, pub only: Vec<String>, pub skip: Vec<String>, pub channel: Option<String>, pub flutter_version: Option<String>, pub dir: Option<PathBuf>, pub json: bool }`
  - `pub fn run(opts: &DoctorOptions) -> anyhow::Result<i32>`
  - `pub fn answer(q: &Question, preset: Option<&str>, yes: bool, input: &mut dyn BufRead, out: &mut dyn Write) -> Option<String>` (in `prompt.rs`)
  - `pub fn render_text(d: &Diagnosis) -> String`, `pub fn render_json(d: &Diagnosis) -> serde_json::Value` (in `report.rs`)

- [ ] **Step 1: Write the failing prompt tests**

Create `src/doctor/prompt.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::types::{Choice, Question};

    fn question() -> Question {
        Question {
            id: "flutter.channel".into(),
            prompt: "Which Flutter channel?".into(),
            options: vec![
                Choice { value: "stable".into(), label: "stable".into(), rationale: "production-ready".into(), recommended: true },
                Choice { value: "beta".into(), label: "beta".into(), rationale: "next release".into(), recommended: false },
            ],
            default: Some("stable".into()),
        }
    }

    fn ask(input: &str, preset: Option<&str>, yes: bool) -> (Option<String>, String) {
        let mut reader = std::io::BufReader::new(input.as_bytes());
        let mut out: Vec<u8> = Vec::new();
        let got = answer(&question(), preset, yes, &mut reader, &mut out);
        (got, String::from_utf8(out).unwrap())
    }

    #[test]
    fn a_preset_answer_skips_the_prompt_entirely() {
        let (got, printed) = ask("", Some("beta"), false);
        assert_eq!(got.as_deref(), Some("beta"));
        assert!(printed.is_empty(), "a preset must not print a prompt");
    }

    #[test]
    fn yes_mode_takes_the_recommended_option_without_reading_input() {
        let (got, printed) = ask("", None, true);
        assert_eq!(got.as_deref(), Some("stable"));
        assert!(printed.is_empty());
    }

    #[test]
    fn selecting_by_number_returns_that_option() {
        let (got, _) = ask("2\n", None, false);
        assert_eq!(got.as_deref(), Some("beta"));
    }

    #[test]
    fn empty_input_takes_the_default() {
        let (got, _) = ask("\n", None, false);
        assert_eq!(got.as_deref(), Some("stable"));
    }

    #[test]
    fn out_of_range_input_reprompts_then_accepts() {
        let (got, printed) = ask("9\n1\n", None, false);
        assert_eq!(got.as_deref(), Some("stable"));
        assert!(printed.contains("Enter 1"), "must explain the valid range: {}", printed);
    }

    #[test]
    fn eof_returns_none_rather_than_looping_forever() {
        let (got, _) = ask("", None, false);
        assert_eq!(got, None);
    }

    #[test]
    fn the_prompt_shows_each_rationale() {
        let (_, printed) = ask("1\n", None, false);
        assert!(printed.contains("production-ready"));
        assert!(printed.contains("next release"));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::prompt`
Expected: FAIL — `cannot find function answer`

- [ ] **Step 3: Implement the prompt**

Prepend to `src/doctor/prompt.rs`:

```rust
//! A numbered-choice prompt over `Question`/`Choice`. Reader and writer are
//! injected so the prompt is testable without a terminal, and so Falcon adds
//! no interactive dependency.

use crate::doctor::types::Question;
use std::io::{BufRead, Write};

/// Resolve one decision.
///
/// `preset` (a CLI flag) wins outright; `yes` takes the recommended option;
/// otherwise the user picks by number. Returns `None` on EOF.
pub fn answer(
    question: &Question,
    preset: Option<&str>,
    yes: bool,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Option<String> {
    if let Some(p) = preset {
        return Some(p.to_string());
    }
    if yes {
        return question
            .options
            .iter()
            .find(|o| o.recommended)
            .map(|o| o.value.clone())
            .or_else(|| question.default.clone());
    }
    if question.options.is_empty() {
        return question.default.clone();
    }

    loop {
        let _ = writeln!(out, "\n{}", question.prompt);
        for (i, opt) in question.options.iter().enumerate() {
            let marker = if opt.recommended { " (recommended)" } else { "" };
            let _ = writeln!(out, "  {}. {} — {}{}", i + 1, opt.label, opt.rationale, marker);
        }
        if let Some(d) = &question.default {
            let _ = write!(out, "Choice [{}]: ", d);
        } else {
            let _ = write!(out, "Choice: ");
        }
        let _ = out.flush();

        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(d) = &question.default {
                return Some(d.clone());
            }
            continue;
        }
        match trimmed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= question.options.len() => {
                return Some(question.options[n - 1].value.clone())
            }
            _ => {
                let _ = writeln!(out, "Enter 1-{}.", question.options.len());
            }
        }
    }
}
```

- [ ] **Step 4: Run them to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::prompt`
Expected: PASS, 7 tests

- [ ] **Step 5: Write the failing report tests**

Create `src/doctor/report.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::host::HostInfo;
    use crate::doctor::types::{CheckResult, Diagnosis, Status};

    fn diagnosis() -> Diagnosis {
        Diagnosis {
            host: HostInfo {
                os: "macos".into(),
                arch: "arm64".into(),
                home: None,
                shell_rc: None,
                package_managers: vec![],
            },
            checks: vec![
                CheckResult {
                    id: "flutter".into(),
                    status: Status::Missing,
                    required_by: vec!["every build command".into()],
                    fix: None,
                },
                CheckResult {
                    id: "xcode".into(),
                    status: Status::Skipped { because: "no ios/ directory".into() },
                    required_by: vec![],
                    fix: None,
                },
            ],
        }
    }

    #[test]
    fn text_report_names_every_check() {
        let text = render_text(&diagnosis());
        assert!(text.contains("flutter"));
        assert!(text.contains("xcode"));
    }

    #[test]
    fn text_report_explains_why_a_check_was_skipped() {
        assert!(render_text(&diagnosis()).contains("no ios/ directory"));
    }

    #[test]
    fn json_report_is_stable_and_machine_readable() {
        let json = render_json(&diagnosis());
        assert_eq!(json["checks"][0]["id"], "flutter");
        assert_eq!(json["checks"][0]["status"]["state"], "missing");
        assert_eq!(json["host"]["arch"], "arm64");
    }

    #[test]
    fn json_report_omits_absent_fixes() {
        let json = render_json(&diagnosis());
        assert!(json["checks"][0].get("fix").is_none());
    }
}
```

- [ ] **Step 6: Run them to verify they fail, then implement**

Run: `source ~/.cargo/env && cargo test --lib doctor::report`
Expected: FAIL — `cannot find function render_text`

Prepend to `src/doctor/report.rs`:

```rust
//! Rendering a diagnosis for humans and for machines.

use crate::doctor::types::{Diagnosis, Status};
use colored::Colorize;

pub fn render_text(d: &Diagnosis) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Falcon doctor — {} {}\n\n",
        d.host.os, d.host.arch
    ));
    for c in &d.checks {
        let (mark, detail) = match &c.status {
            Status::Ok { version } => ("ok".green().to_string(), version.clone()),
            Status::Missing => ("missing".red().to_string(), "not installed".to_string()),
            Status::Outdated { found, needed } => (
                "outdated".yellow().to_string(),
                format!("{} installed, {} required", found, needed),
            ),
            Status::Broken { reason } => ("broken".red().to_string(), reason.clone()),
            Status::Skipped { because } => ("skipped".dimmed().to_string(), because.clone()),
        };
        s.push_str(&format!("  {:<12} {:<10} {}\n", c.id, mark, detail));
        if let Some(fix) = &c.fix {
            for step in &fix.steps {
                s.push_str(&format!("               · {}\n", step.describe));
            }
        }
    }
    s
}

pub fn render_json(d: &Diagnosis) -> serde_json::Value {
    serde_json::to_value(d).unwrap_or(serde_json::Value::Null)
}
```

Run: `source ~/.cargo/env && cargo test --lib doctor::report`
Expected: PASS, 4 tests

- [ ] **Step 7: Write the orchestrator**

Replace `src/doctor/mod.rs` with:

```rust
//! `falcon doctor` — diagnose the project's toolchain and repair what we can.

pub mod checks;
pub mod exec;
pub mod flutter;
pub mod host;
pub mod prompt;
pub mod report;
pub mod types;

pub use types::*;

use crate::doctor::checks::{Check, CheckContext};
use crate::doctor::exec::{execute_plan, CurlDownloader, HandoffPolicy, RealRunner};
use crate::doctor::flutter::install::{dir_is_usable, InstallDecisions};
use crate::doctor::flutter::releases::{manifest_url, ReleaseManifest};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct DoctorOptions {
    pub root: PathBuf,
    pub fix: bool,
    pub yes: bool,
    pub dry_run: bool,
    pub only: Vec<String>,
    pub skip: Vec<String>,
    pub channel: Option<String>,
    pub flutter_version: Option<String>,
    pub dir: Option<PathBuf>,
    pub json: bool,
}

impl DoctorOptions {
    fn wants(&self, id: &str) -> bool {
        if !self.only.is_empty() {
            return self.only.iter().any(|o| o == id);
        }
        !self.skip.iter().any(|s| s == id)
    }

    /// JSON mode and `--yes` both mean "never read stdin".
    fn interactive(&self) -> bool {
        !self.json && !self.yes && std::io::IsTerminal::is_terminal(&std::io::stdin())
    }
}

fn registry() -> Vec<Box<dyn Check>> {
    vec![Box::new(checks::flutter::FlutterCheck)]
}

/// Fetch the release manifest. A failure is not fatal — fixes degrade to Manual.
fn fetch_manifest(host_os: host::Os) -> Option<ReleaseManifest> {
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--proto", "=https", "--tlsv1.2", "--max-time", "20"])
        .arg(manifest_url(host_os))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    ReleaseManifest::parse(&String::from_utf8_lossy(&out.stdout)).ok()
}

pub fn run(opts: &DoctorOptions) -> Result<i32> {
    let host_info = host::detect();
    let arch = host::current_arch();
    let manifest = fetch_manifest(host::current_os());

    let mut results = Vec::new();
    for check in registry() {
        if !opts.wants(check.id()) {
            continue;
        }
        let ctx = CheckContext {
            root: opts.root.clone(),
            host: host_info.clone(),
            arch,
            manifest: manifest.clone(),
            runner: Box::new(RealRunner),
        };
        results.push(check.probe(&ctx));
    }

    let diagnosis = Diagnosis { host: host_info.clone(), checks: results };

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&report::render_json(&diagnosis))?);
        return Ok(exit_code(&diagnosis.checks, &[]));
    }
    print!("{}", report::render_text(&diagnosis));

    let mut outcomes: Vec<Outcome> = Vec::new();
    for check in &diagnosis.checks {
        let Some(offer) = &check.fix else { continue };
        if offer.kind == FixKind::Manual {
            continue;
        }
        if !opts.fix && !confirm(&format!("Fix {} now?", check.id), opts)? {
            continue;
        }
        let decisions = collect_decisions(offer, opts)?;
        let Some(decisions) = decisions else { continue };
        outcomes.extend(apply(check, &decisions, &manifest, &host_info, arch, opts)?);
    }

    Ok(exit_code(&diagnosis.checks, &outcomes))
}

fn confirm(question: &str, opts: &DoctorOptions) -> Result<bool> {
    if !opts.interactive() {
        return Ok(false);
    }
    use std::io::Write;
    print!("{} [Y/n]: ", question);
    std::io::stdout().flush()?;
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line)? == 0 {
        return Ok(false);
    }
    Ok(!matches!(line.trim().to_ascii_lowercase().as_str(), "n" | "no"))
}

/// Ask (or infer) every question the fix needs. `None` means the user bailed.
fn collect_decisions(
    offer: &FixOffer,
    opts: &DoctorOptions,
) -> Result<Option<HashMap<String, String>>> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut out = std::io::stdout();
    let mut answers = HashMap::new();
    for q in &offer.questions {
        let preset = match q.id.as_str() {
            "flutter.channel" => opts.channel.clone(),
            "flutter.version" => opts.flutter_version.clone(),
            "flutter.dir" => opts.dir.as_ref().map(|d| d.to_string_lossy().to_string()),
            _ => None,
        };
        let Some(value) = prompt::answer(q, preset.as_deref(), !opts.interactive(), &mut reader, &mut out)
        else {
            return Ok(None);
        };
        answers.insert(q.id.clone(), value);
    }
    Ok(Some(answers))
}

fn apply(
    check: &CheckResult,
    decisions: &HashMap<String, String>,
    manifest: &Option<ReleaseManifest>,
    host_info: &host::HostInfo,
    arch: host::Arch,
    opts: &DoctorOptions,
) -> Result<Vec<Outcome>> {
    if check.id != "flutter" {
        return Ok(vec![]);
    }
    let Some(manifest) = manifest else {
        return Ok(vec![Outcome::Failed {
            error: "Flutter release manifest unavailable; install manually from \
                    https://docs.flutter.dev/get-started/install"
                .into(),
        }]);
    };

    let dir = decisions
        .get("flutter.dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| flutter::install::default_install_dir(host_info.home.as_deref()));
    if !opts.dry_run {
        if let Err(e) = dir_is_usable(&dir) {
            return Ok(vec![Outcome::Failed { error: e }]);
        }
    }

    let install = InstallDecisions {
        channel: decisions.get("flutter.channel").cloned().unwrap_or_else(|| "stable".into()),
        version: decisions.get("flutter.version").cloned().unwrap_or_else(|| "latest".into()),
        dir: dir.clone(),
    };

    let scratch = std::env::temp_dir().join("falcon-doctor");
    let plan = match flutter::install::build_plan(manifest, host_info, arch, &install, &scratch) {
        Ok(p) => p,
        Err(e) => return Ok(vec![Outcome::Failed { error: e }]),
    };

    let policy = if opts.interactive() { HandoffPolicy::Prompt } else { HandoffPolicy::Report };
    let outcomes = execute_plan(&plan, &RealRunner, &CurlDownloader, policy, opts.dry_run);

    for step in &plan.steps {
        if let Step::PathHint { dir, rc_file } = step {
            println!("\nAdd this to your shell, then restart it:");
            println!("  {}", flutter::install::path_export_line(dir));
            if let Some(rc) = rc_file {
                println!("  (your shell reads {})", rc.display());
            }
            println!("`flutter` will not be on PATH in this shell until you do.");
        }
    }

    Ok(outcomes)
}
```

- [ ] **Step 8: Add the CLI command**

Add to `src/cli_args.rs` inside `pub enum Commands` (before the final `X { .. }` variant):

```rust
    /// Diagnose the project's toolchain and install or repair what's missing
    Doctor {
        /// Path to the Flutter project (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Apply fixes without asking to confirm each one
        #[arg(long)]
        fix: bool,

        /// Accept every recommended default; never prompt
        #[arg(long)]
        yes: bool,

        /// Print the plan without executing anything
        #[arg(long)]
        dry_run: bool,

        /// Only run these checks (flutter, dart, cocoapods, android, xcode)
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,

        /// Skip these checks
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,

        /// Flutter channel to install (stable, beta, master)
        #[arg(long)]
        channel: Option<String>,

        /// Flutter version to install (x.y.z, `latest`, or `project`)
        #[arg(long)]
        flutter_version: Option<String>,

        /// Directory to install the SDK into
        #[arg(long)]
        dir: Option<PathBuf>,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: falcon::preflight::OutputFormat,
    },
```

- [ ] **Step 9: Wire the dispatcher**

In `src/command_dispatch.rs`, add `Environment` to `enum CommandGroup`, add `mod environment;` beside the other module declarations, add the arm `CommandGroup::Environment => environment::handle_command(cli.command),` in `run()`, and add to `command_group()` before the final `X` arm:

```rust
        Commands::Doctor { .. } => CommandGroup::Environment,
```

Create `src/command_dispatch/environment.rs`:

```rust
use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Doctor {
            path,
            fix,
            yes,
            dry_run,
            only,
            skip,
            channel,
            flutter_version,
            dir,
            format,
        } => {
            let opts = falcon::doctor::DoctorOptions {
                root: path,
                fix,
                yes,
                dry_run,
                only,
                skip,
                channel,
                flutter_version,
                dir,
                json: matches!(format, falcon::preflight::OutputFormat::Json),
            };
            let code = falcon::doctor::run(&opts)?;
            std::process::exit(code);
        }
        _ => unreachable!("environment group received a non-environment command"),
    }
}
```

- [ ] **Step 10: Verify it builds and behaves**

Run: `source ~/.cargo/env && cargo build`
Expected: builds clean

Run: `source ~/.cargo/env && cargo run -- doctor --help`
Expected: the flags above are listed

Run: `source ~/.cargo/env && cargo run -- doctor --dry-run --format json .`
Expected: JSON with a `host` object and a `checks` array containing `flutter`

- [ ] **Step 11: Run the whole suite**

Run: `source ~/.cargo/env && cargo test`
Expected: PASS

- [ ] **Step 12: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/ src/cli_args.rs src/command_dispatch.rs src/command_dispatch/environment.rs
git commit -m "feat(doctor): falcon doctor CLI installs Flutter end to end"
```

---

### Task 10: Dart check, deduplicated against the Flutter fixer

**Files:**
- Create: `src/doctor/checks/dart.rs`
- Modify: `src/doctor/checks/mod.rs` (add `pub mod dart;`), `src/doctor/mod.rs` (`registry()` gains `DartCheck`)
- Test: inline in `src/doctor/checks/dart.rs`

**Interfaces:**
- Consumes: `Check`, `CheckContext` (Task 8).
- Produces: `pub struct DartCheck;` implementing `Check`, and `pub fn parse_dart_version(stdout: &str) -> Option<String>`.

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/checks/dart.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::checks::CheckContext;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::{FixKind, Status};
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn ctx(root: &Path, runner: FakeRunner) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: HostInfo {
                os: "macos".into(),
                arch: "arm64".into(),
                home: Some(PathBuf::from("/Users/ada")),
                shell_rc: None,
                package_managers: vec![],
            },
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(runner),
        }
    }

    #[test]
    fn parses_dart_version_output() {
        let out = "Dart SDK version: 3.5.4 (stable) on \"macos_arm64\"";
        assert_eq!(parse_dart_version(out).as_deref(), Some("3.5.4"));
    }

    #[test]
    fn unparseable_output_is_none() {
        assert_eq!(parse_dart_version("nope"), None);
    }

    #[test]
    fn present_dart_reports_ok() {
        let dir = TempDir::new().unwrap();
        let runner = FakeRunner::with_stdout("dart --version", "Dart SDK version: 3.5.4 (stable)");
        let r = DartCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(r.status, Status::Ok { version: "3.5.4".into() });
        assert!(r.fix.is_none());
    }

    #[test]
    fn a_flutter_project_defers_to_the_flutter_fixer() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
        )
        .unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("dart --version");
        let r = DartCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(r.status, Status::Missing);
        let fix = r.fix.expect("must explain how it gets fixed");
        assert_eq!(fix.kind, FixKind::Automatic);
        assert!(
            fix.steps[0].describe.contains("installing Flutter"),
            "must defer to the Flutter fixer, not download a second SDK: {:?}",
            fix.steps[0].describe
        );
        assert!(fix.questions.is_empty(), "deferred fix asks nothing of its own");
    }

    #[test]
    fn a_pure_dart_project_gets_a_manual_fix() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("pubspec.yaml"), "name: cli_tool\n").unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("dart --version");
        let r = DartCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(r.fix.unwrap().kind, FixKind::Manual);
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::checks::dart`
Expected: FAIL — `cannot find type DartCheck`

- [ ] **Step 3: Implement**

Prepend to `src/doctor/checks/dart.rs`:

```rust
//! Dart SDK presence. In a Flutter project this is satisfied by installing
//! Flutter, which bundles Dart at `bin/cache/dart-sdk` — so this check defers
//! rather than downloading a second SDK.

use super::{Check, CheckContext};
use crate::doctor::types::{CheckResult, FixKind, FixOffer, Status, StepSummary};
use std::path::Path;

pub struct DartCheck;

/// Pull `3.5.4` out of `Dart SDK version: 3.5.4 (stable) …`.
pub fn parse_dart_version(stdout: &str) -> Option<String> {
    let token = stdout.split_whitespace().nth(3)?;
    token
        .chars()
        .next()
        .filter(|c| c.is_ascii_digit())
        .map(|_| token.to_string())
}

fn is_flutter_project(root: &Path) -> bool {
    std::fs::read_to_string(root.join("pubspec.yaml"))
        .map(|t| t.contains("sdk: flutter") || t.contains("flutter:"))
        .unwrap_or(false)
}

impl Check for DartCheck {
    fn id(&self) -> &'static str {
        "dart"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        let found = ctx
            .runner
            .run("dart", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .and_then(|o| {
                // `dart --version` writes to stdout on modern SDKs, stderr on old ones.
                parse_dart_version(&o.stdout).or_else(|| parse_dart_version(&o.stderr))
            });

        let status = match &found {
            Some(v) => Status::Ok { version: v.clone() },
            None => Status::Missing,
        };

        let fix = match status {
            Status::Ok { .. } => None,
            _ if is_flutter_project(&ctx.root) => Some(FixOffer {
                kind: FixKind::Automatic,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "deferred".into(),
                    describe: "Will be satisfied by installing Flutter, which bundles Dart".into(),
                }],
            }),
            _ => Some(FixOffer {
                kind: FixKind::Manual,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "manual".into(),
                    describe: "Install the standalone Dart SDK — https://dart.dev/get-dart".into(),
                }],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec!["`dart analyze`, used as Falcon's semantic co-pilot".into()],
            fix,
        }
    }
}
```

Add `pub mod dart;` to `src/doctor/checks/mod.rs` and extend `registry()` in `src/doctor/mod.rs`:

```rust
fn registry() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(checks::flutter::FlutterCheck),
        Box::new(checks::dart::DartCheck),
    ]
}
```

Because the Dart fix is deferred, `apply()` already ignores non-`flutter` check ids and returns no outcomes — no extra guard is needed.

- [ ] **Step 4: Run them to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor::checks::dart`
Expected: PASS, 5 tests

- [ ] **Step 5: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/checks/dart.rs src/doctor/checks/mod.rs src/doctor/mod.rs
git commit -m "feat(doctor): dart check deferring to the Flutter fixer in Flutter projects"
```

---

### Task 11: CocoaPods check and fixer — plus the `plan()` trait method

The second real fixer arrives, so the `check.id == "flutter"` match inside `apply()` must become a trait method. Do that refactor here, driven by the need.

**Files:**
- Create: `src/doctor/checks/cocoapods.rs`
- Modify: `src/doctor/checks/mod.rs` (add `pub mod cocoapods;` and the `plan()` trait method), `src/doctor/checks/flutter.rs` (implement `plan()`), `src/doctor/mod.rs` (`apply()` delegates; `registry()` gains `CocoaPodsCheck`)
- Test: inline in `src/doctor/checks/cocoapods.rs`

**Interfaces:**
- Consumes: `Check`, `CheckContext` (Task 8); `build_plan` (Task 7).
- Produces: `Check` gains `fn plan(&self, ctx: &CheckContext, decisions: &HashMap<String, String>) -> Result<Plan, String>` with a default returning an empty plan; `pub struct CocoaPodsCheck;`.

- [ ] **Step 1: Add the trait method**

In `src/doctor/checks/mod.rs`, extend the trait:

```rust
use crate::doctor::types::Plan;
use std::collections::HashMap;

pub trait Check {
    fn id(&self) -> &'static str;
    fn probe(&self, ctx: &CheckContext) -> CheckResult;

    /// Turn answered questions into an executable plan. Checks whose fix is
    /// deferred to another check (like `dart`) keep the default empty plan.
    fn plan(
        &self,
        _ctx: &CheckContext,
        _decisions: &HashMap<String, String>,
    ) -> Result<Plan, String> {
        Ok(Plan { check_id: self.id().to_string(), steps: vec![] })
    }
}
```

- [ ] **Step 2: Move the Flutter plan behind it**

Add to `src/doctor/checks/flutter.rs`, inside `impl Check for FlutterCheck`:

```rust
    fn plan(
        &self,
        ctx: &CheckContext,
        decisions: &std::collections::HashMap<String, String>,
    ) -> Result<crate::doctor::types::Plan, String> {
        let manifest = ctx
            .manifest
            .as_ref()
            .ok_or_else(|| {
                "Flutter release manifest unavailable; install manually from \
                 https://docs.flutter.dev/get-started/install"
                    .to_string()
            })?;
        let dir = decisions
            .get("flutter.dir")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                crate::doctor::flutter::install::default_install_dir(ctx.host.home.as_deref())
            });
        let decisions = crate::doctor::flutter::install::InstallDecisions {
            channel: decisions.get("flutter.channel").cloned().unwrap_or_else(|| "stable".into()),
            version: decisions.get("flutter.version").cloned().unwrap_or_default(),
            dir,
        };
        let scratch = std::env::temp_dir().join("falcon-doctor");
        crate::doctor::flutter::install::build_plan(
            manifest,
            &ctx.host,
            ctx.arch,
            &decisions,
            &scratch,
        )
    }
```

Then in `src/doctor/mod.rs`, replace the body of `apply()` so it is check-agnostic:

```rust
fn apply(
    check: &dyn Check,
    ctx: &CheckContext,
    decisions: &HashMap<String, String>,
    opts: &DoctorOptions,
) -> Result<Vec<Outcome>> {
    let plan = match check.plan(ctx, decisions) {
        Ok(p) => p,
        Err(e) => return Ok(vec![Outcome::Failed { error: e }]),
    };
    if plan.steps.is_empty() {
        return Ok(vec![]);
    }
    if !opts.dry_run {
        if let Some(Step::Download { .. }) = plan.steps.first() {
            if let Some(dir) = decisions.get("flutter.dir") {
                if let Err(e) = dir_is_usable(std::path::Path::new(dir)) {
                    return Ok(vec![Outcome::Failed { error: e }]);
                }
            }
        }
    }
    let policy = if opts.interactive() { HandoffPolicy::Prompt } else { HandoffPolicy::Report };
    let outcomes = execute_plan(&plan, &RealRunner, &CurlDownloader, policy, opts.dry_run);
    announce_path_hints(&plan);
    Ok(outcomes)
}

fn announce_path_hints(plan: &Plan) {
    for step in &plan.steps {
        if let Step::PathHint { dir, rc_file } = step {
            println!("\nAdd this to your shell, then restart it:");
            println!("  {}", flutter::install::path_export_line(dir));
            if let Some(rc) = rc_file {
                println!("  (your shell reads {})", rc.display());
            }
            println!("`flutter` will not be on PATH in this shell until you do.");
        }
    }
}
```

**The call site must change shape, not just arguments.** Before this task `run()` iterated `diagnosis.checks` (which are `CheckResult` values). `apply()` now needs the `Check` itself, so the fix loop must iterate `registry()` and pair each check with its probed result:

```rust
    for check in registry() {
        if !opts.wants(check.id()) {
            continue;
        }
        let Some(probed) = diagnosis.checks.iter().find(|c| c.id == check.id()) else {
            continue;
        };
        let Some(offer) = &probed.fix else { continue };
        if offer.kind == FixKind::Manual {
            continue;
        }
        if !opts.fix && !confirm(&format!("Fix {} now?", probed.id), opts)? {
            continue;
        }
        let Some(decisions) = collect_decisions(offer, opts)? else { continue };
        let ctx = CheckContext {
            root: opts.root.clone(),
            host: host_info.clone(),
            arch,
            manifest: manifest.clone(),
            runner: Box::new(RealRunner),
        };
        outcomes.extend(apply(check.as_ref(), &ctx, &decisions, opts)?);
    }
```

- [ ] **Step 3: Write the failing CocoaPods tests**

Create `src/doctor/checks/cocoapods.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::checks::CheckContext;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::{FixKind, Status, Step};
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn ctx(root: &Path, runner: FakeRunner, os: &str, pms: &[&str]) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: HostInfo {
                os: os.into(),
                arch: "arm64".into(),
                home: Some(PathBuf::from("/Users/ada")),
                shell_rc: None,
                package_managers: pms.iter().map(|s| s.to_string()).collect(),
            },
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(runner),
        }
    }

    fn with_podfile() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("ios")).unwrap();
        fs::write(dir.path().join("ios/Podfile"), "platform :ios, '13.0'\n").unwrap();
        dir
    }

    #[test]
    fn skipped_when_the_project_has_no_podfile() {
        let dir = TempDir::new().unwrap();
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), FakeRunner::default(), "macos", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
        assert!(r.fix.is_none());
    }

    #[test]
    fn skipped_on_non_macos_even_with_a_podfile() {
        let dir = with_podfile();
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), FakeRunner::default(), "linux", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn present_cocoapods_reports_ok() {
        let dir = with_podfile();
        let runner = FakeRunner::with_stdout("pod --version", "1.15.2\n");
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), runner, "macos", &["brew"]));
        assert_eq!(r.status, Status::Ok { version: "1.15.2".into() });
    }

    #[test]
    fn missing_cocoapods_is_an_automatic_fix_and_names_the_podfile() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), runner, "macos", &["brew"]));
        assert_eq!(r.status, Status::Missing);
        assert_eq!(r.fix.unwrap().kind, FixKind::Automatic);
        assert!(r.required_by.iter().any(|s| s.contains("Podfile")));
    }

    #[test]
    fn brew_is_preferred_when_available() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let c = ctx(dir.path(), runner, "macos", &["brew", "gem"]);
        let plan = CocoaPodsCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Run { program, args, .. } => {
                assert_eq!(program, "brew");
                assert_eq!(args, &vec!["install".to_string(), "cocoapods".to_string()]);
            }
            other => panic!("expected brew install, got {:?}", other),
        }
    }

    #[test]
    fn gem_install_is_user_scoped_never_sudo() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let c = ctx(dir.path(), runner, "macos", &["gem"]);
        let plan = CocoaPodsCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Run { program, args, .. } => {
                assert_eq!(program, "gem");
                assert!(
                    args.contains(&"--user-install".to_string()),
                    "must never install into system Ruby: {:?}",
                    args
                );
                assert_ne!(program, "sudo");
            }
            other => panic!("expected gem install, got {:?}", other),
        }
    }

    #[test]
    fn no_package_manager_yields_a_manual_fix() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), runner, "macos", &[]));
        assert_eq!(r.fix.unwrap().kind, FixKind::Manual);
    }
}
```

- [ ] **Step 4: Run them to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::checks::cocoapods`
Expected: FAIL — `cannot find type CocoaPodsCheck`

- [ ] **Step 5: Implement**

Prepend to `src/doctor/checks/cocoapods.rs`:

```rust
//! CocoaPods, needed only when the project has an iOS Podfile.

use super::{Check, CheckContext};
use crate::doctor::types::{CheckResult, FixKind, FixOffer, Plan, Probe, Status, Step, StepSummary};
use std::collections::HashMap;

pub struct CocoaPodsCheck;

fn installer(ctx: &CheckContext) -> Option<(&'static str, Vec<String>, &'static str)> {
    let has = |p: &str| ctx.host.package_managers.iter().any(|m| m == p);
    if has("brew") {
        Some((
            "brew",
            vec!["install".into(), "cocoapods".into()],
            "Homebrew is available — the least surprising install on macOS",
        ))
    } else if has("gem") {
        Some((
            "gem",
            vec!["install".into(), "cocoapods".into(), "--user-install".into()],
            "installs into your user gem directory, leaving system Ruby untouched",
        ))
    } else {
        None
    }
}

impl Check for CocoaPodsCheck {
    fn id(&self) -> &'static str {
        "cocoapods"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        let podfile = ctx.root.join("ios").join("Podfile");
        if ctx.host.os != "macos" {
            return skipped(self.id(), "CocoaPods is only needed on macOS");
        }
        if !podfile.is_file() {
            return skipped(self.id(), "no ios/Podfile in this project");
        }

        let found = ctx
            .runner
            .run("pod", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .map(|o| o.stdout.trim().to_string())
            .filter(|v| !v.is_empty());

        let status = match &found {
            Some(v) => Status::Ok { version: v.clone() },
            None => Status::Missing,
        };

        let fix = match (&status, installer(ctx)) {
            (Status::Ok { .. }, _) => None,
            (_, Some((program, args, why))) => Some(FixOffer {
                kind: FixKind::Automatic,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "install".into(),
                    describe: format!("{} {} — {}", program, args.join(" "), why),
                }],
            }),
            (_, None) => Some(FixOffer {
                kind: FixKind::Manual,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "manual".into(),
                    describe: "Install CocoaPods — https://guides.cocoapods.org/using/getting-started.html"
                        .into(),
                }],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec!["ios/Podfile is present, so pod install must be able to run".into()],
            fix,
        }
    }

    fn plan(&self, ctx: &CheckContext, _d: &HashMap<String, String>) -> Result<Plan, String> {
        let (program, args, _) = installer(ctx)
            .ok_or_else(|| "no supported package manager for CocoaPods on this host".to_string())?;
        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![
                Step::Run { id: "install".into(), program: program.into(), args, cwd: None },
                Step::Verify {
                    id: "verify".into(),
                    probe: Probe { program: "pod".into(), args: vec!["--version".into()] },
                },
            ],
        })
    }
}

fn skipped(id: &str, because: &str) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status: Status::Skipped { because: because.to_string() },
        required_by: vec![],
        fix: None,
    }
}
```

Add `pub mod cocoapods;` to `src/doctor/checks/mod.rs` and `Box::new(checks::cocoapods::CocoaPodsCheck)` to `registry()`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor`
Expected: PASS — the cocoapods tests plus every earlier doctor test, since the `apply()` refactor must not regress Task 9

- [ ] **Step 7: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/
git commit -m "feat(doctor): CocoaPods fixer and a plan() method on the Check trait"
```

---

### Task 12: Android SDK check with licence handoff

**Files:**
- Create: `src/doctor/checks/android.rs`
- Modify: `src/doctor/checks/mod.rs`, `src/doctor/mod.rs` (`registry()`)
- Test: inline in `src/doctor/checks/android.rs`

**Interfaces:**
- Consumes: `Check`, `CheckContext`, the `plan()` trait method (Task 11).
- Produces: `pub struct AndroidCheck;`, `pub fn sdk_root(host: &HostInfo, get_env: impl Fn(&str) -> Option<String>) -> Option<PathBuf>`, `pub fn compile_sdk_version(root: &Path) -> Option<u32>`.

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/checks/android.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::checks::CheckContext;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::{FixKind, Status, Step};
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn host(os: &str) -> HostInfo {
        HostInfo {
            os: os.into(),
            arch: "arm64".into(),
            home: Some(PathBuf::from("/Users/ada")),
            shell_rc: None,
            package_managers: vec![],
        }
    }

    fn ctx(root: &Path, runner: FakeRunner, os: &str) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: host(os),
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(runner),
        }
    }

    fn android_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("android/app")).unwrap();
        fs::write(
            dir.path().join("android/app/build.gradle"),
            "android {\n    compileSdkVersion 34\n}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn skipped_without_an_android_directory() {
        let dir = TempDir::new().unwrap();
        let r = AndroidCheck.probe(&ctx(dir.path(), FakeRunner::default(), "macos"));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn sdk_root_prefers_android_home_env() {
        let got = sdk_root(&host("macos"), |k| {
            (k == "ANDROID_HOME").then(|| "/opt/android".to_string())
        });
        assert_eq!(got, Some(PathBuf::from("/opt/android")));
    }

    #[test]
    fn sdk_root_falls_back_to_the_platform_default() {
        assert_eq!(
            sdk_root(&host("macos"), |_| None),
            Some(PathBuf::from("/Users/ada/Library/Android/sdk"))
        );
        assert_eq!(
            sdk_root(&host("linux"), |_| None),
            Some(PathBuf::from("/Users/ada/Android/Sdk"))
        );
    }

    #[test]
    fn compile_sdk_version_reads_groovy_gradle() {
        let dir = android_project();
        assert_eq!(compile_sdk_version(dir.path()), Some(34));
    }

    #[test]
    fn compile_sdk_version_reads_kotlin_dsl() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("android/app")).unwrap();
        fs::write(
            dir.path().join("android/app/build.gradle.kts"),
            "android {\n    compileSdk = 35\n}\n",
        )
        .unwrap();
        assert_eq!(compile_sdk_version(dir.path()), Some(35));
    }

    #[test]
    fn indirect_compile_sdk_returns_none_rather_than_guessing_wrong() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("android/app")).unwrap();
        fs::write(
            dir.path().join("android/app/build.gradle"),
            "android {\n    compileSdkVersion flutter.compileSdkVersion\n}\n",
        )
        .unwrap();
        assert_eq!(compile_sdk_version(dir.path()), None);
    }

    #[test]
    fn missing_sdk_is_an_assisted_fix() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let r = AndroidCheck.probe(&ctx(dir.path(), runner, "macos"));
        assert_eq!(r.status, Status::Missing);
        assert_eq!(r.fix.unwrap().kind, FixKind::Assisted);
    }

    #[test]
    fn the_plan_installs_packages_then_hands_off_the_licences() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        let ids: Vec<_> = plan.steps.iter().map(|s| s.id().to_string()).collect();
        assert_eq!(
            ids,
            vec!["download-cmdline-tools", "extract-cmdline-tools", "sdk-packages", "licenses"]
        );
        assert!(
            matches!(plan.steps.last(), Some(Step::Handoff { .. })),
            "licences must be a handoff, never automated"
        );
    }

    #[test]
    fn the_licence_handoff_names_the_exact_command() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        match plan.steps.last().unwrap() {
            Step::Handoff { command, reason, .. } => {
                assert_eq!(command, "flutter doctor --android-licenses");
                assert!(reason.to_lowercase().contains("licence") || reason.to_lowercase().contains("license"));
            }
            other => panic!("expected a handoff, got {:?}", other),
        }
    }

    #[test]
    fn the_plan_targets_the_projects_compile_sdk_version() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[2] {
            Step::Run { args, .. } => {
                assert!(
                    args.iter().any(|a| a == "platforms;android-34"),
                    "must use the project's compileSdkVersion: {:?}",
                    args
                );
            }
            other => panic!("expected sdkmanager run, got {:?}", other),
        }
    }

    #[test]
    fn the_plan_never_uses_sudo() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        for step in &plan.steps {
            if let Step::Run { program, .. } = step {
                assert_ne!(program, "sudo");
            }
        }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::checks::android`
Expected: FAIL — `cannot find type AndroidCheck`

- [ ] **Step 3: Implement**

Prepend to `src/doctor/checks/android.rs`:

```rust
//! Android SDK command-line tools and platform packages.
//!
//! Falcon installs the tooling and the packages, then hands the licence
//! agreements back to the user — accepting a legal agreement on someone
//! else's behalf is not Falcon's call.

use super::{Check, CheckContext};
use crate::doctor::host::HostInfo;
use crate::doctor::types::{CheckResult, FixKind, FixOffer, Plan, Probe, Status, Step, StepSummary};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Latest stable platform used when the project's value cannot be read.
const FALLBACK_COMPILE_SDK: u32 = 34;

const CMDLINE_TOOLS_URL: &str =
    "https://dl.google.com/android/repository/commandlinetools-mac-11076708_latest.zip";
const LICENSES_DOCS: &str = "https://docs.flutter.dev/get-started/install";

pub struct AndroidCheck;

pub fn sdk_root(host: &HostInfo, get_env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(v) = get_env("ANDROID_HOME").or_else(|| get_env("ANDROID_SDK_ROOT")) {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let home = host.home.as_ref()?;
    Some(match host.os.as_str() {
        "macos" => home.join("Library").join("Android").join("sdk"),
        "windows" => home.join("AppData").join("Local").join("Android").join("Sdk"),
        _ => home.join("Android").join("Sdk"),
    })
}

/// Read `compileSdkVersion 34` / `compileSdk = 35`. Returns `None` when the
/// value is an indirection we cannot resolve — the caller falls back loudly.
pub fn compile_sdk_version(root: &Path) -> Option<u32> {
    let app = root.join("android").join("app");
    for name in ["build.gradle", "build.gradle.kts"] {
        let Ok(text) = std::fs::read_to_string(app.join(name)) else { continue };
        for line in text.lines() {
            let t = line.trim();
            let Some(rest) = t
                .strip_prefix("compileSdkVersion")
                .or_else(|| t.strip_prefix("compileSdk"))
            else {
                continue;
            };
            let token = rest.trim_start_matches('=').trim();
            // A literal wins; an indirection (`flutter.compileSdkVersion`)
            // yields None so the caller falls back loudly.
            return token.parse::<u32>().ok();
        }
    }
    None
}

impl Check for AndroidCheck {
    fn id(&self) -> &'static str {
        "android"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        if !ctx.root.join("android").is_dir() {
            return CheckResult {
                id: self.id().to_string(),
                status: Status::Skipped { because: "no android/ directory in this project".into() },
                required_by: vec![],
                fix: None,
            };
        }

        let found = ctx
            .runner
            .run("sdkmanager", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .map(|o| o.stdout.trim().to_string())
            .filter(|v| !v.is_empty());

        let status = match &found {
            Some(v) => Status::Ok { version: v.clone() },
            None => Status::Missing,
        };

        let fix = match status {
            Status::Ok { .. } => None,
            _ => Some(FixOffer {
                kind: FixKind::Assisted,
                questions: vec![],
                steps: vec![
                    StepSummary {
                        id: "download-cmdline-tools".into(),
                        describe: "Download the Android command-line tools".into(),
                    },
                    StepSummary {
                        id: "sdk-packages".into(),
                        describe: "Install platform-tools, the platform and build-tools".into(),
                    },
                    StepSummary {
                        id: "licenses".into(),
                        describe: "You accept the SDK licences — Falcon will not do this for you"
                            .into(),
                    },
                ],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec!["android/ directory is present, so Android builds must work".into()],
            fix,
        }
    }

    fn plan(&self, ctx: &CheckContext, _d: &HashMap<String, String>) -> Result<Plan, String> {
        let root = sdk_root(&ctx.host, |k| std::env::var(k).ok())
            .ok_or_else(|| "cannot determine the Android SDK location (no HOME)".to_string())?;
        let api = compile_sdk_version(&ctx.root).unwrap_or(FALLBACK_COMPILE_SDK);
        let scratch = std::env::temp_dir().join("falcon-doctor");
        let archive = scratch.join("android-cmdline-tools.zip");
        let sdkmanager = root
            .join("cmdline-tools")
            .join("bin")
            .join("sdkmanager")
            .to_string_lossy()
            .to_string();

        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![
                Step::Download {
                    id: "download-cmdline-tools".into(),
                    url: CMDLINE_TOOLS_URL.into(),
                    // Google does not publish a checksum alongside this zip;
                    // the empty string tells the executor the archive is
                    // unverifiable and it will refuse unless --yes is set.
                    sha256: String::new(),
                    dest: archive.clone(),
                },
                Step::Extract {
                    id: "extract-cmdline-tools".into(),
                    archive,
                    dest: root.clone(),
                },
                Step::Run {
                    id: "sdk-packages".into(),
                    program: sdkmanager,
                    args: vec![
                        "platform-tools".into(),
                        format!("platforms;android-{}", api),
                        format!("build-tools;{}.0.0", api),
                    ],
                    cwd: None,
                },
                Step::Handoff {
                    id: "licenses".into(),
                    reason: "Android SDK licence agreements must be accepted by you".into(),
                    command: "flutter doctor --android-licenses".into(),
                    docs_url: LICENSES_DOCS.into(),
                    verify: Probe {
                        program: "flutter".into(),
                        args: vec!["doctor".into(), "--android-licenses".into()],
                    },
                },
            ],
        })
    }
}
```

- [ ] **Step 4: Handle the unverifiable download**

The empty `sha256` above would currently fail every Android install, because the executor compares against it. Add an explicit escape in `run_step` in `src/doctor/exec.rs`, in the `Step::Download` arm, before the checksum comparison:

```rust
            if sha256.is_empty() {
                // Upstream publishes no checksum for this archive. Say so
                // rather than silently trusting it.
                log::warn!("no published checksum for {} — cannot verify integrity", url);
                return Outcome::Done { step_id: id.clone() };
            }
```

Add a test for it in `src/doctor/exec.rs`'s test module:

```rust
    #[test]
    fn an_empty_checksum_is_accepted_but_warned_about() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("whatever");
        let plan = Plan {
            check_id: "android".into(),
            steps: vec![Step::Download {
                id: "download".into(),
                url: "https://dl.google.test/tools.zip".into(),
                sha256: String::new(),
                dest: PathBuf::from("/tmp/scratch/tools.zip"),
            }],
        };
        let out = execute_plan(&plan, &runner, &dl, HandoffPolicy::Report, false);
        assert_eq!(out, vec![Outcome::Done { step_id: "download".into() }]);
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor`
Expected: PASS — 11 android tests plus the new exec test, and every earlier doctor test

Add `pub mod android;` to `src/doctor/checks/mod.rs` and `Box::new(checks::android::AndroidCheck)` to `registry()`.

- [ ] **Step 6: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/
git commit -m "feat(doctor): Android SDK fixer with licence acceptance handed back to the user"
```

---

### Task 13: Xcode check with Apple ID and sudo handoffs

**Files:**
- Create: `src/doctor/checks/xcode.rs`
- Modify: `src/doctor/checks/mod.rs`, `src/doctor/mod.rs` (`registry()`)
- Test: inline in `src/doctor/checks/xcode.rs`

**Interfaces:**
- Consumes: `Check`, `CheckContext`, `plan()` (Task 11).
- Produces: `pub struct XcodeCheck;`, `pub fn parse_xcode_version(stdout: &str) -> Option<String>`.

- [ ] **Step 1: Write the failing tests**

Create `src/doctor/checks/xcode.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::checks::CheckContext;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::{FixKind, Status, Step};
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn ctx(root: &Path, runner: FakeRunner, os: &str, pms: &[&str]) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: HostInfo {
                os: os.into(),
                arch: "arm64".into(),
                home: Some(PathBuf::from("/Users/ada")),
                shell_rc: None,
                package_managers: pms.iter().map(|s| s.to_string()).collect(),
            },
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(runner),
        }
    }

    fn ios_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("ios")).unwrap();
        dir
    }

    #[test]
    fn parses_the_xcode_version() {
        assert_eq!(parse_xcode_version("Xcode 16.1\nBuild version 16B40").as_deref(), Some("16.1"));
    }

    #[test]
    fn unparseable_output_is_none() {
        assert_eq!(parse_xcode_version("xcode-select: error: tool not found"), None);
    }

    #[test]
    fn skipped_on_non_macos() {
        let dir = ios_project();
        let r = XcodeCheck.probe(&ctx(dir.path(), FakeRunner::default(), "linux", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn skipped_when_the_project_has_no_apple_platform() {
        let dir = TempDir::new().unwrap();
        let r = XcodeCheck.probe(&ctx(dir.path(), FakeRunner::default(), "macos", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn a_macos_only_project_still_needs_xcode() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("macos")).unwrap();
        let runner = FakeRunner::with_stdout("xcodebuild -version", "Xcode 16.1\n");
        let r = XcodeCheck.probe(&ctx(dir.path(), runner, "macos", &[]));
        assert_eq!(r.status, Status::Ok { version: "16.1".into() });
    }

    #[test]
    fn missing_xcode_is_an_assisted_fix() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let r = XcodeCheck.probe(&ctx(dir.path(), runner, "macos", &[]));
        assert_eq!(r.status, Status::Missing);
        assert_eq!(r.fix.unwrap().kind, FixKind::Assisted);
    }

    #[test]
    fn the_install_handoff_prefers_xcodes_when_it_is_available() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &["xcodes"]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Handoff { command, .. } => assert!(
                command.contains("xcodes install"),
                "should suggest xcodes: {}",
                command
            ),
            other => panic!("expected a handoff, got {:?}", other),
        }
    }

    #[test]
    fn without_xcodes_the_handoff_points_at_the_app_store() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &[]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Handoff { reason, docs_url, .. } => {
                assert!(reason.contains("Apple ID"), "must explain why: {}", reason);
                assert!(docs_url.contains("apps.apple.com") || docs_url.contains("developer.apple.com"));
            }
            other => panic!("expected a handoff, got {:?}", other),
        }
    }

    #[test]
    fn every_sudo_step_is_a_handoff_never_a_run() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &["xcodes"]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        for step in &plan.steps {
            if let Step::Run { program, args, .. } = step {
                assert_ne!(program, "sudo", "sudo must never be a Run step");
                assert!(!args.iter().any(|a| a == "sudo"));
            }
        }
        assert!(
            plan.steps.iter().any(|s| matches!(s, Step::Handoff { command, .. } if command.contains("sudo"))),
            "the sudo steps must appear as handoffs"
        );
    }

    #[test]
    fn the_licence_is_never_accepted_automatically() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &["xcodes"]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        for step in &plan.steps {
            if let Step::Run { args, .. } = step {
                assert!(
                    !args.iter().any(|a| a == "-license"),
                    "licence acceptance must be a handoff"
                );
            }
        }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `source ~/.cargo/env && cargo test --lib doctor::checks::xcode`
Expected: FAIL — `cannot find type XcodeCheck`

- [ ] **Step 3: Implement**

Prepend to `src/doctor/checks/xcode.rs`:

```rust
//! Xcode and its command-line tools.
//!
//! Installing Xcode needs an Apple ID, and first-launch plus licence
//! acceptance need root. All three are handoffs; Falcon automates only the
//! steps that need neither.

use super::{Check, CheckContext};
use crate::doctor::types::{CheckResult, FixKind, FixOffer, Plan, Probe, Status, Step, StepSummary};
use std::collections::HashMap;

const APP_STORE_URL: &str = "https://apps.apple.com/us/app/xcode/id497799835";

pub struct XcodeCheck;

/// Pull `16.1` out of `Xcode 16.1\nBuild version 16B40`.
pub fn parse_xcode_version(stdout: &str) -> Option<String> {
    let first = stdout.lines().next()?;
    let token = first.strip_prefix("Xcode ")?.trim();
    token
        .chars()
        .next()
        .filter(|c| c.is_ascii_digit())
        .map(|_| token.to_string())
}

impl Check for XcodeCheck {
    fn id(&self) -> &'static str {
        "xcode"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        if ctx.host.os != "macos" {
            return skipped(self.id(), "Xcode only exists on macOS");
        }
        let needs = ctx.root.join("ios").is_dir() || ctx.root.join("macos").is_dir();
        if !needs {
            return skipped(self.id(), "no ios/ or macos/ directory in this project");
        }

        let found = ctx
            .runner
            .run("xcodebuild", &["-version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .and_then(|o| parse_xcode_version(&o.stdout));

        let status = match &found {
            Some(v) => Status::Ok { version: v.clone() },
            None => Status::Missing,
        };

        let fix = match status {
            Status::Ok { .. } => None,
            _ => Some(FixOffer {
                kind: FixKind::Assisted,
                questions: vec![],
                steps: vec![
                    StepSummary {
                        id: "install-xcode".into(),
                        describe: "You install Xcode — it needs your Apple ID".into(),
                    },
                    StepSummary {
                        id: "select-and-license".into(),
                        describe: "You run the two sudo steps Falcon will print".into(),
                    },
                    StepSummary {
                        id: "first-launch".into(),
                        describe: "Falcon runs xcodebuild -runFirstLaunch".into(),
                    },
                    StepSummary {
                        id: "ios-platform".into(),
                        describe: "Falcon downloads the iOS platform and simulator runtime".into(),
                    },
                ],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec!["ios/ or macos/ directory is present, so Apple builds must work".into()],
            fix,
        }
    }

    fn plan(&self, ctx: &CheckContext, _d: &HashMap<String, String>) -> Result<Plan, String> {
        let has_xcodes = ctx.host.package_managers.iter().any(|m| m == "xcodes");
        let install_command = if has_xcodes {
            "xcodes install --latest".to_string()
        } else {
            format!("open {}", APP_STORE_URL)
        };

        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![
                Step::Handoff {
                    id: "install-xcode".into(),
                    reason: "Installing Xcode requires signing in with your Apple ID, which \
                             Falcon will not do on your behalf"
                        .into(),
                    command: install_command,
                    docs_url: APP_STORE_URL.into(),
                    verify: Probe {
                        program: "xcodebuild".into(),
                        args: vec!["-version".into()],
                    },
                },
                Step::Handoff {
                    id: "select-and-license".into(),
                    reason: "Selecting the toolchain and accepting the licence both need root, \
                             and the licence is a legal agreement only you can accept"
                        .into(),
                    command: "sudo xcode-select -s /Applications/Xcode.app/Contents/Developer && \
                              sudo xcodebuild -license accept"
                        .into(),
                    docs_url: "https://developer.apple.com/support/xcode/".into(),
                    verify: Probe {
                        program: "xcode-select".into(),
                        args: vec!["-p".into()],
                    },
                },
                Step::Run {
                    id: "first-launch".into(),
                    program: "xcodebuild".into(),
                    args: vec!["-runFirstLaunch".into()],
                    cwd: None,
                },
                Step::Run {
                    id: "ios-platform".into(),
                    program: "xcodebuild".into(),
                    args: vec!["-downloadPlatform".into(), "iOS".into()],
                    cwd: None,
                },
            ],
        })
    }
}

fn skipped(id: &str, because: &str) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status: Status::Skipped { because: because.to_string() },
        required_by: vec![],
        fix: None,
    }
}
```

Add `pub mod xcode;` to `src/doctor/checks/mod.rs` and `Box::new(checks::xcode::XcodeCheck)` to `registry()`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `source ~/.cargo/env && cargo test --lib doctor`
Expected: PASS, 10 xcode tests plus all earlier doctor tests

- [ ] **Step 5: Add a cross-cutting invariant test**

Create the guard that keeps the two hard invariants true as checks are added. Append to `src/doctor/checks/mod.rs`:

```rust
#[cfg(test)]
mod invariant_tests {
    use super::*;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::Step;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// A project with every platform directory, so no check short-circuits.
    fn maximal_ctx(dir: &std::path::Path) -> CheckContext {
        for sub in ["ios", "macos", "android/app"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join("ios/Podfile"), "platform :ios, '13.0'\n").unwrap();
        CheckContext {
            root: dir.to_path_buf(),
            host: HostInfo {
                os: "macos".into(),
                arch: "arm64".into(),
                home: Some(PathBuf::from("/Users/ada")),
                shell_rc: None,
                package_managers: vec!["brew".into(), "xcodes".into()],
            },
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(FakeRunner::default()),
        }
    }

    #[test]
    fn no_check_ever_plans_a_sudo_run_step() {
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = maximal_ctx(dir.path());
        let checks: Vec<Box<dyn Check>> = vec![
            Box::new(flutter::FlutterCheck),
            Box::new(dart::DartCheck),
            Box::new(cocoapods::CocoaPodsCheck),
            Box::new(android::AndroidCheck),
            Box::new(xcode::XcodeCheck),
        ];
        for check in checks {
            let Ok(plan) = check.plan(&ctx, &HashMap::new()) else { continue };
            for step in &plan.steps {
                if let Step::Run { program, args, .. } = step {
                    assert_ne!(program, "sudo", "{} planned a sudo Run step", check.id());
                    assert!(
                        !args.iter().any(|a| a == "sudo"),
                        "{} planned a sudo argument",
                        check.id()
                    );
                }
            }
        }
    }

    #[test]
    fn no_check_ever_auto_accepts_a_licence() {
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = maximal_ctx(dir.path());
        let checks: Vec<Box<dyn Check>> = vec![
            Box::new(android::AndroidCheck),
            Box::new(xcode::XcodeCheck),
        ];
        for check in checks {
            let Ok(plan) = check.plan(&ctx, &HashMap::new()) else { continue };
            for step in &plan.steps {
                if let Step::Run { args, .. } = step {
                    assert!(
                        !args.iter().any(|a| a.contains("license") || a.contains("licence")),
                        "{} planned automatic licence acceptance",
                        check.id()
                    );
                }
            }
        }
    }
}
```

Run: `source ~/.cargo/env && cargo test --lib doctor::checks`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/doctor/
git commit -m "feat(doctor): Xcode fixer plus invariant tests banning sudo and auto-licensing"
```

---

### Task 14: MCP `doctor` tool and the Trust gate

**Files:**
- Modify: `src/mcp/schema.rs` (add `DoctorArgs` + `doctor_input_schema`), `src/mcp/tools.rs` (surface 5 → 6, `Trust` parameter), `src/mcp/server.rs:140` (pass `Trust::Local`), `src/api/server.rs:83` (advertise diagnosis-only), `tests/v20_tests.rs:10`, `tests/mcp_tool_surface.rs:25`
- Test: `tests/mcp_tool_surface.rs` and inline in `src/mcp/tools.rs`

**Interfaces:**
- Consumes: `doctor::run`, `doctor::DoctorOptions`, `Trust` (Tasks 2 and 9).
- Produces: `pub fn execute_tool(name: &str, args: &Value, trust: Trust) -> Result<Value, String>` (signature change), `pub fn doctor_input_schema() -> Value`, `pub struct DoctorArgs`, `pub fn list_tools_for(trust: Trust) -> Vec<ToolDefinition>`.

**Note on the signature change:** `execute_tool` currently takes two arguments and is called from `src/mcp/server.rs:140` and from four test files (`tests/mcp_tool_surface.rs`, `tests/edge_analysis_platform_tests.rs`). Every call site must gain a `Trust` argument. Existing tests pass `Trust::Local`.

- [ ] **Step 1: Write the failing surface tests**

Append to `tests/mcp_tool_surface.rs`:

```rust
use falcon::doctor::Trust;
use serde_json::json;

#[test]
fn doctor_is_advertised_on_the_local_surface() {
    let tools = falcon::mcp::tools::list_tools();
    assert!(
        tools.iter().any(|t| t.name == "doctor"),
        "doctor must be advertised"
    );
}

#[test]
fn doctor_diagnosis_does_not_require_execute() {
    let result = falcon::mcp::tools::execute_tool("doctor", &json!({ "path": "." }), Trust::Local);
    let value = result.expect("diagnosis must succeed");
    assert!(value.get("checks").is_some(), "diagnosis must return checks");
}

#[test]
fn doctor_defaults_to_not_executing() {
    let schema = falcon::mcp::schema::doctor_input_schema();
    assert_eq!(schema["properties"]["execute"]["default"], json!(false));
}

#[test]
fn remote_trust_refuses_to_execute() {
    let args = json!({ "path": ".", "execute": true, "decisions": {} });
    let err = falcon::mcp::tools::execute_tool("doctor", &args, Trust::Remote)
        .expect_err("remote execution must be refused");
    assert!(
        err.to_lowercase().contains("not permitted") || err.to_lowercase().contains("refus"),
        "refusal must be explicit: {}",
        err
    );
}

#[test]
fn remote_trust_still_allows_diagnosis() {
    let result = falcon::mcp::tools::execute_tool("doctor", &json!({ "path": "." }), Trust::Remote);
    assert!(result.is_ok(), "diagnosis is read-only and stays available");
}

#[test]
fn remote_listing_marks_doctor_diagnosis_only() {
    let remote = falcon::mcp::tools::list_tools_for(Trust::Remote);
    let doctor = remote.iter().find(|t| t.name == "doctor").expect("still listed");
    assert!(
        doctor.description.contains("diagnosis only"),
        "remote description must say so: {}",
        doctor.description
    );
}
```

Change the existing assertion at `tests/mcp_tool_surface.rs:25` from `5` to `6`, and update its message. Do the same at `tests/v20_tests.rs:10`.

- [ ] **Step 2: Run them to verify they fail**

Run: `source ~/.cargo/env && cargo test --test mcp_tool_surface`
Expected: FAIL — surface size is 5, and `execute_tool` takes 2 arguments

- [ ] **Step 3: Add the schema**

Append to `src/mcp/schema.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorArgs {
    pub path: String,
    #[serde(default)]
    pub execute: bool,
    #[serde(default)]
    pub decisions: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only: Vec<String>,
}

pub fn doctor_input_schema() -> Value {
    serde_json::json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to the Flutter project to diagnose"
            },
            "execute": {
                "type": "boolean",
                "default": false,
                "description": "Apply the fixes. Requires `decisions` to answer every question returned by the diagnosis call. Never available over the HTTP bridge."
            },
            "decisions": {
                "type": "object",
                "additionalProperties": { "type": "string" },
                "description": "Answers keyed by question id, e.g. {\"flutter.channel\":\"stable\",\"flutter.version\":\"3.24.5\"}"
            },
            "only": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Restrict to these checks: flutter, dart, cocoapods, android, xcode"
            }
        },
        "required": ["path"]
    })
}
```

- [ ] **Step 4: Add the tool and the Trust gate**

In `src/mcp/tools.rs`:

Change the doc comment at line 36 to `/// The locked surface — exactly 6 tools advertised to clients.` and the assertion message at line 485 to `"MCP surface must be exactly 6 tools, got {}"`, with the expected value `6`.

Add to the `vec![…]` in `list_tools()`:

```rust
        ToolDefinition {
            name: "doctor".to_string(),
            description: "Diagnose the project's Flutter toolchain and install what is missing. Call with just `path` to get every check's status plus the questions each fix needs (channel, version, install directory), each option carrying the reason it is suggested. Call again with `execute: true` and `decisions` filled in to perform the install.".to_string(),
            input_schema: doctor_input_schema(),
        },
```

Add the trust-aware listing and thread `Trust` through execution:

```rust
use crate::doctor::Trust;

/// The surface as seen from a given transport. Remote callers see `doctor`
/// described as diagnosis-only, because remote execution is refused.
pub fn list_tools_for(trust: Trust) -> Vec<ToolDefinition> {
    let mut tools = list_tools();
    if trust == Trust::Remote {
        for tool in tools.iter_mut() {
            if tool.name == "doctor" {
                tool.description =
                    "Diagnose the project's Flutter toolchain (diagnosis only over this transport; \
                     installing is refused)"
                        .to_string();
            }
        }
    }
    tools
}

pub fn execute_tool(name: &str, args: &Value, trust: Trust) -> Result<Value, String> {
    let started = Instant::now();
    let canonical_name = canonical_tool_name(name);
    let result = execute_tool_inner(name, args, trust);
    super::telemetry::record_mcp_tool_invocation(
        name,
        canonical_name,
        result.is_ok(),
        started.elapsed(),
    );
    result
}
```

Add `"doctor"` to the `canonical_tool_name` match (mapping to `"doctor"`), give `execute_tool_inner` the extra `trust: Trust` parameter, and add the dispatch arm:

```rust
        "doctor" => execute_doctor(args, trust),
```

Then the handler:

```rust
fn execute_doctor(args: &Value, trust: Trust) -> Result<Value, String> {
    let parsed: crate::mcp::schema::DoctorArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("invalid doctor args: {}", e))?;

    if parsed.execute && trust == Trust::Remote {
        return Err(
            "executing a fix is not permitted over this transport; run `falcon doctor --fix` \
             locally, or call this tool from the MCP stdio server"
                .to_string(),
        );
    }

    let opts = crate::doctor::DoctorOptions {
        root: PathBuf::from(&parsed.path),
        fix: parsed.execute,
        yes: true,
        dry_run: false,
        only: parsed.only.clone(),
        skip: vec![],
        channel: parsed.decisions.get("flutter.channel").cloned(),
        flutter_version: parsed.decisions.get("flutter.version").cloned(),
        dir: parsed.decisions.get("flutter.dir").map(PathBuf::from),
        json: false,
    };

    if !parsed.execute {
        let diagnosis = crate::doctor::diagnose(&opts).map_err(|e| e.to_string())?;
        return serde_json::to_value(&diagnosis).map_err(|e| e.to_string());
    }

    let report = crate::doctor::execute(&opts).map_err(|e| e.to_string())?;
    serde_json::to_value(&report).map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Split `doctor::run` into `diagnose` and `execute`**

The MCP handler needs the two phases as values, not as console output. In `src/doctor/mod.rs`, extract them and make `run` the CLI adapter over both:

```rust
/// Phase one: probe every relevant check. Never mutates anything.
pub fn diagnose(opts: &DoctorOptions) -> Result<Diagnosis> {
    let host_info = host::detect();
    let arch = host::current_arch();
    let manifest = fetch_manifest(host::current_os());
    let mut checks = Vec::new();
    for check in registry() {
        if !opts.wants(check.id()) {
            continue;
        }
        let ctx = CheckContext {
            root: opts.root.clone(),
            host: host_info.clone(),
            arch,
            manifest: manifest.clone(),
            runner: Box::new(RealRunner),
        };
        checks.push(check.probe(&ctx));
    }
    Ok(Diagnosis { host: host_info, checks })
}

/// What an execute call reports back.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionReport {
    pub steps: Vec<Outcome>,
    /// "ok" | "awaiting_manual_step" | "failed"
    pub status: String,
}

/// Phase two: run each fixable check's plan using the supplied decisions.
pub fn execute(opts: &DoctorOptions) -> Result<ExecutionReport> {
    let host_info = host::detect();
    let arch = host::current_arch();
    let manifest = fetch_manifest(host::current_os());
    let mut outcomes = Vec::new();

    for check in registry() {
        if !opts.wants(check.id()) {
            continue;
        }
        let ctx = CheckContext {
            root: opts.root.clone(),
            host: host_info.clone(),
            arch,
            manifest: manifest.clone(),
            runner: Box::new(RealRunner),
        };
        let probed = check.probe(&ctx);
        let Some(offer) = &probed.fix else { continue };
        if offer.kind == FixKind::Manual {
            continue;
        }
        let mut decisions = HashMap::new();
        if let Some(c) = &opts.channel {
            decisions.insert("flutter.channel".to_string(), c.clone());
        }
        if let Some(v) = &opts.flutter_version {
            decisions.insert("flutter.version".to_string(), v.clone());
        }
        if let Some(d) = &opts.dir {
            decisions.insert("flutter.dir".to_string(), d.to_string_lossy().to_string());
        }
        outcomes.extend(apply(check.as_ref(), &ctx, &decisions, opts)?);
    }

    let status = if outcomes.iter().any(|o| matches!(o, Outcome::AwaitingManual { .. })) {
        "awaiting_manual_step"
    } else if outcomes.iter().any(|o| matches!(o, Outcome::Failed { .. })) {
        "failed"
    } else {
        "ok"
    };

    Ok(ExecutionReport { steps: outcomes, status: status.to_string() })
}
```

Rewrite `run()` to call `diagnose()`, render it, ask the questions interactively as it already does, and then run `apply()` per check — keeping the interactive path unchanged in behaviour.

- [ ] **Step 6: Update every call site**

- `src/mcp/server.rs:140` → `tools::execute_tool(tool_name, &arguments, Trust::Local)`
- `src/api/server.rs:83` → `crate::mcp::tools::list_tools_for(Trust::Remote)`
- `tests/edge_analysis_platform_tests.rs` (3 call sites) → add `Trust::Local`
- `tests/mcp_tool_surface.rs` (existing call sites) → add `Trust::Local`

- [ ] **Step 7: Run the tests**

Run: `source ~/.cargo/env && cargo test`
Expected: PASS — including the six new surface tests and the updated counts

- [ ] **Step 8: Verify the MCP server by hand**

Run:

```bash
source ~/.cargo/env && echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run --bin falcon-mcp
```

Expected: six tools listed, `doctor` among them.

- [ ] **Step 9: Commit**

```bash
source ~/.cargo/env && cargo fmt && cargo clippy -- -D warnings
git add src/mcp/ src/api/server.rs src/doctor/mod.rs tests/
git commit -m "feat(mcp): two-phase doctor tool with a trust gate on the HTTP bridge"
```

---

### Task 15: Integration tests and documentation

**Files:**
- Create: `tests/doctor_cli.rs`
- Modify: `README.md`, `CLAUDE.md`, `CHANGELOG.md`
- Test: `tests/doctor_cli.rs`

**Interfaces:**
- Consumes: the `falcon` binary and `falcon::doctor` (all prior tasks).
- Produces: no new library API.

- [ ] **Step 1: Write the failing integration tests**

Create `tests/doctor_cli.rs`:

```rust
//! End-to-end checks on the `falcon doctor` CLI. These run the built binary
//! with `--dry-run`, so nothing is installed and nothing is downloaded.

use std::process::Command;
use tempfile::TempDir;

fn falcon() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}

fn flutter_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("pubspec.yaml"),
        "name: app\nenvironment:\n  sdk: '>=3.4.0 <4.0.0'\n  flutter: '>=3.22.0'\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )
    .unwrap();
    dir
}

#[test]
fn doctor_json_output_is_parseable_and_has_the_expected_shape() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--dry-run", "--format", "json"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    assert!(json["host"]["os"].is_string());
    assert!(json["checks"].is_array());
    let ids: Vec<_> = json["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&"flutter".to_string()), "checks were {:?}", ids);
}

#[test]
fn json_mode_never_prompts_even_without_a_tty() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--format", "json", "--dry-run"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("Choice"), "JSON mode must never print a prompt");
}

#[test]
fn only_flag_restricts_the_checks_that_run() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--dry-run", "--format", "json", "--only", "flutter"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let checks = json["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["id"], "flutter");
}

#[test]
fn skip_flag_removes_a_check() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--dry-run", "--format", "json", "--skip", "flutter"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<_> = json["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!ids.contains(&"flutter".to_string()), "checks were {:?}", ids);
}

#[test]
fn a_project_without_apple_or_android_directories_skips_those_checks() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--dry-run", "--format", "json"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for id in ["xcode", "android", "cocoapods"] {
        let check = json["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id);
        if let Some(c) = check {
            assert_eq!(c["status"]["state"], "skipped", "{} should be skipped", id);
        }
    }
}

#[test]
fn doctor_help_lists_every_documented_flag() {
    let out = falcon().args(["doctor", "--help"]).output().expect("falcon should run");
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in ["--fix", "--yes", "--dry-run", "--only", "--skip", "--channel", "--flutter-version", "--dir", "--format"] {
        assert!(text.contains(flag), "help is missing {}", flag);
    }
}
```

- [ ] **Step 2: Run them**

Run: `source ~/.cargo/env && cargo test --test doctor_cli`
Expected: PASS. If `a_project_without_apple_or_android_directories_skips_those_checks` fails, the relevance gating in Tasks 11-13 is wrong — fix the check, not the test.

- [ ] **Step 3: Document the command in the README**

Add to `README.md`, in the command list, a section:

````markdown
### `falcon doctor` — fix your toolchain

```
falcon doctor                      # diagnose, then offer to fix what's broken
falcon doctor --fix --yes          # unattended: install everything with recommended defaults
falcon doctor --dry-run            # show the plan without touching anything
falcon doctor --only flutter --channel stable --flutter-version 3.24.5
```

Falcon checks Flutter, Dart, CocoaPods, the Android SDK and Xcode — but only
the ones your project actually needs. A project with no `ios/` directory is
never asked about Xcode.

When the Flutter SDK is missing, Falcon reads Google's release manifest,
offers the version your project pins (`.fvmrc`, `.tool-versions`, your CI
workflow, or the constraints in `pubspec.yaml`) alongside the latest on your
chosen channel, downloads it, verifies its checksum, and extracts it.

Two things Falcon will never do: run `sudo`, or accept a licence agreement for
you. Those steps are printed for you to run, and Falcon re-checks afterwards.

Exit codes: `0` healthy, `1` warnings, `2` a check or fix failed,
`3` manual action required.
````

- [ ] **Step 4: Document the module in CLAUDE.md**

In the Architecture list in `CLAUDE.md`, add after the `src/manage/` line:

```markdown
- `src/doctor/` — environment diagnosis and repair (`falcon doctor`): checks, fixers, Flutter installer
```

- [ ] **Step 5: Add a changelog entry**

Add to the top of `CHANGELOG.md` under a new `## Unreleased` heading:

```markdown
### Added
- `falcon doctor` — diagnoses Flutter, Dart, CocoaPods, Android SDK and Xcode for the current project, and installs or repairs what it can. Reads the Flutter release manifest to offer the version your project pins alongside the latest on your channel; verifies checksums; prints (never edits) the PATH line.
- `doctor` MCP tool — two-phase: call it with a path to get every check's status and the questions each fix needs, then again with `execute: true` and `decisions` to perform the install. Execution is refused over the HTTP bridge.
```

- [ ] **Step 6: Full verification**

Run: `source ~/.cargo/env && cargo test`
Expected: PASS, the whole suite

Run: `source ~/.cargo/env && cargo clippy --all-targets -- -D warnings`
Expected: clean

Run: `source ~/.cargo/env && cargo build --release`
Expected: builds

- [ ] **Step 7: Confirm no dependency crept in**

Run: `git diff master --stat -- Cargo.toml Cargo.lock`
Expected: **no changes to `[dependencies]`.** If `Cargo.toml` changed, a task added a crate it should not have — find it and remove it.

- [ ] **Step 8: Commit**

```bash
source ~/.cargo/env && cargo fmt
git add tests/doctor_cli.rs README.md CLAUDE.md CHANGELOG.md
git commit -m "test(doctor): CLI integration tests; docs for falcon doctor"
```

---

## Appendix: requirement-to-task map

| Spec requirement | Task |
|---|---|
| Core types, `FixKind`, exit codes incl. 3 | 2 |
| Host OS/arch/home/shell detection | 3 |
| No-network, no-sudo executor with injected IO | 4 |
| Release manifest, arch filtering, master fallback | 5 |
| Version ladder: FVM → asdf → CI → pubspec → head | 6 |
| Latest always offered alongside the pin | 6 |
| Pin/pubspec conflict stated explicitly | 6 |
| Download → checksum → extract → prime → PATH hint | 7 |
| Refuse non-empty install directory | 7 |
| Flutter check + channel/version/dir questions | 8 |
| CLI flags, exit codes, three interactivity modes | 9 |
| Prompt with no new dependency | 9 |
| Text and JSON reports | 9 |
| Dart deduped against the Flutter fixer | 10 |
| CocoaPods via brew or user-scoped gem | 11 |
| Android cmdline-tools + packages + licence handoff | 12 |
| Xcode Apple ID and sudo handoffs | 13 |
| Invariants enforced by test, not convention | 13 |
| Two-phase MCP `doctor` tool, surface 5 → 6 | 14 |
| Trust gate on the HTTP bridge | 14 |
| Relevance gating verified end to end | 15 |
| README, CLAUDE.md, CHANGELOG | 15 |
| No new dependencies | 15 (verified) |
