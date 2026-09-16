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
use crate::doctor::flutter::install::dir_is_usable;
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
    /// Suppress every stdout write reached while applying a fix. The CLI
    /// (`run()`) never sets this — it always stays `false`, so `run()`'s
    /// printed output is unchanged. The MCP tool sets it, because MCP stdio
    /// speaks JSON-RPC over stdout: any stray `println!` interleaved with a
    /// response line corrupts the transport. Diagnostic failures still go to
    /// stderr regardless of this flag — stderr is not the protocol stream,
    /// and nothing here reads it.
    pub silent: bool,
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
        Box::new(checks::cocoapods::CocoaPodsCheck),
        Box::new(checks::android::AndroidCheck),
        Box::new(checks::xcode::XcodeCheck),
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

/// Probe every relevant check against one shared host/arch/manifest snapshot.
/// The single loop both `run()` and `diagnose()` build their result on, so
/// the two phases can never drift on what counts as "relevant" or how a
/// check is probed.
fn probe_all(
    opts: &DoctorOptions,
    host_info: &host::HostInfo,
    arch: host::Arch,
    manifest: &Option<ReleaseManifest>,
) -> Diagnosis {
    let mut results = Vec::new();
    for check in registry() {
        if !opts.wants(check.id()) {
            continue;
        }
        results.push(check.probe(&context(opts, host_info, arch, manifest)));
    }
    Diagnosis {
        host: host_info.clone(),
        checks: results,
    }
}

/// Phase one: probe every relevant check. Never mutates anything — safe to
/// call speculatively (this is what the MCP `doctor` tool does by default).
pub fn diagnose(opts: &DoctorOptions) -> Result<Diagnosis> {
    let host_info = host::detect();
    let arch = host::current_arch();
    let manifest = fetch_manifest(host::current_os());
    Ok(probe_all(opts, &host_info, arch, &manifest))
}

/// What an `execute()` call reports back.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionReport {
    pub steps: Vec<Outcome>,
    /// "ok" | "awaiting_manual_step" | "failed"
    pub status: String,
    /// What a CLI user would see printed after a successful install — the
    /// `export PATH=...` line and which shell rc file to add it to. `apply()`
    /// never prints this when `opts.silent`, so a silent (MCP) caller gets it
    /// here as data instead of losing it. In practice at most one entry —
    /// only Flutter's install plan carries a `PathHint` step — but this stays
    /// a `Vec` so a future check that also needs PATH changes isn't dropped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_hints: Vec<String>,
}

/// Phase two: run every fixable check's plan using the decisions already on
/// `opts` (`channel` / `flutter_version` / `dir`). Never prompts — this is
/// the non-interactive path the MCP `doctor` tool's `execute: true` call
/// uses, so every question it needs must already be answered by the caller.
pub fn execute(opts: &DoctorOptions) -> Result<ExecutionReport> {
    let host_info = host::detect();
    let arch = host::current_arch();
    let manifest = fetch_manifest(host::current_os());
    let diagnosis = probe_all(opts, &host_info, arch, &manifest);

    let mut outcomes = Vec::new();
    let mut path_hints = Vec::new();
    for check in registry() {
        let Some(probed) = diagnosis.checks.iter().find(|c| c.id == check.id()) else {
            continue;
        };
        let Some(offer) = &probed.fix else { continue };
        if offer.kind == FixKind::Manual {
            continue;
        }
        let mut decisions = HashMap::new();
        if let Some(c) = &opts.channel {
            decisions.insert("flutter.channel".to_string(), c.clone());
        }
        if let Some(v) = &opts.flutter_version {
            decisions.insert("flutter.version".to_string(), v.clone());
        }
        if let Some(d) = &opts.dir {
            decisions.insert("flutter.dir".to_string(), d.to_string_lossy().to_string());
        }
        let ctx = context(opts, &host_info, arch, &manifest);
        let (check_outcomes, check_hints) = apply(check.as_ref(), &ctx, &decisions, opts)?;
        outcomes.extend(check_outcomes);
        path_hints.extend(check_hints);
    }

    let status = if outcomes
        .iter()
        .any(|o| matches!(o, Outcome::AwaitingManual { .. }))
    {
        "awaiting_manual_step"
    } else if outcomes.iter().any(|o| matches!(o, Outcome::Failed { .. })) {
        "failed"
    } else {
        "ok"
    };

    Ok(ExecutionReport {
        steps: outcomes,
        status: status.to_string(),
        path_hints,
    })
}

pub fn run(opts: &DoctorOptions) -> Result<i32> {
    let host_info = host::detect();
    let arch = host::current_arch();
    let manifest = fetch_manifest(host::current_os());

    let diagnosis = probe_all(opts, &host_info, arch, &manifest);

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
    // `apply()` needs the `Check` itself (to call `.plan()`), not just its
    // probed `CheckResult`, so pair each registered check with its result.
    for check in registry() {
        let Some(probed) = diagnosis.checks.iter().find(|c| c.id == check.id()) else {
            continue;
        };
        let Some(offer) = &probed.fix else { continue };
        if offer.kind == FixKind::Manual {
            continue;
        }
        if !opts.fix && !confirm(&format!("Fix {} now?", probed.id), opts)? {
            if !opts.interactive() && !opts.silent {
                println!(
                    "\n  a fix is available for {} — run with --fix to apply",
                    probed.id
                );
            }
            continue;
        }
        let Some(decisions) = collect_decisions(offer, opts)? else {
            if !opts.silent {
                println!("\n  skipping {}: no answer given", probed.id);
            }
            continue;
        };
        let ctx = context(opts, host_info, arch, manifest);
        let (outcomes, _hints) = apply(check.as_ref(), &ctx, &decisions, opts)?;
        applied.insert(probed.id.clone(), outcomes);
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

/// Build and run one check's fix. Check-agnostic: the plan itself comes from
/// `check.plan()`, so this has no `if check.id == "flutter"` branch — a new
/// check only needs to implement `plan()` to be fixable here.
///
/// Returns the outcomes plus any PATH hint text the plan produced. When
/// `opts.silent` is false (the CLI's only mode) both are also printed, byte
/// for byte the same as before this function returned anything — a silent
/// caller gets the hint text back as data instead of losing it, since it
/// never appears in its terminal.
fn apply(
    check: &dyn Check,
    ctx: &CheckContext,
    decisions: &HashMap<String, String>,
    opts: &DoctorOptions,
) -> Result<(Vec<Outcome>, Vec<String>)> {
    let plan = match check.plan(ctx, decisions) {
        Ok(p) => p,
        Err(e) => return Ok((vec![announced_failure(&e)], vec![])),
    };
    if plan.steps.is_empty() {
        return Ok((vec![], vec![]));
    }

    // Only Flutter's plan carries a `flutter.dir` decision; for every other
    // check this is a no-op, so the check stays generic rather than gated on
    // check id.
    if !opts.dry_run {
        if let Some(dir) = decisions.get("flutter.dir") {
            if let Err(e) = dir_is_usable(std::path::Path::new(dir)) {
                return Ok((vec![announced_failure(&e)], vec![]));
            }
        }
    }

    let policy = if opts.interactive() {
        HandoffPolicy::Prompt
    } else {
        HandoffPolicy::Report
    };
    if opts.dry_run && !opts.silent {
        print!("{}", report::render_plan_preview(&plan));
    }
    let outcomes = execute_plan(&plan, &RealRunner, &CurlDownloader, policy, opts.dry_run);
    announce(&outcomes, opts.silent);
    let hints = if outcomes.iter().any(|o| matches!(o, Outcome::Failed { .. })) {
        vec![]
    } else {
        let hints = path_hints(&plan);
        if !opts.silent {
            for hint in &hints {
                print!("{}", hint);
            }
        }
        hints
    };
    Ok((outcomes, hints))
}

/// Build a failure outcome *and* say so on stderr, so a fix that stops early
/// is never silent. Always stderr, never gated on `silent`: it's diagnostic
/// prose, not protocol data, and the failure is already returned as data in
/// the `Outcome` itself for any caller that only reads the response.
fn announced_failure(error: &str) -> Outcome {
    eprintln!("\n  failed: {}", error);
    Outcome::Failed {
        error: error.to_string(),
    }
}

/// Surface the outcomes a user must act on. A failure always goes to
/// stderr — that stays unconditional even when `silent`, since stderr is
/// never the MCP stdio JSON-RPC stream. `AwaitingManual`'s stdout line is
/// suppressed when `silent` because its command is already returned as data
/// in the `Outcome` itself (see `ExecutionReport::steps`); a silent caller
/// must not have it land in the same stream as the JSON-RPC response.
fn announce(outcomes: &[Outcome], silent: bool) {
    for o in outcomes {
        match o {
            Outcome::Failed { error } => eprintln!("\n  failed: {}", error),
            Outcome::AwaitingManual { command } => {
                if !silent {
                    println!("\n  waiting on you to run: {}", command)
                }
            }
            Outcome::Done { .. } | Outcome::Skipped { .. } => {}
        }
    }
}

/// Render the PATH hint for every `PathHint` step in the plan (in practice
/// zero or one — only Flutter's install plan carries one). Falcon prints the
/// export line; it never edits a shell rc file. Text, not a print: shared by
/// `apply()`'s printed hint and `ExecutionReport::path_hints`, so the two
/// can never drift apart on wording.
fn path_hints(plan: &Plan) -> Vec<String> {
    plan.steps
        .iter()
        .filter_map(|step| match step {
            Step::PathHint { dir, rc_file, .. } => {
                let mut hint = format!(
                    "\nAdd this to your shell, then restart it:\n  {}\n",
                    flutter::install::path_export_line(dir)
                );
                if let Some(rc) = rc_file {
                    hint.push_str(&format!("  (your shell reads {})\n", rc.display()));
                }
                hint.push_str("`flutter` will not be on PATH in this shell until you do.\n");
                Some(hint)
            }
            _ => None,
        })
        .collect()
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

    // ─── DoctorOptions::silent: apply() must not write to stdout ──────────
    //
    // MCP stdio speaks JSON-RPC over stdout, so a stray `println!` reached
    // while executing a fix would corrupt that stream. These tests prove
    // the suppression is real — not just that `apply()`'s return value is
    // unaffected — by actually capturing stdout.
    //
    // `apply()` is private, so this can only be driven from inside the
    // crate, in the shared `--lib` test binary that hundreds of unrelated
    // tests run in concurrently across threads. Redirecting real stdout
    // in-process here would race with all of them. So each test below
    // re-execs this exact test binary, filtered to just itself, and reads
    // back that lone child process's own stdout pipe — which nothing else
    // can write to, by construction of `std::process::Command`.

    /// A `Check` stub whose `.plan()` returns a fixed plan regardless of
    /// decisions, so these tests exercise `apply()` itself, not any real
    /// check's probing logic.
    struct FixedPlanCheck(Plan);

    impl Check for FixedPlanCheck {
        fn id(&self) -> &'static str {
            "fixture"
        }
        fn probe(&self, _ctx: &CheckContext) -> CheckResult {
            unreachable!("apply() never calls probe()")
        }
        fn plan(
            &self,
            _ctx: &CheckContext,
            _decisions: &HashMap<String, String>,
        ) -> std::result::Result<Plan, String> {
            Ok(self.0.clone())
        }
    }

    fn silent_test_opts(silent: bool) -> DoctorOptions {
        DoctorOptions {
            yes: true,
            silent,
            ..opts()
        }
    }

    fn handoff_plan(command: &str) -> Plan {
        Plan {
            check_id: "fixture".into(),
            steps: vec![Step::Handoff {
                id: "licenses".into(),
                reason: "test fixture".into(),
                command: command.into(),
                docs_url: "https://example.test".into(),
                verify: Probe {
                    program: "true".into(),
                    args: vec![],
                },
            }],
        }
    }

    fn path_hint_plan(dir: &str) -> Plan {
        Plan {
            check_id: "fixture".into(),
            steps: vec![Step::PathHint {
                id: "path".into(),
                dir: PathBuf::from(dir),
                rc_file: None,
            }],
        }
    }

    /// Re-exec this exact test binary, filtered to run only
    /// `qualified_test_name` (a `mod::path::fn_name` as `cargo test --lib
    /// -- --list` prints it), single-threaded. The returned `Output`'s
    /// `stdout` is that lone child's real, unshared stdout stream.
    fn run_as_solitary_child(qualified_test_name: &str) -> std::process::Output {
        let exe = std::env::current_exe().expect("test binary path");
        std::process::Command::new(&exe)
            .args([
                "--exact",
                qualified_test_name,
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FALCON_DOCTOR_STDOUT_CHILD", "1")
            .output()
            .expect("failed to re-exec test binary for stdout capture")
    }

    #[test]
    fn silent_suppresses_the_awaiting_manual_print_but_not_the_outcome() {
        if std::env::var_os("FALCON_DOCTOR_STDOUT_CHILD").is_none() {
            let out = run_as_solitary_child(
                "doctor::tests::silent_suppresses_the_awaiting_manual_print_but_not_the_outcome",
            );
            assert!(
                out.status.success(),
                "child test failed:\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                stdout.contains("run-this-when-not-silent"),
                "control case: a non-silent apply() must still print the \
                 AwaitingManual command, or this test can't tell suppression \
                 from a broken capture:\n{stdout}"
            );
            assert!(
                !stdout.contains("run-this-when-silent"),
                "silent execution must not print the AwaitingManual message \
                 to stdout — over MCP stdio that would corrupt the JSON-RPC \
                 stream:\n{stdout}"
            );
            return;
        }

        let ctx = fake_ctx(std::path::Path::new("."), "");
        let decisions = HashMap::new();

        let silent_check = FixedPlanCheck(handoff_plan("run-this-when-silent"));
        let (silent_outcomes, silent_hints) =
            apply(&silent_check, &ctx, &decisions, &silent_test_opts(true)).unwrap();
        assert_eq!(
            silent_outcomes,
            vec![Outcome::AwaitingManual {
                command: "run-this-when-silent".into()
            }],
            "silent must not change what apply() returns"
        );
        assert!(silent_hints.is_empty());

        let verbose_check = FixedPlanCheck(handoff_plan("run-this-when-not-silent"));
        let (verbose_outcomes, _) =
            apply(&verbose_check, &ctx, &decisions, &silent_test_opts(false)).unwrap();
        assert_eq!(
            verbose_outcomes,
            vec![Outcome::AwaitingManual {
                command: "run-this-when-not-silent".into()
            }]
        );
    }

    #[test]
    fn silent_suppresses_the_path_hint_print_but_returns_it_as_data() {
        if std::env::var_os("FALCON_DOCTOR_STDOUT_CHILD").is_none() {
            let out = run_as_solitary_child(
                "doctor::tests::silent_suppresses_the_path_hint_print_but_returns_it_as_data",
            );
            assert!(
                out.status.success(),
                "child test failed:\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                stdout.contains("falcon-test-verbose-marker-dir"),
                "control case: a non-silent apply() must still print the \
                 PATH hint, or this test can't tell suppression from a \
                 broken capture:\n{stdout}"
            );
            assert!(
                !stdout.contains("falcon-test-silent-marker-dir"),
                "silent execution must not print the PATH hint to stdout — \
                 over MCP stdio that would corrupt the JSON-RPC stream:\n{stdout}"
            );
            return;
        }

        let ctx = fake_ctx(std::path::Path::new("."), "");
        let decisions = HashMap::new();

        let silent_check = FixedPlanCheck(path_hint_plan("/tmp/falcon-test-silent-marker-dir"));
        let (silent_outcomes, silent_hints) =
            apply(&silent_check, &ctx, &decisions, &silent_test_opts(true)).unwrap();
        assert_eq!(
            silent_outcomes,
            vec![Outcome::Done {
                step_id: "path".into()
            }]
        );
        assert_eq!(
            silent_hints.len(),
            1,
            "the hint must still reach the caller as data when silent"
        );
        assert!(
            silent_hints[0].contains("falcon-test-silent-marker-dir"),
            "the returned hint must describe the actual install dir: {}",
            silent_hints[0]
        );

        let verbose_check = FixedPlanCheck(path_hint_plan("/tmp/falcon-test-verbose-marker-dir"));
        let (_, verbose_hints) =
            apply(&verbose_check, &ctx, &decisions, &silent_test_opts(false)).unwrap();
        assert_eq!(verbose_hints.len(), 1);
        assert!(verbose_hints[0].contains("falcon-test-verbose-marker-dir"));
    }
}
