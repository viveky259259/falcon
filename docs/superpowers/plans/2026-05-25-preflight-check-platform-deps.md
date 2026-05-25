# Preflight: `falcon check-platform-deps` — Implementation Plan (PR 4 of 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Ship `falcon check-platform-deps`: read each plugin's source, derive required Info.plist keys (from Apple-SDK API calls) and Android permissions (from the plugin's own AndroidManifest.xml), diff against the project's `ios/Runner/Info.plist` and `android/app/src/main/AndroidManifest.xml`, and emit errors for missing keys/permissions. Cached in `.falcon/plugin-requirements.yaml` (refreshed on `pubspec.lock` SHA-256 change or `--refresh`).

**Architecture:** Composes the shared `preflight::pub_cache` (from PR3). New modules under `src/check_platform_deps/`: Apple-API map, iOS scanner, Android scanner, project-manifest reader, `.falcon/plugin-requirements.yaml` read/write, run orchestrator.

**Spec:** `docs/superpowers/specs/2026-05-25-preflight-checks-design.md` v2 (§check-platform-deps, Appendix A).

---

## File Structure

**Created:**
- `src/check_platform_deps/mod.rs` — orchestrator + rule_ids.
- `src/check_platform_deps/apple_api_map.rs` — `APPLE_API_MAP` const array (22 entries from spec Appendix A).
- `src/check_platform_deps/ios_scan.rs` — grep plugin's iOS sources for API substrings → required Info.plist keys.
- `src/check_platform_deps/android_scan.rs` — parse plugin's `<uses-permission>` list.
- `src/check_platform_deps/project_manifest.rs` — read user's Info.plist + AndroidManifest.xml.
- `src/check_platform_deps/generated.rs` — read/write `.falcon/plugin-requirements.yaml`.
- `tests/preflight_platform_deps_tests.rs` — integration test.

**Modified:**
- `src/lib.rs` — register `pub mod check_platform_deps;`.
- `src/main.rs` — `Commands::CheckPlatformDeps` variant + dispatch.

---

## Task 1: Apple-SDK API → Info.plist key map

**File:** Create `src/check_platform_deps/apple_api_map.rs`, `src/check_platform_deps/mod.rs` (stub), modify `src/lib.rs`.

- [ ] **Step 1: Create `src/check_platform_deps/apple_api_map.rs`**

```rust
//! Apple-SDK API → Info.plist key dictionary. Stable Apple territory (changes
//! rarely), so we ship this in source rather than maintaining a per-plugin map.

/// One entry: an Apple-SDK substring that, when found in a plugin's iOS source,
/// implies the plugin needs `info_plist_key` declared in the project's
/// Info.plist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppleApi {
    /// Substring Falcon greps for inside the plugin's `ios/` / `darwin/` sources.
    pub api: &'static str,
    /// The Info.plist key the API requires. Empty string means "no specific key
    /// needed" (capability-only API; we still record the hit for telemetry).
    pub info_plist_key: &'static str,
    /// The Apple framework (informational; surfaces in the report).
    pub framework: &'static str,
}

/// The v1 map. ~22 entries — derived from the spec's Appendix A.
pub const APPLE_API_MAP: &[AppleApi] = &[
    AppleApi { api: "requestWhenInUseAuthorization", info_plist_key: "NSLocationWhenInUseUsageDescription", framework: "CoreLocation" },
    AppleApi { api: "requestAlwaysAuthorization", info_plist_key: "NSLocationAlwaysAndWhenInUseUsageDescription", framework: "CoreLocation" },
    AppleApi { api: "AVCaptureDevice.requestAccess", info_plist_key: "NSCameraUsageDescription", framework: "AVFoundation" },
    AppleApi { api: "AVAudioSession.requestRecordPermission", info_plist_key: "NSMicrophoneUsageDescription", framework: "AVFoundation" },
    AppleApi { api: "PHPhotoLibrary.requestAuthorization", info_plist_key: "NSPhotoLibraryUsageDescription", framework: "Photos" },
    AppleApi { api: "PHAsset.creationRequestForAssetFromImage", info_plist_key: "NSPhotoLibraryAddUsageDescription", framework: "Photos" },
    AppleApi { api: "CNContactStore", info_plist_key: "NSContactsUsageDescription", framework: "Contacts" },
    AppleApi { api: "EKEventStore", info_plist_key: "NSCalendarsUsageDescription", framework: "EventKit" },
    AppleApi { api: "EKReminder", info_plist_key: "NSRemindersUsageDescription", framework: "EventKit" },
    AppleApi { api: "CMMotionManager", info_plist_key: "NSMotionUsageDescription", framework: "CoreMotion" },
    AppleApi { api: "HKHealthStore", info_plist_key: "NSHealthShareUsageDescription", framework: "HealthKit" },
    AppleApi { api: "CBCentralManager", info_plist_key: "NSBluetoothAlwaysUsageDescription", framework: "CoreBluetooth" },
    AppleApi { api: "MFMessageComposeViewController", info_plist_key: "", framework: "MessageUI" },
    AppleApi { api: "SFSpeechRecognizer.requestAuthorization", info_plist_key: "NSSpeechRecognitionUsageDescription", framework: "Speech" },
    AppleApi { api: "LAContext.canEvaluatePolicy", info_plist_key: "NSFaceIDUsageDescription", framework: "LocalAuthentication" },
    AppleApi { api: "MPMediaLibrary", info_plist_key: "NSAppleMusicUsageDescription", framework: "MediaPlayer" },
    AppleApi { api: "HMHomeManager", info_plist_key: "NSHomeKitUsageDescription", framework: "HomeKit" },
    AppleApi { api: "NEHotspotConfigurationManager", info_plist_key: "", framework: "NetworkExtension" },
    AppleApi { api: "UNUserNotificationCenter.requestAuthorization", info_plist_key: "", framework: "UserNotifications" },
    AppleApi { api: "CBPeripheralManager", info_plist_key: "NSBluetoothPeripheralUsageDescription", framework: "CoreBluetooth" },
    AppleApi { api: "CTCellularData", info_plist_key: "", framework: "CoreTelephony" },
    AppleApi { api: "INPreferences.requestSiriAuthorization", info_plist_key: "NSSiriUsageDescription", framework: "Intents" },
];

/// Look up entries whose `api` substring is contained in `source_text`.
pub fn find_apis_in_source(source_text: &str) -> Vec<&'static AppleApi> {
    APPLE_API_MAP
        .iter()
        .filter(|entry| !entry.api.is_empty() && source_text.contains(entry.api))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_has_22_entries() {
        assert_eq!(APPLE_API_MAP.len(), 22);
    }

    #[test]
    fn no_duplicate_api_substrings() {
        let mut seen = std::collections::HashSet::new();
        for entry in APPLE_API_MAP {
            assert!(seen.insert(entry.api), "duplicate API substring: {}", entry.api);
        }
    }

    #[test]
    fn find_apis_finds_location_when_use_authorization() {
        let src = "[CLLocationManager.shared requestWhenInUseAuthorization];";
        let hits = find_apis_in_source(src);
        assert!(hits
            .iter()
            .any(|e| e.info_plist_key == "NSLocationWhenInUseUsageDescription"));
    }

    #[test]
    fn find_apis_finds_camera_when_request_access() {
        let src = "AVCaptureDevice.requestAccess(for: .video) { granted in }";
        let hits = find_apis_in_source(src);
        assert!(hits.iter().any(|e| e.info_plist_key == "NSCameraUsageDescription"));
    }

    #[test]
    fn find_apis_finds_multiple_in_same_source() {
        let src = r#"
import CoreLocation
import Photos
[CLLocationManager.shared requestWhenInUseAuthorization];
PHPhotoLibrary.requestAuthorization { status in }
"#;
        let hits = find_apis_in_source(src);
        let keys: std::collections::HashSet<_> = hits.iter().map(|e| e.info_plist_key).collect();
        assert!(keys.contains("NSLocationWhenInUseUsageDescription"));
        assert!(keys.contains("NSPhotoLibraryUsageDescription"));
    }

    #[test]
    fn find_apis_empty_for_unrelated_source() {
        let src = "print(\"hello world\")";
        assert!(find_apis_in_source(src).is_empty());
    }

    #[test]
    fn entries_with_empty_key_still_found_but_no_plist_key() {
        let src = "MFMessageComposeViewController.canSendText();";
        let hits = find_apis_in_source(src);
        // Found, but the key is empty (capability-only).
        assert!(hits.iter().any(|e| e.api == "MFMessageComposeViewController"));
        assert!(hits.iter().any(|e| e.info_plist_key.is_empty()));
    }
}
```

- [ ] **Step 2: Create `src/check_platform_deps/mod.rs` (stub)**

```rust
//! `falcon check-platform-deps` — verify Info.plist / AndroidManifest.xml
//! declare every key/permission the project's plugins require.

pub mod apple_api_map;
```

- [ ] **Step 3: Register in `src/lib.rs`**

Add `pub mod check_platform_deps;` near `pub mod check_pods;`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib check_platform_deps::apple_api_map::tests 2>&1 | tail -12`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/check_platform_deps/apple_api_map.rs src/check_platform_deps/mod.rs src/lib.rs
git commit -m "feat(check-platform-deps): scaffold module + Apple API map

Adds APPLE_API_MAP (22 entries per spec Appendix A) mapping Apple-SDK
API substrings to required Info.plist keys, plus find_apis_in_source()
helper. The map is intentionally small and stable — Apple's frameworks
change rarely, unlike Flutter's plugin ecosystem.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: iOS scanner (grep plugin source for Apple APIs)

**File:** Create `src/check_platform_deps/ios_scan.rs`, modify `src/check_platform_deps/mod.rs`.

- [ ] **Step 1: Create `src/check_platform_deps/ios_scan.rs`**

```rust
//! Scan a plugin's iOS source files for Apple-SDK API substrings and report
//! the required Info.plist keys.

use super::apple_api_map::{find_apis_in_source, AppleApi};
use std::path::{Path, PathBuf};

/// One required Info.plist key derived from a plugin's iOS source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredInfoPlistKey {
    /// Apple-SDK API call that triggered this requirement.
    pub api: &'static str,
    /// The Info.plist key the project must declare.
    pub key: &'static str,
    /// Apple framework (informational).
    pub framework: &'static str,
    /// Path to the source file inside the plugin where the API was found.
    pub source_file: PathBuf,
}

/// Walk every `.m` / `.swift` file under `<plugin_root>/ios/` and
/// `<plugin_root>/darwin/` and collect the Info.plist-key requirements.
pub fn scan_ios_sources(plugin_root: &Path) -> Vec<RequiredInfoPlistKey> {
    let mut out = Vec::new();
    for sub in ["ios", "darwin"] {
        let dir = plugin_root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if ext != "m" && ext != "swift" && ext != "mm" && ext != "h" {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            for api in find_apis_in_source(&text) {
                if api.info_plist_key.is_empty() {
                    // Capability-only API; skip — no Info.plist key required.
                    continue;
                }
                out.push(RequiredInfoPlistKey {
                    api: api.api,
                    key: api.info_plist_key,
                    framework: api.framework,
                    source_file: path.to_path_buf(),
                });
            }
        }
    }
    dedupe(out)
}

fn dedupe(mut v: Vec<RequiredInfoPlistKey>) -> Vec<RequiredInfoPlistKey> {
    v.sort_by(|a, b| a.key.cmp(b.key).then(a.api.cmp(b.api)));
    v.dedup_by(|a, b| a.api == b.api && a.key == b.key);
    v
}

/// Convenience: collapse a `Vec<RequiredInfoPlistKey>` to the set of required
/// keys, dropping the per-file/per-API detail.
pub fn unique_keys(reqs: &[RequiredInfoPlistKey]) -> Vec<&'static str> {
    let mut set: Vec<&'static str> = reqs.iter().map(|r| r.key).collect();
    set.sort();
    set.dedup();
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn scan_ios_sources_no_ios_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(scan_ios_sources(tmp.path()).is_empty());
    }

    #[test]
    fn scan_ios_sources_finds_location_requirement() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/LocationPlugin.m"),
            "[CLLocationManager.shared requestWhenInUseAuthorization];",
        );
        let result = scan_ios_sources(tmp.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "NSLocationWhenInUseUsageDescription");
        assert!(result[0].source_file.to_string_lossy().ends_with("LocationPlugin.m"));
    }

    #[test]
    fn scan_ios_sources_walks_darwin_folder() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("darwin/Classes/CameraPlugin.swift"),
            "AVCaptureDevice.requestAccess(for: .video) { _ in }",
        );
        let result = scan_ios_sources(tmp.path());
        assert!(result.iter().any(|r| r.key == "NSCameraUsageDescription"));
    }

    #[test]
    fn scan_ios_sources_deduplicates_same_api_in_multiple_files() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/A.swift"),
            "AVCaptureDevice.requestAccess(for: .video) { _ in }",
        );
        write(
            &tmp.path().join("ios/Classes/B.swift"),
            "AVCaptureDevice.requestAccess(for: .audio) { _ in }",
        );
        let result = scan_ios_sources(tmp.path());
        // One required key, deduped despite appearing in two files.
        let cam = result
            .iter()
            .filter(|r| r.key == "NSCameraUsageDescription")
            .count();
        assert_eq!(cam, 1);
    }

    #[test]
    fn scan_ios_sources_collects_multiple_distinct_keys() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/Multi.swift"),
            r#"
[CLLocationManager.shared requestWhenInUseAuthorization];
AVCaptureDevice.requestAccess(for: .video) { _ in }
PHPhotoLibrary.requestAuthorization { _ in }
"#,
        );
        let keys = unique_keys(&scan_ios_sources(tmp.path()));
        assert!(keys.contains(&"NSLocationWhenInUseUsageDescription"));
        assert!(keys.contains(&"NSCameraUsageDescription"));
        assert!(keys.contains(&"NSPhotoLibraryUsageDescription"));
    }

    #[test]
    fn scan_ios_sources_ignores_non_source_extensions() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/README.md"),
            "AVCaptureDevice.requestAccess",
        );
        assert!(scan_ios_sources(tmp.path()).is_empty());
    }

    #[test]
    fn scan_ios_sources_picks_up_h_and_mm_files_too() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/Cam.h"),
            "AVCaptureDevice.requestAccess",
        );
        write(
            &tmp.path().join("ios/Classes/Cam.mm"),
            "[CLLocationManager.shared requestWhenInUseAuthorization];",
        );
        let keys = unique_keys(&scan_ios_sources(tmp.path()));
        assert!(keys.contains(&"NSCameraUsageDescription"));
        assert!(keys.contains(&"NSLocationWhenInUseUsageDescription"));
    }

    #[test]
    fn scan_ios_sources_capability_only_api_filtered_out() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/Sms.swift"),
            "MFMessageComposeViewController.canSendText()",
        );
        // No Info.plist key for MessageUI; result is empty.
        assert!(scan_ios_sources(tmp.path()).is_empty());
    }
}
```

- [ ] **Step 2: Register submodule**

Add `pub mod ios_scan;` to `src/check_platform_deps/mod.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib check_platform_deps::ios_scan::tests 2>&1 | tail -12`
Expected: 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/check_platform_deps/ios_scan.rs src/check_platform_deps/mod.rs
git commit -m "feat(check-platform-deps): iOS source scanner

Adds scan_ios_sources() walking <plugin>/ios/ and <plugin>/darwin/ for
.m/.mm/.h/.swift files and matching each against APPLE_API_MAP.
Collapses duplicates and skips capability-only APIs (those without an
Info.plist key).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Android scanner (parse plugin's AndroidManifest.xml)

**File:** Create `src/check_platform_deps/android_scan.rs`, modify mod.rs.

- [ ] **Step 1: Create `src/check_platform_deps/android_scan.rs`**

```rust
//! Read the plugin's own `android/src/main/AndroidManifest.xml` template and
//! extract the `<uses-permission>` block. The plugin's manifest IS the source
//! of truth — no dictionary needed for Android.

use std::path::Path;

/// Extract every `android:name` value from `<uses-permission .../>` tags in
/// the given AndroidManifest.xml content.
pub fn extract_uses_permissions(manifest_xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut remaining = manifest_xml;
    while let Some(idx) = remaining.find("<uses-permission") {
        let after = &remaining[idx..];
        let end = match after.find('>') {
            Some(e) => e,
            None => break,
        };
        let tag = &after[..=end];
        if let Some(name) = extract_attr(tag, "android:name") {
            out.push(name);
        }
        remaining = &after[end + 1..];
    }
    out.sort();
    out.dedup();
    out
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=");
    let idx = tag.find(&needle)?;
    let after = &tag[idx + needle.len()..];
    let first = after.chars().next()?;
    if first != '"' && first != '\'' {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find(first)?;
    Some(rest[..end].to_string())
}

/// Scan the plugin's own AndroidManifest.xml file and return the declared
/// `<uses-permission>` list. Returns empty if the file does not exist.
pub fn scan_plugin_manifest(plugin_root: &Path) -> Vec<String> {
    // Plugins put their manifest at android/src/main/AndroidManifest.xml.
    let manifest_path = plugin_root
        .join("android")
        .join("src")
        .join("main")
        .join("AndroidManifest.xml");
    let Ok(text) = std::fs::read_to_string(&manifest_path) else {
        return Vec::new();
    };
    extract_uses_permissions(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_uses_permissions_basic() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<manifest>
    <uses-permission android:name="android.permission.ACCESS_FINE_LOCATION"/>
    <uses-permission android:name="android.permission.ACCESS_COARSE_LOCATION" />
</manifest>"#;
        let perms = extract_uses_permissions(xml);
        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&"android.permission.ACCESS_FINE_LOCATION".to_string()));
        assert!(perms.contains(&"android.permission.ACCESS_COARSE_LOCATION".to_string()));
    }

    #[test]
    fn extract_uses_permissions_handles_single_quotes() {
        let xml = "<uses-permission android:name='android.permission.CAMERA'/>";
        let perms = extract_uses_permissions(xml);
        assert_eq!(perms, vec!["android.permission.CAMERA"]);
    }

    #[test]
    fn extract_uses_permissions_empty_when_no_tags() {
        let xml = "<manifest><application/></manifest>";
        assert!(extract_uses_permissions(xml).is_empty());
    }

    #[test]
    fn extract_uses_permissions_dedupes() {
        let xml = r#"
<uses-permission android:name="android.permission.CAMERA"/>
<uses-permission android:name="android.permission.CAMERA"/>
"#;
        let perms = extract_uses_permissions(xml);
        assert_eq!(perms.len(), 1);
    }

    #[test]
    fn scan_plugin_manifest_returns_permissions() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
<manifest>
  <uses-permission android:name="android.permission.RECORD_AUDIO"/>
</manifest>
"#;
        std::fs::create_dir_all(tmp.path().join("android/src/main")).unwrap();
        std::fs::write(
            tmp.path().join("android/src/main/AndroidManifest.xml"),
            manifest,
        )
        .unwrap();
        let perms = scan_plugin_manifest(tmp.path());
        assert_eq!(perms, vec!["android.permission.RECORD_AUDIO"]);
    }

    #[test]
    fn scan_plugin_manifest_no_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(scan_plugin_manifest(tmp.path()).is_empty());
    }
}
```

- [ ] **Step 2: Register**

Add `pub mod android_scan;` to `src/check_platform_deps/mod.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib check_platform_deps::android_scan::tests 2>&1 | tail -12`
Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/check_platform_deps/android_scan.rs src/check_platform_deps/mod.rs
git commit -m "feat(check-platform-deps): Android plugin manifest scanner

Adds scan_plugin_manifest() reading <plugin>/android/src/main/
AndroidManifest.xml and extract_uses_permissions() pulling every
android:name from <uses-permission> tags. Handles single/double quotes
and dedupes. The plugin's manifest IS the source of truth — no
dictionary needed for Android.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Project manifest reader

**File:** Create `src/check_platform_deps/project_manifest.rs`, modify mod.rs.

- [ ] **Step 1: Create `src/check_platform_deps/project_manifest.rs`**

```rust
//! Read the user's `ios/Runner/Info.plist` (XML form) and
//! `android/app/src/main/AndroidManifest.xml` to discover which keys / permissions
//! the project already declares.

use super::android_scan::extract_uses_permissions;
use std::collections::HashSet;
use std::path::Path;

/// Extract every `<key>...</key>` name from a plist's `<dict>` block.
/// We do not parse the plist proper — we just regex out `<key>NAME</key>`,
/// which is the only thing we care about (presence test).
pub fn extract_info_plist_keys(plist_xml: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut remaining = plist_xml;
    while let Some(idx) = remaining.find("<key>") {
        let after = &remaining[idx + 5..];
        let end = match after.find("</key>") {
            Some(e) => e,
            None => break,
        };
        out.insert(after[..end].trim().to_string());
        remaining = &after[end + 6..];
    }
    out
}

/// Read `<project>/ios/Runner/Info.plist` and return the set of `<key>` names
/// declared. Returns an empty set when the file is missing.
pub fn read_project_info_plist(project_root: &Path) -> HashSet<String> {
    let path = project_root.join("ios").join("Runner").join("Info.plist");
    match std::fs::read_to_string(&path) {
        Ok(text) => extract_info_plist_keys(&text),
        Err(_) => HashSet::new(),
    }
}

/// Read `<project>/android/app/src/main/AndroidManifest.xml` and return the
/// declared permissions. Empty set if the file is missing.
pub fn read_project_android_manifest(project_root: &Path) -> HashSet<String> {
    let path = project_root
        .join("android")
        .join("app")
        .join("src")
        .join("main")
        .join("AndroidManifest.xml");
    match std::fs::read_to_string(&path) {
        Ok(text) => extract_uses_permissions(&text).into_iter().collect(),
        Err(_) => HashSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_info_plist_keys_basic() {
        let plist = r#"<?xml version="1.0"?>
<plist><dict>
  <key>CFBundleName</key><string>App</string>
  <key>NSLocationWhenInUseUsageDescription</key><string>We need it</string>
</dict></plist>
"#;
        let keys = extract_info_plist_keys(plist);
        assert!(keys.contains("CFBundleName"));
        assert!(keys.contains("NSLocationWhenInUseUsageDescription"));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn extract_info_plist_keys_empty_for_empty_dict() {
        let plist = "<plist><dict></dict></plist>";
        assert!(extract_info_plist_keys(plist).is_empty());
    }

    #[test]
    fn read_project_info_plist_finds_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("ios/Runner")).unwrap();
        std::fs::write(
            tmp.path().join("ios/Runner/Info.plist"),
            "<plist><dict><key>NSCameraUsageDescription</key><string>Y</string></dict></plist>",
        )
        .unwrap();
        let keys = read_project_info_plist(tmp.path());
        assert!(keys.contains("NSCameraUsageDescription"));
    }

    #[test]
    fn read_project_info_plist_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(read_project_info_plist(tmp.path()).is_empty());
    }

    #[test]
    fn read_project_android_manifest_finds_permissions() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("android/app/src/main")).unwrap();
        std::fs::write(
            tmp.path().join("android/app/src/main/AndroidManifest.xml"),
            r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#,
        )
        .unwrap();
        let perms = read_project_android_manifest(tmp.path());
        assert!(perms.contains("android.permission.CAMERA"));
    }

    #[test]
    fn read_project_android_manifest_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(read_project_android_manifest(tmp.path()).is_empty());
    }
}
```

- [ ] **Step 2: Register**

Add `pub mod project_manifest;` to `src/check_platform_deps/mod.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib check_platform_deps::project_manifest::tests 2>&1 | tail -10`
Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/check_platform_deps/project_manifest.rs src/check_platform_deps/mod.rs
git commit -m "feat(check-platform-deps): project Info.plist + AndroidManifest readers

Adds read_project_info_plist() returning a HashSet<String> of <key>
names from ios/Runner/Info.plist, and read_project_android_manifest()
returning declared android:name permissions from
android/app/src/main/AndroidManifest.xml.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `.falcon/plugin-requirements.yaml` reader/writer

**File:** Create `src/check_platform_deps/generated.rs`, modify mod.rs.

- [ ] **Step 1: Create `src/check_platform_deps/generated.rs`**

```rust
//! Read and write the `.falcon/plugin-requirements.yaml` cache file.
//!
//! Cache key: SHA-256 of the contents of `pubspec.lock`. When the lock changes,
//! the cache is invalidated and rebuilt.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneratedRequirements {
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub pubspec_lock_sha256: String,
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginRequirements>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginRequirements {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosRequirements>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidRequirements>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IosRequirements {
    #[serde(default)]
    pub info_plist_keys: Vec<IosKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IosKey {
    pub key: String,
    pub api: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AndroidRequirements {
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Hex SHA-256 of the contents of `pubspec.lock` at `project_root`. Returns
/// an empty string if the file is missing.
pub fn pubspec_lock_sha256(project_root: &Path) -> String {
    let path = project_root.join("pubspec.lock");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let digest = Sha256::digest(&bytes);
            format!("{digest:x}")
        }
        Err(_) => String::new(),
    }
}

/// Path to the cache file inside the project.
pub fn cache_path(project_root: &Path) -> PathBuf {
    project_root.join(".falcon").join("plugin-requirements.yaml")
}

/// Read the cache if present and well-formed. Returns None on absence, parse
/// failure, or schema_version mismatch.
pub fn read_cache(project_root: &Path) -> Option<GeneratedRequirements> {
    let path = cache_path(project_root);
    let text = std::fs::read_to_string(&path).ok()?;
    let parsed: GeneratedRequirements = serde_yaml::from_str(&text).ok()?;
    if parsed.schema_version != SCHEMA_VERSION {
        return None;
    }
    Some(parsed)
}

/// Write the cache, creating `.falcon/` if needed.
pub fn write_cache(project_root: &Path, doc: &GeneratedRequirements) -> Result<()> {
    let path = cache_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Creating {}", parent.display()))?;
    }
    let yaml = serde_yaml::to_string(doc)?;
    std::fs::write(&path, yaml).with_context(|| format!("Writing {}", path.display()))?;
    Ok(())
}

/// Construct a fresh GeneratedRequirements with current timestamp + lock SHA.
pub fn new_empty_for(project_root: &Path) -> GeneratedRequirements {
    GeneratedRequirements {
        schema_version: SCHEMA_VERSION,
        generated_at: now_iso8601(),
        pubspec_lock_sha256: pubspec_lock_sha256(project_root),
        plugins: BTreeMap::new(),
    }
}

fn now_iso8601() -> String {
    // Avoid pulling chrono; use a minimal hand-rolled UTC stamp.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;
    format!(
        "1970-01-01T{:02}:{:02}:{:02}Z+{}d",
        hours % 24,
        mins % 60,
        secs % 60,
        days
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn pubspec_lock_sha256_empty_when_missing() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(pubspec_lock_sha256(tmp.path()), "");
    }

    #[test]
    fn pubspec_lock_sha256_stable_for_same_contents() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        std::fs::write(tmp1.path().join("pubspec.lock"), "abc\n").unwrap();
        std::fs::write(tmp2.path().join("pubspec.lock"), "abc\n").unwrap();
        let a = pubspec_lock_sha256(tmp1.path());
        let b = pubspec_lock_sha256(tmp2.path());
        assert_eq!(a, b);
        assert_eq!(a.len(), 64, "sha256 hex should be 64 chars");
    }

    #[test]
    fn read_cache_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(read_cache(tmp.path()).is_none());
    }

    #[test]
    fn write_then_read_round_trip() {
        let tmp = TempDir::new().unwrap();
        let mut doc = new_empty_for(tmp.path());
        doc.pubspec_lock_sha256 = "deadbeef".into();
        doc.plugins.insert(
            "location".into(),
            PluginRequirements {
                version: "8.0.0".into(),
                source: "hosted".into(),
                ios: Some(IosRequirements {
                    info_plist_keys: vec![IosKey {
                        key: "NSLocationWhenInUseUsageDescription".into(),
                        api: "requestWhenInUseAuthorization".into(),
                        source: "ios/Classes/Loc.m".into(),
                    }],
                }),
                android: None,
                skipped: None,
            },
        );
        write_cache(tmp.path(), &doc).unwrap();
        let round = read_cache(tmp.path()).unwrap();
        assert_eq!(round.pubspec_lock_sha256, "deadbeef");
        assert_eq!(round.plugins["location"].version, "8.0.0");
        assert_eq!(
            round.plugins["location"].ios.as_ref().unwrap().info_plist_keys[0].key,
            "NSLocationWhenInUseUsageDescription"
        );
    }

    #[test]
    fn read_cache_rejects_wrong_schema_version() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".falcon")).unwrap();
        std::fs::write(
            cache_path(tmp.path()),
            "schema_version: 999\ngenerated_at: '2026'\n",
        )
        .unwrap();
        assert!(read_cache(tmp.path()).is_none());
    }
}
```

NOTE: This task uses `sha2`. Check `Cargo.toml` — if `sha2` is not yet a dep, add it: `sha2 = "0.10"`. If it's already there, skip the Cargo.toml change.

- [ ] **Step 2: Check Cargo.toml**

```bash
grep "^sha2" Cargo.toml
```
If empty, add `sha2 = "0.10"` to `[dependencies]` in Cargo.toml.

- [ ] **Step 3: Register**

Add `pub mod generated;` to `src/check_platform_deps/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib check_platform_deps::generated::tests 2>&1 | tail -10`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/check_platform_deps/generated.rs src/check_platform_deps/mod.rs Cargo.toml Cargo.lock
git commit -m "feat(check-platform-deps): .falcon/plugin-requirements.yaml read/write

Adds GeneratedRequirements (schema_version 1, pubspec_lock_sha256, plugins
map) serializable to/from YAML, pubspec_lock_sha256() helper using SHA-256
of pubspec.lock contents, and read_cache/write_cache that gate on
schema_version match.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `check_platform_deps::run` orchestrator

**File:** Modify `src/check_platform_deps/mod.rs` (replace contents).

- [ ] **Step 1: Replace `src/check_platform_deps/mod.rs` with:**

```rust
//! `falcon check-platform-deps` — verify Info.plist / AndroidManifest.xml
//! declare every key/permission the project's plugins require.

pub mod android_scan;
pub mod apple_api_map;
pub mod generated;
pub mod ios_scan;
pub mod project_manifest;

use crate::config::{FalconConfig, Severity};
use crate::preflight::pub_cache::{iter_installed_plugins, locate_pub_cache, InstalledPlugin};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue, TargetPlatform};
use anyhow::Result;
use std::path::{Path, PathBuf};

const RULE_ID_MISSING_INFO_PLIST_KEY: &str = "platform-deps/missing-info-plist-key";
const RULE_ID_MISSING_ANDROID_PERMISSION: &str = "platform-deps/missing-android-permission";
const RULE_ID_NON_PUB_DEV_SKIPPED: &str = "platform-deps/non-pub-dev-skipped";

pub fn run(
    root: &Path,
    format: OutputFormat,
    config: &FalconConfig,
    refresh: bool,
    platform: Option<TargetPlatform>,
) -> Result<i32> {
    let pub_cache = locate_pub_cache();
    let installed = iter_installed_plugins(root, &pub_cache).unwrap_or_default();

    // Decide whether to use the cache.
    let lock_sha = generated::pubspec_lock_sha256(root);
    let cached = if refresh {
        None
    } else {
        generated::read_cache(root).filter(|c| c.pubspec_lock_sha256 == lock_sha && !lock_sha.is_empty())
    };

    let doc = match cached {
        Some(c) => c,
        None => {
            let fresh = scan_all(&installed, root);
            // Best-effort write; ignore errors so unwritable .falcon/ doesn't
            // block analysis (e.g. read-only sandbox).
            let _ = generated::write_cache(root, &fresh);
            fresh
        }
    };

    // Project-side keys/permissions.
    let project_keys = project_manifest::read_project_info_plist(root);
    let project_perms = project_manifest::read_project_android_manifest(root);

    let mut issues: Vec<PreflightIssue> = Vec::new();
    let want_ios = matches!(platform, None | Some(TargetPlatform::Ios) | Some(TargetPlatform::Both));
    let want_android = matches!(platform, None | Some(TargetPlatform::Android) | Some(TargetPlatform::Both));

    for (name, plugin) in &doc.plugins {
        if let Some(reason) = &plugin.skipped {
            issues.push(PreflightIssue {
                rule_id: RULE_ID_NON_PUB_DEV_SKIPPED.into(),
                severity: Severity::Info,
                title: "Plugin skipped (non-pub.dev source)".into(),
                file: None,
                line: None,
                plugin: Some(name.clone()),
                message: format!("Plugin `{name}` skipped: {reason}"),
                suggestion: None,
            });
            continue;
        }

        if want_ios {
            if let Some(ios) = &plugin.ios {
                for key in &ios.info_plist_keys {
                    if !project_keys.contains(&key.key) {
                        issues.push(PreflightIssue {
                            rule_id: RULE_ID_MISSING_INFO_PLIST_KEY.into(),
                            severity: Severity::Error,
                            title: "Missing Required Info.plist Key".into(),
                            file: Some(PathBuf::from("ios/Runner/Info.plist")),
                            line: None,
                            plugin: Some(format!("{} {}", name, plugin.version)),
                            message: format!(
                                "Plugin `{name}` calls `{}` which requires <key>{}</key> in Info.plist (found in {}).",
                                key.api, key.key, key.source
                            ),
                            suggestion: Some(format!(
                                "Add <key>{}</key><string>...</string> to ios/Runner/Info.plist.",
                                key.key
                            )),
                        });
                    }
                }
            }
        }
        if want_android {
            if let Some(android) = &plugin.android {
                for perm in &android.permissions {
                    if !project_perms.contains(perm) {
                        issues.push(PreflightIssue {
                            rule_id: RULE_ID_MISSING_ANDROID_PERMISSION.into(),
                            severity: Severity::Error,
                            title: "Missing Android Permission".into(),
                            file: Some(PathBuf::from("android/app/src/main/AndroidManifest.xml")),
                            line: None,
                            plugin: Some(format!("{} {}", name, plugin.version)),
                            message: format!(
                                "Plugin `{name}` declares <uses-permission android:name=\"{perm}\"/> in its own manifest, but the project's manifest does not."
                            ),
                            suggestion: Some(format!(
                                "Add <uses-permission android:name=\"{perm}\"/> to android/app/src/main/AndroidManifest.xml."
                            )),
                        });
                    }
                }
            }
        }
    }

    issues.retain(|issue| !is_suppressed(issue, config));
    let exit = exit_code_for_issues(&issues);
    print!("{}", reporter::render(&issues, format));
    Ok(exit)
}

fn scan_all(installed: &[InstalledPlugin], _root: &Path) -> generated::GeneratedRequirements {
    let mut doc = generated::GeneratedRequirements {
        schema_version: 1,
        generated_at: String::new(),
        pubspec_lock_sha256: String::new(),
        plugins: Default::default(),
    };
    doc.generated_at = "generated".into();
    for plugin in installed {
        if plugin.source != "hosted" {
            doc.plugins.insert(
                plugin.name.clone(),
                generated::PluginRequirements {
                    version: plugin.version.clone(),
                    source: plugin.source.clone(),
                    ios: None,
                    android: None,
                    skipped: Some("non-pub.dev source — Falcon cannot infer requirements".into()),
                },
            );
            continue;
        }
        let Some(plugin_root) = &plugin.root else { continue };
        let ios_reqs = ios_scan::scan_ios_sources(plugin_root);
        let ios = if ios_reqs.is_empty() {
            None
        } else {
            Some(generated::IosRequirements {
                info_plist_keys: ios_reqs
                    .iter()
                    .map(|r| generated::IosKey {
                        key: r.key.to_string(),
                        api: r.api.to_string(),
                        source: r
                            .source_file
                            .strip_prefix(plugin_root)
                            .unwrap_or(&r.source_file)
                            .display()
                            .to_string(),
                    })
                    .collect(),
            })
        };
        let android_perms = android_scan::scan_plugin_manifest(plugin_root);
        let android = if android_perms.is_empty() {
            None
        } else {
            Some(generated::AndroidRequirements {
                permissions: android_perms,
            })
        };
        doc.plugins.insert(
            plugin.name.clone(),
            generated::PluginRequirements {
                version: plugin.version.clone(),
                source: "hosted".into(),
                ios,
                android,
                skipped: None,
            },
        );
    }
    doc
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
    use std::sync::{Mutex, MutexGuard};
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());
    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fixture() -> (TempDir, TempDir, MutexGuard<'static, ()>) {
        let guard = env_lock();
        let project = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        std::env::set_var("FALCON_PUB_CACHE", pub_cache.path());
        (project, pub_cache, guard)
    }

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    /// Build a faux hosted plugin under the pub-cache with optional iOS + Android contents.
    fn write_plugin(
        pub_cache: &Path,
        name: &str,
        version: &str,
        ios_source: Option<(&str, &str)>,
        android_manifest_xml: Option<&str>,
    ) {
        let plugin_root = pub_cache
            .join("hosted/pub.dev")
            .join(format!("{name}-{version}"));
        if let Some((filename, src)) = ios_source {
            write(&plugin_root.join("ios/Classes").join(filename), src);
        }
        if let Some(xml) = android_manifest_xml {
            write(&plugin_root.join("android/src/main/AndroidManifest.xml"), xml);
        }
    }

    fn write_lock(project: &Path, packages: &[(&str, &str, &str)]) {
        let mut lock = String::from("packages:\n");
        for (name, version, source) in packages {
            lock.push_str(&format!("  {name}:\n    source: {source}\n    version: \"{version}\"\n"));
        }
        write(&project.join("pubspec.lock"), &lock);
    }

    #[test]
    fn empty_project_exits_zero() {
        let (project, _cache, _guard) = fixture();
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn ios_missing_key_exits_two() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("location", "8.0.0", "hosted")]);
        write_plugin(
            cache.path(),
            "location",
            "8.0.0",
            Some((
                "Location.m",
                "[CLLocationManager.shared requestWhenInUseAuthorization];",
            )),
            None,
        );
        // No Info.plist on project side.
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn ios_key_present_in_project_exits_zero() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("location", "8.0.0", "hosted")]);
        write_plugin(
            cache.path(),
            "location",
            "8.0.0",
            Some((
                "Location.m",
                "[CLLocationManager.shared requestWhenInUseAuthorization];",
            )),
            None,
        );
        write(
            &project.path().join("ios/Runner/Info.plist"),
            r#"<plist><dict>
  <key>NSLocationWhenInUseUsageDescription</key><string>Y</string>
</dict></plist>"#,
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn android_missing_permission_exits_two() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("camera", "0.10.0", "hosted")]);
        write_plugin(
            cache.path(),
            "camera",
            "0.10.0",
            None,
            Some(
                r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#,
            ),
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn android_permission_present_in_project_exits_zero() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("camera", "0.10.0", "hosted")]);
        write_plugin(
            cache.path(),
            "camera",
            "0.10.0",
            None,
            Some(
                r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#,
            ),
        );
        write(
            &project.path().join("android/app/src/main/AndroidManifest.xml"),
            r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#,
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn non_pub_dev_plugin_emits_info_only() {
        let (project, _cache, _guard) = fixture();
        write_lock(project.path(), &[("git_plugin", "0.0.0", "git")]);
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0); // Info-only → 0
    }

    #[test]
    fn platform_filter_ios_only_skips_android_checks() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("camera", "0.10.0", "hosted")]);
        write_plugin(
            cache.path(),
            "camera",
            "0.10.0",
            None,
            Some(
                r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#,
            ),
        );
        // platform = ios → android check skipped → exit 0
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, Some(TargetPlatform::Ios)).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn cache_file_written_to_falcon_dir() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("location", "8.0.0", "hosted")]);
        write_plugin(
            cache.path(),
            "location",
            "8.0.0",
            Some(("Location.m", "no-api-here")),
            None,
        );
        let _ = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        let cache_file = project.path().join(".falcon/plugin-requirements.yaml");
        assert!(cache_file.exists(), "expected .falcon/plugin-requirements.yaml");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib check_platform_deps::tests 2>&1 | tail -15`
Expected: 8 tests pass.

- [ ] **Step 3: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add src/check_platform_deps/mod.rs
git commit -m "feat(check-platform-deps): implement run() orchestrator

Wires Apple API scan, Android manifest scan, project-manifest readers,
.falcon/plugin-requirements.yaml caching, suppression, and platform
filter. Errors when an Info.plist key or Android permission required
by a plugin is missing from the project. Info-only entries for
non-pub.dev plugins (git/path sources).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: CLI wiring

**File:** Modify `src/main.rs`.

- [ ] **Step 1: Add variant**

```rust
    /// Verify Info.plist / AndroidManifest.xml declare every key the project's plugins require.
    CheckPlatformDeps {
        /// Path to the Flutter project.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Re-scan plugins even if .falcon/plugin-requirements.yaml is current.
        #[arg(long)]
        refresh: bool,
        /// Restrict scan to one platform.
        #[arg(long, value_enum)]
        platform: Option<falcon::preflight::TargetPlatform>,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: PreflightOutputFormat,
    },
```

- [ ] **Step 2: Add dispatch arm**

```rust
        Commands::CheckPlatformDeps { path, refresh, platform, format } => {
            let config = falcon::config::FalconConfig::load(&path).unwrap_or_default();
            let code = falcon::check_platform_deps::run(&path, format, &config, refresh, platform)?;
            process::exit(code);
        }
```

- [ ] **Step 3: Build + smoke test**

Run: `cargo build 2>&1 | tail -5` — clean.
Run: `cargo run --bin falcon -- check-platform-deps --help 2>&1 | head -18` — expect help text.

- [ ] **Step 4: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): wire Commands::CheckPlatformDeps

Adds the \`falcon check-platform-deps [path] [--refresh] [--platform ios|android|both] [--format text|json|sarif]\`
subcommand that delegates to falcon::check_platform_deps::run().

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Integration test

**File:** Create `tests/preflight_platform_deps_tests.rs`.

- [ ] **Step 1: Create the file:**

```rust
//! End-to-end test for `falcon check-platform-deps`.

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
fn check_platform_deps_exits_two_on_missing_info_plist_key() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.lock"),
        "packages:\n  location:\n    source: hosted\n    version: \"8.0.0\"\n",
    );
    write(
        &cache
            .path()
            .join("hosted/pub.dev/location-8.0.0/ios/Classes/Loc.m"),
        "[CLLocationManager.shared requestWhenInUseAuthorization];",
    );
    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("check-platform-deps")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("NSLocationWhenInUseUsageDescription"),
        "stdout: {stdout}"
    );
}

#[test]
fn check_platform_deps_exits_zero_when_key_present() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.lock"),
        "packages:\n  location:\n    source: hosted\n    version: \"8.0.0\"\n",
    );
    write(
        &cache
            .path()
            .join("hosted/pub.dev/location-8.0.0/ios/Classes/Loc.m"),
        "[CLLocationManager.shared requestWhenInUseAuthorization];",
    );
    write(
        &tmp.path().join("ios/Runner/Info.plist"),
        r#"<plist><dict>
  <key>NSLocationWhenInUseUsageDescription</key><string>Y</string>
</dict></plist>"#,
    );
    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("check-platform-deps")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn check_platform_deps_json_format() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    write(
        &tmp.path().join("pubspec.lock"),
        "packages:\n  location:\n    source: hosted\n    version: \"8.0.0\"\n",
    );
    write(
        &cache
            .path()
            .join("hosted/pub.dev/location-8.0.0/ios/Classes/Loc.m"),
        "[CLLocationManager.shared requestWhenInUseAuthorization];",
    );
    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("check-platform-deps")
        .arg(tmp.path())
        .args(["--format", "json"])
        .output()
        .expect("failed to execute falcon");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["schema_version"], serde_json::json!(1));
    assert!(parsed["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["rule_id"] == "platform-deps/missing-info-plist-key"));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test preflight_platform_deps_tests 2>&1 | tail -8`
Expected: 3 tests pass.

- [ ] **Step 3: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add tests/preflight_platform_deps_tests.rs
git commit -m "test(check-platform-deps): integration test against the CLI binary

Three e2e tests: missing-info-plist-key (exit 2 + stdout substring),
present-info-plist-key (exit 0), and --format json validity.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Finalize + push

- [ ] **Smoke test on falcon_dart**

Run: `cargo run --quiet --bin falcon -- check-platform-deps falcon_dart 2>&1 | tail -10`
Expected: exit 0 (no plugins, no ios/, no android/ → nothing to flag).

- [ ] **Push**

Run: `git push 2>&1 | tail -3`

- [ ] **Summary**

PR4 (check-platform-deps) complete. Branch ahead of origin by ~14 commits.
