#![cfg(unix)]

use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new(events: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("integration_test")).unwrap();
        fs::write(root.path().join("pubspec.yaml"), "name: fixture\n").unwrap();
        fs::write(
            root.path().join("integration_test/flow_test.dart"),
            "void main() {}\n",
        )
        .unwrap();
        fs::write(root.path().join("events"), events).unwrap();
        fs::write(
            root.path().join("devices"),
            json!([{
                "id": "phone with space", "name": "Test phone", "isSupported": true,
                "targetPlatform": "android-arm64", "emulator": false
            }])
            .to_string(),
        )
        .unwrap();
        Self { root }
    }

    fn script(&self, body: &str) {
        let path = self.root.path().join("flutter-fixture");
        fs::write(
            &path,
            format!("#!/bin/sh\ncd '{}'\n{body}\n", self.root.path().display()),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn normal_script(&self) {
        self.script("if [ \"$1\" = devices ]; then cat devices; exit 0; fi\nprintf '%s\\n' \"$@\" > args\ncat events\nexit 0");
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_falcon"));
        command.current_dir(self.root.path()).args([
            "x",
            "verify",
            ".",
            "--target",
            "integration_test/flow_test.dart",
            "--device",
            "phone with space",
            "--timeout",
            "2",
            "--output-dir",
            "proof",
            "--flutter",
            "./flutter-fixture",
        ]);
        command
    }

    fn report(&self) -> Value {
        serde_json::from_slice(&fs::read(self.root.path().join("proof/summary.json")).unwrap())
            .unwrap()
    }
}

fn passing_events() -> String {
    [
        json!({"type":"start","protocolVersion":"0.1.1"}),
        json!({"type":"testStart","test":{"id":1,"name":"scan badge"}}),
        json!({"type":"testDone","testID":1,"result":"success","hidden":false,"skipped":false}),
        json!({"type":"done","success":true}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn verify_runs_the_exact_target_device_and_preserves_evidence() {
    let fixture = Fixture::new(&passing_events());
    fixture.normal_script();
    let output = fixture.command().output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fixture.report();
    assert_eq!(report["status"], "passed");
    assert_eq!(report["tests"]["passed"], 1);
    assert_eq!(report["device"]["id"], "phone with space");
    assert_eq!(report["environment"], "physical_mobile");
    assert_eq!(report["native_claim"], "requires_review_of_test_assertions");
    assert_eq!(
        fs::read_to_string(fixture.root.path().join("args")).unwrap(),
        "test\nintegration_test/flow_test.dart\n-d\nphone with space\n--machine\n"
    );
    assert!(fixture
        .root
        .path()
        .join("proof/test-events.jsonl")
        .is_file());
    assert!(fixture.root.path().join("proof/test-stderr.log").is_file());
}

#[test]
fn verify_rejects_missing_or_unsupported_device_without_testing() {
    for device in [
        json!([]),
        json!([{"id":"phone with space","isSupported":false,"targetPlatform":"android-arm64","emulator":false,"name":"phone"}]),
    ] {
        let fixture = Fixture::new(&passing_events());
        fixture.normal_script();
        fs::write(fixture.root.path().join("devices"), device.to_string()).unwrap();
        let output = fixture.command().output().unwrap();
        assert!(!output.status.success());
        assert!(!fixture.root.path().join("args").exists());
        assert_eq!(fixture.report()["status"], "blocked");
    }
}

#[test]
fn verify_never_accepts_empty_skipped_failed_or_incomplete_runs() {
    let pass = passing_events();
    for events in [
        String::new(),
        pass.replace("\"skipped\":false", "\"skipped\":true"),
        pass.replace("\"result\":\"success\"", "\"result\":\"failure\""),
        pass.lines().take(3).collect::<Vec<_>>().join("\n"),
        pass.replace("\"success\":true", "\"success\":false"),
        format!("{pass}\n{{broken json"),
        format!(
            "{pass}\n{}",
            json!({"type":"error","testID":1,"error":"late failure"})
        ),
        pass.replace("\"hidden\":false", "\"hidden\":true"),
        pass.replace("\"testID\":1", "\"testID\":999"),
    ] {
        let fixture = Fixture::new(&events);
        fixture.normal_script();
        let output = fixture.command().output().unwrap();
        assert!(!output.status.success(), "accepted {events}");
        assert_eq!(fixture.report()["status"], "failed");
    }
}

#[test]
fn verify_nonzero_exit_overrides_passing_events() {
    let fixture = Fixture::new(&passing_events());
    fixture.script("if [ \"$1\" = devices ]; then cat devices; exit 0; fi\ncat events\nexit 3");
    assert!(!fixture.command().output().unwrap().status.success());
    assert_eq!(fixture.report()["exit_code"], 3);
    assert_eq!(fixture.report()["status"], "failed");
}

#[test]
fn verify_timeout_kills_owned_descendants_and_keeps_diagnostics() {
    let fixture = Fixture::new(&passing_events());
    fixture.script("if [ \"$1\" = devices ]; then cat devices; exit 0; fi\necho started\n(sleep 4; touch survived) &\nwait");
    assert!(!fixture.command().output().unwrap().status.success());
    assert_eq!(fixture.report()["status"], "timed_out");
    assert!(
        fs::read_to_string(fixture.root.path().join("proof/test-events.jsonl"))
            .unwrap()
            .contains("started")
    );
    std::thread::sleep(std::time::Duration::from_secs(3));
    assert!(!fixture.root.path().join("survived").exists());
}

#[test]
fn verify_blocks_before_execution_for_invalid_target_and_report_directory() {
    let fixture = Fixture::new(&passing_events());
    fixture.normal_script();
    fs::remove_file(fixture.root.path().join("integration_test/flow_test.dart")).unwrap();
    assert!(!fixture.command().output().unwrap().status.success());
    assert!(!fixture.root.path().join("args").exists());
    let fixture = Fixture::new(&passing_events());
    fixture.normal_script();
    fs::write(fixture.root.path().join("proof"), "not a directory").unwrap();
    assert!(!fixture.command().output().unwrap().status.success());
    assert!(!fixture.root.path().join("args").exists());
}

#[test]
fn verify_output_limit_is_failure_not_a_truncated_pass() {
    let fixture = Fixture::new(&passing_events());
    fixture.script("if [ \"$1\" = devices ]; then cat devices; exit 0; fi\ncat events\nhead -c 18000000 /dev/zero");
    assert!(!fixture.command().output().unwrap().status.success());
    assert_eq!(fixture.report()["status"], "failed");
    let events = fixture.root.path().join("proof/test-events.jsonl");
    assert!(fs::metadata(events).unwrap().len() <= 16 * 1024 * 1024);
}

#[test]
fn verify_help_documents_test_scope() {
    let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args(["x", "verify", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--device"));
    assert!(help.contains("--target"));
}

#[test]
fn failed_test_counts_and_names_remain_in_summary() {
    let fixture =
        Fixture::new(&passing_events().replace("\"result\":\"success\"", "\"result\":\"failure\""));
    fixture.normal_script();
    assert!(!fixture.command().output().unwrap().status.success());
    assert_eq!(fixture.report()["tests"]["failed"], 1);
    assert_eq!(fixture.report()["tests"]["names"][0], "scan badge");
}

#[test]
fn native_flutter_build_logs_and_process_events_are_not_test_failures() {
    let events = passing_events().replace(
        "{\"hidden\":false",
        "✓ Built build/macos/Debug/probe.app\n[{\"event\":\"test.startedProcess\",\"params\":{\"vmServiceUri\":\"http://127.0.0.1:1234/\"}}]\n{\"hidden\":false",
    );
    let fixture = Fixture::new(&events);
    fixture.normal_script();
    let output = fixture.command().output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.report()["tests"]["passed"], 1);
}

#[test]
fn verify_does_not_overwrite_prior_evidence() {
    let fixture = Fixture::new(&passing_events());
    fixture.normal_script();
    fs::create_dir(fixture.root.path().join("proof")).unwrap();
    fs::write(
        fixture.root.path().join("proof/summary.json"),
        "previous evidence",
    )
    .unwrap();
    assert!(!fixture.command().output().unwrap().status.success());
    assert_eq!(
        fs::read_to_string(fixture.root.path().join("proof/summary.json")).unwrap(),
        "previous evidence"
    );
    assert!(!fixture.root.path().join("args").exists());
}

#[test]
fn report_write_failure_never_reports_success() {
    let fixture = Fixture::new(&passing_events());
    fixture.script("if [ \"$1\" = devices ]; then cat devices; exit 0; fi\nmkdir proof/summary.json\ncat events");
    assert!(!fixture.command().output().unwrap().status.success());
}

#[test]
fn discovery_timeout_retains_logs_and_never_runs_tests() {
    let fixture = Fixture::new(&passing_events());
    fixture.script("echo discovery >&2\nsleep 5");
    assert!(!fixture.command().output().unwrap().status.success());
    assert_eq!(fixture.report()["status"], "timed_out");
    assert!(
        fs::read_to_string(fixture.root.path().join("proof/devices-stderr.log"))
            .unwrap()
            .contains("discovery")
    );
    assert!(!fixture.root.path().join("args").exists());
}

#[test]
fn symlink_targets_cannot_escape_integration_test_directory() {
    let fixture = Fixture::new(&passing_events());
    fixture.normal_script();
    let target = fixture.root.path().join("integration_test/flow_test.dart");
    fs::remove_file(&target).unwrap();
    fs::write(
        fixture.root.path().join("other_test.dart"),
        "void main() {}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(fixture.root.path().join("other_test.dart"), target).unwrap();
    assert!(!fixture.command().output().unwrap().status.success());
    assert!(!fixture.root.path().join("args").exists());
}

#[test]
fn physical_camera_requires_a_real_mobile_device() {
    let fixture = Fixture::new(&passing_events());
    fixture.normal_script();
    fs::write(
        fixture.root.path().join("devices"),
        json!([{
            "id":"phone with space", "name":"simulator", "isSupported":true,
            "targetPlatform":"ios", "emulator":true
        }])
        .to_string(),
    )
    .unwrap();
    let output = fixture.command().arg("--physical-camera").output().unwrap();
    assert!(!output.status.success());
    assert!(!fixture.root.path().join("args").exists());
    assert_eq!(fixture.report()["status"], "blocked");
}

#[test]
fn physical_camera_remains_subject_to_assertion_review() {
    let fixture = Fixture::new(&passing_events());
    fixture.normal_script();
    assert!(fixture
        .command()
        .arg("--physical-camera")
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(
        fixture.report()["native_claim"],
        "physical_camera_requires_review_of_optical_assertions"
    );
}
