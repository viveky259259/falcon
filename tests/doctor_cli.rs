//! End-to-end checks on the `falcon doctor` CLI. These run the built binary
//! with `--dry-run`, so nothing is ever installed, and no assertion here
//! depends on the Flutter release manifest's contents — so these tests pass
//! offline. `--dry-run` still attempts a manifest fetch (the preview needs
//! it to resolve a concrete version and URL), so a network-isolated run can
//! pay up to the fetch's `curl --max-time 20` per test before falling back.

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
