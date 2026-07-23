//! Dependency Manager — analyze pubspec.yaml, detect outdated deps,
//! find security issues, and manage dependency health.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepReport {
    pub total_deps: usize,
    pub direct_deps: Vec<DepInfo>,
    pub dev_deps: Vec<DepInfo>,
    pub unused: Vec<String>,
    pub issues: Vec<DepIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepInfo {
    pub name: String,
    pub version_constraint: String,
    pub is_used: bool,
    pub import_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepIssue {
    pub dep: String,
    pub issue_type: DepIssueType,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DepIssueType {
    Unused,
    TooPermissive,
    PinnedVersion,
    GitDependency,
    PathDependency,
    OverrideDep,
}

impl std::fmt::Display for DepIssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unused => write!(f, "unused"),
            Self::TooPermissive => write!(f, "permissive"),
            Self::PinnedVersion => write!(f, "pinned"),
            Self::GitDependency => write!(f, "git-dep"),
            Self::PathDependency => write!(f, "path-dep"),
            Self::OverrideDep => write!(f, "override"),
        }
    }
}

/// Analyze dependencies in a Flutter project.
pub fn analyze_dependencies(root: &Path) -> anyhow::Result<DepReport> {
    let pubspec_path = root.join("pubspec.yaml");
    if !pubspec_path.exists() {
        anyhow::bail!("No pubspec.yaml found at {}", root.display());
    }

    let content = std::fs::read_to_string(&pubspec_path)?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let mut direct_deps = Vec::new();
    let mut dev_deps = Vec::new();
    let mut issues = Vec::new();

    let used_packages = collect_imported_packages(root);

    if let Some(deps) = yaml.get("dependencies").and_then(|d| d.as_mapping()) {
        for (key, value) in deps {
            let name = key.as_str().unwrap_or("").to_string();
            if name == "flutter" || name == "flutter_localizations" {
                continue;
            }

            let constraint = format_constraint(value);
            let is_used = used_packages.contains(&name);
            let import_count = used_packages.iter().filter(|p| **p == name).count();

            if !is_used {
                issues.push(DepIssue {
                    dep: name.clone(),
                    issue_type: DepIssueType::Unused,
                    message: format!("'{}' is declared but not imported in any Dart file", name),
                });
            }

            if value.is_mapping() {
                if value.get("git").is_some() {
                    issues.push(DepIssue {
                        dep: name.clone(),
                        issue_type: DepIssueType::GitDependency,
                        message: format!(
                            "'{}' uses git dependency — pin to a published version for stability",
                            name
                        ),
                    });
                }
                if value.get("path").is_some() {
                    issues.push(DepIssue {
                        dep: name.clone(),
                        issue_type: DepIssueType::PathDependency,
                        message: format!(
                            "'{}' uses path dependency — ensure CI can resolve it",
                            name
                        ),
                    });
                }
            }

            if let Some(ver) = value.as_str() {
                if ver == "any" || ver.is_empty() {
                    issues.push(DepIssue {
                        dep: name.clone(),
                        issue_type: DepIssueType::TooPermissive,
                        message: format!("'{}' has no version constraint — use ^x.y.z", name),
                    });
                }
            }

            direct_deps.push(DepInfo {
                name,
                version_constraint: constraint,
                is_used,
                import_count,
            });
        }
    }

    if let Some(deps) = yaml.get("dev_dependencies").and_then(|d| d.as_mapping()) {
        for (key, value) in deps {
            let name = key.as_str().unwrap_or("").to_string();
            if name == "flutter_test" || name == "flutter_lints" || name == "lints" {
                continue;
            }
            let constraint = format_constraint(value);
            dev_deps.push(DepInfo {
                name,
                version_constraint: constraint,
                is_used: true,
                import_count: 0,
            });
        }
    }

    if yaml.get("dependency_overrides").is_some() {
        issues.push(DepIssue {
            dep: "(project)".to_string(),
            issue_type: DepIssueType::OverrideDep,
            message: "dependency_overrides found — ensure these are temporary and not shipped to production".to_string(),
        });
    }

    let unused: Vec<String> = direct_deps
        .iter()
        .filter(|d| !d.is_used)
        .map(|d| d.name.clone())
        .collect();

    Ok(DepReport {
        total_deps: direct_deps.len() + dev_deps.len(),
        direct_deps,
        dev_deps,
        unused,
        issues,
    })
}

fn format_constraint(value: &serde_yaml::Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else if value.is_mapping() {
        if value.get("git").is_some() {
            "git".to_string()
        } else if value.get("path").is_some() {
            "path".to_string()
        } else {
            "complex".to_string()
        }
    } else {
        "any".to_string()
    }
}

fn collect_imported_packages(root: &Path) -> Vec<String> {
    let mut packages = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("import") && trimmed.contains("package:") {
                    if let Some(pkg) = extract_pkg(trimmed) {
                        packages.push(pkg);
                    }
                }
            }
        }
    }
    packages
}

fn extract_pkg(line: &str) -> Option<String> {
    let start = line.find("package:")? + 8;
    let rest = &line[start..];
    let end = rest.find('/')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── DepIssueType::Display ────────────────────────────────────────────────

    #[test]
    fn display_unused() {
        assert_eq!(DepIssueType::Unused.to_string(), "unused");
    }

    #[test]
    fn display_too_permissive() {
        assert_eq!(DepIssueType::TooPermissive.to_string(), "permissive");
    }

    #[test]
    fn display_pinned_version() {
        assert_eq!(DepIssueType::PinnedVersion.to_string(), "pinned");
    }

    #[test]
    fn display_git_dependency() {
        assert_eq!(DepIssueType::GitDependency.to_string(), "git-dep");
    }

    #[test]
    fn display_path_dependency() {
        assert_eq!(DepIssueType::PathDependency.to_string(), "path-dep");
    }

    #[test]
    fn display_override_dep() {
        assert_eq!(DepIssueType::OverrideDep.to_string(), "override");
    }

    // ── extract_pkg (pure) ──────────────────────────────────────────────────

    #[test]
    fn extract_pkg_basic() {
        let line = r#"import 'package:provider/provider.dart';"#;
        assert_eq!(extract_pkg(line), Some("provider".to_string()));
    }

    #[test]
    fn extract_pkg_nested_path() {
        let line = r#"import 'package:flutter_bloc/src/bloc.dart';"#;
        assert_eq!(extract_pkg(line), Some("flutter_bloc".to_string()));
    }

    #[test]
    fn extract_pkg_no_slash_returns_none() {
        let line = r#"import 'package:something';"#;
        assert_eq!(extract_pkg(line), None);
    }

    #[test]
    fn extract_pkg_no_package_prefix_returns_none() {
        let line = r#"import 'dart:async';"#;
        assert_eq!(extract_pkg(line), None);
    }

    #[test]
    fn extract_pkg_empty_line_returns_none() {
        assert_eq!(extract_pkg(""), None);
    }

    // ── format_constraint (pure via serde_yaml) ─────────────────────────────

    fn yaml_str(s: &str) -> serde_yaml::Value {
        serde_yaml::from_str(s).unwrap()
    }

    #[test]
    fn format_constraint_string_value() {
        let v: serde_yaml::Value = serde_yaml::Value::String("^1.2.3".to_string());
        assert_eq!(format_constraint(&v), "^1.2.3");
    }

    #[test]
    fn format_constraint_any_string() {
        let v: serde_yaml::Value = serde_yaml::Value::String("any".to_string());
        assert_eq!(format_constraint(&v), "any");
    }

    #[test]
    fn format_constraint_git_mapping() {
        let v = yaml_str("git:\n  url: https://github.com/foo/bar.git");
        assert_eq!(format_constraint(&v), "git");
    }

    #[test]
    fn format_constraint_path_mapping() {
        let v = yaml_str("path: ../my_package");
        assert_eq!(format_constraint(&v), "path");
    }

    #[test]
    fn format_constraint_other_mapping() {
        let v = yaml_str("hosted:\n  name: foo\n  url: https://example.com");
        assert_eq!(format_constraint(&v), "complex");
    }

    #[test]
    fn format_constraint_null_value() {
        let v = serde_yaml::Value::Null;
        assert_eq!(format_constraint(&v), "any");
    }

    // ── collect_imported_packages (TempDir) ─────────────────────────────────

    fn make_dir_with_dart(content: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        let lib = dir.path().join("lib");
        fs::create_dir_all(&lib).unwrap();
        fs::write(lib.join("main.dart"), content).unwrap();
        dir
    }

    #[test]
    fn collect_finds_single_import() {
        let dir = make_dir_with_dart("import 'package:provider/provider.dart';\n");
        let pkgs = collect_imported_packages(dir.path());
        assert!(pkgs.contains(&"provider".to_string()));
    }

    #[test]
    fn collect_finds_multiple_imports() {
        let dir = make_dir_with_dart(
            "import 'package:provider/provider.dart';\nimport 'package:dio/dio.dart';\n",
        );
        let pkgs = collect_imported_packages(dir.path());
        assert!(pkgs.contains(&"provider".to_string()));
        assert!(pkgs.contains(&"dio".to_string()));
    }

    #[test]
    fn collect_ignores_dart_scheme_imports() {
        let dir = make_dir_with_dart("import 'dart:async';\nimport 'dart:io';\n");
        let pkgs = collect_imported_packages(dir.path());
        assert!(pkgs.is_empty());
    }

    #[test]
    fn collect_empty_dir_returns_empty() {
        let dir = TempDir::new().unwrap();
        let pkgs = collect_imported_packages(dir.path());
        assert!(pkgs.is_empty());
    }

    // ── analyze_dependencies (TempDir orchestrator) ─────────────────────────

    fn write_pubspec(dir: &TempDir, content: &str) {
        fs::write(dir.path().join("pubspec.yaml"), content).unwrap();
    }

    #[test]
    fn analyze_no_pubspec_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = analyze_dependencies(dir.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("No pubspec.yaml"));
    }

    #[test]
    fn analyze_empty_deps() {
        let dir = TempDir::new().unwrap();
        write_pubspec(
            &dir,
            "name: my_app\ndependencies:\n  flutter:\n    sdk: flutter\n",
        );
        let report = analyze_dependencies(dir.path()).unwrap();
        assert_eq!(report.total_deps, 0);
        assert!(report.direct_deps.is_empty());
        assert!(report.issues.is_empty());
    }

    #[test]
    fn analyze_unused_dep_creates_issue() {
        let dir = TempDir::new().unwrap();
        write_pubspec(&dir, "name: my_app\ndependencies:\n  provider: ^6.0.0\n");
        let report = analyze_dependencies(dir.path()).unwrap();
        let issue_types: Vec<String> = report
            .issues
            .iter()
            .map(|i| i.issue_type.to_string())
            .collect();
        assert!(issue_types.contains(&"unused".to_string()));
    }

    #[test]
    fn analyze_git_dep_creates_issue() {
        let dir = TempDir::new().unwrap();
        write_pubspec(
            &dir,
            "name: my_app\ndependencies:\n  my_pkg:\n    git:\n      url: https://github.com/foo/my_pkg.git\n",
        );
        let report = analyze_dependencies(dir.path()).unwrap();
        let issue_types: Vec<String> = report
            .issues
            .iter()
            .map(|i| i.issue_type.to_string())
            .collect();
        assert!(issue_types.contains(&"git-dep".to_string()));
    }

    #[test]
    fn analyze_path_dep_creates_issue() {
        let dir = TempDir::new().unwrap();
        write_pubspec(
            &dir,
            "name: my_app\ndependencies:\n  local_pkg:\n    path: ../local_pkg\n",
        );
        let report = analyze_dependencies(dir.path()).unwrap();
        let issue_types: Vec<String> = report
            .issues
            .iter()
            .map(|i| i.issue_type.to_string())
            .collect();
        assert!(issue_types.contains(&"path-dep".to_string()));
    }

    #[test]
    fn analyze_any_version_creates_too_permissive_issue() {
        let dir = TempDir::new().unwrap();
        write_pubspec(&dir, "name: my_app\ndependencies:\n  dio: any\n");
        let report = analyze_dependencies(dir.path()).unwrap();
        let issue_types: Vec<String> = report
            .issues
            .iter()
            .map(|i| i.issue_type.to_string())
            .collect();
        assert!(issue_types.contains(&"permissive".to_string()));
    }

    #[test]
    fn analyze_dependency_overrides_creates_issue() {
        let dir = TempDir::new().unwrap();
        write_pubspec(
            &dir,
            "name: my_app\ndependencies: {}\ndependency_overrides:\n  some_pkg: ^1.0.0\n",
        );
        let report = analyze_dependencies(dir.path()).unwrap();
        let issue_types: Vec<String> = report
            .issues
            .iter()
            .map(|i| i.issue_type.to_string())
            .collect();
        assert!(issue_types.contains(&"override".to_string()));
    }

    #[test]
    fn analyze_used_dep_not_in_unused_list() {
        let dir = TempDir::new().unwrap();
        write_pubspec(&dir, "name: my_app\ndependencies:\n  provider: ^6.0.0\n");
        let lib = dir.path().join("lib");
        fs::create_dir_all(&lib).unwrap();
        fs::write(
            lib.join("main.dart"),
            "import 'package:provider/provider.dart';\n",
        )
        .unwrap();
        let report = analyze_dependencies(dir.path()).unwrap();
        assert!(!report.unused.contains(&"provider".to_string()));
    }

    #[test]
    fn analyze_dev_deps_skips_flutter_test_and_lints() {
        let dir = TempDir::new().unwrap();
        write_pubspec(
            &dir,
            "name: my_app\ndev_dependencies:\n  flutter_test:\n    sdk: flutter\n  flutter_lints: ^3.0.0\n  lints: ^3.0.0\n  build_runner: ^2.4.0\n",
        );
        let report = analyze_dependencies(dir.path()).unwrap();
        // Only build_runner should appear — flutter_test, flutter_lints, lints are skipped
        assert_eq!(report.dev_deps.len(), 1);
        assert_eq!(report.dev_deps[0].name, "build_runner");
    }

    #[test]
    fn analyze_total_deps_counts_direct_plus_dev() {
        let dir = TempDir::new().unwrap();
        write_pubspec(
            &dir,
            "name: my_app\ndependencies:\n  provider: ^6.0.0\ndev_dependencies:\n  build_runner: ^2.4.0\n",
        );
        let report = analyze_dependencies(dir.path()).unwrap();
        assert_eq!(
            report.total_deps,
            report.direct_deps.len() + report.dev_deps.len()
        );
    }
}

/// Print dependency report.
pub fn print_dep_report(report: &DepReport) {
    println!();
    println!(
        "  {} Dependency Manager",
        "falcon manage".bright_cyan().bold()
    );
    println!();

    println!(
        "  Total dependencies: {} ({} direct, {} dev)",
        report.total_deps,
        report.direct_deps.len(),
        report.dev_deps.len()
    );

    if !report.unused.is_empty() {
        println!();
        println!("  {} Unused dependencies:", "⚠".yellow());
        for dep in &report.unused {
            println!(
                "    {} {} — not imported in any file",
                "✗".red(),
                dep.bright_white()
            );
        }
    }

    if !report.issues.is_empty() {
        println!();
        println!("  {} Issues:", "▸".bright_cyan());
        for issue in &report.issues {
            let icon = match issue.issue_type {
                DepIssueType::Unused => "✗".red(),
                DepIssueType::GitDependency | DepIssueType::PathDependency => "⚠".yellow(),
                _ => "·".dimmed(),
            };
            println!(
                "    {} [{}] {}",
                icon,
                issue.issue_type.to_string().bright_yellow(),
                issue.message
            );
        }
    }

    if report.issues.is_empty() && report.unused.is_empty() {
        println!();
        println!("  {} Dependencies are healthy!", "✓".green().bold());
    }

    println!();
}
