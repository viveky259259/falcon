//! Toolchain checks. Each one probes the host and, when it can, offers a fix.

pub mod android;
pub mod cocoapods;
pub mod dart;
pub mod flutter;
pub mod xcode;

use crate::doctor::exec::CommandRunner;
use crate::doctor::flutter::releases::ReleaseManifest;
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::{CheckResult, Plan};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct CheckContext {
    pub root: PathBuf,
    pub host: HostInfo,
    pub arch: Arch,
    /// `None` when the release manifest could not be fetched — fixes then
    /// degrade to `Manual` rather than guessing.
    pub manifest: Option<ReleaseManifest>,
    pub runner: Box<dyn CommandRunner>,
}

pub trait Check {
    fn id(&self) -> &'static str;
    fn probe(&self, ctx: &CheckContext) -> CheckResult;

    /// Turn answered questions into an executable plan. Checks whose fix is
    /// deferred to another check (like `dart`) keep the default empty plan.
    fn plan(
        &self,
        _ctx: &CheckContext,
        _decisions: &HashMap<String, String>,
    ) -> Result<Plan, String> {
        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![],
        })
    }
}

#[cfg(test)]
mod invariant_tests {
    use super::*;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::Step;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// A project with every platform directory, so no check short-circuits.
    fn maximal_ctx(dir: &std::path::Path) -> CheckContext {
        for sub in ["ios", "macos", "android/app"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join("ios/Podfile"), "platform :ios, '13.0'\n").unwrap();
        CheckContext {
            root: dir.to_path_buf(),
            host: HostInfo {
                os: "macos".into(),
                arch: "arm64".into(),
                home: Some(PathBuf::from("/Users/ada")),
                shell_rc: None,
                package_managers: vec!["brew".into(), "xcodes".into()],
            },
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(FakeRunner::default()),
        }
    }

    #[test]
    fn no_check_ever_plans_a_sudo_run_step() {
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = maximal_ctx(dir.path());
        let checks: Vec<Box<dyn Check>> = vec![
            Box::new(flutter::FlutterCheck),
            Box::new(dart::DartCheck),
            Box::new(cocoapods::CocoaPodsCheck),
            Box::new(android::AndroidCheck),
            Box::new(xcode::XcodeCheck),
        ];
        for check in checks {
            let Ok(plan) = check.plan(&ctx, &HashMap::new()) else {
                continue;
            };
            for step in &plan.steps {
                if let Step::Run { program, args, .. } = step {
                    assert_ne!(program, "sudo", "{} planned a sudo Run step", check.id());
                    assert!(
                        !args.iter().any(|a| a == "sudo"),
                        "{} planned a sudo argument",
                        check.id()
                    );
                }
            }
        }
    }

    #[test]
    fn no_check_ever_auto_accepts_a_licence() {
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = maximal_ctx(dir.path());
        let checks: Vec<Box<dyn Check>> =
            vec![Box::new(android::AndroidCheck), Box::new(xcode::XcodeCheck)];
        for check in checks {
            let Ok(plan) = check.plan(&ctx, &HashMap::new()) else {
                continue;
            };
            for step in &plan.steps {
                if let Step::Run { args, .. } = step {
                    assert!(
                        !args
                            .iter()
                            .any(|a| a.contains("license") || a.contains("licence")),
                        "{} planned automatic licence acceptance",
                        check.id()
                    );
                }
            }
        }
    }
}
