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

        let fix = match &status {
            Status::Ok { .. } => None,
            // `Status::Outdated` is only ever produced above when `found`
            // is `Some` — i.e. a Flutter binary is already on PATH. Offering
            // the same fresh-install plan here either hard-refuses (the
            // default install dir is occupied by the SDK we just found) or
            // silently lands a second SDK in a directory PATH never
            // resolves, leaving the machine looking untouched. A real
            // in-place upgrade is a follow-up; for now, say what actually
            // works.
            Status::Outdated { .. } => Some(FixOffer {
                kind: FixKind::Manual,
                questions: vec![],
                steps: vec![StepSummary {
                    id: "manual-upgrade".into(),
                    describe: "Run `flutter upgrade` to update the SDK already on PATH".into(),
                }],
            }),
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
        let channel = decisions
            .get("flutter.channel")
            .cloned()
            .unwrap_or_else(|| "stable".into());
        // The CLI's `--channel` flag is a `clap::ValueEnum` and rejects a
        // typo before this ever runs, but the MCP `doctor` tool takes
        // `decisions` as free-form strings — that surface has no such
        // gate. Left unchecked, `channel_has_archives` returns false for
        // any unrecognised channel and the plan falls straight through to
        // a `git clone -b <channel>` against a branch that does not exist,
        // and the typo only surfaces as a confusing git failure mid-install.
        if !CHANNELS.contains(&channel.as_str()) {
            return Err(format!(
                "unknown Flutter channel {:?} — valid channels are: {}",
                channel,
                CHANNELS.join(", ")
            ));
        }
        let requested_version = decisions
            .get("flutter.version")
            .cloned()
            .unwrap_or_else(|| "latest".into());
        let version = resolve_version(&requested_version, &channel, &ctx.root, manifest, ctx.arch)?;
        let install_decisions = crate::doctor::flutter::install::InstallDecisions {
            channel,
            version,
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

/// Resolve the `--flutter-version` sentinels `latest` and `project` into a
/// concrete version string before handing decisions to `build_plan`, which
/// only ever does an exact `manifest.find(channel, version, arch)` lookup.
/// Left unresolved, the literal string `"latest"` (the CLI's own documented
/// value, and `execute()`'s silent fallback when an MCP caller answers only
/// `flutter.channel`) is passed straight through and fails with "no latest
/// arm64 build for macos on the stable channel" — there is no release
/// literally named `latest`.
fn resolve_version(
    version: &str,
    channel: &str,
    root: &Path,
    manifest: &ReleaseManifest,
    arch: Arch,
) -> Result<String, String> {
    match version {
        "latest" => manifest
            .latest(channel, arch)
            .map(|r| r.version.clone())
            .ok_or_else(|| {
                format!(
                    "no releases found for the {} channel ({})",
                    channel,
                    arch.manifest_name()
                )
            }),
        "project" => crate::doctor::flutter::version::candidates(root, manifest, channel, arch)
            .into_iter()
            .next()
            .map(|c| c.version)
            .ok_or_else(|| {
                format!(
                    "no version candidates found for this project on the {} channel",
                    channel
                )
            }),
        exact => Ok(exact.to_string()),
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
    fn an_outdated_sdk_already_on_path_offers_a_manual_upgrade_not_a_fresh_install() {
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
        let fix = result.fix.expect("an outdated SDK must still offer a fix");
        assert_eq!(
            fix.kind,
            FixKind::Manual,
            "a fresh-install plan either refuses (dir occupied by the SDK \
             we just found) or silently lands a second SDK PATH never sees"
        );
        assert!(
            fix.steps
                .iter()
                .any(|s| s.describe.contains("flutter upgrade")),
            "the manual step must name the actual fix: {:?}",
            fix.steps
        );
    }

    #[test]
    fn resolve_version_turns_latest_into_the_channel_head() {
        let m = ReleaseManifest::parse(FIXTURE).unwrap();
        let dir = TempDir::new().unwrap();
        let got = resolve_version("latest", "stable", dir.path(), &m, Arch::Arm64).unwrap();
        assert_eq!(got, "3.24.5", "must match manifest.latest(\"stable\", ..)");
    }

    #[test]
    fn resolve_version_turns_project_into_the_top_candidate() {
        let m = ReleaseManifest::parse(FIXTURE).unwrap();
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let got = resolve_version("project", "stable", dir.path(), &m, Arch::X64).unwrap();
        assert_eq!(
            got, "3.19.0",
            "must match candidates(..).first(), the project's own pin"
        );
    }

    #[test]
    fn resolve_version_leaves_an_exact_version_untouched() {
        let m = ReleaseManifest::parse(FIXTURE).unwrap();
        let dir = TempDir::new().unwrap();
        let got = resolve_version("3.24.5", "stable", dir.path(), &m, Arch::Arm64).unwrap();
        assert_eq!(got, "3.24.5");
    }

    #[test]
    fn plan_resolves_the_latest_sentinel_before_building_the_install_plan() {
        // This is the exact bug: `--flutter-version latest` (the CLI's own
        // documented value) and the MCP silent fallback both used to pass
        // the literal string "latest" straight to `manifest.find`, which
        // has no release named that, and the plan failed with "no latest
        // arm64 build for macos on the stable channel".
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let c = ctx(dir.path(), runner);
        let mut decisions = std::collections::HashMap::new();
        decisions.insert("flutter.channel".to_string(), "stable".to_string());
        decisions.insert("flutter.version".to_string(), "latest".to_string());
        let plan = FlutterCheck.plan(&c, &decisions).unwrap();
        match &plan.steps[0] {
            crate::doctor::types::Step::Download { url, .. } => {
                assert!(
                    url.contains("3.24.5"),
                    "must have resolved `latest` to a concrete release: {}",
                    url
                );
            }
            other => panic!("expected a download step, got {:?}", other),
        }
    }

    #[test]
    fn plan_resolves_the_project_sentinel_before_building_the_install_plan() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        // X64, not the shared `ctx()` helper's Arm64: the fixture's 3.19.0
        // entry carries no `dart_sdk_arch` (so it is X64 by default), and
        // this test needs the pin to actually resolve to a real release
        // rather than coincidentally match the channel head.
        let c = CheckContext {
            root: dir.path().to_path_buf(),
            host: host(),
            arch: Arch::X64,
            manifest: Some(ReleaseManifest::parse(FIXTURE).unwrap()),
            runner: Box::new(runner),
        };
        let mut decisions = std::collections::HashMap::new();
        decisions.insert("flutter.channel".to_string(), "stable".to_string());
        decisions.insert("flutter.version".to_string(), "project".to_string());
        let plan = FlutterCheck.plan(&c, &decisions).unwrap();
        match &plan.steps[0] {
            crate::doctor::types::Step::Download { url, .. } => {
                assert!(
                    url.contains("3.19.0"),
                    "must have resolved `project` to the pinned version: {}",
                    url
                );
            }
            other => panic!("expected a download step, got {:?}", other),
        }
    }

    #[test]
    fn plan_rejects_an_unknown_channel_before_touching_the_manifest() {
        // This is FINDING 3: `channel_has_archives` returns false for
        // anything that isn't stable/beta, so an unrecognised channel used
        // to fall straight through to the git-clone path and get baked
        // into the plan — the typo only surfaced as a confusing git
        // failure mid-install. The MCP surface takes `decisions` as
        // free-form strings, so this must be checked here, not only by the
        // CLI's `clap::ValueEnum`.
        let dir = TempDir::new().unwrap();
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let c = ctx(dir.path(), runner);
        let mut decisions = std::collections::HashMap::new();
        decisions.insert("flutter.channel".to_string(), "nightly".to_string());
        decisions.insert("flutter.version".to_string(), "latest".to_string());
        let err = FlutterCheck.plan(&c, &decisions).unwrap_err();
        assert!(
            err.contains("nightly"),
            "error must name the offender: {err}"
        );
        assert!(
            err.contains("stable") && err.contains("beta") && err.contains("master"),
            "error must list the valid channels: {err}"
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
