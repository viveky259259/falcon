# Falcon Doctor — environment diagnosis and repair

**Date:** 2026-09-14
**Status:** Approved design, ready for implementation planning

## Problem

Falcon assumes a working Flutter toolchain. When it is missing, every path that
shells out fails with a dead end: `handle_flutter` (`src/main.rs:941`) prints
"Is the Flutter SDK on your PATH?", `flutter_run` (`src/flutter_run/mod.rs:131`)
prints "Is Flutter installed and on PATH?", and `analyzer_bridge` simply cannot
run `dart`. The user is told what is wrong and nothing about how to fix it.

The same gap exists for the rest of the toolchain — CocoaPods, the Android SDK
and its licences, Xcode and its command line tools — each of which blocks a
different Falcon command.

Falcon should diagnose the whole environment, and repair what it can.

## Goals

1. One command, `falcon doctor`, that reports the health of every toolchain
   component the current project actually needs.
2. For each broken component, install or repair it — genuinely, not by printing
   a link — asking the user for the decisions only they can make.
3. For the steps that cannot be automated (Apple ID authentication, licence
   acceptance, anything needing root), hand off with an exact command and
   re-verify afterwards.
4. Expose the same flow over MCP so an AI agent can diagnose, choose a channel
   and version with real reasons, and drive the install.

## Non-goals

- Editing the user's shell rc files. Falcon prints the `export PATH=` line; the
  user owns their dotfiles.
- Running `sudo`. Ever. See invariants.
- Auto-accepting licence agreements. See invariants.
- Managing multiple concurrent Flutter versions. That is FVM's job; Falcon
  reads FVM's pin but does not replace it.

## Invariants

These are enforced in the executor, not left to the discretion of each fixer.

1. **Falcon never runs `sudo`.** A step requiring root is always a `Handoff`,
   even where Falcon could technically shell it.
2. **Licences are never auto-accepted.** `flutter doctor --android-licenses`
   and `xcodebuild -license accept` are legal agreements. Piping `yes` into
   them on a user's behalf is not Falcon's call. Both are `Handoff`.
3. **No shell rc file is modified.** PATH guidance is printed.
4. **Tests never touch the network and never write outside a tempdir.**
5. **The execute phase is unreachable from the HTTP bridge.** See Trust gate.

## Architecture

New module `src/doctor/`, registered in `src/lib.rs`, dispatched through a new
`CommandGroup::Environment` in `src/command_dispatch.rs`.

```
src/doctor/
  mod.rs            Doctor orchestrator: probe() / plan() / execute()
  types.rs          Diagnosis, CheckResult, Status, FixOffer, Question, Plan, Step
  host.rs           OS, arch, libc, shell rc path, available package managers
  prompt.rs         numbered-choice TTY prompt (no new dependency)
  report.rs         text + json rendering
  checks/
    flutter.rs  dart.rs  cocoapods.rs  android.rs  xcode.rs
  flutter/
    releases.rs     Google releases_<os>.json manifest client
    version.rs      pin discovery + pubspec constraint resolution
    install.rs      download -> checksum -> extract -> verify -> PATH guidance
```

A `Check` probes and produces a `CheckResult`. A `Fixer` turns *decisions* into
an ordered `Plan` of `Step`s, which the executor runs. CLI and MCP are thin
adapters over this identical core, so interactive prompts and an agent's JSON
decisions cannot drift apart.

### Core types

```rust
pub enum Status {
    Ok { version: String },
    Missing,
    Outdated { found: String, needed: String },
    Broken { reason: String },
    Skipped { because: String },
}

/// Automatic = Falcon completes it unattended.
/// Assisted  = Falcon completes part; at least one Handoff step remains.
/// Manual    = Falcon can only describe the fix.
pub enum FixKind { Automatic, Assisted, Manual }

pub struct CheckResult {
    pub id: &'static str,            // "flutter", "xcode", ...
    pub status: Status,
    pub required_by: Vec<String>,    // why this project needs it
    pub fix: Option<FixOffer>,
}

pub struct FixOffer {
    pub kind: FixKind,
    pub questions: Vec<Question>,
    pub steps: Vec<StepSummary>,
}

pub struct Question {
    pub id: String,                  // "flutter.channel"
    pub prompt: String,
    pub options: Vec<Choice>,
    pub default: Option<String>,
}

pub struct Choice {
    pub value: String,
    pub label: String,
    pub rationale: String,           // *why* this option is suggested
    pub recommended: bool,
}
```

`Question`/`Choice` is what serves the MCP requirement: phase one returns the
options *and the reason behind each*, so an agent chooses with justification
rather than guessing flag values. The same structs drive the TTY prompt.

### Relevance gating

Every check reports `required_by`. A check the project does not need is
`Skipped`, not failed — no `ios/` directory means Xcode and CocoaPods do not
nag. The Podfile signal reuses the existing `check-pods` detection.

## Flutter: version resolution

Resolution ladder, first hit wins. Each hit produces a `Choice` carrying its
rationale string:

| # | Source | Rationale shown |
|---|--------|-----------------|
| 1 | `.fvmrc` / `.fvm/fvm_config.json` | "pinned by FVM in this repo" |
| 2 | `.tool-versions` (asdf) | "pinned by asdf" |
| 3 | `.github/workflows/*.yml` -> `flutter-version:` | "what CI builds with" |
| 4 | `pubspec.yaml` `environment.flutter` + `environment.sdk` | "newest release satisfying >=3.22.0 and Dart >=3.4.0 <4.0.0" |
| 5 | channel head | "latest on stable" |

Latest-on-channel is **always** offered alongside the winning pin, so the choice
between "what the project declares" and "newest available" stays visible. If a
pin exists *and* conflicts with the pubspec constraints, both options are
presented with the conflict stated explicitly.

Constraint satisfaction for row 4 walks the channel's releases newest-first and
takes the first build whose `version` satisfies `environment.flutter` and whose
`dart_sdk_version` satisfies `environment.sdk`.

## Flutter: install pipeline

**Source of truth:** `https://storage.googleapis.com/flutter_infra_release/releases/releases_{macos,linux,windows}.json`.
Each release entry carries `version`, `channel`, `dart_sdk_version`,
`dart_sdk_arch`, `archive` and `sha256`; the document carries `base_url` and
`current_release` per channel.

Falcon filters by channel **and** by `dart_sdk_arch` against the host
(arm64 vs x64). Getting the architecture wrong produces the classic
"Flutter runs under Rosetta and everything is slow" failure.

**Channels** are `stable`, `beta` and `master`. There is no "alpha" channel.
The release manifest publishes archives for **stable and beta only** — `master`
has no archive, so choosing master switches the plan to
`git clone -b master https://github.com/flutter/flutter.git` for that case
alone. The branch is explicit rather than hidden.

**No new dependencies.** Falcon has no HTTP client crate: `src/self_update.rs`
already shells out to `curl` (`:49`) and `tar` (`:245`). The installer follows
that established pattern instead of pulling in `reqwest`. `sha2` is already a
dependency, so checksum verification is free. `dirs` is gated behind the
`ai-local` feature, so home resolution uses the
`env::var("HOME").or_else(|_| env::var("USERPROFILE"))` pattern from
`src/main.rs:3103`.

Pipeline, each an explicit `Step`, previewable in full with `--dry-run`:

1. Resolve target dir — `$HOME/development/flutter`, overridable with `--dir`.
   **Refuse** if it exists and is non-empty.
2. `curl -fL --proto '=https' --tlsv1.2` the archive into a scratch dir.
3. Verify `sha256` against the manifest. **Abort and delete** on mismatch.
4. Extract (`tar -xJf` / `unzip`), applying a `git config --global --add
   safe.directory` guard where the SDK's own git checks require it.
5. Run `<dir>/bin/flutter --version` once — primes the bundled Dart SDK and
   proves the install actually executes.
6. Detect the shell's rc file and **print** the exact `export PATH=` line.
7. Re-probe and report the resolved version.

Because of step 6, `flutter` may still be absent from the *current* shell after
a successful install. The report states this explicitly rather than claiming
success and leaving the user confused.

## The other four checks

| Check | Needed when | FixKind | Approach |
|-------|-------------|---------|----------|
| **dart** | always | Automatic | Normally satisfied *by* the Flutter fixer, since Flutter bundles Dart at `bin/cache/dart-sdk`. Reported as "will be satisfied by installing Flutter" and deduped so nothing downloads twice. Standalone Dart only when the project has no Flutter dependency. |
| **cocoapods** | macOS + `ios/Podfile` | Automatic | `brew install cocoapods` when Homebrew is present, otherwise `gem install cocoapods --user-install`. Never `sudo gem install` into the system Ruby. |
| **android** | `android/` exists | Assisted | Automatic: fetch cmdline-tools from `dl.google.com`, extract into the platform SDK root, run `sdkmanager "platform-tools" "platforms;android-NN" "build-tools;NN.0.0"` with NN read from `android/app/build.gradle`. Handoff: licence acceptance. Missing JDK is fixed via the platform package manager, or reported Manual. |
| **xcode** | macOS + `ios/` or `macos/` | Assisted | Automatic: `xcodebuild -runFirstLaunch`, `xcodebuild -downloadPlatform iOS`. Handoff: installing Xcode itself (Apple ID required — `xcodes install --latest` when `xcodes` is present, else the App Store), and every `sudo` step. |

### The handoff step

One type, shared by both Assisted fixers:

```rust
Step::Handoff {
    reason: String,
    command: String,
    docs_url: String,
    verify: Probe,
}
```

Its three consumers behave differently, and that difference is the point:

- **CLI (TTY)** — prints the command in a box, waits for Enter, runs `verify`,
  loops up to 3 times before giving up with the command still on screen.
- **CLI (non-TTY / CI)** — prints and exits **3**, "manual action required",
  distinct from 1 and 2, so a pipeline can distinguish "needs a human" from
  "failed".
- **MCP** — returns `status: "awaiting_manual_step"` with the command and its
  verification. An agent with shell access can run the interactive or `sudo`
  step itself and call back.

The MCP path is therefore strictly more capable than the CLI path, which is
exactly the "AI drives the install" requirement falling out of the design
rather than being bolted onto it.

## CLI surface

```
falcon doctor [PATH]
  --fix                       apply fixes (default: diagnose + offer interactively)
  --yes                       accept every recommended default, never prompt
  --dry-run                   print the full plan, execute nothing
  --only <check>...           flutter | dart | cocoapods | android | xcode
  --skip <check>...
  --channel <stable|beta|master>
  --flutter-version <x.y.z|latest|project>
  --dir <path>                SDK install directory
  --format <text|json>
```

Exit codes: `0` healthy, `1` warnings, `2` a check or fix failed,
`3` manual action required.

`--fix` and the interactive default differ in exactly one respect: without
`--fix`, each broken check asks "fix this now?" before its plan runs; with
`--fix`, that per-check confirmation is skipped. Both still ask the *decision*
questions (channel, version, directory) unless those are supplied by flag or
`--yes`. `--fix --yes` is therefore the fully unattended form, and `--dry-run`
overrides both and executes nothing.

### Interactivity

Three modes, decided once at startup and never changed mid-run:

- **TTY with no flags** — prompt per decision using `Question`/`Choice`
  (channel, then version, then directory), recommended option preselected.
- **`--yes`, or no TTY** — take `Choice.recommended`. If a decision has no safe
  default, stop with exit 3 rather than guessing.
- **`--format json`** — never prompts. Unanswered decisions are returned as
  questions in the payload.

`prompt.rs` implements a numbered-choice reader over stdin. Falcon currently
has no interactive dependency (no `dialoguer`, no `inquire`) and this does not
add one.

## MCP surface

The advertised surface grows from 5 tools to 6. `doctor` is a new tool, not a
rename, so `DEPRECATED_TOOLS` is untouched.

Two phases on one tool:

```jsonc
// phase 1 — diagnose
{ "path": "/repo" }
// -> { host, checks: [ { id, status, required_by,
//        fix: { kind, questions: [ { id, prompt,
//          options: [ { value, label, rationale, recommended } ] } ],
//        steps } } ] }

// phase 2 — execute
{ "path": "/repo", "execute": true,
  "decisions": {
    "flutter.channel": "stable",
    "flutter.version": "3.24.5",
    "flutter.dir": "~/development/flutter"
  } }
// -> { steps: [ { id, outcome } ],
//      status: "ok" | "awaiting_manual_step" | "failed" }
```

`execute` defaults to `false`, so a speculative call can never install
anything.

Four places assert the locked surface size and must move together:

- the doc comment at `src/mcp/tools.rs:36`
- the assertion at `src/mcp/tools.rs:485`
- `tests/v20_tests.rs:10`
- `tests/mcp_tool_surface.rs:25`

`list_tools`, `execute_tool`, `canonical_tool_name` and `src/mcp/schema.rs`
(new `DoctorArgs` + `doctor_input_schema`) all gain the new entry.

### Trust gate

`src/api/server.rs:83` re-publishes `mcp::tools::list_tools()` over an HTTP
server that binds a configurable host with no authentication. Today that
endpoint only *lists* tools — `/analyze`, `/score` and `/check-file` are
hand-written and never call `execute_tool` — so nothing is currently
exploitable. A tool that downloads and runs installers must not become
reachable there by a later change.

`execute_tool` therefore takes an explicit `Trust` enum:

```rust
pub enum Trust { Local, Remote }
```

`Local` for the CLI and MCP stdio transports, `Remote` for the HTTP bridge.
`doctor` with `execute: true` returns a refusal under `Remote`, and `/tools`
advertises it as diagnosis-only there. This is a type the call site must
supply, not a string check inside the HTTP layer, so the gate cannot be
forgotten.

## Testing

The binding constraint: no test touches the network, and no test writes outside
a tempdir.

- `trait CommandRunner` and `trait Downloader` are injected into the executor.
  Tests assert the **generated plan** — exact command strings, URLs, target
  paths — without executing anything.
- `flutter/releases.rs` parses a checked-in fixture manifest: architecture
  filtering (arm64 vs x64), channel filtering, and the master-has-no-archive
  fallback to git clone.
- `flutter/version.rs` ladder precedence over tempdir fixtures — `.fvmrc` beats
  `.tool-versions` beats CI workflow beats pubspec — plus constraint
  satisfaction units and the pin-conflicts-with-pubspec case.
- `doctor_input_schema` is validated with the existing `jsonschema`
  dev-dependency, as the other five tool schemas are.
- Exit-code mapping, including 3.
- Trust gate: `execute: true` under `Trust::Remote` is refused.
- Integration: `falcon doctor --dry-run --format json` against a fixture
  project produces stable JSON.

## Open risks

- **Archive layout drift.** Google's release manifest schema is stable in
  practice but unversioned. The parser tolerates unknown fields and fails with
  a clear message, falling back to git clone, rather than panicking.
- **Android API level detection.** Reading `android/app/build.gradle` handles
  both Groovy and Kotlin DSL; projects using unusual variable indirection fall
  back to the latest stable platform with a warning.
- **Windows coverage.** The pipeline is designed for it (`releases_windows.json`,
  `unzip`), but Falcon's CI has no Windows runner, so Windows support ships
  tested only at the unit level.
