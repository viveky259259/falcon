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

/// Build a Flutter-ish project that triggers errors in multiple pre-flight checks.
fn fixture_with_problems(tmp: &TempDir, cache: &TempDir) {
    // Explicitly opt in to the rollup (default is false in v0.5 soft rollout).
    write(
        &tmp.path().join("falcon.yaml"),
        "analyze:\n  preflight:\n    enabled: true\n",
    );
    write(
        &tmp.path().join("pubspec.yaml"),
        "name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n    - assets/missing.png\n",
    );
    write(
        &tmp.path().join("pubspec.lock"),
        "packages:\n  location:\n    source: hosted\n    version: \"8.0.0\"\n",
    );
    write(
        &tmp.path().join("lib/main.dart"),
        "void main() { runApp(MyApp()); }\n",
    );
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
    assert!(stdout.contains("check-assets"), "stdout: {stdout}");
    assert!(stdout.contains("check-a11y"), "stdout: {stdout}");
    assert!(stdout.contains("check-pods"), "stdout: {stdout}");
    assert!(stdout.contains("check-platform-deps"), "stdout: {stdout}");
    // check-assets fired on the missing asset
    assert!(
        stdout.contains("Missing Asset") || stdout.contains("assets/missing.png"),
        "check-assets did not flag the missing asset; stdout: {stdout}"
    );
    // check-platform-deps fired on the missing Info.plist key
    assert!(
        stdout.contains("NSLocationWhenInUseUsageDescription")
            || stdout.contains("missing-info-plist-key"),
        "check-platform-deps did not flag the missing Info.plist key; stdout: {stdout}"
    );
    // At least one preflight Error means exit code is 2 (max of analyze + preflight).
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn analyze_rollup_skip_list_omits_named_checks() {
    let tmp = TempDir::new().unwrap();
    let cache = TempDir::new().unwrap();
    fixture_with_problems(&tmp, &cache);
    write(
        &tmp.path().join("falcon.yaml"),
        "analyze:\n  preflight:\n    enabled: true\n    skip:\n      - check-pods\n      - check-platform-deps\n",
    );

    let output = Command::new(falcon_bin())
        .env("FALCON_PUB_CACHE", cache.path())
        .arg("analyze")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    let stdout = String::from_utf8_lossy(&output.stdout);
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
