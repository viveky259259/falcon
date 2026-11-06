#![cfg(target_os = "macos")]
use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

fn script(path: &std::path::Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn qr_check_preserves_fixture_evidence_and_rejects_mismatch() {
    for payload in ["expected", "wrong", ""] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("video.mp4"), "fixture").unwrap();
        fs::write(
            root.path().join("manifest.json"),
            json!([
                {"start_seconds":8,"duration_seconds":18,"payload":"expected"}
            ])
            .to_string(),
        )
        .unwrap();
        script(
            &root.path().join("ffmpeg"),
            "for last do :; done; touch \"$last\"",
        );
        script(
            &root.path().join("swiftc"),
            &format!(
                "for last do :; done; cp '{}' \"$last\"; chmod +x \"$last\"",
                root.path().join("decoder").display()
            ),
        );
        script(
            &root.path().join("decoder"),
            &format!("echo '{{\"payloads\":[\"{payload}\"]}}'"),
        );
        let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
            .current_dir(root.path())
            .args([
                "x",
                "qr-check",
                "--video",
                "video.mp4",
                "--manifest",
                "manifest.json",
                "--output-dir",
                "proof",
                "--ffmpeg",
                "./ffmpeg",
                "--swiftc",
                "./swiftc",
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            payload == "expected",
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value =
            serde_json::from_slice(&fs::read(root.path().join("proof/summary.json")).unwrap())
                .unwrap();
        assert_eq!(report["scope"], "encoded_qr_fixture_only");
        assert_eq!(report["samples"][0]["passed"], payload == "expected");
        assert!(root.path().join("proof/sample-000.png").exists());
    }
}

#[test]
fn qr_check_rejects_empty_and_invalid_manifests() {
    for manifest in [
        "[]",
        r#"[{"start_seconds":-1,"duration_seconds":10,"payload":"x"}]"#,
        r#"[{"start_seconds":0,"duration_seconds":0,"payload":"x"}]"#,
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("video.mp4"), "fixture").unwrap();
        fs::write(root.path().join("manifest.json"), manifest).unwrap();
        assert!(!Command::new(env!("CARGO_BIN_EXE_falcon"))
            .current_dir(root.path())
            .args([
                "x",
                "qr-check",
                "--video",
                "video.mp4",
                "--manifest",
                "manifest.json"
            ])
            .output()
            .unwrap()
            .status
            .success());
    }
}

#[test]
fn qr_check_retains_failures_and_does_not_reuse_evidence() {
    for body in [
        "echo compile-failure >&2; exit 4",
        "echo timed-out >&2; sleep 3",
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("video.mp4"), "fixture").unwrap();
        fs::write(
            root.path().join("manifest.json"),
            r#"[{"start_seconds":0,"duration_seconds":10,"payload":"x"}]"#,
        )
        .unwrap();
        script(&root.path().join("swiftc"), body);
        let mut command = Command::new(env!("CARGO_BIN_EXE_falcon"));
        command.current_dir(root.path()).args([
            "x",
            "qr-check",
            "--video",
            "video.mp4",
            "--manifest",
            "manifest.json",
            "--output-dir",
            "proof",
            "--swiftc",
            "./swiftc",
            "--timeout",
            "1",
        ]);
        assert!(!command.output().unwrap().status.success());
        let prior = fs::read(root.path().join("proof/summary.json")).unwrap();
        let report: Value = serde_json::from_slice(&prior).unwrap();
        assert_eq!(report["status"], "failed");
        assert!(!report["problems"].as_array().unwrap().is_empty());
        assert!(
            !fs::read_to_string(root.path().join("proof/compile-stderr.log"))
                .unwrap()
                .is_empty()
        );
        assert!(!command.output().unwrap().status.success());
        assert_eq!(
            fs::read(root.path().join("proof/summary.json")).unwrap(),
            prior
        );
    }
}
