use falcon::{config::FalconConfig, sdk::FalconSdk, Falcon};
use serde_json::Value;
use std::{fs, process::Command};

fn project(config: &str, source: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("lib")).unwrap();
    fs::write(temp.path().join("lib/main.dart"), source).unwrap();
    fs::write(temp.path().join("falcon.yaml"), config).unwrap();
    temp
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}
const CLEAN: &str = "rules: []\nunused:\n  enabled: false\n";

#[test]
fn explicit_config_file_controls_severity() {
    let root = project(CLEAN, "void main() { dynamic value = 1; }\n");
    let config = root.path().join("custom.yaml");
    fs::write(
        &config,
        "rules:\n  - avoid-dynamic:\n      severity: error\nunused:\n  enabled: false\n",
    )
    .unwrap();
    let output = cli()
        .arg("check")
        .arg(root.path())
        .arg("--config")
        .arg(config)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(doc["summary"]["errors"], 1);
}

#[test]
fn explicit_missing_config_is_an_error() {
    let root = project(CLEAN, "void main() {}\n");
    let output = cli()
        .arg("check")
        .arg(root.path())
        .arg("--config")
        .arg(root.path().join("missing.yaml"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing.yaml"));
}

#[test]
fn missing_analysis_root_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let falcon = Falcon::new(FalconConfig::default()).unwrap();
    assert!(falcon.analyze(&root.path().join("absent")).is_err());
}

#[test]
fn unreadable_dart_source_cannot_be_reported_as_clean() {
    let root = project(CLEAN, "void main() {}\n");
    fs::write(root.path().join("lib/main.dart"), [0xff, 0xfe]).unwrap();
    let falcon = Falcon::new(FalconConfig::load(root.path()).unwrap()).unwrap();
    assert!(falcon.analyze(root.path()).is_err());
}

#[test]
fn source_overlay_replaces_existing_file_declarations() {
    let source = "class ScreenState extends State<W> {\n final c = TextEditingController();\n}\n";
    let root = project(
        "rules:\n  - dispose-not-called:\n      severity: error\nunused:\n  enabled: false\n",
        source,
    );
    let issues = FalconSdk::new()
        .analyze_source_with_project_context(source, "lib/main.dart", root.path().to_str().unwrap())
        .unwrap();
    assert!(issues
        .iter()
        .any(|issue| issue.rule == "dispose-not-called"));
}

#[test]
fn source_overlay_uses_updated_hierarchy_and_methods() {
    let root = project(
        "rules:\n  - dispose-not-called:\n      severity: error\nunused:\n  enabled: false\n",
        "class ScreenState extends State<W> {\n final c = TextEditingController();\n}\n",
    );
    let source = "class ScreenState extends State<W> {\n final c = TextEditingController();\n void dispose() { c.dispose(); super.dispose(); }\n}\n";
    let issues = FalconSdk::new()
        .analyze_source_with_project_context(source, "lib/main.dart", root.path().to_str().unwrap())
        .unwrap();
    assert!(!issues
        .iter()
        .any(|issue| issue.rule == "dispose-not-called"));
}

#[test]
fn preflight_json_remains_one_document_with_findings() {
    let root = project("rules: []\nunused:\n  enabled: false\nanalyze:\n  preflight:\n    enabled: true\n    skip: [check-a11y, check-pods, check-platform-deps]\n", "void main() {}\n");
    fs::write(
        root.path().join("pubspec.yaml"),
        "name: sample\nflutter:\n  assets:\n    - missing.png\n",
    )
    .unwrap();
    let output = cli()
        .arg("check")
        .arg(root.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).expect("one valid JSON document");
    assert_eq!(output.status.code(), Some(2));
    assert!(doc["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["rule"] == "assets/missing-file"));
}

#[test]
fn file_report_write_failure_is_an_error_for_every_format() {
    let root = project(CLEAN, "void main() {}\n");
    for format in ["sarif", "html", "checkstyle", "codeclimate", "sonar"] {
        let output = cli()
            .arg("check")
            .arg(root.path())
            .args(["--format", format, "--output"])
            .arg(root.path().join("absent/report"))
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "{format} falsely reported success"
        );
    }
}

#[test]
fn relative_incremental_root_analyzes_transitive_dependents() {
    let root = project("rules:\n  - dispose-not-called:\n      severity: error\nunused:\n  enabled: false\n", "import 'base.dart';\nclass ScreenState extends BaseState {\n final c = TextEditingController();\n}\n");
    fs::write(root.path().join("lib/base.dart"), "class BaseState {}\n").unwrap();
    for args in [
        vec!["init"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.test",
            "commit",
            "-m",
            "fixture",
        ],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(root.path())
            .output()
            .unwrap()
            .status
            .success());
    }
    fs::write(
        root.path().join("lib/base.dart"),
        "class BaseState extends State<W> {}\n",
    )
    .unwrap();
    let output = cli()
        .current_dir(root.path())
        .args(["check", ".", "--since", "HEAD", "--format", "json"])
        .output()
        .unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["summary"]["files_analyzed"], 2);
    assert!(doc["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["rule"] == "dispose-not-called"));
}

#[test]
fn fixes_preserve_strings_comments_and_combine_same_line_edits() {
    let source = "void main() { final xs = [1]; final a = xs[0]; final b = xs[0]; final label = '[0] !!'; } // [0] !!\n";
    let root = project(
        "rules:\n  - prefer-first-last\nunused:\n  enabled: false\n",
        source,
    );
    let output = cli().arg("fix").arg(root.path()).output().unwrap();
    assert!(output.status.success());
    let fixed = fs::read_to_string(root.path().join("lib/main.dart")).unwrap();
    assert_eq!(fixed, source.replace("xs[0]", "xs.first"));
}

#[test]
fn multiline_trailing_comma_fix_edits_the_final_argument() {
    let source = "void main() {\n  foo(\n    'value'\n  );\n}\n";
    let root = project(
        "rules:\n  - prefer-trailing-comma\nunused:\n  enabled: false\n",
        source,
    );
    let output = cli().arg("fix").arg(root.path()).output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(root.path().join("lib/main.dart")).unwrap(),
        source.replace("'value'", "'value',")
    );
}

#[test]
fn double_literal_fix_follows_rule_and_preserves_other_literals() {
    let source = "void main() { final value = .5; final label = '0.5'; }\n";
    let root = project(
        "rules:\n  - double-literal-format\nunused:\n  enabled: false\n",
        source,
    );
    cli().arg("fix").arg(root.path()).output().unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("lib/main.dart")).unwrap(),
        source.replace("= .5", "= 0.5")
    );
}

#[test]
fn spacer_fix_keeps_an_existing_const_modifier() {
    let source = "void main() { final widget = const Expanded(child: SizedBox()); }\n";
    let root = project(
        "rules:\n  - avoid-expanded-as-spacer\nunused:\n  enabled: false\n",
        source,
    );
    // Supply the concrete issue because this case also covers editor-supplied diagnostics.
    let issue = falcon::reporters::Issue {
        rule: "avoid-expanded-as-spacer".into(),
        message: "Use Spacer".into(),
        severity: falcon::config::Severity::Info,
        file: root.path().join("lib/main.dart"),
        line: 1,
        column: 1,
    };
    let fixes = falcon::ai::fix::generate_fixes(&[issue], root.path());
    assert_eq!(falcon::ai::fix::apply_fixes(&fixes), 1);
    let fixed = fs::read_to_string(root.path().join("lib/main.dart")).unwrap();
    assert_eq!(
        fixed,
        source.replace("Expanded(child: SizedBox())", "Spacer()")
    );
}
