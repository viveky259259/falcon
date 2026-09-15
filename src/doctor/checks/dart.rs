//! Dart SDK presence. In a Flutter project this is satisfied by installing
//! Flutter, which bundles Dart at `bin/cache/dart-sdk` — so this check defers
//! rather than downloading a second SDK.

use super::{Check, CheckContext};
use crate::doctor::types::{CheckResult, FixKind, FixOffer, Status, StepSummary};
use std::path::Path;

pub struct DartCheck;

/// Pull `3.5.4` out of `Dart SDK version: 3.5.4 (stable) …`.
pub fn parse_dart_version(stdout: &str) -> Option<String> {
    let token = stdout.split_whitespace().nth(3)?;
    token
        .chars()
        .next()
        .filter(|c| c.is_ascii_digit())
        .map(|_| token.to_string())
}

fn is_flutter_project(root: &Path) -> bool {
    std::fs::read_to_string(root.join("pubspec.yaml"))
        .map(|t| t.contains("sdk: flutter") || t.contains("flutter:"))
        .unwrap_or(false)
}

impl Check for DartCheck {
    fn id(&self) -> &'static str {
        "dart"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        let found = ctx
            .runner
            .run("dart", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .and_then(|o| {
                // `dart --version` writes to stdout on modern SDKs, stderr on old ones.
                parse_dart_version(&o.stdout).or_else(|| parse_dart_version(&o.stderr))
            });

        let status = match &found {
            Some(v) => Status::Ok { version: v.clone() },
            None => Status::Missing,
        };

        let fix = match status {
            Status::Ok { .. } => None,
            _ if is_flutter_project(&ctx.root) => Some(FixOffer {
                kind: FixKind::Automatic,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "deferred".into(),
                    describe: "Will be satisfied by installing Flutter, which bundles Dart".into(),
                }],
            }),
            _ => Some(FixOffer {
                kind: FixKind::Manual,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "manual".into(),
                    describe: "Install the standalone Dart SDK — https://dart.dev/get-dart".into(),
                }],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec!["`dart analyze`, used as Falcon's semantic co-pilot".into()],
            fix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::checks::CheckContext;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::{FixKind, Status};
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn ctx(root: &Path, runner: FakeRunner) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: HostInfo {
                os: "macos".into(),
                arch: "arm64".into(),
                home: Some(PathBuf::from("/Users/ada")),
                shell_rc: None,
                package_managers: vec![],
            },
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(runner),
        }
    }

    #[test]
    fn parses_dart_version_output() {
        let out = "Dart SDK version: 3.5.4 (stable) on \"macos_arm64\"";
        assert_eq!(parse_dart_version(out).as_deref(), Some("3.5.4"));
    }

    #[test]
    fn unparseable_output_is_none() {
        assert_eq!(parse_dart_version("nope"), None);
    }

    #[test]
    fn present_dart_reports_ok() {
        let dir = TempDir::new().unwrap();
        let runner = FakeRunner::with_stdout("dart --version", "Dart SDK version: 3.5.4 (stable)");
        let r = DartCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(
            r.status,
            Status::Ok {
                version: "3.5.4".into()
            }
        );
        assert!(r.fix.is_none());
    }

    #[test]
    fn a_flutter_project_defers_to_the_flutter_fixer() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
        )
        .unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("dart --version");
        let r = DartCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(r.status, Status::Missing);
        let fix = r.fix.expect("must explain how it gets fixed");
        assert_eq!(fix.kind, FixKind::Automatic);
        assert!(
            fix.steps[0].describe.contains("installing Flutter"),
            "must defer to the Flutter fixer, not download a second SDK: {:?}",
            fix.steps[0].describe
        );
        assert!(
            fix.questions.is_empty(),
            "deferred fix asks nothing of its own"
        );
    }

    #[test]
    fn a_pure_dart_project_gets_a_manual_fix() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("pubspec.yaml"), "name: cli_tool\n").unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("dart --version");
        let r = DartCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(r.fix.unwrap().kind, FixKind::Manual);
    }
}
