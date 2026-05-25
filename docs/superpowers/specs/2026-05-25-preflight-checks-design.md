# Falcon Pre-flight Checks — Design

**Date:** 2026-05-25
**Status:** Draft v2 (self-review applied), awaiting approval
**Target version:** falcon 0.4.0 → next
**Source spec:** `docs/falcon-requirements.md` (external workspace, copied here for reference)
**Severity enum:** reuses `crate::config::Severity` (`Error | Warning | Info`) — same one used by the rules engine.

## Goal

Add four new `falcon check-*` commands that fail loudly **before** `flutter build` or `maestro test` starts, catching mechanical configuration errors that are present in the repo on disk but invisible to today's static-analysis flow:

1. `falcon check-assets` — verify every entry in `pubspec.yaml`'s `flutter.assets:` exists on disk.
2. `falcon check-a11y` — verify `SemanticsBinding.instance.ensureSemantics()` is called in `main()` and a configurable % of interactive widgets carry `Semantics(identifier:)`.
3. `falcon check-pods` — verify `ios/Podfile`'s `platform :ios, 'X.Y'` is ≥ every transitive plugin's `s.ios.deployment_target`.
4. `falcon check-platform-deps` — verify the project's `Info.plist` / `AndroidManifest.xml` declare every key/permission required by its plugins.

Each command exits `0` clean, `1` warnings, `2` errors. All four roll up into `falcon analyze`.

## Source-of-truth strategy (key decision)

**No hand-curated plugin map.** Instead, every run scans the project's actual pub-cache:

- For each plugin in `pubspec.yaml`, resolve to `~/.pub-cache/hosted/pub.dev/<plugin>-<version>/`.
- Read its **own** `ios/`, `android/`, `macos/` folder for the source of truth:
  - **Android:** the plugin's `android/src/main/AndroidManifest.xml` template literally declares its `<uses-permission>` block — we copy it. No map needed.
  - **iOS:** grep the plugin's `ios/Classes/**/*.{m,swift}` and `darwin/Classes/**/*.{m,swift}` for Apple-SDK API calls (e.g. `requestWhenInUseAuthorization`, `AVCaptureDevice`, `CNContactStore`, `PHPhotoLibrary`, `EKEventStore`). A small **Apple-SDK API → Info.plist key dictionary** (≈22 entries at v1, see Appendix A; stable across Apple SDK versions; lives inside Falcon) translates each API to the required key.
  - **iOS deployment target:** read **every** `*.podspec` under the plugin's `ios/` and `darwin/` folders and take the max `s.ios.deployment_target`. (Plugins like `image_picker` ship multiple podspecs.) Absent value defaults to 9.0 (CocoaPods default).
- The scan output is written to `.falcon/plugin-requirements.yaml`. Refreshed when the SHA-256 of `pubspec.lock` changes (or with `--refresh`). The file is committable so reviewers see what Falcon inferred.
- **Non-pub.dev plugins** (`git:`, `path:`, `hosted:` against private servers): emit an Info-severity "skipped — non-pub.dev source" entry in the report; do not attempt to scan. Revisit if real-world dogfooding shows demand.

This eliminates the maintenance burden of a 200-entry plugin map and means new plugins, plugin upgrades, and uncommon plugins all work without code changes. The ≈22-entry Apple-SDK API map is stable Apple territory, not Flutter ecosystem churn.

**Directory convention (resolved open question):** This spec uses `.falcon/` for **committable** generated artifacts (`.falcon/plugin-requirements.yaml`). The existing `.falcon-data/` continues to hold **local-only** state (history snapshots, run caches). `.gitignore` template snippets in `docs/` should reflect this split.

**Pub-cache override (resolved open question):** Tests point Falcon at a fake pub-cache via the env var **`FALCON_PUB_CACHE`**. No CLI flag in v1 — keeps the surface minimal. The default is the platform-conventional pub-cache: `$PUB_CACHE` if set, else `~/.pub-cache` on Unix / `%LOCALAPPDATA%\Pub\Cache` on Windows.

## Architecture

### Module layout

Scaffolded in parallel so the CLI wiring + module skeletons land in one initial commit per command; detection logic fills in per command in subsequent commits.

```
src/
├── preflight/
│   ├── mod.rs              # re-exports + shared types (PreflightIssue, OutputFormat, TargetPlatform)
│   ├── pub_cache.rs        # locate_pub_cache(), iter_installed_plugins() — shared by check_pods + check_platform_deps
│   └── reporter.rs         # text / json / sarif emitters for PreflightIssue lists
├── check_assets/
│   ├── mod.rs              # pub fn run(root: &Path, format: OutputFormat) -> anyhow::Result<i32>
│   ├── pubspec.rs          # parse pubspec.yaml flutter.assets list (line-tracked)
│   └── tests inline
├── check_a11y/
│   ├── mod.rs
│   ├── main_check.rs       # ensureSemantics() presence in main()
│   ├── widget_scan.rs      # interactive-widget count + Semantics(identifier:) coverage
│   └── tests inline
├── check_pods/
│   ├── mod.rs
│   ├── podfile.rs          # parse Podfile platform :ios|:osx, 'X.Y'
│   ├── podspec.rs          # scan pub-cache podspecs (via preflight::pub_cache)
│   └── tests inline
└── check_platform_deps/
    ├── mod.rs
    ├── apple_api_map.rs    # Apple-SDK API → Info.plist key entries (stable, see Appendix A)
    ├── ios_scan.rs         # grep plugin ios/darwin sources via the API map
    ├── android_scan.rs     # parse <uses-permission> from plugin AndroidManifest.xml templates
    ├── project_manifest.rs # parse the user's Info.plist + AndroidManifest.xml
    ├── generated.rs        # read/write .falcon/plugin-requirements.yaml
    └── tests inline
```

Each `check_*::run(root, format) -> anyhow::Result<i32>` returns the exit code (0/1/2). The shared `preflight::pub_cache` module is the single source for plugin enumeration, so `check_pods` and `check_platform_deps` walk the cache exactly once when invoked together (via the analyze rollup).

### CLI wiring

`enum Commands` in `src/main.rs` gets four new variants:

```rust
/// Verify every asset declared in pubspec.yaml exists on disk.
CheckAssets {
    /// Path to the Flutter project.
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Output format.
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
},
/// Verify Flutter project is ready for Maestro UI testing.
CheckA11y {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
},
/// Verify ios/Podfile deployment target is ≥ every plugin's required minimum.
CheckPods {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Re-scan plugin podspecs even if .falcon/plugin-requirements.yaml is current.
    #[arg(long)]
    refresh: bool,
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
},
/// Verify Info.plist / AndroidManifest.xml declare every key the project's plugins require.
CheckPlatformDeps {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Re-scan plugins even if .falcon/plugin-requirements.yaml is current.
    #[arg(long)]
    refresh: bool,
    /// Restrict scan to one platform.
    #[arg(long, value_enum)]
    platform: Option<TargetPlatform>,
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
},
```

A new shared `OutputFormat` enum (`Text`, `Json`, `Sarif`) consolidates what `--format` accepts. `TargetPlatform` is `Ios | Android | Both`.

### Reporting

A shared `PreflightIssue` struct, serialized by an `OutputFormat`-aware reporter:

```rust
pub struct PreflightIssue {
    pub rule_id: String,                // e.g. "platform-deps/missing-info-plist-key"
    pub severity: crate::config::Severity, // Error | Warning | Info — same enum the rules engine uses
    pub title: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub plugin: Option<String>,         // populated for platform-deps issues
    pub message: String,
    pub suggestion: Option<String>,
}
```

Text format mirrors existing rule output (colored, bold). JSON output is a `{ "version": 1, "rule_id": ..., ... }[]` array. SARIF emits the [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/) shape that GitHub code-scanning expects.

### `.falcon/plugin-requirements.yaml` (generated)

Written by `check-platform-deps` (and `check-pods`, which shares the pub-cache scan).

```yaml
# Generated by `falcon check-platform-deps` — do not hand-edit unless overriding.
# Regenerate with --refresh after `flutter pub get`.
schema_version: 1
generated_at: 2026-05-25T12:34:56Z
pubspec_lock_sha256: 5dba9c61e3c8a9c4...   # 64 hex chars; SHA-256 of pubspec.lock contents
plugins:
  location:
    version: 8.0.0
    source: hosted/pub.dev
    ios:
      info_plist_keys:
        - key: NSLocationWhenInUseUsageDescription
          source: ios/Classes/LocationPlugin.m  # where Falcon found the API call
          api: CLLocationManager.requestWhenInUseAuthorization
      deployment_target: "12.0"
    android:
      permissions:
        - android.permission.ACCESS_FINE_LOCATION
        - android.permission.ACCESS_COARSE_LOCATION
      min_sdk: 21
  some_git_plugin:
    version: 0.0.0
    source: git
    skipped: non-pub.dev source — Falcon cannot infer requirements
```

Refresh policy:
- If `.falcon/plugin-requirements.yaml` exists, `schema_version` matches, and `pubspec_lock_sha256` equals SHA-256 of the current `pubspec.lock` contents → reuse.
- Otherwise re-scan and overwrite. `--refresh` (on either `check-pods` or `check-platform-deps`) forces re-scan.

### Suppression

Existing `falcon.yaml` schema gains a `preflight:` section split into two concerns: **`suppress:`** (skip-list of specific issues) and **`config:`** (tuning knobs that change detection thresholds or severity).

```yaml
preflight:
  # Issues to skip entirely.
  suppress:
    - rule_id: platform-deps/missing-info-plist-key
      plugin: googleapis
      key: NSPhotoLibraryUsageDescription
      reason: "We never call ImagePicker; only googleapis OAuth."

  # Tunable thresholds and severities per check.
  config:
    check-a11y:
      require_ensure_semantics: error     # error | warning | info | off
      interactive_semantics_coverage:
        threshold: 0.6                    # fraction of widgets that must have identifiers
        severity: warning
    check-assets:
      warn_on_empty_directory: true       # warn if asset dir exists but contains no files
```

### Rollup in `falcon analyze`

After the existing analyze flow finishes, run the four pre-flight checks in sequence (they're each <1s on a typical project). The overall exit code is `max(existing_exit, preflight_exits)`. Each pre-flight section gets a header and appears in JSON/SARIF output alongside the existing analyze results.

Opt-out via `falcon.yaml`:

```yaml
analyze:
  preflight:
    enabled: true  # default
    skip: [check-pods]  # optional skip list
```

## Per-command detection logic

### check-assets

1. Read `pubspec.yaml`. Pull the `flutter.assets:` list (Flutter accepts only the `flutter: { assets: [...] }` form; same key elsewhere is ignored).
2. For each entry:
   - If the entry ends in `/`, resolve as a directory; require it exists; warn if empty (suppressible via `config.check-assets.warn_on_empty_directory`).
   - Otherwise resolve as a literal file; require it exists.
3. **Glob entries are not supported by Flutter's pubspec.** If an entry contains a glob metacharacter (`*`, `?`, `[`), emit a Warning that Flutter will reject it at build time — Falcon does not expand globs.
4. Source location of the declaration: track the line number in `pubspec.yaml` (via `serde_yaml::with_*` or a hand-rolled line tracker) so error output points the user at the exact declaration.

### check-a11y

1. Parse `lib/main.dart` with the existing `DartParser`. Walk for the `main()` function. Check whether `SemanticsBinding.instance.ensureSemantics()` appears (and isn't inside a commented-out line). Emit error if missing.
2. Walk every `.dart` file under `lib/` (skip `*.g.dart` / `*.freezed.dart` / `test/`). For each interactive widget kind in `INTERACTIVE_WIDGETS` (see below), check whether one of the 3 nearest enclosing widgets is `Semantics(identifier: ...)`.
3. Compute `wrapped_count / total_count`. Compare against `interactive_semantics_coverage.threshold`. Emit warning if below.
4. For each unwrapped interactive widget, emit a Suggestion-severity issue with the recommended wrap pattern (per the spec's §4.4 decision matrix: TextField goes inside `Container` first, etc.).

`INTERACTIVE_WIDGETS = ["ElevatedButton", "TextButton", "OutlinedButton", "IconButton", "FloatingActionButton", "TextField", "TextFormField", "GestureDetector", "InkWell", "InkResponse", "DropdownButton", "DropdownButtonFormField", "Checkbox", "CheckboxListTile", "Switch", "SwitchListTile", "Radio", "RadioListTile"]`

### check-pods

1. Read `ios/Podfile`. Regex out `platform :ios, '(\d+\.\d+)'`. If absent, treat as 12.0 (CocoaPods default).
2. For every plugin in `pubspec.lock` (via `preflight::pub_cache`), enumerate **all** `*.podspec` files under `<plugin>/ios/` and `<plugin>/darwin/`.
3. From each podspec extract `s.ios.deployment_target = '(\d+\.\d+)'`. If absent on an iOS-platform podspec, treat as 9.0 (CocoaPods default for iOS).
4. Compute `max(plugin_targets)` across all podspecs. If `podfile_target < max_plugin_target`, emit Error naming each binding plugin (multiple plugins may share the binding minimum).
5. Same logic for `macos/Podfile` and `s.osx.deployment_target` if `macos/Podfile` exists.
6. **Out of scope for v1:** Android equivalent (`check-gradle`). Mentioned here for awareness; not implemented in this initiative.

### check-platform-deps

The biggest command. Pipeline:

1. **Plugin scan.** Read `pubspec.lock` for direct + transitive deps. For each `hosted: pub.dev` entry, locate `<pub_cache>/hosted/pub.dev/<plugin>-<version>/` via `preflight::pub_cache`. Plugins with `source: git` or `source: path` are emitted as Info-severity "skipped" entries (see `.falcon/plugin-requirements.yaml` example above). Plugins with no `ios/`, `darwin/`, or `android/` folder are pure-Dart — skipped silently.
2. **iOS extraction.** Grep the plugin's `ios/Classes/**/*.{m,swift}` AND `darwin/Classes/**/*.{m,swift}` for Apple-SDK API substrings in `apple_api_map.rs`. Each hit records `(plugin, api, required_info_plist_key, source_file_in_plugin)`.
3. **Android extraction.** Parse the plugin's `android/src/main/AndroidManifest.xml` (the plugin's *own* template, not the user's) and pull every `<uses-permission android:name="..."/>` entry.
4. **Write `.falcon/plugin-requirements.yaml`.** SHA-256 of `pubspec.lock` contents stamped under `pubspec_lock_sha256`.
5. **Diff.** Read the project's `ios/Runner/Info.plist` and `android/app/src/main/AndroidManifest.xml`. For each required-key/permission that's missing from the project, emit an Error.
6. **Output.** Per-issue: plugin name + version, missing key, Apple API or `<uses-permission>` source, suggested patch.

`apple_api_map.rs` initial contents (the spec's "small dictionary" — see Appendix A for the full list).

## Cross-cutting

### Performance

- check-assets: <50ms (small YAML + fs::exists).
- check-a11y: <500ms (tree-sitter parse over `lib/` Dart files).
- check-pods: <200ms (regex over ~200 podspec files).
- check-platform-deps: <1s first run (full grep over pub-cache for the project's plugins), <100ms on subsequent runs (cache hit).

### Output formats

Shared `--format` flag: `text` (ANSI), `json`, `sarif`. JSON schema versioned (`{ "version": 1, ... }`); SARIF version 2.1.0.

### Telemetry

Each issue carries a stable `rule_id`. The existing `falcon fix-track` framework will pick these up without changes.

### Testing

- Each module gets a `#[cfg(test)] mod tests` with TempDir fixtures that:
  - Construct a minimal `pubspec.yaml` + `pubspec.lock` + fake pub-cache files under `tmp/.pub-cache/hosted/pub.dev/<plugin>-<version>/...` and point Falcon at it via the **`FALCON_PUB_CACHE`** env var.
  - Construct iOS/Android project structures with intentional misses.
  - Assert the right issues fire at the right severity.
- Integration test under `tests/preflight_tests.rs` that runs `falcon check-* <fixture>` and asserts exit codes.

## Acceptance criteria

Same as source spec §7:

1. On a project missing required Info.plist keys for `location`, `check-platform-deps` exits 2 with `NSLocationWhenInUseUsageDescription` named in the output.
2. On a project with one or more entries declared in `flutter.assets:` that do not exist on disk, `check-assets` exits 2 and names every missing entry.
3. On a project with a Podfile lacking a `platform :ios, 'X.Y'` directive (defaulting to 12.0) while at least one plugin podspec requires ≥13.0, `check-pods` exits 2 naming the binding plugin(s).
4. On a project where `main()` contains no active call to `SemanticsBinding.instance.ensureSemantics()`, `check-a11y` exits 1 (warning) and reports a coverage ratio for interactive widgets.
5. After fixes, all four commands exit 0 on the same project.
6. `falcon analyze` rollup exits with `max(existing_exit_code, ...preflight_exit_codes)`.
7. `--format json` schema documented in `docs/preflight-json-schema.md` and stable (schema_version field).
8. Suppression via `falcon.yaml`'s `preflight.suppress` list works for at least one rule per command.

## Open questions

The two implementation-affecting open questions from v1 (`.falcon/` directory convention and pub-cache override) are now decided in the Source-of-truth section above. Remaining open question:

- **Plugin author opt-in.** Could plugin authors ship a `falcon-requirements.yaml` inside their own package so we don't have to grep their source? Out of scope for v1 — revisit after dogfooding for 1-2 months.

## Appendix A — Apple-SDK API → Info.plist key dictionary

Initial set (22 entries; lives in `src/check_platform_deps/apple_api_map.rs`). Each entry: `{ api: &str, key: &str, framework: &str }`. Additions land via small follow-up PRs as real plugins surface new patterns.

| API call substring | Info.plist key | Framework |
|---------------------|----------------|-----------|
| `requestWhenInUseAuthorization` | NSLocationWhenInUseUsageDescription | CoreLocation |
| `requestAlwaysAuthorization` | NSLocationAlwaysAndWhenInUseUsageDescription | CoreLocation |
| `AVCaptureDevice.requestAccess` | NSCameraUsageDescription | AVFoundation |
| `AVAudioSession.requestRecordPermission` | NSMicrophoneUsageDescription | AVFoundation |
| `PHPhotoLibrary.requestAuthorization` | NSPhotoLibraryUsageDescription | Photos |
| `PHAsset.creationRequestForAssetFromImage` | NSPhotoLibraryAddUsageDescription | Photos |
| `CNContactStore` | NSContactsUsageDescription | Contacts |
| `EKEventStore` | NSCalendarsUsageDescription | EventKit |
| `EKReminder` | NSRemindersUsageDescription | EventKit |
| `CMMotionManager` | NSMotionUsageDescription | CoreMotion |
| `HKHealthStore` | NSHealthShareUsageDescription / NSHealthUpdateUsageDescription | HealthKit |
| `CBCentralManager` | NSBluetoothAlwaysUsageDescription | CoreBluetooth |
| `MFMessageComposeViewController` | (no key; capability) | MessageUI |
| `SFSpeechRecognizer.requestAuthorization` | NSSpeechRecognitionUsageDescription | Speech |
| `LAContext.canEvaluatePolicy` | NSFaceIDUsageDescription | LocalAuthentication |
| `MPMediaLibrary` | NSAppleMusicUsageDescription | MediaPlayer |
| `HMHomeManager` | NSHomeKitUsageDescription | HomeKit |
| `NEHotspotConfigurationManager` | (capability, not key) | NetworkExtension |
| `UNUserNotificationCenter.requestAuthorization` | (capability via Push entitlement) | UserNotifications |
| `CBPeripheralManager` | NSBluetoothPeripheralUsageDescription | CoreBluetooth |
| `CTCellularData` | (no key needed) | CoreTelephony |
| `SiriKit.INPreferences.requestSiriAuthorization` | NSSiriUsageDescription | Intents |

Additional entries added during implementation as the team discovers them via real plugins. The map lives in source so PRs adding entries are trivially reviewable.

## Approval

This spec is the contract. Each of the four `check-*` commands lands as its own PR with its own commit history. The Apple-API map lands with `check-platform-deps`. The rollup in `falcon analyze` lands in a fifth small PR after the four commands are merged.
