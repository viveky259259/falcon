# Falcon Pre-flight Checks — Design

**Date:** 2026-05-25
**Status:** Draft, awaiting approval
**Target version:** falcon 0.4.0 → next
**Source spec:** `docs/falcon-requirements.md` (external workspace, copied here for reference)

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
  - **iOS:** grep the plugin's `ios/Classes/**/*.{m,swift}` for Apple-SDK API calls (e.g. `requestWhenInUseAuthorization`, `AVCaptureDevice`, `CNContactStore`, `PHPhotoLibrary`, `EKEventStore`). A small **Apple-SDK API → Info.plist key dictionary** (~30 entries, stable across Apple SDK versions, lives inside Falcon) translates each API to the required key.
  - **iOS deployment target:** the plugin's `*.podspec`'s `s.ios.deployment_target`. No dictionary.
- The scan output is written to `.falcon/plugin-requirements.yaml`. Refreshed when `pubspec.lock` mtime changes (or with `--refresh`). The file is committable so reviewers see what Falcon inferred.

This eliminates the maintenance burden of a 200-entry plugin map and means new plugins, plugin upgrades, and uncommon plugins all work without code changes. The ~30-entry Apple-SDK API map is stable Apple territory, not Flutter ecosystem churn.

## Architecture

### Module layout

Scaffolded in parallel so the CLI wiring + module skeletons land in one initial commit per command; detection logic fills in per command in subsequent commits.

```
src/
├── check_assets/
│   ├── mod.rs          # public API: pub fn run(root: &Path, format: OutputFormat) -> ExitCode
│   ├── pubspec.rs      # parse pubspec.yaml flutter.assets list
│   └── report.rs       # Issue struct + Reporter impl
├── check_a11y/
│   ├── mod.rs
│   ├── main_check.rs   # ensureSemantics() presence in main()
│   ├── widget_scan.rs  # interactive-widget count + Semantics(identifier:) coverage
│   └── report.rs
├── check_pods/
│   ├── mod.rs
│   ├── podfile.rs      # parse Podfile platform :ios, 'X.Y'
│   ├── podspec.rs      # scan ~/.pub-cache/**/ios/*.podspec for s.ios.deployment_target
│   └── report.rs
└── check_platform_deps/
    ├── mod.rs
    ├── plugin_scan.rs       # walk pub-cache for installed plugins
    ├── apple_api_map.rs     # ~30 Apple-SDK API → Info.plist key entries (stable)
    ├── android_manifest.rs  # parse <uses-permission> from plugin AND project manifests
    ├── ios_info_plist.rs    # parse keys from Info.plist
    ├── generated.rs         # read/write .falcon/plugin-requirements.yaml
    └── report.rs
```

Each module exposes `pub fn run(root: &Path, format: OutputFormat) -> anyhow::Result<i32>` (returns the exit code).

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
    pub rule_id: String,         // e.g. "platform-deps/missing-info-plist-key"
    pub severity: Severity,      // Error | Warning | Info
    pub title: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub plugin: Option<String>,  // populated for platform-deps issues
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
generated_at: 2026-05-25T12:34:56Z
pubspec_lock_sha: sha256:abc123…
plugins:
  location:
    version: 8.0.0
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
```

Refresh policy:
- If `.falcon/plugin-requirements.yaml` exists and `pubspec_lock_sha` matches the current `pubspec.lock`, reuse.
- Otherwise re-scan and overwrite. `--refresh` forces re-scan.

### Suppression

Existing falcon.yaml schema gains a `preflight:` section. Per-issue suppression by `rule_id` + optional `plugin` / `key` selector:

```yaml
preflight:
  check-platform-deps:
    suppress:
      - rule_id: platform-deps/missing-info-plist-key
        plugin: googleapis
        key: NSPhotoLibraryUsageDescription
        reason: "We never call ImagePicker; only googleapis OAuth."
  check-a11y:
    require_ensure_semantics: error      # error | warning | info | off
    interactive_semantics_coverage:
      threshold: 0.6                     # fraction of widgets that must have identifiers
      severity: warning
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

1. Read `pubspec.yaml`. Pull the `flutter.assets:` list (it can also be `flutter: { assets: [...] }`).
2. For each entry:
   - If the entry ends in `/`, resolve as a directory; require it exists; warn if empty.
   - Otherwise resolve as a file; require it exists.
3. Glob entries (`assets/icons/*.png`) expand and require ≥ 1 match.
4. Source location of the declaration: track the line number in pubspec.yaml so error output points the user at the right place.

### check-a11y

1. Parse `lib/main.dart` with the existing `DartParser`. Walk for the `main()` function. Check whether `SemanticsBinding.instance.ensureSemantics()` appears (and isn't inside a commented-out line). Emit error if missing.
2. Walk every `.dart` file under `lib/` (skip `*.g.dart` / `*.freezed.dart` / `test/`). For each interactive widget kind in `INTERACTIVE_WIDGETS` (see below), check whether one of the 3 nearest enclosing widgets is `Semantics(identifier: ...)`.
3. Compute `wrapped_count / total_count`. Compare against `interactive_semantics_coverage.threshold`. Emit warning if below.
4. For each unwrapped interactive widget, emit a Suggestion-severity issue with the recommended wrap pattern (per the spec's §4.4 decision matrix: TextField goes inside `Container` first, etc.).

`INTERACTIVE_WIDGETS = ["ElevatedButton", "TextButton", "OutlinedButton", "IconButton", "FloatingActionButton", "TextField", "TextFormField", "GestureDetector", "InkWell", "InkResponse", "DropdownButton", "DropdownButtonFormField", "Checkbox", "CheckboxListTile", "Switch", "SwitchListTile", "Radio", "RadioListTile"]`

### check-pods

1. Read `ios/Podfile`. Regex out `platform :ios, '(\d+\.\d+)'`. If absent, treat as 12.0 (CocoaPods default).
2. Walk `~/.pub-cache/hosted/pub.dev/<plugin>-<version>/ios/*.podspec` for every plugin in `pubspec.lock`.
3. From each podspec extract `s.ios.deployment_target = '(\d+\.\d+)'`. If absent, treat as 9.0 (CocoaPods default for iOS).
4. Compute `max(plugin_targets)`. If `podfile_target < max_plugin_target`, emit Error naming the binding plugin(s).
5. Same logic for `macos/Podfile` and `s.osx.deployment_target` if `macos/` exists.
6. (Stretch.) Mirror command structure for `check-gradle`: parse `android/app/build.gradle`'s `minSdkVersion` vs each plugin's `android/build.gradle` `minSdkVersion`. Out of scope for v1.

### check-platform-deps

The biggest command. Pipeline:

1. **Plugin scan.** Walk `pubspec.lock` for direct + transitive deps. For each, locate `~/.pub-cache/hosted/pub.dev/<plugin>-<version>/`. Skip plugins without iOS or Android folders (pure Dart).
2. **iOS extraction.** Grep the plugin's `ios/Classes/**/*.{m,swift}` for Apple-SDK API calls in `apple_api_map.rs`. Each hit records `(plugin, api, required_info_plist_key, source_file_in_plugin)`.
3. **Android extraction.** Parse the plugin's `android/src/main/AndroidManifest.xml` (the plugin's *own* template, not the user's) and pull every `<uses-permission android:name="..."/>` entry.
4. **Write `.falcon/plugin-requirements.yaml`.** Pubspec-lock SHA stamped.
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
  - Construct a minimal `pubspec.yaml` + `pubspec.lock` + fake `~/.pub-cache/<plugin>/...` files (override pub-cache root via env var `FALCON_PUB_CACHE_DIR` for testability).
  - Construct iOS/Android project structures with intentional misses.
  - Assert the right issues fire at the right severity.
- Integration test under `tests/preflight_tests.rs` that runs `falcon check-* <fixture>` and asserts exit codes.

## Acceptance criteria

Same as source spec §7:

1. On a project missing required Info.plist keys for `location`, `check-platform-deps` exits 2 with `NSLocationWhenInUseUsageDescription` named.
2. On a project with `.env*` entries declared but missing, `check-assets` exits 2 with all missing paths named.
3. On a project with a Podfile lacking `platform :ios, '14.0'` while a plugin requires 13.0, `check-pods` exits 2 naming the binding plugin.
4. On a project with `main()` missing `ensureSemantics()`, `check-a11y` exits 1 (warning) and reports 0% coverage.
5. After fixes, all four commands exit 0.
6. `falcon analyze` rollup matches `max(exit_codes)`.
7. `--format json` schema documented in `docs/preflight-json-schema.md` and stable.
8. Suppression via `falcon.yaml` works for at least one rule per command.

## Open questions

- **`.falcon/` directory convention.** Falcon already uses `.falcon-data/` for some artifacts. Standardize: use `.falcon/` for committable generated artifacts (this one), keep `.falcon-data/` for local-only state. Decide in implementation.
- **Plugin author opt-in.** Could plugin authors ship a `falcon-requirements.yaml` inside their package? Out of scope for v1 — revisit after dogfooding for 1-2 months.
- **Pub-cache location override.** Test fixtures need to point Falcon at a fake pub-cache. Either `FALCON_PUB_CACHE_DIR` env var or a `--pub-cache <path>` flag. Decide in implementation.

## Appendix A — Apple-SDK API → Info.plist key dictionary

Initial set (lives in `src/check_platform_deps/apple_api_map.rs`). Each entry: `{ api: &str, key: &str, framework: &str }`.

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
