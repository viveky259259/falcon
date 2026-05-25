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
