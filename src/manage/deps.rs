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
