//! `falcon check-a11y` — verify Flutter project is ready for Maestro UI testing.

pub mod main_check;
pub mod widget_scan;

use crate::config::{FalconConfig, Severity};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue};
use anyhow::Result;
use std::path::{Path, PathBuf};

const RULE_ID_MISSING_ENSURE_SEMANTICS: &str = "a11y/missing-ensure-semantics";
const RULE_ID_COMMENTED_ENSURE_SEMANTICS: &str = "a11y/commented-ensure-semantics";
const RULE_ID_LOW_COVERAGE: &str = "a11y/low-interactive-coverage";
const RULE_ID_UNWRAPPED_WIDGET: &str = "a11y/unwrapped-interactive-widget";

/// Parse the `require_ensure_semantics` config knob to a `Severity` or `None` (off).
/// Accepted values: "error" | "warning" | "info" | "off". Default: "warning".
fn ensure_semantics_severity(config: &FalconConfig) -> Option<Severity> {
    let knob = config
        .preflight
        .config
        .get("check-a11y")
        .and_then(|c| c.get("require_ensure_semantics"))
        .and_then(|v| v.as_str())
        .unwrap_or("warning");
    match knob {
        "error" => Some(Severity::Error),
        "warning" => Some(Severity::Warning),
        "info" => Some(Severity::Info),
        "off" => None,
        _ => Some(Severity::Warning),
    }
}

/// Run the check. Returns exit code: 0 clean, 1 warnings only, 2 errors.
pub fn run(root: &Path, format: OutputFormat, config: &FalconConfig) -> Result<i32> {
    let mut issues: Vec<PreflightIssue> = Vec::new();

    let threshold = config
        .preflight
        .config
        .get("check-a11y")
        .and_then(|c| c.get("interactive_semantics_coverage"))
        .and_then(|c| c.get("threshold"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.6);

    // 1) ensureSemantics() in main.dart
    let main_path = root.join("lib").join("main.dart");
    if let Ok(source) = std::fs::read_to_string(&main_path) {
        let rel = PathBuf::from("lib/main.dart");
        let result = main_check::check_main_dart(&source);
        match result {
            main_check::MainCheckResult::Present => {}
            main_check::MainCheckResult::Missing
            | main_check::MainCheckResult::NoMainFunction => {
                if let Some(severity) = ensure_semantics_severity(config) {
                    issues.push(PreflightIssue {
                        rule_id: RULE_ID_MISSING_ENSURE_SEMANTICS.into(),
                        severity,
                        title: "Missing ensureSemantics() call".into(),
                        file: Some(rel.clone()),
                        line: None,
                        plugin: None,
                        message: "lib/main.dart does not call SemanticsBinding.instance.ensureSemantics(). \
Maestro UI tests on iOS will see an empty accessibility tree."
                            .into(),
                        suggestion: Some(
                            "Add `SemanticsBinding.instance.ensureSemantics();` after \
WidgetsFlutterBinding.ensureInitialized() and before runApp() in main()."
                                .into(),
                        ),
                    });
                }
            }
            main_check::MainCheckResult::CommentedOut => {
                if let Some(severity) = ensure_semantics_severity(config) {
                    issues.push(PreflightIssue {
                        rule_id: RULE_ID_COMMENTED_ENSURE_SEMANTICS.into(),
                        severity,
                        title: "ensureSemantics() is commented out".into(),
                        file: Some(rel),
                        line: None,
                        plugin: None,
                        message:
                            "lib/main.dart contains a commented-out call to ensureSemantics(); \
Maestro UI tests on iOS will see an empty accessibility tree."
                                .into(),
                        suggestion: Some("Uncomment the SemanticsBinding.instance.ensureSemantics() call.".into()),
                    });
                }
            }
        }
    } else {
        issues.push(PreflightIssue {
            rule_id: RULE_ID_MISSING_ENSURE_SEMANTICS.into(),
            severity: Severity::Info,
            title: "lib/main.dart not found".into(),
            file: Some(PathBuf::from("lib/main.dart")),
            line: None,
            plugin: None,
            message: "Falcon could not find lib/main.dart to verify the ensureSemantics() call.".into(),
            suggestion: None,
        });
    }

    // 2) Interactive widget coverage
    let mut total = 0usize;
    let mut wrapped = 0usize;
    let lib_dir = root.join("lib");
    if lib_dir.is_dir() {
        for entry in walkdir::WalkDir::new(&lib_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let path_str = path.to_string_lossy();
            if !path_str.ends_with(".dart")
                || path_str.ends_with(".g.dart")
                || path_str.ends_with(".freezed.dart")
            {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(path) else { continue };
            let occurrences = widget_scan::scan_widgets(&source);
            for occ in &occurrences {
                total += 1;
                if occ.wrapped {
                    wrapped += 1;
                } else {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(path)
                        .to_path_buf();
                    issues.push(PreflightIssue {
                        rule_id: RULE_ID_UNWRAPPED_WIDGET.into(),
                        severity: Severity::Info,
                        title: "Interactive widget without Semantics(identifier:)".into(),
                        file: Some(rel),
                        line: Some(occ.line),
                        plugin: None,
                        message: format!(
                            "{} is not wrapped in Semantics(identifier:) within 3 ancestor widgets.",
                            occ.kind
                        ),
                        suggestion: Some(format!(
                            "Wrap with Semantics(identifier: '<unique-id>', child: ...). \
For {}, the Container-then-Semantics pattern works around Flutter web bug #155323.",
                            occ.kind
                        )),
                    });
                }
            }
        }
    }

    if total > 0 {
        let coverage = wrapped as f64 / total as f64;
        if coverage < threshold {
            issues.push(PreflightIssue {
                rule_id: RULE_ID_LOW_COVERAGE.into(),
                severity: Severity::Warning,
                title: format!(
                    "Interactive widget Semantics coverage {:.0}% < {:.0}% threshold",
                    coverage * 100.0,
                    threshold * 100.0
                ),
                file: None,
                line: None,
                plugin: None,
                message: format!(
                    "{} of {} interactive widgets are wrapped in Semantics(identifier:). \
Maestro tests may fall back to fragile text-based selectors.",
                    wrapped, total
                ),
                suggestion: Some(format!(
                    "Add Semantics(identifier: '<unique-id>') around interactive widgets to \
reach at least {:.0}% coverage.",
                    threshold * 100.0
                )),
            });
        }
    }

    issues.retain(|issue| !is_suppressed(issue, config));

    let exit = exit_code_for_issues(&issues);
    let rendered = reporter::render(&issues, format);
    print!("{rendered}");
    Ok(exit)
}

fn is_suppressed(issue: &PreflightIssue, config: &FalconConfig) -> bool {
    config
        .preflight
        .suppress
        .iter()
        .any(|s| s.rule_id == issue.rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PreflightConfig, PreflightSuppression};
    use tempfile::TempDir;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn basic_main(with_ensure: bool) -> String {
        let mut src = String::from("void main() {\n");
        if with_ensure {
            src.push_str("  SemanticsBinding.instance.ensureSemantics();\n");
        }
        src.push_str("  runApp(MyApp());\n}\n");
        src
    }

    #[test]
    fn empty_project_no_main_emits_info_only() {
        let tmp = TempDir::new().unwrap();
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn missing_ensure_semantics_exits_one_by_default() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(false));
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn present_ensure_semantics_with_no_widgets_exits_zero() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn unwrapped_widgets_below_threshold_exits_one() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        write(
            &tmp.path().join("lib/page.dart"),
            r#"
class Page extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(children: [
      TextField(controller: a),
      TextField(controller: b),
      TextField(controller: c),
    ]);
  }
}
"#,
        );
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn wrapped_widgets_above_threshold_exits_zero() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        write(
            &tmp.path().join("lib/page.dart"),
            r#"
class Page extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(children: [
      Semantics(identifier: 'a', child: TextField(controller: a)),
      Semantics(identifier: 'b', child: TextField(controller: b)),
      Semantics(identifier: 'c', child: TextField(controller: c)),
    ]);
  }
}
"#,
        );
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn threshold_configurable_via_falcon_yaml() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        write(
            &tmp.path().join("lib/page.dart"),
            r#"
class Page extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(children: [
      Semantics(identifier: 'a', child: TextField(controller: a)),
      TextField(controller: b),
    ]);
  }
}
"#,
        );
        let mut cfg = FalconConfig::default();
        let mut tunings = std::collections::HashMap::new();
        let yaml: serde_yaml::Value =
            serde_yaml::from_str("interactive_semantics_coverage:\n  threshold: 0.4").unwrap();
        tunings.insert("check-a11y".to_string(), yaml);
        cfg.preflight = PreflightConfig { suppress: vec![], config: tunings };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn suppression_silences_missing_ensure_semantics() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(false));
        let cfg = FalconConfig {
            preflight: PreflightConfig {
                suppress: vec![PreflightSuppression {
                    rule_id: RULE_ID_MISSING_ENSURE_SEMANTICS.into(),
                    plugin: None,
                    key: None,
                    reason: "App is not Maestro-tested.".into(),
                }],
                config: Default::default(),
            },
            ..FalconConfig::default()
        };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn commented_out_ensure_semantics_exits_one_by_default() {
        let tmp = TempDir::new().unwrap();
        let src = "void main() {\n  // SemanticsBinding.instance.ensureSemantics();\n  runApp(MyApp());\n}\n";
        write(&tmp.path().join("lib/main.dart"), src);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn missing_ensure_semantics_with_require_error_config_exits_two() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(false));
        let mut cfg = FalconConfig::default();
        let mut tunings = std::collections::HashMap::new();
        let yaml: serde_yaml::Value =
            serde_yaml::from_str("require_ensure_semantics: \"error\"").unwrap();
        tunings.insert("check-a11y".to_string(), yaml);
        cfg.preflight = PreflightConfig { suppress: vec![], config: tunings };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn missing_ensure_semantics_with_off_config_exits_zero() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(false));
        let mut cfg = FalconConfig::default();
        let mut tunings = std::collections::HashMap::new();
        let yaml: serde_yaml::Value =
            serde_yaml::from_str("require_ensure_semantics: \"off\"").unwrap();
        tunings.insert("check-a11y".to_string(), yaml);
        cfg.preflight = PreflightConfig { suppress: vec![], config: tunings };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn generated_files_skipped() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        write(
            &tmp.path().join("lib/generated.g.dart"),
            "class X { Widget build() => TextField(); }",
        );
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }
}
