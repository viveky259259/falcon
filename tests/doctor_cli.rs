//! End-to-end checks on the `falcon doctor` CLI. These run the built binary
//! with `--dry-run`, so nothing is ever installed. Every test sets
//! `FALCON_DOCTOR_MANIFEST_FILE` to a local fixture (see `fetch_manifest` in
//! `src/doctor/mod.rs`), so the release manifest is read from disk instead of
//! curled — these tests are hermetic: no assertion here depends on network
//! access, and none pays the manifest fetch's `curl --max-time 20` in a
//! network-isolated sandbox.

use std::process::Command;
use tempfile::TempDir;

fn falcon() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_falcon"));
    cmd.env("FALCON_DOCTOR_MANIFEST_FILE", manifest_fixture());
    cmd
}

/// Absolute path to the manifest fixture — the fixture file `ReleaseManifest`
/// tests parse directly, reused here so both suites agree on what "the
/// manifest" contains: stable's channel head is 3.24.5, beta's is 3.27.0.
fn manifest_fixture() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/releases_macos.json"
    )
    .to_string()
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

/// A `falcon()` command with `PATH` narrowed to exclude any real `flutter`
/// binary, so the `flutter` check probes as `Missing` — and therefore
/// offers an `Automatic` fix — regardless of whether the machine actually
/// running this test suite has Flutter installed. Safe under `--dry-run`:
/// nothing in a dry run's `execute_plan` path shells out to anything this
/// narrowed `PATH` would need to find.
fn falcon_without_flutter_on_path() -> Command {
    let mut cmd = falcon();
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
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
    assert!(
        ids.contains(&"flutter".to_string()),
        "checks were {:?}",
        ids
    );
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
    assert!(
        !text.contains("Choice"),
        "JSON mode must never print a prompt"
    );
}

#[test]
fn only_flag_restricts_the_checks_that_run() {
    let dir = flutter_project();
    let out = falcon()
        .args([
            "doctor",
            "--dry-run",
            "--format",
            "json",
            "--only",
            "flutter",
        ])
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
        .args([
            "doctor",
            "--dry-run",
            "--format",
            "json",
            "--skip",
            "flutter",
        ])
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
    assert!(
        !ids.contains(&"flutter".to_string()),
        "checks were {:?}",
        ids
    );
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
    let out = falcon()
        .args(["doctor", "--help"])
        .output()
        .expect("falcon should run");
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in [
        "--fix",
        "--yes",
        "--dry-run",
        "--only",
        "--skip",
        "--channel",
        "--flutter-version",
        "--dir",
        "--format",
    ] {
        assert!(text.contains(flag), "help is missing {}", flag);
    }
}

#[test]
fn dry_run_resolves_the_fixtures_stable_channel_head_and_its_archive_url() {
    // With a known, hermetic manifest the tests can assert the actual
    // resolved version and archive URL, not merely that the JSON has the
    // right shape.
    let dir = flutter_project();
    let out = falcon_without_flutter_on_path()
        .args([
            "doctor",
            "--dry-run",
            "--format",
            "text",
            "--only",
            "flutter",
            "--channel",
            "stable",
            "--flutter-version",
            "latest",
            "--yes",
        ])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("3.24.5"),
        "must resolve `latest` on stable to the fixture's channel head (3.24.5): {}",
        text
    );
    assert!(
        text.contains(
            "https://storage.googleapis.com/flutter_infra_release/releases/stable/macos/flutter_macos_arm64_3.24.5-stable.zip"
        ) || text.contains(
            "https://storage.googleapis.com/flutter_infra_release/releases/stable/macos/flutter_macos_3.24.5-stable.zip"
        ),
        "must show the concrete archive URL for the resolved release: {}",
        text
    );
}

#[test]
fn dry_run_alone_previews_the_plan_without_requiring_fix() {
    // The confirm gate used to run even under `--dry-run`, and answering
    // anything but yes on a non-tty (every test harness) meant `--dry-run`
    // alone printed only "a fix is available ... run with --fix to apply" —
    // never the plan the README promises `--dry-run` shows.
    let dir = flutter_project();
    let out = falcon_without_flutter_on_path()
        .args([
            "doctor",
            "--dry-run",
            "--format",
            "text",
            "--only",
            "flutter",
        ])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Dry run") && text.contains("would run"),
        "`--dry-run` without `--fix` must still preview the plan: {}",
        text
    );
    assert!(
        !text.contains("run with --fix to apply"),
        "a dry run must not tell the user to pass --fix just to see the plan: {}",
        text
    );
}
