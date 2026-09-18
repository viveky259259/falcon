//! Toolchain checks. Each one probes the host and, when it can, offers a fix.

pub mod android;
pub mod cocoapods;
pub mod dart;
pub mod flutter;
pub mod xcode;

use crate::doctor::exec::CommandRunner;
use crate::doctor::flutter::releases::ReleaseManifest;
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::{CheckResult, Plan, Status};
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

/// Build a `Skipped` result. Shared by every check that is only relevant to
/// some projects (`android`, `xcode`, `cocoapods`) so "not needed here" is
/// reported identically everywhere.
pub(crate) fn skipped(id: &str, because: &str) -> CheckResult {
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
mod invariant_tests {
    use super::*;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::flutter::releases::ReleaseManifest;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::Step;
    use std::collections::HashMap;
    use std::path::PathBuf;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    /// A project with every platform directory, so no check short-circuits.
    ///
    /// Carries a real, parsed release manifest — `FlutterCheck::plan()`
    /// returns `Err` immediately when `ctx.manifest` is `None`, and an `Err`
    /// must never look like a pass in these tests (see `plan_or_fail`).
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
            manifest: Some(ReleaseManifest::parse(FIXTURE).unwrap()),
            runner: Box::new(FakeRunner::default()),
        }
    }

    /// Decisions that let every check in the registry build a *complete*
    /// plan under `maximal_ctx` — in particular `flutter.channel` /
    /// `flutter.version`, naming a release the fixture actually has, so
    /// `FlutterCheck::plan()` does not error out on an unresolved version
    /// and get mistaken for "nothing to check".
    fn full_decisions() -> HashMap<String, String> {
        let mut d = HashMap::new();
        d.insert("flutter.channel".to_string(), "stable".to_string());
        d.insert("flutter.version".to_string(), "3.24.5".to_string());
        d
    }

    /// `plan()` returning `Err` must be a hard test failure, not a silently
    /// skipped check — a check that errors under this context looks
    /// identical to one that produced an empty, harmless plan unless we
    /// name it and panic. This is what makes the two invariants below
    /// actually cover every check in `checks`, instead of only the ones
    /// whose `plan()` happens to succeed.
    fn plan_or_fail(
        check: &dyn Check,
        ctx: &CheckContext,
        decisions: &HashMap<String, String>,
    ) -> Plan {
        check.plan(ctx, decisions).unwrap_or_else(|e| {
            panic!(
                "{} could not build a plan in the invariant test: {e}",
                check.id()
            )
        })
    }

    #[test]
    fn no_check_ever_plans_a_sudo_run_step() {
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = maximal_ctx(dir.path());
        let decisions = full_decisions();
        // Iterates the real registry (`crate::doctor::registry()`) rather
        // than a hand-copied `vec![]` of checks — a sixth check added there
        // but not duplicated here would otherwise escape this invariant
        // with no compile error, no test failure, nothing.
        for check in crate::doctor::registry() {
            let plan = plan_or_fail(check.as_ref(), &ctx, &decisions);
            for step in &plan.steps {
                if let Step::Run { program, args, .. } = step {
                    assert!(
                        !program.contains("sudo"),
                        "{} planned a sudo program: {:?}",
                        check.id(),
                        step
                    );
                    assert!(
                        !args.iter().any(|a| a.contains("sudo")),
                        "{} planned a sudo argument: {:?}",
                        check.id(),
                        step
                    );
                }
            }
        }
    }

    #[test]
    fn no_check_ever_auto_accepts_a_licence() {
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = maximal_ctx(dir.path());
        let decisions = full_decisions();
        for check in crate::doctor::registry() {
            let plan = plan_or_fail(check.as_ref(), &ctx, &decisions);
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
