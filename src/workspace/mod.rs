use crate::config::FalconConfig;
use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

/// Represents a detected workspace package.
#[derive(Debug, Clone)]
pub struct WorkspacePackage {
    pub name: String,
    pub path: PathBuf,
    pub config: FalconConfig,
}

/// Detected workspace type.
#[derive(Debug, Clone)]
pub enum WorkspaceType {
    Melos,
    PubWorkspace,
    SinglePackage,
}

/// Detects workspace type and discovers packages.
pub fn detect_workspace(root: &Path) -> (WorkspaceType, Vec<WorkspacePackage>) {
    if let Some(packages) = detect_melos(root) {
        return (WorkspaceType::Melos, packages);
    }
    if let Some(packages) = detect_pub_workspace(root) {
        return (WorkspaceType::PubWorkspace, packages);
    }
    let config = FalconConfig::load(root).unwrap_or_default();
    let name = extract_package_name(root);
    (
        WorkspaceType::SinglePackage,
        vec![WorkspacePackage {
            name,
            path: root.to_path_buf(),
            config,
        }],
    )
}

fn detect_melos(root: &Path) -> Option<Vec<WorkspacePackage>> {
    let melos_path = root.join("melos.yaml");
    if !melos_path.exists() {
        return None;
    }

    let contents = std::fs::read_to_string(&melos_path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&contents).ok()?;

    let packages_globs = yaml
        .get("packages")
        .and_then(|p| p.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["packages/**".to_string()]);

    let root_config = FalconConfig::load(root).unwrap_or_default();
    let mut packages = Vec::new();

    for glob_pat in &packages_globs {
        let pattern = root.join(glob_pat).join("pubspec.yaml");
        let pattern_str = pattern.to_string_lossy().to_string();

        if let Ok(paths) = glob::glob(&pattern_str) {
            for entry in paths.flatten() {
                if let Some(pkg_dir) = entry.parent() {
                    let pkg_name = extract_package_name(pkg_dir);
                    let config = load_with_inheritance(pkg_dir, &root_config);
                    packages.push(WorkspacePackage {
                        name: pkg_name,
                        path: pkg_dir.to_path_buf(),
                        config,
                    });
                }
            }
        }
    }

    if packages.is_empty() {
        return None;
    }

    Some(packages)
}

fn detect_pub_workspace(root: &Path) -> Option<Vec<WorkspacePackage>> {
    let pubspec_path = root.join("pubspec.yaml");
    if !pubspec_path.exists() {
        return None;
    }

    let contents = std::fs::read_to_string(&pubspec_path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&contents).ok()?;

    let workspace = yaml.get("workspace")?;
    let workspace_dirs = workspace.as_sequence()?;

    let root_config = FalconConfig::load(root).unwrap_or_default();
    let mut packages = Vec::new();

    packages.push(WorkspacePackage {
        name: extract_package_name(root),
        path: root.to_path_buf(),
        config: root_config.clone(),
    });

    for dir in workspace_dirs {
        if let Some(dir_str) = dir.as_str() {
            let pkg_dir = root.join(dir_str);
            if pkg_dir.join("pubspec.yaml").exists() {
                let pkg_name = extract_package_name(&pkg_dir);
                let config = load_with_inheritance(&pkg_dir, &root_config);
                packages.push(WorkspacePackage {
                    name: pkg_name,
                    path: pkg_dir,
                    config,
                });
            }
        }
    }

    if packages.len() <= 1 {
        return None;
    }

    Some(packages)
}

/// Load package config with inheritance from root config.
fn load_with_inheritance(pkg_dir: &Path, root_config: &FalconConfig) -> FalconConfig {
    let pkg_config_path = pkg_dir.join("falcon.yaml");
    if pkg_config_path.exists() {
        FalconConfig::load(pkg_dir).unwrap_or_else(|_| root_config.clone())
    } else {
        root_config.clone()
    }
}

fn extract_package_name(pkg_dir: &Path) -> String {
    let pubspec_path = pkg_dir.join("pubspec.yaml");
    if let Ok(contents) = std::fs::read_to_string(&pubspec_path) {
        if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&contents) {
            if let Some(name) = yaml.get("name").and_then(|n| n.as_str()) {
                return name.to_string();
            }
        }
    }
    pkg_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Run analysis across all workspace packages and print aggregate results.
pub fn analyze_workspace(root: &Path) -> Result<WorkspaceReport> {
    let (ws_type, packages) = detect_workspace(root);

    let type_label = match ws_type {
        WorkspaceType::Melos => "Melos",
        WorkspaceType::PubWorkspace => "Dart Pub Workspace",
        WorkspaceType::SinglePackage => "Single Package",
    };

    println!(
        "\n{} {} workspace detected — {} package(s)\n",
        "falcon".bright_cyan().bold(),
        type_label.bright_green(),
        packages.len()
    );

    let mut report = WorkspaceReport {
        workspace_type: type_label.to_string(),
        packages: Vec::new(),
        total_issues: 0,
        total_files: 0,
        total_errors: 0,
        total_warnings: 0,
    };

    for pkg in &packages {
        println!(
            "  {} {} ({})",
            "▶".bright_yellow(),
            pkg.name.bright_white().bold(),
            pkg.path.display()
        );

        let falcon = crate::Falcon::new(pkg.config.clone())?;
        match falcon.analyze(&pkg.path) {
            Ok(analysis) => {
                let errors = analysis.error_count();
                let warnings = analysis.warning_count();
                let infos = analysis.info_count();
                let total = analysis.issues.len();
                let files = analysis.file_count;

                let status = if errors > 0 {
                    format!("{} errors", errors).red().to_string()
                } else if warnings > 0 {
                    format!("{} warnings", warnings).yellow().to_string()
                } else {
                    "✓ clean".green().to_string()
                };

                println!(
                    "    {} {} files, {} issues ({})",
                    "└".dimmed(),
                    files,
                    total,
                    status,
                );

                report.total_issues += total;
                report.total_files += files;
                report.total_errors += errors;
                report.total_warnings += warnings;
                report.packages.push(PackageReport {
                    name: pkg.name.clone(),
                    path: pkg.path.clone(),
                    files,
                    issues: total,
                    errors,
                    warnings,
                    infos,
                });
            }
            Err(e) => {
                println!("    {} {}", "└ error:".red(), e);
            }
        }
    }

    println!("\n{}", "── Workspace Summary ──".bright_cyan());
    println!(
        "  {} packages │ {} files │ {} issues ({} errors, {} warnings)",
        report.packages.len(),
        report.total_files,
        report.total_issues,
        report.total_errors,
        report.total_warnings,
    );

    Ok(report)
}

#[derive(Debug)]
pub struct WorkspaceReport {
    pub workspace_type: String,
    pub packages: Vec<PackageReport>,
    pub total_issues: usize,
    pub total_files: usize,
    pub total_errors: usize,
    pub total_warnings: usize,
}

#[derive(Debug)]
pub struct PackageReport {
    pub name: String,
    pub path: PathBuf,
    pub files: usize,
    pub issues: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}
