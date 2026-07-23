//! `falcon check-assets` — verify every asset declared in pubspec.yaml exists.

pub mod pubspec;

use crate::config::{FalconConfig, Severity};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const RULE_ID_MISSING_FILE: &str = "assets/missing-file";
const RULE_ID_MISSING_DIR: &str = "assets/missing-directory";
const RULE_ID_EMPTY_DIR: &str = "assets/empty-directory";
const RULE_ID_GLOB_PATTERN: &str = "assets/glob-pattern";

/// Run the check. Returns exit code: 0 clean, 1 warnings only, 2 errors.
pub fn run(root: &Path, format: OutputFormat, config: &FalconConfig) -> Result<i32> {
    let pubspec_path = root.join("pubspec.yaml");
    if !pubspec_path.exists() {
        anyhow::bail!(
            "No pubspec.yaml found at {}. Is this a Flutter project?",
            root.display()
        );
    }
    let pubspec_text = std::fs::read_to_string(&pubspec_path)
        .with_context(|| format!("Reading {}", pubspec_path.display()))?;
    let decls = pubspec::parse_assets(&pubspec_text)?;

    let warn_on_empty_dir = config
        .preflight
        .config
        .get("check-assets")
        .and_then(|c| c.get("warn_on_empty_directory"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut issues: Vec<PreflightIssue> = Vec::new();
    for decl in &decls {
        let issue = inspect_decl(root, decl, warn_on_empty_dir);
        if let Some(issue) = issue {
            if !is_suppressed(&issue, config) {
                issues.push(issue);
            }
        }
    }

    let exit = exit_code_for_issues(&issues);
    let rendered = reporter::render(&issues, format);
    print!("{rendered}");
    Ok(exit)
}

fn inspect_decl(
    root: &Path,
    decl: &pubspec::AssetDecl,
    warn_on_empty_dir: bool,
) -> Option<PreflightIssue> {
    let pubspec_path = PathBuf::from("pubspec.yaml");

    // Reject glob patterns up front — Flutter's pubspec doesn't expand them.
    if decl.path.chars().any(|c| c == '*' || c == '?' || c == '[') {
        return Some(PreflightIssue {
            rule_id: RULE_ID_GLOB_PATTERN.into(),
            severity: Severity::Warning,
            title: "Glob Pattern in pubspec Assets".into(),
            file: Some(pubspec_path),
            line: Some(decl.line),
            plugin: None,
            message: format!(
                "Asset entry `{}` contains a glob metacharacter. \
                Flutter does not expand globs in pubspec.yaml — list each file or use a directory entry ending in `/`.",
                decl.path
            ),
            suggestion: Some(
                "Replace with the directory form (e.g. `assets/icons/`) or list files individually."
                    .into(),
            ),
        });
    }

    let on_disk = root.join(&decl.path);

    if decl.path.ends_with('/') {
        // Directory entry.
        if !on_disk.exists() {
            return Some(PreflightIssue {
                rule_id: RULE_ID_MISSING_DIR.into(),
                severity: Severity::Error,
                title: "Missing Asset Directory".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!(
                    "Directory `{}` is declared in pubspec.yaml but does not exist on disk.",
                    decl.path
                ),
                suggestion: Some(
                    "Create the directory and add files to it, or remove the declaration.".into(),
                ),
            });
        }
        if !on_disk.is_dir() {
            return Some(PreflightIssue {
                rule_id: RULE_ID_MISSING_DIR.into(),
                severity: Severity::Error,
                title: "Asset Directory Is Not a Directory".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!(
                    "Path `{}` is declared as a directory (trailing `/`) but is a file on disk.",
                    decl.path
                ),
                suggestion: Some("Remove the trailing slash, or replace with a directory.".into()),
            });
        }
        let empty = std::fs::read_dir(&on_disk)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if empty && warn_on_empty_dir {
            return Some(PreflightIssue {
                rule_id: RULE_ID_EMPTY_DIR.into(),
                severity: Severity::Warning,
                title: "Empty Asset Directory".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!("Directory `{}` exists but contains no files.", decl.path),
                suggestion: Some(
                    "Add files to the directory, or remove the declaration if unused.".into(),
                ),
            });
        }
        None
    } else {
        // File entry.
        if !on_disk.exists() {
            return Some(PreflightIssue {
                rule_id: RULE_ID_MISSING_FILE.into(),
                severity: Severity::Error,
                title: "Missing Asset".into(),
                file: Some(pubspec_path),
                line: Some(decl.line),
                plugin: None,
                message: format!(
                    "Asset `{}` is declared in pubspec.yaml but does not exist on disk.",
                    decl.path
                ),
                suggestion: Some(
                    "Create the file or remove the declaration. If it is gitignored, document the bootstrap step in README."
                        .into(),
                ),
            });
        }
        None
    }
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

    fn write_pubspec(dir: &Path, assets: &[&str]) {
        let mut yaml = String::from("name: testapp\nversion: 1.0.0\nflutter:\n  assets:\n");
        for a in assets {
            yaml.push_str(&format!("    - {a}\n"));
        }
        std::fs::write(dir.join("pubspec.yaml"), yaml).unwrap();
    }

    fn touch(dir: &Path, rel: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "").unwrap();
    }

    #[test]
    fn no_pubspec_yaml_returns_error() {
        let tmp = TempDir::new().unwrap();
        let result = run(tmp.path(), OutputFormat::Text, &FalconConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn no_assets_declared_exits_zero() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pubspec.yaml"), "name: testapp\n").unwrap();
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn all_assets_present_exits_zero() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/a.png", "assets/b.png"]);
        touch(tmp.path(), "assets/a.png");
        touch(tmp.path(), "assets/b.png");
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn missing_file_exits_two() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/missing.png"]);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn missing_directory_exits_two() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/"]);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn empty_directory_exits_one_warning() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/"]);
        std::fs::create_dir_all(tmp.path().join("assets/icons")).unwrap();
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn empty_directory_silenced_by_config() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/"]);
        std::fs::create_dir_all(tmp.path().join("assets/icons")).unwrap();
        let mut cfg = FalconConfig::default();
        let mut tunings = std::collections::HashMap::new();
        let yaml: serde_yaml::Value =
            serde_yaml::from_str("warn_on_empty_directory: false").unwrap();
        tunings.insert("check-assets".to_string(), yaml);
        cfg.preflight = PreflightConfig {
            suppress: vec![],
            config: tunings,
        };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn glob_pattern_emits_warning() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/icons/*.png"]);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn suppression_silences_missing_file() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/missing.png"]);
        let cfg = FalconConfig {
            preflight: PreflightConfig {
                suppress: vec![PreflightSuppression {
                    rule_id: RULE_ID_MISSING_FILE.into(),
                    plugin: None,
                    key: None,
                    reason: "Test fixture.".into(),
                }],
                config: Default::default(),
            },
            ..FalconConfig::default()
        };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn mixed_missing_and_present_reports_only_missing() {
        let tmp = TempDir::new().unwrap();
        write_pubspec(tmp.path(), &["assets/present.png", "assets/missing.png"]);
        touch(tmp.path(), "assets/present.png");
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }
}
