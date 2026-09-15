//! Is a usable Flutter SDK present, and if not, what would installing one take?

use super::{Check, CheckContext};
use crate::doctor::flutter::install::default_install_dir;
use crate::doctor::flutter::releases::{ReleaseManifest, CHANNELS};
use crate::doctor::flutter::version::{candidates, read_pubspec_constraints};
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::{CheckResult, Choice, FixKind, FixOffer, Question, Status, StepSummary};
use crate::version_util::satisfies;
use std::path::Path;

pub struct FlutterCheck;

/// Pull `3.24.5` out of `Flutter 3.24.5 • channel stable • …`.
pub fn parse_flutter_version(stdout: &str) -> Option<String> {
    let token = stdout.split_whitespace().nth(1)?;
    token
        .chars()
        .next()
        .filter(|c| c.is_ascii_digit())
        .map(|_| token.to_string())
}

impl Check for FlutterCheck {
    fn id(&self) -> &'static str {
        "flutter"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        let required_by = vec!["every Falcon command that builds or runs the app".to_string()];
        let constraints = read_pubspec_constraints(&ctx.root);

        let found = ctx
            .runner
            .run("flutter", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .and_then(|o| parse_flutter_version(&o.stdout));

        let status = match &found {
            None => Status::Missing,
            Some(v) => match &constraints.flutter {
                Some(c) if !satisfies(v, c) => Status::Outdated {
                    found: v.clone(),
                    needed: c.clone(),
                },
                _ => Status::Ok { version: v.clone() },
            },
        };

        let fix = match status {
            Status::Ok { .. } => None,
            _ => Some(build_offer(
                &ctx.root,
                ctx.manifest.as_ref(),
                ctx.arch,
                &ctx.host,
            )),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by,
            fix,
        }
    }

    fn plan(
        &self,
        ctx: &CheckContext,
        decisions: &std::collections::HashMap<String, String>,
    ) -> Result<crate::doctor::types::Plan, String> {
        let manifest = ctx.manifest.as_ref().ok_or_else(|| {
            "Flutter release manifest unavailable; install manually from \
             https://docs.flutter.dev/get-started/install"
                .to_string()
        })?;
        let dir = decisions
            .get("flutter.dir")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                crate::doctor::flutter::install::default_install_dir(ctx.host.home.as_deref())
            });
        let install_decisions = crate::doctor::flutter::install::InstallDecisions {
            channel: decisions
                .get("flutter.channel")
                .cloned()
                .unwrap_or_else(|| "stable".into()),
            version: decisions
                .get("flutter.version")
                .cloned()
                .unwrap_or_else(|| "latest".into()),
            dir,
        };
        let scratch = std::env::temp_dir().join("falcon-doctor");
        crate::doctor::flutter::install::build_plan(
            manifest,
            &ctx.host,
            ctx.arch,
            &install_decisions,
            &scratch,
        )
    }
}

fn build_offer(
    root: &Path,
    manifest: Option<&ReleaseManifest>,
    arch: Arch,
    host: &HostInfo,
) -> FixOffer {
    let Some(manifest) = manifest else {
        return FixOffer {
            kind: FixKind::Manual,
            questions: vec![],
            steps: vec![StepSummary {
                id: "manual".into(),
                describe: "Could not reach the Flutter release manifest. \
                           Install manually from https://docs.flutter.dev/get-started/install"
                    .into(),
            }],
        };
    };

    FixOffer {
        kind: FixKind::Automatic,
        questions: flutter_questions(root, manifest, arch, host),
        steps: vec![
            StepSummary {
                id: "download".into(),
                describe: "Download the SDK archive".into(),
            },
            StepSummary {
                id: "extract".into(),
                describe: "Verify its checksum and extract".into(),
            },
            StepSummary {
                id: "prime".into(),
                describe: "Run flutter --version once".into(),
            },
            StepSummary {
                id: "path-hint".into(),
                describe: "Print the PATH line to add".into(),
            },
        ],
    }
}

pub fn flutter_questions(
    root: &Path,
    manifest: &ReleaseManifest,
    arch: Arch,
    host: &HostInfo,
) -> Vec<Question> {
    let channel_q = Question {
        id: "flutter.channel".into(),
        prompt: "Which Flutter channel?".into(),
        options: CHANNELS
            .iter()
            .map(|c| Choice {
                value: c.to_string(),
                label: c.to_string(),
                rationale: match *c {
                    "stable" => "production-ready; what almost every project wants".into(),
                    "beta" => "next release, broadly usable, occasional breakage".into(),
                    _ => "bleeding edge, built from source, not recommended for production".into(),
                },
                recommended: *c == "stable",
            })
            .collect(),
        default: Some("stable".into()),
    };

    // Known limitation: the version list is computed for stable, because the
    // channel question has not been answered yet when the offer is built. If
    // the caller picks beta and then a stable-only version, `build_plan`
    // refuses with an error naming both the version and the channel — the
    // failure is loud, not a wrong install. Widening this to per-channel
    // option groups is a follow-up, not part of this plan.
    let found = candidates(root, manifest, "stable", arch);
    let version_q = Question {
        id: "flutter.version".into(),
        prompt: "Which Flutter version?".into(),
        options: found
            .iter()
            .enumerate()
            .map(|(i, c)| Choice {
                value: c.version.clone(),
                label: c.version.clone(),
                rationale: c.rationale.clone(),
                recommended: i == 0,
            })
            .collect(),
        default: found.first().map(|c| c.version.clone()),
    };

    let dir = default_install_dir(host.home.as_deref());
    let dir_q = Question {
        id: "flutter.dir".into(),
        prompt: "Where should the SDK be installed?".into(),
        options: vec![Choice {
            value: dir.to_string_lossy().to_string(),
            label: dir.to_string_lossy().to_string(),
            rationale: "the location flutter.dev's own instructions use".into(),
            recommended: true,
        }],
        default: Some(dir.to_string_lossy().to_string()),
    };

    vec![channel_q, version_q, dir_q]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::flutter::releases::ReleaseManifest;
    use crate::doctor::host::{Arch, HostInfo};
    use crate::doctor::types::Status;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn host() -> HostInfo {
        HostInfo {
            os: "macos".into(),
            arch: "arm64".into(),
            home: Some(PathBuf::from("/Users/ada")),
            shell_rc: None,
            package_managers: vec![],
        }
    }

    fn ctx(root: &Path, runner: FakeRunner) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: host(),
            arch: Arch::Arm64,
            manifest: Some(ReleaseManifest::parse(FIXTURE).unwrap()),
            runner: Box::new(runner),
        }
    }

    #[test]
    fn parses_the_version_out_of_flutter_version_output() {
        let out = "Flutter 3.24.5 • channel stable • https://github.com/flutter/flutter.git";
        assert_eq!(parse_flutter_version(out).as_deref(), Some("3.24.5"));
    }

    #[test]
    fn unparseable_version_output_is_none() {
        assert_eq!(parse_flutter_version("command not found"), None);
    }

    #[test]
    fn missing_flutter_offers_an_automatic_fix() {
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(result.status, Status::Missing);
        let fix = result.fix.expect("a missing SDK must offer a fix");
        assert_eq!(fix.kind, FixKind::Automatic);
        let ids: Vec<_> = fix.questions.iter().map(|q| q.id.clone()).collect();
        assert_eq!(
            ids,
            vec!["flutter.channel", "flutter.version", "flutter.dir"]
        );
    }

    #[test]
    fn channel_question_offers_stable_beta_and_master_with_stable_recommended() {
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        let q = &result.fix.unwrap().questions[0];
        let values: Vec<_> = q.options.iter().map(|o| o.value.clone()).collect();
        assert_eq!(values, vec!["stable", "beta", "master"]);
        assert!(q.options[0].recommended);
        assert!(
            q.options[2]
                .rationale
                .contains("not recommended for production"),
            "master must be labelled: {:?}",
            q.options[2].rationale
        );
    }

    #[test]
    fn version_options_carry_a_rationale_for_every_choice() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        let q = &result.fix.unwrap().questions[1];
        assert!(q.options.len() >= 2);
        assert!(q.options.iter().all(|o| !o.rationale.is_empty()));
        assert_eq!(q.options.iter().filter(|o| o.recommended).count(), 1);
    }

    #[test]
    fn a_working_flutter_reports_ok_and_offers_no_fix() {
        let dir = TempDir::new().unwrap();
        let runner = FakeRunner::with_stdout(
            "flutter --version",
            "Flutter 3.24.5 • channel stable • https://github.com/flutter/flutter.git",
        );
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(
            result.status,
            Status::Ok {
                version: "3.24.5".into()
            }
        );
        assert!(result.fix.is_none());
    }

    #[test]
    fn a_version_below_the_pubspec_constraint_is_outdated() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\nenvironment:\n  flutter: '>=3.22.0'\n",
        )
        .unwrap();
        let runner = FakeRunner::with_stdout(
            "flutter --version",
            "Flutter 3.19.0 • channel stable • https://github.com/flutter/flutter.git",
        );
        let result = FlutterCheck.probe(&ctx(dir.path(), runner));
        assert_eq!(
            result.status,
            Status::Outdated {
                found: "3.19.0".into(),
                needed: ">=3.22.0".into()
            }
        );
        assert!(
            result.fix.is_some(),
            "an outdated SDK must still offer an upgrade"
        );
    }

    #[test]
    fn without_a_manifest_the_fix_degrades_to_manual() {
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let mut c = ctx(dir.path(), runner);
        c.manifest = None;
        let result = FlutterCheck.probe(&c);
        assert_eq!(result.status, Status::Missing);
        assert_eq!(result.fix.unwrap().kind, FixKind::Manual);
    }
}
