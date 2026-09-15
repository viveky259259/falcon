//! Turns install decisions into an executable plan. Builds the plan only —
//! running it is the executor's job, which is what makes `--dry-run` free.

use super::releases::{channel_has_archives, ReleaseManifest};
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::{Plan, Probe, Step};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallDecisions {
    pub channel: String,
    pub version: String,
    pub dir: PathBuf,
}

pub fn default_install_dir(home: Option<&Path>) -> PathBuf {
    match home {
        Some(h) => h.join("development").join("flutter"),
        None => PathBuf::from("flutter"),
    }
}

/// Refuse to install over an existing non-empty directory.
pub fn dir_is_usable(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    if !dir.is_dir() {
        return Err(format!("{} exists and is not a directory", dir.display()));
    }
    let mut entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    if entries.next().is_some() {
        return Err(format!(
            "{} is not empty — remove it or pass --dir to install elsewhere",
            dir.display()
        ));
    }
    Ok(())
}

pub fn path_export_line(dir: &Path) -> String {
    format!(r#"export PATH="$PATH:{}/bin""#, dir.display())
}

pub fn build_plan(
    manifest: &ReleaseManifest,
    host: &HostInfo,
    arch: Arch,
    decisions: &InstallDecisions,
    scratch: &Path,
) -> Result<Plan, String> {
    let flutter_bin = decisions.dir.join("bin").join("flutter");
    let flutter_bin_s = flutter_bin.to_string_lossy().to_string();

    let mut steps: Vec<Step> = Vec::new();

    if channel_has_archives(&decisions.channel) {
        let release = manifest
            .find(&decisions.channel, &decisions.version, arch)
            .ok_or_else(|| {
                format!(
                    "no {} {} build for {} on the {} channel",
                    decisions.version,
                    arch.manifest_name(),
                    host.os,
                    decisions.channel
                )
            })?;
        let archive_name = Path::new(&release.archive)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "flutter-sdk-archive".to_string());
        let archive_path = scratch.join(archive_name);
        let parent = decisions
            .dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        steps.push(Step::Download {
            id: "download".into(),
            url: manifest.archive_url(release),
            sha256: release.sha256.clone(),
            dest: archive_path.clone(),
        });
        steps.push(Step::Extract {
            id: "extract".into(),
            archive: archive_path,
            dest: parent,
        });
    } else {
        // master ships no archive; clone it instead.
        steps.push(Step::Run {
            id: "clone".into(),
            program: "git".into(),
            args: vec![
                "clone".into(),
                "--depth".into(),
                "1".into(),
                "-b".into(),
                decisions.channel.clone(),
                "https://github.com/flutter/flutter.git".into(),
                decisions.dir.to_string_lossy().to_string(),
            ],
            cwd: None,
        });
    }

    // Priming downloads the bundled Dart SDK and proves the binary runs.
    steps.push(Step::Run {
        id: "prime".into(),
        program: flutter_bin_s.clone(),
        args: vec!["--version".into()],
        cwd: None,
    });
    steps.push(Step::Verify {
        id: "verify".into(),
        probe: Probe {
            program: flutter_bin_s,
            args: vec!["--version".into()],
        },
    });
    steps.push(Step::PathHint {
        id: "path-hint".into(),
        dir: decisions.dir.clone(),
        rc_file: host.shell_rc.clone(),
    });

    Ok(Plan {
        check_id: "flutter".into(),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::flutter::releases::ReleaseManifest;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::Step;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn manifest() -> ReleaseManifest {
        ReleaseManifest::parse(FIXTURE).unwrap()
    }

    fn host() -> HostInfo {
        HostInfo {
            os: "macos".into(),
            arch: "arm64".into(),
            home: Some(PathBuf::from("/Users/ada")),
            shell_rc: Some(PathBuf::from("/Users/ada/.zshrc")),
            package_managers: vec![],
        }
    }

    fn decisions(channel: &str, version: &str) -> InstallDecisions {
        InstallDecisions {
            channel: channel.into(),
            version: version.into(),
            dir: PathBuf::from("/Users/ada/development/flutter"),
        }
    }

    #[test]
    fn default_dir_is_under_home() {
        assert_eq!(
            default_install_dir(Some(Path::new("/Users/ada"))),
            PathBuf::from("/Users/ada/development/flutter")
        );
    }

    #[test]
    fn default_dir_without_home_is_relative_to_cwd() {
        assert_eq!(default_install_dir(None), PathBuf::from("flutter"));
    }

    #[test]
    fn missing_directory_is_usable() {
        let dir = TempDir::new().unwrap();
        assert!(dir_is_usable(&dir.path().join("nope")).is_ok());
    }

    #[test]
    fn empty_directory_is_usable() {
        let dir = TempDir::new().unwrap();
        assert!(dir_is_usable(dir.path()).is_ok());
    }

    #[test]
    fn non_empty_directory_is_refused() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("something"), "x").unwrap();
        let err = dir_is_usable(dir.path()).unwrap_err();
        assert!(err.contains("not empty"), "unhelpful error: {}", err);
    }

    #[test]
    fn plan_downloads_extracts_verifies_and_hints_path() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "3.24.5"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        let ids: Vec<_> = plan.steps.iter().map(|s| s.id().to_string()).collect();
        assert_eq!(
            ids,
            vec!["download", "extract", "prime", "verify", "path-hint"],
            "plan step order changed"
        );
        assert_eq!(plan.check_id, "flutter");
    }

    #[test]
    fn plan_uses_the_arch_specific_archive_and_its_checksum() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "3.24.5"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        match &plan.steps[0] {
            Step::Download { url, sha256, .. } => {
                assert!(url.contains("arm64"), "wrong architecture archive: {}", url);
                assert_eq!(sha256, &"1".repeat(64));
            }
            other => panic!("expected a download step, got {:?}", other),
        }
    }

    #[test]
    fn master_channel_falls_back_to_a_git_clone() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("master", "master"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        match &plan.steps[0] {
            Step::Run { program, args, .. } => {
                assert_eq!(program, "git");
                assert!(args.contains(&"master".to_string()), "args were {:?}", args);
            }
            other => panic!("expected a git clone, got {:?}", other),
        }
        assert!(
            !plan
                .steps
                .iter()
                .any(|s| matches!(s, Step::Download { .. })),
            "master has no published archive"
        );
    }

    #[test]
    fn unknown_version_is_an_error_not_a_panic() {
        let err = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "9.9.9"),
            Path::new("/tmp/scratch"),
        )
        .unwrap_err();
        assert!(
            err.contains("9.9.9"),
            "error should name the version: {}",
            err
        );
    }

    #[test]
    fn plan_never_contains_sudo() {
        let plan = build_plan(
            &manifest(),
            &host(),
            Arch::Arm64,
            &decisions("stable", "3.24.5"),
            Path::new("/tmp/scratch"),
        )
        .unwrap();
        for step in &plan.steps {
            if let Step::Run { program, args, .. } = step {
                assert_ne!(program, "sudo");
                assert!(!args.iter().any(|a| a == "sudo"));
            }
        }
    }

    #[test]
    fn export_line_points_at_the_sdk_bin_directory() {
        let line = path_export_line(Path::new("/Users/ada/development/flutter"));
        assert_eq!(
            line,
            r#"export PATH="$PATH:/Users/ada/development/flutter/bin""#
        );
    }
}
