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

/// A `falcon()` command that deliberately does *not* set
/// `FALCON_DOCTOR_MANIFEST_FILE` and narrows `PATH` to exclude any real
/// `flutter` binary. Used only to prove `--offline` skips the manifest
/// fetch entirely (see `offline_never_reads_the_manifest_and_degrades_to_manual`)
/// — if `--offline` were a no-op, this process would either read a manifest
/// it was never told about (impossible — it isn't set) or try to curl the
/// real network, which is exactly what these tests must never do.
fn falcon_offline_without_flutter_on_path() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_falcon"));
    cmd.env("PATH", "/usr/bin:/bin");
    cmd.env_remove("FALCON_DOCTOR_MANIFEST_FILE");
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

// ─── FINDING 1: --only/--skip must name real checks ────────────────────────

#[test]
fn an_unknown_only_name_is_a_loud_error_not_an_empty_healthy_report() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--only", "bogus"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    assert!(
        !out.status.success(),
        "an unknown --only name must exit non-zero"
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "must exit non-zero on the error path"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty(),
        "must not print a report table for a typo'd check name: {}",
        stdout
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bogus"),
        "error must name the offender: {}",
        stderr
    );
    assert!(
        stderr.contains("flutter"),
        "error must list the valid checks: {}",
        stderr
    );
}

#[test]
fn an_unknown_skip_name_is_also_a_loud_error() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--skip", "not-a-real-check"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not-a-real-check"), "stderr: {}", stderr);
}

// ─── FINDING 2: a nonexistent path must never produce a confident report ───

#[test]
fn a_nonexistent_path_is_a_loud_error_not_a_confident_report() {
    let out = falcon()
        .args(["doctor", "--dry-run"])
        .arg("/no/such/path/falcon-doctor-ux-test")
        .output()
        .expect("falcon should run");
    assert!(
        !out.status.success(),
        "a nonexistent path must exit non-zero"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty(),
        "must not print a report for a path that was never probed: {}",
        stdout
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not exist"),
        "error must be clear about what's wrong: {}",
        stderr
    );
}

#[test]
fn a_directory_without_pubspec_yaml_still_reports_but_warns_loudly() {
    let dir = TempDir::new().unwrap(); // deliberately no pubspec.yaml
    let out = falcon()
        .args(["doctor", "--dry-run", "--format", "text"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("pubspec.yaml"),
        "a non-Dart directory must be called out prominently, not silently \
         reported as if it were a normal project: {}",
        text
    );
    // It still runs the checks — this is a warning, not a hard refusal, so
    // real information ("flutter is missing") still reaches the user.
    assert!(
        text.contains("flutter"),
        "the report must still run despite the warning: {}",
        text
    );
}

#[test]
fn json_mode_also_carries_the_missing_pubspec_warning() {
    let dir = TempDir::new().unwrap();
    let out = falcon()
        .args(["doctor", "--dry-run", "--format", "json"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let warning = json["project_warning"]
        .as_str()
        .expect("project_warning must be present as a string in JSON mode");
    assert!(warning.contains("pubspec.yaml"), "warning: {}", warning);
}

#[test]
fn a_real_flutter_project_carries_no_project_warning() {
    let dir = flutter_project();
    let out = falcon()
        .args(["doctor", "--dry-run", "--format", "json"])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert!(
        json.get("project_warning").is_none(),
        "a real Dart project must not carry the warning: {}",
        json
    );
}

// ─── FINDING 3: --channel must reject anything but stable/beta/master ──────

#[test]
fn an_unknown_channel_is_rejected_by_clap_before_anything_runs() {
    let dir = flutter_project();
    let out = falcon()
        .args([
            "doctor",
            "--dry-run",
            "--fix",
            "--yes",
            "--only",
            "flutter",
            "--channel",
            "nightly",
        ])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    assert!(
        !out.status.success(),
        "an unknown channel must be refused, not baked into a git-clone plan"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("nightly"), "stderr: {}", stderr);
    assert!(
        stderr.contains("stable") && stderr.contains("beta") && stderr.contains("master"),
        "clap must list the valid channels: {}",
        stderr
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("git clone"),
        "an unknown channel must never reach the plan preview: {}",
        stdout
    );
}

#[test]
fn known_channels_are_still_accepted() {
    // Whether the machine running this test happens to have a working
    // Flutter SDK on PATH is irrelevant here — a `Missing` status still
    // exits non-zero even for a perfectly valid `--channel`. What this
    // test actually checks is that clap's `ValueEnum` never rejects a real
    // channel name (no "invalid value" parse error on stderr) and that the
    // report still prints.
    let dir = flutter_project();
    for channel in ["stable", "beta", "master"] {
        let out = falcon()
            .args([
                "doctor",
                "--dry-run",
                "--format",
                "text",
                "--only",
                "flutter",
                "--channel",
                channel,
            ])
            .arg(dir.path())
            .output()
            .expect("falcon should run");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !stderr.contains("invalid value"),
            "--channel {} must be accepted by clap: {}",
            channel,
            stderr
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("Falcon doctor"),
            "the report must still print for --channel {}: {}",
            channel,
            stdout
        );
    }
}

// ─── FINDING 5: --offline must never touch the network ─────────────────────

#[test]
fn offline_never_reads_the_manifest_and_degrades_to_manual() {
    let dir = flutter_project();
    let out = falcon_offline_without_flutter_on_path()
        .args([
            "doctor",
            "--offline",
            "--dry-run",
            "--format",
            "json",
            "--only",
            "flutter",
        ])
        .arg(dir.path())
        .output()
        .expect("falcon should run");
    assert!(
        out.status.success() || out.status.code() == Some(2),
        "process must complete without hanging on the network: {:?}",
        out.status
    );
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must still be valid JSON");
    let checks = json["checks"].as_array().expect("checks array");
    let flutter = checks
        .iter()
        .find(|c| c["id"] == "flutter")
        .expect("flutter check present");
    assert_eq!(
        flutter["fix"]["kind"], "manual",
        "without a manifest (because --offline skipped fetching it), the fix \
         must degrade to manual rather than pretend it knows what to install: {}",
        flutter
    );
}
