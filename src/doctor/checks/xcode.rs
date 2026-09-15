//! Xcode and its command-line tools.
//!
//! Installing Xcode needs an Apple ID, and first-launch plus licence
//! acceptance need root. All three are handoffs; Falcon automates only the
//! steps that need neither.

use super::{Check, CheckContext};
use crate::doctor::types::{
    CheckResult, FixKind, FixOffer, Plan, Probe, Status, Step, StepSummary,
};
use std::collections::HashMap;

const APP_STORE_URL: &str = "https://apps.apple.com/us/app/xcode/id497799835";

pub struct XcodeCheck;

/// Pull `16.1` out of `Xcode 16.1\nBuild version 16B40`.
pub fn parse_xcode_version(stdout: &str) -> Option<String> {
    let first = stdout.lines().next()?;
    let token = first.strip_prefix("Xcode ")?.trim();
    token
        .chars()
        .next()
        .filter(|c| c.is_ascii_digit())
        .map(|_| token.to_string())
}

impl Check for XcodeCheck {
    fn id(&self) -> &'static str {
        "xcode"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        if ctx.host.os != "macos" {
            return skipped(self.id(), "Xcode only exists on macOS");
        }
        let needs = ctx.root.join("ios").is_dir() || ctx.root.join("macos").is_dir();
        if !needs {
            return skipped(self.id(), "no ios/ or macos/ directory in this project");
        }

        let found = ctx
            .runner
            .run("xcodebuild", &["-version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .and_then(|o| parse_xcode_version(&o.stdout));

        let status = match &found {
            Some(v) => Status::Ok { version: v.clone() },
            None => Status::Missing,
        };

        let fix = match status {
            Status::Ok { .. } => None,
            _ => Some(FixOffer {
                kind: FixKind::Assisted,
                questions: vec![],
                steps: vec![
                    StepSummary {
                        id: "install-xcode".into(),
                        describe: "You install Xcode — it needs your Apple ID".into(),
                    },
                    StepSummary {
                        id: "select-and-license".into(),
                        describe: "You run the two sudo steps Falcon will print".into(),
                    },
                    StepSummary {
                        id: "first-launch".into(),
                        describe: "Falcon runs xcodebuild -runFirstLaunch".into(),
                    },
                    StepSummary {
                        id: "ios-platform".into(),
                        describe: "Falcon downloads the iOS platform and simulator runtime".into(),
                    },
                ],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec![
                "ios/ or macos/ directory is present, so Apple builds must work".into(),
            ],
            fix,
        }
    }

    fn plan(&self, ctx: &CheckContext, _d: &HashMap<String, String>) -> Result<Plan, String> {
        let has_xcodes = ctx.host.package_managers.iter().any(|m| m == "xcodes");
        let install_command = if has_xcodes {
            "xcodes install --latest".to_string()
        } else {
            format!("open {}", APP_STORE_URL)
        };

        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![
                Step::Handoff {
                    id: "install-xcode".into(),
                    reason: "Installing Xcode requires signing in with your Apple ID, which \
                             Falcon will not do on your behalf"
                        .into(),
                    command: install_command,
                    docs_url: APP_STORE_URL.into(),
                    verify: Probe {
                        program: "xcodebuild".into(),
                        args: vec!["-version".into()],
                    },
                },
                Step::Handoff {
                    id: "select-and-license".into(),
                    reason: "Selecting the toolchain and accepting the licence both need root, \
                             and the licence is a legal agreement only you can accept"
                        .into(),
                    command: "sudo xcode-select -s /Applications/Xcode.app/Contents/Developer && \
                              sudo xcodebuild -license accept"
                        .into(),
                    docs_url: "https://developer.apple.com/support/xcode/".into(),
                    verify: Probe {
                        program: "xcode-select".into(),
                        args: vec!["-p".into()],
                    },
                },
                Step::Run {
                    id: "first-launch".into(),
                    program: "xcodebuild".into(),
                    args: vec!["-runFirstLaunch".into()],
                    cwd: None,
                },
                Step::Run {
                    id: "ios-platform".into(),
                    program: "xcodebuild".into(),
                    args: vec!["-downloadPlatform".into(), "iOS".into()],
                    cwd: None,
                },
            ],
        })
    }
}

fn skipped(id: &str, because: &str) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status: Status::Skipped {
            because: because.to_string(),
        },
        required_by: vec![],
        fix: None,
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

    fn ios_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("ios")).unwrap();
        dir
    }

    #[test]
    fn parses_the_xcode_version() {
        assert_eq!(
            parse_xcode_version("Xcode 16.1\nBuild version 16B40").as_deref(),
            Some("16.1")
        );
    }

    #[test]
    fn unparseable_output_is_none() {
        assert_eq!(
            parse_xcode_version("xcode-select: error: tool not found"),
            None
        );
    }

    #[test]
    fn skipped_on_non_macos() {
        let dir = ios_project();
        let r = XcodeCheck.probe(&ctx(dir.path(), FakeRunner::default(), "linux", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn skipped_when_the_project_has_no_apple_platform() {
        let dir = TempDir::new().unwrap();
        let r = XcodeCheck.probe(&ctx(dir.path(), FakeRunner::default(), "macos", &[]));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn a_macos_only_project_still_needs_xcode() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("macos")).unwrap();
        let runner = FakeRunner::with_stdout("xcodebuild -version", "Xcode 16.1\n");
        let r = XcodeCheck.probe(&ctx(dir.path(), runner, "macos", &[]));
        assert_eq!(
            r.status,
            Status::Ok {
                version: "16.1".into()
            }
        );
    }

    #[test]
    fn missing_xcode_is_an_assisted_fix() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let r = XcodeCheck.probe(&ctx(dir.path(), runner, "macos", &[]));
        assert_eq!(r.status, Status::Missing);
        assert_eq!(r.fix.unwrap().kind, FixKind::Assisted);
    }

    #[test]
    fn the_install_handoff_prefers_xcodes_when_it_is_available() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &["xcodes"]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Handoff { command, .. } => assert!(
                command.contains("xcodes install"),
                "should suggest xcodes: {}",
                command
            ),
            other => panic!("expected a handoff, got {:?}", other),
        }
    }

    #[test]
    fn without_xcodes_the_handoff_points_at_the_app_store() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &[]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Handoff {
                reason, docs_url, ..
            } => {
                assert!(reason.contains("Apple ID"), "must explain why: {}", reason);
                assert!(
                    docs_url.contains("apps.apple.com") || docs_url.contains("developer.apple.com")
                );
            }
            other => panic!("expected a handoff, got {:?}", other),
        }
    }

    #[test]
    fn every_sudo_step_is_a_handoff_never_a_run() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &["xcodes"]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        for step in &plan.steps {
            if let Step::Run { program, args, .. } = step {
                assert_ne!(program, "sudo", "sudo must never be a Run step");
                assert!(!args.iter().any(|a| a == "sudo"));
            }
        }
        assert!(
            plan.steps
                .iter()
                .any(|s| matches!(s, Step::Handoff { command, .. } if command.contains("sudo"))),
            "the sudo steps must appear as handoffs"
        );
    }

    #[test]
    fn the_licence_is_never_accepted_automatically() {
        let dir = ios_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("xcodebuild -version");
        let c = ctx(dir.path(), runner, "macos", &["xcodes"]);
        let plan = XcodeCheck.plan(&c, &HashMap::new()).unwrap();
        for step in &plan.steps {
            if let Step::Run { args, .. } = step {
                assert!(
                    !args.iter().any(|a| a == "-license"),
                    "licence acceptance must be a handoff"
                );
            }
        }
    }
}
