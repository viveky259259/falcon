//! `falcon doctor` — diagnose the project's toolchain and repair what we can.

pub mod checks;
pub mod exec;
pub mod flutter;
pub mod host;
pub mod prompt;
pub mod report;
pub mod types;

pub use types::*;

use crate::doctor::checks::{Check, CheckContext};
use crate::doctor::exec::{execute_plan, CurlDownloader, HandoffPolicy, RealRunner};
use crate::doctor::flutter::install::{dir_is_usable, InstallDecisions};
use crate::doctor::flutter::releases::{manifest_url, ReleaseManifest};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct DoctorOptions {
    pub root: PathBuf,
    pub fix: bool,
    pub yes: bool,
    pub dry_run: bool,
    pub only: Vec<String>,
    pub skip: Vec<String>,
    pub channel: Option<String>,
    pub flutter_version: Option<String>,
    pub dir: Option<PathBuf>,
    pub json: bool,
}

impl DoctorOptions {
    fn wants(&self, id: &str) -> bool {
        if !self.only.is_empty() {
            return self.only.iter().any(|o| o == id);
        }
        !self.skip.iter().any(|s| s == id)
    }

    /// JSON mode and `--yes` both mean "never read stdin".
    fn interactive(&self) -> bool {
        !self.json && !self.yes && std::io::IsTerminal::is_terminal(&std::io::stdin())
    }
}

fn registry() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(checks::flutter::FlutterCheck),
        Box::new(checks::dart::DartCheck),
    ]
}

/// Fetch the release manifest. A failure is not fatal — fixes degrade to Manual.
fn fetch_manifest(host_os: host::Os) -> Option<ReleaseManifest> {
    let out = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--max-time",
            "20",
        ])
        .arg(manifest_url(host_os))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    ReleaseManifest::parse(&String::from_utf8_lossy(&out.stdout)).ok()
}

/// The context a check probes against. Re-probing after a fix must ask the
/// machine exactly the way the first probe did, so both go through here.
fn context(
    opts: &DoctorOptions,
    host_info: &host::HostInfo,
    arch: host::Arch,
    manifest: &Option<ReleaseManifest>,
) -> CheckContext {
    CheckContext {
        root: opts.root.clone(),
        host: host_info.clone(),
        arch,
        manifest: manifest.clone(),
        runner: Box::new(RealRunner),
    }
}

/// Did this plan actually change the machine? A dry run reports every step
/// `Skipped`, so it completed nothing and must not be treated as a repair.
fn plan_completed(outcomes: &[Outcome]) -> bool {
    outcomes.iter().any(|o| matches!(o, Outcome::Done { .. }))
        && !outcomes
            .iter()
            .any(|o| matches!(o, Outcome::Failed { .. } | Outcome::AwaitingManual { .. }))
}

/// The exit code must describe the machine as it is *now*, not as it was before
/// we repaired it. Any check whose plan ran to completion is asked again rather
/// than assumed fixed — the honest answer comes from the machine, not the plan.
fn settle(
    checks: Vec<CheckResult>,
    applied: &HashMap<String, Vec<Outcome>>,
    reprobe: impl Fn(&str) -> Option<CheckResult>,
) -> Vec<CheckResult> {
    checks
        .into_iter()
        .map(|c| match applied.get(&c.id) {
            Some(outcomes) if plan_completed(outcomes) => reprobe(&c.id).unwrap_or(c),
            _ => c,
        })
        .collect()
}

pub fn run(opts: &DoctorOptions) -> Result<i32> {
    let host_info = host::detect();
    let arch = host::current_arch();
    let manifest = fetch_manifest(host::current_os());

    let mut results = Vec::new();
    for check in registry() {
        if !opts.wants(check.id()) {
            continue;
        }
        results.push(check.probe(&context(opts, &host_info, arch, &manifest)));
    }

    let diagnosis = Diagnosis {
        host: host_info.clone(),
        checks: results,
    };

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report::render_json_with_fix_status(
                &diagnosis, opts.fix
            ))?
        );
        return Ok(exit_code(&diagnosis.checks, &[]));
    }
    print!("{}", report::render_text(&diagnosis));

    let applied = apply_fixes(&diagnosis, &manifest, &host_info, arch, opts)?;
    let settled = settle(diagnosis.checks.clone(), &applied, |id| {
        registry()
            .into_iter()
            .find(|c| c.id() == id)
            .map(|c| c.probe(&context(opts, &host_info, arch, &manifest)))
    });
    let outcomes: Vec<Outcome> = applied.into_values().flatten().collect();
    Ok(exit_code(&settled, &outcomes))
}

/// Offer, confirm and run every fix the diagnosis makes available, keyed by
/// check id so each check's outcomes can be judged on their own.
fn apply_fixes(
    diagnosis: &Diagnosis,
    manifest: &Option<ReleaseManifest>,
    host_info: &host::HostInfo,
    arch: host::Arch,
    opts: &DoctorOptions,
) -> Result<HashMap<String, Vec<Outcome>>> {
    let mut applied: HashMap<String, Vec<Outcome>> = HashMap::new();
    for check in &diagnosis.checks {
        let Some(offer) = &check.fix else { continue };
        if offer.kind == FixKind::Manual {
            continue;
        }
        if !opts.fix && !confirm(&format!("Fix {} now?", check.id), opts)? {
            if !opts.interactive() {
                println!(
                    "\n  a fix is available for {} — run with --fix to apply",
                    check.id
                );
            }
            continue;
        }
        let Some(decisions) = collect_decisions(offer, opts)? else {
            println!("\n  skipping {}: no answer given", check.id);
            continue;
        };
        applied.insert(
            check.id.clone(),
            apply(check, &decisions, manifest, host_info, arch, opts)?,
        );
    }
    Ok(applied)
}

fn confirm(question: &str, opts: &DoctorOptions) -> Result<bool> {
    if !opts.interactive() {
        return Ok(false);
    }
    use std::io::Write;
    print!("{} [Y/n]: ", question);
    std::io::stdout().flush()?;
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line)? == 0 {
        return Ok(false);
    }
    Ok(!matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "n" | "no"
    ))
}

/// Ask (or infer) every question the fix needs. `None` means the user bailed.
fn collect_decisions(
    offer: &FixOffer,
    opts: &DoctorOptions,
) -> Result<Option<HashMap<String, String>>> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut out = std::io::stdout();
    let mut answers = HashMap::new();
    for q in &offer.questions {
        let preset = match q.id.as_str() {
            "flutter.channel" => opts.channel.clone(),
            "flutter.version" => opts.flutter_version.clone(),
            "flutter.dir" => opts.dir.as_ref().map(|d| d.to_string_lossy().to_string()),
            _ => None,
        };
        let Some(value) = prompt::answer(
            q,
            preset.as_deref(),
            !opts.interactive(),
            &mut reader,
            &mut out,
        ) else {
            return Ok(None);
        };
        answers.insert(q.id.clone(), value);
    }
    Ok(Some(answers))
}

fn apply(
    check: &CheckResult,
    decisions: &HashMap<String, String>,
    manifest: &Option<ReleaseManifest>,
    host_info: &host::HostInfo,
    arch: host::Arch,
    opts: &DoctorOptions,
) -> Result<Vec<Outcome>> {
    if check.id != "flutter" {
        return Ok(vec![]);
    }
    let Some(manifest) = manifest else {
        return Ok(vec![announced_failure(
            "Flutter release manifest unavailable; install manually from \
             https://docs.flutter.dev/get-started/install",
        )]);
    };

    let dir = decisions
        .get("flutter.dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| flutter::install::default_install_dir(host_info.home.as_deref()));
    if !opts.dry_run {
        if let Err(e) = dir_is_usable(&dir) {
            return Ok(vec![announced_failure(&e)]);
        }
    }

    let install = InstallDecisions {
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
    let plan = match flutter::install::build_plan(manifest, host_info, arch, &install, &scratch) {
        Ok(p) => p,
        Err(e) => return Ok(vec![announced_failure(&e)]),
    };

    let policy = if opts.interactive() {
        HandoffPolicy::Prompt
    } else {
        HandoffPolicy::Report
    };
    if opts.dry_run {
        print!("{}", report::render_plan_preview(&plan));
    }
    let outcomes = execute_plan(&plan, &RealRunner, &CurlDownloader, policy, opts.dry_run);
    announce(&outcomes);
    if !outcomes.iter().any(|o| matches!(o, Outcome::Failed { .. })) {
        print_path_hint(&plan);
    }
    Ok(outcomes)
}

/// Build a failure outcome *and* say so on stdout, so a fix that stops early
/// is never silent.
fn announced_failure(error: &str) -> Outcome {
    let outcome = Outcome::Failed {
        error: error.to_string(),
    };
    announce(std::slice::from_ref(&outcome));
    outcome
}

/// Surface the outcomes a user must act on. Exit codes alone are not an error
/// message, and a silent failure after a long download is indistinguishable
/// from a hang.
fn announce(outcomes: &[Outcome]) {
    for o in outcomes {
        match o {
            Outcome::Failed { error } => eprintln!("\n  failed: {}", error),
            Outcome::AwaitingManual { command } => {
                println!("\n  waiting on you to run: {}", command)
            }
            Outcome::Done { .. } | Outcome::Skipped { .. } => {}
        }
    }
}

/// Falcon prints the export line; it never edits a shell rc file.
fn print_path_hint(plan: &Plan) {
    for step in &plan.steps {
        if let Step::PathHint { dir, rc_file, .. } = step {
            println!("\nAdd this to your shell, then restart it:");
            println!("  {}", flutter::install::path_export_line(dir));
            if let Some(rc) = rc_file {
                println!("  (your shell reads {})", rc.display());
            }
            println!("`flutter` will not be on PATH in this shell until you do.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> DoctorOptions {
        DoctorOptions {
            root: PathBuf::from("."),
            ..Default::default()
        }
    }

    use crate::doctor::checks::flutter::FlutterCheck;
    use crate::doctor::exec::FakeRunner;
    use crate::doctor::host::HostInfo;

    fn missing_flutter() -> CheckResult {
        CheckResult {
            id: "flutter".into(),
            status: Status::Missing,
            required_by: vec![],
            fix: None,
        }
    }

    fn done(step: &str) -> Outcome {
        Outcome::Done {
            step_id: step.into(),
        }
    }

    /// A probe context backed by a fake runner: no network, no filesystem
    /// writes, and `flutter --version` answers with whatever we say it does.
    fn fake_ctx(root: &std::path::Path, version_output: &str) -> CheckContext {
        CheckContext {
            root: root.to_path_buf(),
            host: HostInfo {
                os: "macos".into(),
                arch: "arm64".into(),
                home: None,
                shell_rc: None,
                package_managers: vec![],
            },
            arch: host::Arch::Arm64,
            manifest: None,
            runner: Box::new(FakeRunner::with_stdout("flutter --version", version_output)),
        }
    }

    #[test]
    fn a_fix_that_completed_is_re_probed_so_a_repaired_machine_exits_zero() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = fake_ctx(
            tmp.path(),
            "Flutter 3.24.5 • channel stable • https://github.com",
        );
        let applied = HashMap::from([(
            "flutter".to_string(),
            vec![
                done("download"),
                done("extract"),
                done("prime"),
                done("verify"),
            ],
        )]);

        let settled = settle(vec![missing_flutter()], &applied, |_| {
            Some(FlutterCheck.probe(&ctx))
        });

        assert!(
            matches!(settled[0].status, Status::Ok { .. }),
            "the re-probe should see the SDK we just installed, got {:?}",
            settled[0].status
        );
        let outcomes: Vec<Outcome> = applied.into_values().flatten().collect();
        assert_eq!(
            exit_code(&settled, &outcomes),
            0,
            "a machine we just repaired is healthy"
        );
    }

    #[test]
    fn a_fix_that_failed_is_not_re_probed_and_still_exits_two() {
        let applied = HashMap::from([(
            "flutter".to_string(),
            vec![
                done("download"),
                Outcome::Failed {
                    error: "checksum mismatch".into(),
                },
            ],
        )]);

        let settled = settle(
            vec![missing_flutter()],
            &applied,
            |_| -> Option<CheckResult> { panic!("a failed fix must not be re-probed") },
        );

        assert!(matches!(settled[0].status, Status::Missing));
        let outcomes: Vec<Outcome> = applied.into_values().flatten().collect();
        assert_eq!(exit_code(&settled, &outcomes), 2);
    }

    #[test]
    fn a_dry_run_changed_nothing_so_nothing_is_re_probed() {
        let applied = HashMap::from([(
            "flutter".to_string(),
            vec![
                Outcome::Skipped {
                    step_id: "download".into(),
                },
                Outcome::Skipped {
                    step_id: "extract".into(),
                },
            ],
        )]);

        let settled = settle(
            vec![missing_flutter()],
            &applied,
            |_| -> Option<CheckResult> {
                panic!("a dry run installed nothing, so there is nothing to re-probe")
            },
        );

        assert!(matches!(settled[0].status, Status::Missing));
        assert_eq!(exit_code(&settled, &[]), 2);
    }

    #[test]
    fn a_handoff_is_not_a_completed_plan() {
        assert!(!plan_completed(&[
            done("download"),
            Outcome::AwaitingManual {
                command: "sudo xcodebuild -license".into()
            },
        ]));
    }

    #[test]
    fn json_mode_is_never_interactive_so_it_cannot_prompt() {
        let o = DoctorOptions {
            json: true,
            ..opts()
        };
        assert!(!o.interactive());
    }

    #[test]
    fn yes_mode_is_never_interactive() {
        let o = DoctorOptions {
            yes: true,
            ..opts()
        };
        assert!(!o.interactive());
    }

    #[test]
    fn only_restricts_the_registry_to_the_named_checks() {
        let o = DoctorOptions {
            only: vec!["flutter".into()],
            ..opts()
        };
        assert!(o.wants("flutter"));
        assert!(!o.wants("android"));
    }

    #[test]
    fn skip_drops_the_named_checks_and_keeps_the_rest() {
        let o = DoctorOptions {
            skip: vec!["android".into()],
            ..opts()
        };
        assert!(o.wants("flutter"));
        assert!(!o.wants("android"));
    }

    #[test]
    fn only_wins_over_skip() {
        let o = DoctorOptions {
            only: vec!["flutter".into()],
            skip: vec!["flutter".into()],
            ..opts()
        };
        assert!(o.wants("flutter"));
    }
}
