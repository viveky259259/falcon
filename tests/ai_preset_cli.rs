//! `falcon check --preset ai-generated` and `falcon score` on the mistakes
//! agents make most: `setState` after an `await`, and a committed key.

use serde_json::Value;
use std::fs;
use std::process::Command;

const LIFECYCLE: &str = r#"import 'package:flutter/material.dart';

class Profile extends StatefulWidget {
  const Profile({super.key});
  @override
  State<Profile> createState() => _ProfileState();
}

class _ProfileState extends State<Profile> {
  String _value = '';

  Future<void> _load() async {
    await Future<void>.delayed(const Duration(seconds: 2));
    setState(() {
      _value = 'loaded';
    });
  }

  @override
  Widget build(BuildContext context) {
    return TextButton(onPressed: _load, child: Text(_value));
  }
}
"#;

/// A file holding a made-up AWS-format key, put together at run time so the
/// source holds nothing a secret scanner would stop.
fn secret_source() -> String {
    format!(
        "class Config {{\n  static const String awsId = '{}{}';\n}}\n",
        "AKIA", "IOSFODNN7EXAMPLE"
    )
}

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("lib")).unwrap();
    fs::write(dir.path().join("pubspec.yaml"), "name: sample\n").unwrap();
    for (name, source) in files {
        fs::write(dir.path().join("lib").join(name), source).unwrap();
    }
    dir
}

fn falcon(args: &[&str], dir: &tempfile::TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args(args)
        .arg(dir.path())
        .output()
        .expect("run falcon")
}

fn findings(output: &std::process::Output, rule: &str) -> Vec<Value> {
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid json: {e}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    json["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .filter(|issue| issue["rule"] == rule)
        .cloned()
        .collect()
}

#[test]
fn ai_generated_preset_fails_on_set_state_after_await() {
    let dir = project(&[("profile.dart", LIFECYCLE)]);
    let output = falcon(
        &[
            "check",
            "--no-graph",
            "--format",
            "json",
            "--preset",
            "ai-generated",
        ],
        &dir,
    );
    let found = findings(&output, "set-state-after-dispose");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["severity"], "error");
    assert_eq!(found[0]["line"], 14);
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn default_check_warns_on_set_state_after_await_without_failing() {
    let dir = project(&[("profile.dart", LIFECYCLE)]);
    let output = falcon(&["check", "--no-graph", "--format", "json"], &dir);
    let found = findings(&output, "set-state-after-dispose");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["severity"], "warning");
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn score_never_calls_a_project_with_a_committed_key_production_ready() {
    let secret = secret_source();
    let dir = project(&[("config.dart", &secret)]);
    let output = falcon(&["score"], &dir);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("hardcoded credential"), "{text}");
    assert!(!text.contains("Production Ready"), "{text}");
    assert!(!text.contains("Certified"), "{text}");
}
