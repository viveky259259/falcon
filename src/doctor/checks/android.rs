//! Android SDK command-line tools and platform packages.
//!
//! Falcon installs the tooling and the packages, then hands the licence
//! agreements back to the user — accepting a legal agreement on someone
//! else's behalf is not Falcon's call.

use super::{Check, CheckContext};
use crate::doctor::host::HostInfo;
use crate::doctor::types::{
    CheckResult, FixKind, FixOffer, Plan, Probe, Status, Step, StepSummary,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Latest stable platform used when the project's value cannot be read.
const FALLBACK_COMPILE_SDK: u32 = 34;

/// Revision shared across all three per-OS archives Google publishes.
const CMDLINE_TOOLS_REVISION: &str = "11076708";
const LICENSES_DOCS: &str = "https://docs.flutter.dev/get-started/install";

pub struct AndroidCheck;

/// Google publishes three archives for this revision, distinguished only by
/// a `-mac-` / `-linux-` / `-win-` infix. Picking the wrong one means a
/// Linux or Windows host downloads a macOS binary that will not run.
fn cmdline_tools_url(os: &str) -> String {
    let infix = match os {
        "macos" => "mac",
        "windows" => "win",
        _ => "linux",
    };
    format!(
        "https://dl.google.com/android/repository/commandlinetools-{}-{}_latest.zip",
        infix, CMDLINE_TOOLS_REVISION
    )
}

pub fn sdk_root(host: &HostInfo, get_env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(v) = get_env("ANDROID_HOME").or_else(|| get_env("ANDROID_SDK_ROOT")) {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let home = host.home.as_ref()?;
    Some(match host.os.as_str() {
        "macos" => home.join("Library").join("Android").join("sdk"),
        "windows" => home
            .join("AppData")
            .join("Local")
            .join("Android")
            .join("Sdk"),
        _ => home.join("Android").join("Sdk"),
    })
}

/// Read `compileSdkVersion 34` / `compileSdk = 35`. Returns `None` when the
/// value is an indirection we cannot resolve — the caller falls back loudly.
pub fn compile_sdk_version(root: &Path) -> Option<u32> {
    let app = root.join("android").join("app");
    for name in ["build.gradle", "build.gradle.kts"] {
        let Ok(text) = std::fs::read_to_string(app.join(name)) else {
            continue;
        };
        for line in text.lines() {
            let t = line.trim();
            let Some(rest) = t
                .strip_prefix("compileSdkVersion")
                .or_else(|| t.strip_prefix("compileSdk"))
            else {
                continue;
            };
            let token = rest.trim().trim_start_matches('=').trim();
            // A literal wins; an indirection (`flutter.compileSdkVersion`)
            // yields None so the caller falls back loudly.
            return token.parse::<u32>().ok();
        }
    }
    None
}

impl Check for AndroidCheck {
    fn id(&self) -> &'static str {
        "android"
    }

    fn probe(&self, ctx: &CheckContext) -> CheckResult {
        if !ctx.root.join("android").is_dir() {
            return CheckResult {
                id: self.id().to_string(),
                status: Status::Skipped {
                    because: "no android/ directory in this project".into(),
                },
                required_by: vec![],
                fix: None,
            };
        }

        let found = ctx
            .runner
            .run("sdkmanager", &["--version".to_string()], None)
            .ok()
            .filter(|o| o.status == 0)
            .map(|o| o.stdout.trim().to_string())
            .filter(|v| !v.is_empty());

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
                        id: "download-cmdline-tools".into(),
                        describe: "Download the Android command-line tools".into(),
                    },
                    StepSummary {
                        id: "sdk-packages".into(),
                        describe: "Install platform-tools, the platform and build-tools".into(),
                    },
                    StepSummary {
                        id: "licenses".into(),
                        describe: "You accept the SDK licences — Falcon will not do this for you"
                            .into(),
                    },
                ],
            }),
        };

        CheckResult {
            id: self.id().to_string(),
            status,
            required_by: vec!["android/ directory is present, so Android builds must work".into()],
            fix,
        }
    }

    fn plan(&self, ctx: &CheckContext, _d: &HashMap<String, String>) -> Result<Plan, String> {
        let root = sdk_root(&ctx.host, |k| std::env::var(k).ok())
            .ok_or_else(|| "cannot determine the Android SDK location (no HOME)".to_string())?;
        let api = compile_sdk_version(&ctx.root).unwrap_or(FALLBACK_COMPILE_SDK);
        let scratch = std::env::temp_dir().join("falcon-doctor");
        let archive = scratch.join("android-cmdline-tools.zip");
        let sdkmanager = root
            .join("cmdline-tools")
            .join("bin")
            .join("sdkmanager")
            .to_string_lossy()
            .to_string();

        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![
                Step::Download {
                    id: "download-cmdline-tools".into(),
                    url: cmdline_tools_url(&ctx.host.os),
                    // Google does not publish a checksum alongside this zip;
                    // the empty string tells the executor the archive is
                    // unverifiable and it will refuse unless --yes is set.
                    sha256: String::new(),
                    dest: archive.clone(),
                },
                Step::Extract {
                    id: "extract-cmdline-tools".into(),
                    archive,
                    dest: root.clone(),
                },
                Step::Run {
                    id: "sdk-packages".into(),
                    program: sdkmanager,
                    args: vec![
                        "platform-tools".into(),
                        format!("platforms;android-{}", api),
                        format!("build-tools;{}.0.0", api),
                    ],
                    cwd: None,
                },
                Step::Handoff {
                    id: "licenses".into(),
                    reason: "Android SDK licence agreements must be accepted by you".into(),
                    command: "flutter doctor --android-licenses".into(),
                    docs_url: LICENSES_DOCS.into(),
                    verify: Probe {
                        program: "flutter".into(),
                        args: vec!["doctor".into(), "--android-licenses".into()],
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

    fn host(os: &str) -> HostInfo {
        HostInfo {
            os: os.into(),
            arch: "arm64".into(),
            home: Some(PathBuf::from("/Users/ada")),
            shell_rc: None,
            package_managers: vec![],
        }
    }

    fn ctx(root: &Path, runner: FakeRunner, os: &str) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: host(os),
            arch: Arch::Arm64,
            manifest: None,
            runner: Box::new(runner),
        }
    }

    fn android_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("android/app")).unwrap();
        fs::write(
            dir.path().join("android/app/build.gradle"),
            "android {\n    compileSdkVersion 34\n}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn skipped_without_an_android_directory() {
        let dir = TempDir::new().unwrap();
        let r = AndroidCheck.probe(&ctx(dir.path(), FakeRunner::default(), "macos"));
        assert!(matches!(r.status, Status::Skipped { .. }));
    }

    #[test]
    fn sdk_root_prefers_android_home_env() {
        let got = sdk_root(&host("macos"), |k| {
            (k == "ANDROID_HOME").then(|| "/opt/android".to_string())
        });
        assert_eq!(got, Some(PathBuf::from("/opt/android")));
    }

    #[test]
    fn sdk_root_falls_back_to_the_platform_default() {
        assert_eq!(
            sdk_root(&host("macos"), |_| None),
            Some(PathBuf::from("/Users/ada/Library/Android/sdk"))
        );
        assert_eq!(
            sdk_root(&host("linux"), |_| None),
            Some(PathBuf::from("/Users/ada/Android/Sdk"))
        );
    }

    #[test]
    fn compile_sdk_version_reads_groovy_gradle() {
        let dir = android_project();
        assert_eq!(compile_sdk_version(dir.path()), Some(34));
    }

    #[test]
    fn compile_sdk_version_reads_kotlin_dsl() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("android/app")).unwrap();
        fs::write(
            dir.path().join("android/app/build.gradle.kts"),
            "android {\n    compileSdk = 35\n}\n",
        )
        .unwrap();
        assert_eq!(compile_sdk_version(dir.path()), Some(35));
    }

    #[test]
    fn indirect_compile_sdk_returns_none_rather_than_guessing_wrong() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("android/app")).unwrap();
        fs::write(
            dir.path().join("android/app/build.gradle"),
            "android {\n    compileSdkVersion flutter.compileSdkVersion\n}\n",
        )
        .unwrap();
        assert_eq!(compile_sdk_version(dir.path()), None);
    }

    #[test]
    fn missing_sdk_is_an_assisted_fix() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let r = AndroidCheck.probe(&ctx(dir.path(), runner, "macos"));
        assert_eq!(r.status, Status::Missing);
        assert_eq!(r.fix.unwrap().kind, FixKind::Assisted);
    }

    #[test]
    fn the_plan_installs_packages_then_hands_off_the_licences() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        let ids: Vec<_> = plan.steps.iter().map(|s| s.id().to_string()).collect();
        assert_eq!(
            ids,
            vec![
                "download-cmdline-tools",
                "extract-cmdline-tools",
                "sdk-packages",
                "licenses"
            ]
        );
        assert!(
            matches!(plan.steps.last(), Some(Step::Handoff { .. })),
            "licences must be a handoff, never automated"
        );
    }

    #[test]
    fn the_licence_handoff_names_the_exact_command() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        match plan.steps.last().unwrap() {
            Step::Handoff {
                command, reason, ..
            } => {
                assert_eq!(command, "flutter doctor --android-licenses");
                assert!(
                    reason.to_lowercase().contains("licence")
                        || reason.to_lowercase().contains("license")
                );
            }
            other => panic!("expected a handoff, got {:?}", other),
        }
    }

    #[test]
    fn the_plan_targets_the_projects_compile_sdk_version() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[2] {
            Step::Run { args, .. } => {
                assert!(
                    args.iter().any(|a| a == "platforms;android-34"),
                    "must use the project's compileSdkVersion: {:?}",
                    args
                );
            }
            other => panic!("expected sdkmanager run, got {:?}", other),
        }
    }

    #[test]
    fn the_plan_never_uses_sudo() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "macos");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        for step in &plan.steps {
            if let Step::Run { program, .. } = step {
                assert_ne!(program, "sudo");
            }
        }
    }

    #[test]
    fn cmdline_tools_url_picks_the_host_specific_archive() {
        assert!(cmdline_tools_url("macos").contains("-mac-"));
        assert!(cmdline_tools_url("linux").contains("-linux-"));
        assert!(cmdline_tools_url("windows").contains("-win-"));
    }

    #[test]
    fn a_linux_host_never_gets_the_mac_archive() {
        let dir = android_project();
        let mut runner = FakeRunner::default();
        runner.fail_on("sdkmanager --version");
        let c = ctx(dir.path(), runner, "linux");
        let plan = AndroidCheck.plan(&c, &HashMap::new()).unwrap();
        match &plan.steps[0] {
            Step::Download { url, .. } => {
                assert!(
                    !url.contains("-mac-"),
                    "linux host got the mac archive: {}",
                    url
                );
                assert!(
                    url.contains("-linux-"),
                    "expected the linux archive: {}",
                    url
                );
            }
            other => panic!("expected a download step, got {:?}", other),
        }
    }
}
