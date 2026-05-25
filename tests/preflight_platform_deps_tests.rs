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
