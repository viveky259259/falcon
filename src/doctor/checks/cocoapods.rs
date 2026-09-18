//! CocoaPods, needed only when the project has an iOS Podfile.

use super::{skipped, Check, CheckContext};
use crate::doctor::types::{
    CheckResult, FixKind, FixOffer, Plan, Probe, Status, Step, StepSummary,
};
use std::collections::HashMap;

pub struct CocoaPodsCheck;

fn installer(ctx: &CheckContext) -> Option<(&'static str, Vec<String>, &'static str)> {
    let has = |p: &str| ctx.host.package_managers.iter().any(|m| m == p);
    if has("brew") {
        Some((
            "brew",
            vec!["install".into(), "cocoapods".into()],
            "Homebrew is available — the least surprising install on macOS",
        ))
    } else if has("gem") {
        Some((
            "gem",
            vec![
                "install".into(),
                "cocoapods".into(),
                "--user-install".into(),
            ],
            "installs into your user gem directory, leaving system Ruby untouched",
        ))
    } else {
        None
    }
}

impl Check for CocoaPodsCheck {
    fn id(&self) -> &'static str {
        "cocoapods"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        let podfile = ctx.root.join("ios").join("Podfile");
        if ctx.host.os != "macos" {
            return skipped(self.id(), "CocoaPods is only needed on macOS");
        }
        if !podfile.is_file() {
            return skipped(self.id(), "no ios/Podfile in this project");
        }

        let found = ctx
            .runner
            .run("pod", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .map(|o| o.stdout.trim().to_string())
            .filter(|v| !v.is_empty());

        let status = match &found {
            Some(v) => Status::Ok { version: v.clone() },
            None => Status::Missing,
        };

        let fix = match (&status, installer(ctx)) {
            (Status::Ok { .. }, _) => None,
            (_, Some((program, args, why))) => Some(FixOffer {
                kind: FixKind::Automatic,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "install".into(),
                    describe: format!("{} {} — {}", program, args.join(" "), why),
                }],
            }),
            (_, None) => Some(FixOffer {
                kind: FixKind::Manual,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "manual".into(),
                    describe:
                        "Install CocoaPods — https://guides.cocoapods.org/using/getting-started.html"
                            .into(),
                }],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec!["ios/Podfile is present, so pod install must be able to run".into()],
            fix,
        }
    }

    fn plan(
        &self,
        ctx: &CheckContext,
        _decisions: &HashMap<String, String>,
    ) -> Result<Plan, String> {
        let (program, args, _) = installer(ctx)
            .ok_or_else(|| "no supported package manager for CocoaPods on this host".to_string())?;
        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![
                Step::Run {
                    id: "install".into(),
                    program: program.into(),
                    args,
                    cwd: None,
                },
                Step::Verify {
                    id: "verify".into(),
                    probe: Probe {
                        program: "pod".into(),
                        args: vec!["--version".into()],
                    },
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::checks::CheckContext;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::{FixKind, Status, Step};
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn ctx(root: &Path, runner: FakeRunner, os: &str, pms: &[&str]) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: HostInfo {
                os: os.into(),
                arch: "arm64".into(),
                home: Some(PathBuf::from("/Users/ada")),
                shell_rc: None,
                package_managers: pms.iter().map(|s| s.to_string()).collect(),
            },
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(runner),
        }
    }

    fn with_podfile() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("ios")).unwrap();
        fs::write(dir.path().join("ios/Podfile"), "platform :ios, '13.0'\n").unwrap();
        dir
    }

    #[test]
    fn skipped_when_the_project_has_no_podfile() {
        let dir = TempDir::new().unwrap();
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), FakeRunner::default(), "macos", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
        assert!(r.fix.is_none());
    }

    #[test]
    fn skipped_on_non_macos_even_with_a_podfile() {
        let dir = with_podfile();
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), FakeRunner::default(), "linux", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn present_cocoapods_reports_ok() {
        let dir = with_podfile();
        let runner = FakeRunner::with_stdout("pod --version", "1.15.2\n");
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), runner, "macos", &["brew"]));
        assert_eq!(
            r.status,
            Status::Ok {
                version: "1.15.2".into()
            }
        );
    }

    #[test]
    fn missing_cocoapods_is_an_automatic_fix_and_names_the_podfile() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), runner, "macos", &["brew"]));
        assert_eq!(r.status, Status::Missing);
        assert_eq!(r.fix.unwrap().kind, FixKind::Automatic);
        assert!(r.required_by.iter().any(|s| s.contains("Podfile")));
    }

    #[test]
    fn brew_is_preferred_when_available() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let c = ctx(dir.path(), runner, "macos", &["brew", "gem"]);
        let plan = CocoaPodsCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Run { program, args, .. } => {
                assert_eq!(program, "brew");
                assert_eq!(args, &vec!["install".to_string(), "cocoapods".to_string()]);
            }
            other => panic!("expected brew install, got {:?}", other),
        }
    }

    #[test]
    fn gem_install_is_user_scoped_never_sudo() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let c = ctx(dir.path(), runner, "macos", &["gem"]);
        let plan = CocoaPodsCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Run { program, args, .. } => {
                assert_eq!(program, "gem");
                assert!(
                    args.contains(&"--user-install".to_string()),
                    "must never install into system Ruby: {:?}",
                    args
                );
                assert_ne!(program, "sudo");
            }
            other => panic!("expected gem install, got {:?}", other),
        }
    }

    #[test]
    fn no_package_manager_yields_a_manual_fix() {
        let dir = with_podfile();
        let mut runner = FakeRunner::default();
        runner.fail_on("pod --version");
        let r = CocoaPodsCheck.probe(&ctx(dir.path(), runner, "macos", &[]));
        assert_eq!(r.fix.unwrap().kind, FixKind::Manual);
    }
}
