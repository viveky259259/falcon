//! AGENTS.md generators.
//!
//! Two outputs:
//! 1. A root AGENTS.md for the falcon repo itself, mirroring the conventions
//!    in CLAUDE.md so non-Claude agents (Codex, Cursor, etc.) follow the same
//!    rules when editing falcon.
//! 2. Per-feature AGENTS.md inside a target Flutter app — one file at the root
//!    of each detected feature directory, telling agents how to operate within
//!    that feature (lint preset, test entry points, falcon commands to run).

pub mod features;
pub mod render;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Top-level result of `falcon agents init`.
#[derive(Debug, Default)]
pub struct InitReport {
    pub root_written: Option<PathBuf>,
    pub feature_files: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

/// Run the agents init flow against `path`.
///
/// - Always writes `<path>/AGENTS.md`.
/// - If `<path>/pubspec.yaml` exists OR `<path>/lib/` exists, also discovers
///   feature dirs and writes one AGENTS.md per feature.
/// - `force = true` overwrites; otherwise existing files are skipped.
pub fn run_init(path: &Path, force: bool) -> Result<InitReport> {
    let mut report = InitReport::default();

    let is_flutter = path.join("pubspec.yaml").is_file() || path.join("lib").is_dir();

    let root_md = path.join("AGENTS.md");
    let root_content = if is_flutter {
        render::render_root_flutter_agents_md(path)
    } else {
        render::render_root_falcon_agents_md()
    };
    if write_if_allowed(&root_md, &root_content, force)? {
        report.root_written = Some(root_md);
    } else {
        report.skipped.push(root_md);
    }

    if is_flutter {
        let features = features::discover_features(path);
        for feat in features {
            let target = feat.dir.join("AGENTS.md");
            let content = render::render_feature_agents_md(&feat);
            if write_if_allowed(&target, &content, force)? {
                report.feature_files.push(target);
            } else {
                report.skipped.push(target);
            }
        }
    }

    Ok(report)
}

fn write_if_allowed(target: &Path, content: &str, force: bool) -> Result<bool> {
    if target.exists() && !force {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating parent dir for {}", target.display()))?;
    }
    std::fs::write(target, content)
        .with_context(|| format!("writing {}", target.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn writes_root_agents_for_non_flutter_repo() {
        let dir = tempdir().unwrap();
        let report = run_init(dir.path(), false).unwrap();
        assert!(report.root_written.is_some());
        assert!(report.feature_files.is_empty());
        let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(content.contains("falcon"));
    }

    #[test]
    fn writes_root_and_features_for_flutter_app() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("pubspec.yaml"), "name: demo\n").unwrap();
        fs::create_dir_all(root.join("lib/features/auth")).unwrap();
        fs::write(root.join("lib/features/auth/login.dart"), "// auth").unwrap();
        fs::create_dir_all(root.join("lib/features/profile")).unwrap();
        fs::write(root.join("lib/features/profile/profile.dart"), "// p").unwrap();

        let report = run_init(root, false).unwrap();
        assert!(report.root_written.is_some());
        assert_eq!(report.feature_files.len(), 2, "got {:?}", report.feature_files);
    }

    #[test]
    fn does_not_overwrite_without_force() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "EXISTING").unwrap();
        let report = run_init(dir.path(), false).unwrap();
        assert!(report.root_written.is_none());
        assert_eq!(report.skipped.len(), 1);
        let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert_eq!(content, "EXISTING");
    }

    #[test]
    fn force_overwrites_existing() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "EXISTING").unwrap();
        let report = run_init(dir.path(), true).unwrap();
        assert!(report.root_written.is_some());
        let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert_ne!(content, "EXISTING");
    }
}
