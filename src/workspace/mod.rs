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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── helpers ────────────────────────────────────────────────────────────────

    fn write_file(dir: &Path, name: &str, contents: &str) {
        fs::write(dir.join(name), contents).unwrap();
    }

    fn make_pubspec(dir: &Path, name: &str) {
        write_file(dir, "pubspec.yaml", &format!("name: {}\n", name));
    }

    fn make_pubspec_with_workspace(dir: &Path, name: &str, workspace_dirs: &[&str]) {
        let workspace_list = workspace_dirs
            .iter()
            .map(|d| format!("  - {}", d))
            .collect::<Vec<_>>()
            .join("\n");
        write_file(
            dir,
            "pubspec.yaml",
            &format!("name: {}\nworkspace:\n{}\n", name, workspace_list),
        );
    }

    fn make_falcon_yaml(dir: &Path) {
        // minimal valid falcon.yaml
        write_file(
            dir,
            "falcon.yaml",
            "metrics:\n  cyclomatic_complexity: 5\n",
        );
    }

    // ── extract_package_name ───────────────────────────────────────────────────

    #[test]
    fn extract_name_from_pubspec() {
        let tmp = TempDir::new().unwrap();
        make_pubspec(tmp.path(), "my_awesome_package");
        let name = extract_package_name(tmp.path());
        assert_eq!(name, "my_awesome_package");
    }

    #[test]
    fn extract_name_falls_back_to_dirname_when_no_pubspec() {
        let tmp = TempDir::new().unwrap();
        // no pubspec.yaml present
        let name = extract_package_name(tmp.path());
        // tempfile dir names are random but non-empty; just ensure it's not "unknown"
        // unless the OS can't resolve the last component (practically never)
        assert!(!name.is_empty());
    }

    #[test]
    fn extract_name_falls_back_to_dirname_on_invalid_pubspec() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "pubspec.yaml", "not: valid: yaml: [");
        // directory name used as fallback
        let name = extract_package_name(tmp.path());
        assert!(!name.is_empty());
    }

    #[test]
    fn extract_name_when_pubspec_has_no_name_field() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "pubspec.yaml", "version: 1.0.0\n");
        // no "name:" key → directory name fallback
        let name = extract_package_name(tmp.path());
        assert!(!name.is_empty());
    }

    // ── load_with_inheritance ──────────────────────────────────────────────────

    #[test]
    fn load_with_inheritance_uses_root_when_no_pkg_config() {
        let tmp = TempDir::new().unwrap();
        let root_config = FalconConfig::default();
        // no falcon.yaml in pkg_dir
        let cfg = load_with_inheritance(tmp.path(), &root_config);
        // should be the root config (default values match)
        assert_eq!(
            cfg.metrics.cyclomatic_complexity,
            root_config.metrics.cyclomatic_complexity
        );
    }

    #[test]
    fn load_with_inheritance_uses_pkg_config_when_present() {
        let tmp = TempDir::new().unwrap();
        make_falcon_yaml(tmp.path());
        let root_config = FalconConfig::default();
        let cfg = load_with_inheritance(tmp.path(), &root_config);
        // the pkg falcon.yaml overrides cyclomatic_complexity to 5
        assert_eq!(cfg.metrics.cyclomatic_complexity, 5);
    }

    #[test]
    fn load_with_inheritance_falls_back_to_root_on_bad_pkg_config() {
        let tmp = TempDir::new().unwrap();
        // Write an invalid falcon.yaml
        write_file(tmp.path(), "falcon.yaml", "}{invalid yaml");
        let root_config = FalconConfig::default();
        let cfg = load_with_inheritance(tmp.path(), &root_config);
        // falls back to root config
        assert_eq!(
            cfg.metrics.cyclomatic_complexity,
            root_config.metrics.cyclomatic_complexity
        );
    }

    // ── detect_melos ──────────────────────────────────────────────────────────

    #[test]
    fn detect_melos_returns_none_when_no_melos_yaml() {
        let tmp = TempDir::new().unwrap();
        // no melos.yaml
        let result = detect_melos(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn detect_melos_returns_none_when_melos_yaml_has_no_packages() {
        let tmp = TempDir::new().unwrap();
        // melos.yaml exists but default glob "packages/**" matches nothing
        write_file(tmp.path(), "melos.yaml", "name: my_workspace\n");
        let result = detect_melos(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn detect_melos_discovers_packages_via_glob() {
        let tmp = TempDir::new().unwrap();

        // Create packages/pkg_a and packages/pkg_b
        let pkg_a = tmp.path().join("packages").join("pkg_a");
        let pkg_b = tmp.path().join("packages").join("pkg_b");
        fs::create_dir_all(&pkg_a).unwrap();
        fs::create_dir_all(&pkg_b).unwrap();
        make_pubspec(&pkg_a, "pkg_a");
        make_pubspec(&pkg_b, "pkg_b");

        write_file(
            tmp.path(),
            "melos.yaml",
            "name: my_workspace\npackages:\n  - packages/**\n",
        );

        let result = detect_melos(tmp.path());
        assert!(result.is_some());
        let pkgs = result.unwrap();
        assert_eq!(pkgs.len(), 2);
        let names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"pkg_a"));
        assert!(names.contains(&"pkg_b"));
    }

    #[test]
    fn detect_melos_respects_custom_package_globs() {
        let tmp = TempDir::new().unwrap();

        let app = tmp.path().join("apps").join("my_app");
        fs::create_dir_all(&app).unwrap();
        make_pubspec(&app, "my_app");

        write_file(
            tmp.path(),
            "melos.yaml",
            "name: my_workspace\npackages:\n  - apps/**\n",
        );

        let result = detect_melos(tmp.path());
        assert!(result.is_some());
        let pkgs = result.unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "my_app");
    }

    #[test]
    fn detect_melos_package_inherits_pkg_config_over_root() {
        let tmp = TempDir::new().unwrap();

        let pkg = tmp.path().join("packages").join("override_pkg");
        fs::create_dir_all(&pkg).unwrap();
        make_pubspec(&pkg, "override_pkg");
        // pkg-level falcon.yaml with custom threshold
        write_file(
            &pkg,
            "falcon.yaml",
            "metrics:\n  cyclomatic_complexity: 3\n",
        );

        write_file(
            tmp.path(),
            "melos.yaml",
            "name: ws\npackages:\n  - packages/**\n",
        );

        let result = detect_melos(tmp.path());
        assert!(result.is_some());
        let pkgs = result.unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].config.metrics.cyclomatic_complexity, 3);
    }

    // ── detect_pub_workspace ──────────────────────────────────────────────────

    #[test]
    fn detect_pub_workspace_returns_none_when_no_pubspec() {
        let tmp = TempDir::new().unwrap();
        let result = detect_pub_workspace(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn detect_pub_workspace_returns_none_when_no_workspace_key() {
        let tmp = TempDir::new().unwrap();
        make_pubspec(tmp.path(), "root_pkg");
        // no workspace: key
        let result = detect_pub_workspace(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn detect_pub_workspace_returns_none_when_workspace_dirs_missing_pubspecs() {
        let tmp = TempDir::new().unwrap();
        // workspace lists a dir but that dir has no pubspec
        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        // no pubspec in sub
        make_pubspec_with_workspace(tmp.path(), "root", &["sub"]);
        // only root counted → len <= 1 → None
        let result = detect_pub_workspace(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn detect_pub_workspace_discovers_packages() {
        let tmp = TempDir::new().unwrap();

        let pkg_a = tmp.path().join("packages").join("alpha");
        let pkg_b = tmp.path().join("packages").join("beta");
        fs::create_dir_all(&pkg_a).unwrap();
        fs::create_dir_all(&pkg_b).unwrap();
        make_pubspec(&pkg_a, "alpha");
        make_pubspec(&pkg_b, "beta");

        make_pubspec_with_workspace(tmp.path(), "root", &["packages/alpha", "packages/beta"]);

        let result = detect_pub_workspace(tmp.path());
        assert!(result.is_some());
        let pkgs = result.unwrap();
        // root + alpha + beta
        assert_eq!(pkgs.len(), 3);
        let names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"root"));
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn detect_pub_workspace_skips_missing_package_dirs() {
        let tmp = TempDir::new().unwrap();

        let pkg_a = tmp.path().join("packages").join("present");
        fs::create_dir_all(&pkg_a).unwrap();
        make_pubspec(&pkg_a, "present");

        // "missing" dir does not exist
        make_pubspec_with_workspace(
            tmp.path(),
            "root",
            &["packages/present", "packages/missing"],
        );

        let result = detect_pub_workspace(tmp.path());
        assert!(result.is_some());
        let pkgs = result.unwrap();
        // root + present only
        assert_eq!(pkgs.len(), 2);
    }

    // ── detect_workspace ──────────────────────────────────────────────────────

    #[test]
    fn detect_workspace_prefers_melos_over_pub_workspace() {
        let tmp = TempDir::new().unwrap();

        // Set up both melos.yaml and pubspec.yaml with workspace:
        let pkg = tmp.path().join("packages").join("melos_pkg");
        fs::create_dir_all(&pkg).unwrap();
        make_pubspec(&pkg, "melos_pkg");

        write_file(
            tmp.path(),
            "melos.yaml",
            "name: ws\npackages:\n  - packages/**\n",
        );

        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        make_pubspec(&sub, "sub_pkg");
        make_pubspec_with_workspace(tmp.path(), "root", &["sub"]);

        let (ws_type, pkgs) = detect_workspace(tmp.path());
        assert!(matches!(ws_type, WorkspaceType::Melos));
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "melos_pkg");
    }

    #[test]
    fn detect_workspace_falls_back_to_pub_workspace() {
        let tmp = TempDir::new().unwrap();

        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        make_pubspec(&sub, "sub_pkg");
        make_pubspec_with_workspace(tmp.path(), "root", &["sub"]);

        let (ws_type, pkgs) = detect_workspace(tmp.path());
        assert!(matches!(ws_type, WorkspaceType::PubWorkspace));
        assert_eq!(pkgs.len(), 2);
    }

    #[test]
    fn detect_workspace_returns_single_package_when_no_workspace() {
        let tmp = TempDir::new().unwrap();
        make_pubspec(tmp.path(), "lone_pkg");

        let (ws_type, pkgs) = detect_workspace(tmp.path());
        assert!(matches!(ws_type, WorkspaceType::SinglePackage));
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "lone_pkg");
    }

    #[test]
    fn detect_workspace_single_package_path_equals_root() {
        let tmp = TempDir::new().unwrap();
        make_pubspec(tmp.path(), "solo");

        let (_, pkgs) = detect_workspace(tmp.path());
        assert_eq!(pkgs[0].path, tmp.path());
    }

    #[test]
    fn detect_workspace_melos_package_paths_are_absolute() {
        let tmp = TempDir::new().unwrap();

        let pkg_dir = tmp.path().join("packages").join("foo");
        fs::create_dir_all(&pkg_dir).unwrap();
        make_pubspec(&pkg_dir, "foo");

        write_file(
            tmp.path(),
            "melos.yaml",
            "name: ws\npackages:\n  - packages/**\n",
        );

        let (_, pkgs) = detect_workspace(tmp.path());
        assert!(pkgs[0].path.is_absolute());
    }
}
