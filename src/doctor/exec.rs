//! Plan execution behind injectable IO, so tests assert the plan without
//! running commands or touching the network.

use crate::doctor::types::{Outcome, Plan, Step};
use std::path::Path;
use std::sync::Mutex;

pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait CommandRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
    ) -> Result<CommandOutput, String>;
}

pub trait Downloader {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), String>;
    fn sha256(&self, path: &Path) -> Result<String, String>;
}

/// What to do when a plan reaches a step Falcon will not perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffPolicy {
    /// Interactive: ask the user to run it, then re-verify. Used by the CLI on a TTY.
    Prompt,
    /// Report it and stop. Used by `--yes`, CI, and MCP.
    Report,
}

pub struct RealRunner;

impl CommandRunner for RealRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
    ) -> Result<CommandOutput, String> {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("failed to run `{}`: {}", program, e))?;
        Ok(CommandOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }
}

/// Shells out to `curl`, matching the pattern in `src/self_update.rs:49`.
pub struct CurlDownloader;

impl Downloader for CurlDownloader {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), String> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let status = std::process::Command::new("curl")
            .args([
                "-fL",
                "--proto",
                "=https",
                // `--proto` constrains only the initial request; curl's
                // default redirect protocol set still includes plain http,
                // so without this a redirect could silently downgrade the
                // connection and hand back an arbitrary payload.
                "--proto-redir",
                "=https",
                "--tlsv1.2",
                "--retry",
                "2",
                "-o",
            ])
            .arg(dest)
            .arg(url)
            .status()
            .map_err(|e| format!("failed to run curl: {}", e))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("curl failed downloading {}", url))
        }
    }

    fn sha256(&self, path: &Path) -> Result<String, String> {
        use sha2::{Digest, Sha256};
        // Hash by streaming through the file rather than `std::fs::read`ing
        // it whole — the Flutter macOS arm64 archive alone runs to roughly a
        // gigabyte, and reading that into memory is an RSS spike that can
        // OOM a constrained CI container.
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
        Ok(format!("{:x}", hasher.finalize()))
    }
}

/// The message shown to the user before an archive with no published
/// checksum is installed. Factored out so the exact user-visible text is
/// assertable from a test without redirecting the process's real stderr.
fn unverifiable_download_warning(url: &str) -> String {
    format!(
        "warning: installing {} without integrity verification — no \
         checksum is published for this archive",
        url
    )
}

#[derive(Default)]
pub struct FakeRunner {
    calls: Mutex<Vec<String>>,
    failing: Mutex<Vec<String>>,
    stdout: Mutex<std::collections::HashMap<String, String>>,
}

impl FakeRunner {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub fn fail_on(&mut self, invocation: &str) {
        self.failing.lock().unwrap().push(invocation.to_string());
    }

    /// Build a runner that answers one invocation with canned stdout.
    pub fn with_stdout(invocation: &str, stdout: &str) -> Self {
        let runner = Self::default();
        runner
            .stdout
            .lock()
            .unwrap()
            .insert(invocation.to_string(), stdout.to_string());
        runner
    }
}

impl CommandRunner for FakeRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        _cwd: Option<&Path>,
    ) -> Result<CommandOutput, String> {
        let invocation = if args.is_empty() {
            program.to_string()
        } else {
            format!("{} {}", program, args.join(" "))
        };
        self.calls.lock().unwrap().push(invocation.clone());
        if self.failing.lock().unwrap().contains(&invocation) {
            return Ok(CommandOutput {
                status: 1,
                stdout: String::new(),
                stderr: "boom".into(),
            });
        }
        let stdout = self
            .stdout
            .lock()
            .unwrap()
            .get(&invocation)
            .cloned()
            .unwrap_or_default();
        Ok(CommandOutput {
            status: 0,
            stdout,
            stderr: String::new(),
        })
    }
}

pub struct FakeDownloader {
    digest: String,
    fetches: Mutex<usize>,
}

impl FakeDownloader {
    pub fn new(digest: &str) -> Self {
        Self {
            digest: digest.to_string(),
            fetches: Mutex::new(0),
        }
    }

    pub fn fetches(&self) -> usize {
        *self.fetches.lock().unwrap()
    }
}

impl Downloader for FakeDownloader {
    fn fetch(&self, _url: &str, _dest: &Path) -> Result<(), String> {
        *self.fetches.lock().unwrap() += 1;
        Ok(())
    }

    fn sha256(&self, _path: &Path) -> Result<String, String> {
        Ok(self.digest.clone())
    }
}

/// Run a plan step by step, stopping at the first failure or handoff.
pub fn execute_plan(
    plan: &Plan,
    runner: &dyn CommandRunner,
    dl: &dyn Downloader,
    policy: HandoffPolicy,
    dry_run: bool,
) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    for step in &plan.steps {
        if dry_run {
            outcomes.push(Outcome::Skipped {
                step_id: step.id().to_string(),
            });
            continue;
        }
        let outcome = run_step(step, runner, dl, policy);
        let stop = matches!(
            outcome,
            Outcome::Failed { .. } | Outcome::AwaitingManual { .. }
        );
        outcomes.push(outcome);
        if stop {
            break;
        }
    }
    outcomes
}

fn run_step(
    step: &Step,
    runner: &dyn CommandRunner,
    dl: &dyn Downloader,
    policy: HandoffPolicy,
) -> Outcome {
    match step {
        Step::Download {
            id,
            url,
            sha256,
            dest,
        } => {
            if let Err(e) = dl.fetch(url, dest) {
                return Outcome::Failed { error: e };
            }
            if sha256.is_empty() {
                // Upstream publishes no checksum for this archive. Say so
                // loudly — this must reach the user, so it goes to stderr,
                // not just the `log` crate, which the CLI does not surface
                // by default and which would otherwise leave the user
                // watching an unverified archive get extracted and its
                // binary executed with no visible warning at all.
                eprintln!("\n  {}", unverifiable_download_warning(url));
                log::warn!(
                    "no published checksum for {} — cannot verify integrity",
                    url
                );
                return Outcome::Done {
                    step_id: id.clone(),
                };
            }
            match dl.sha256(dest) {
                Ok(actual) if &actual == sha256 => Outcome::Done {
                    step_id: id.clone(),
                },
                Ok(actual) => {
                    let deleted = std::fs::remove_file(dest).is_ok();
                    let disposition = if deleted {
                        "download deleted"
                    } else {
                        "download NOT deleted — remove it manually before retrying"
                    };
                    Outcome::Failed {
                        error: format!(
                            "checksum mismatch for {} (expected {}, got {}); {}",
                            url, sha256, actual, disposition
                        ),
                    }
                }
                Err(e) => Outcome::Failed { error: e },
            }
        }
        Step::Extract { id, archive, dest } => {
            // The destination is the *parent* of the install dir, which on a
            // fresh machine (`$HOME/development`) does not exist yet. `unzip -d`
            // would create it but `tar -C` would not, so the Linux archive path
            // failed only after the whole SDK had been downloaded.
            if let Err(e) = std::fs::create_dir_all(dest) {
                return Outcome::Failed {
                    error: format!("could not create {}: {}", dest.display(), e),
                };
            }
            let archive_s = archive.to_string_lossy().to_string();
            let dest_s = dest.to_string_lossy().to_string();
            let (program, args) = if archive_s.ends_with(".zip") {
                (
                    "unzip",
                    vec!["-q".to_string(), archive_s, "-d".to_string(), dest_s],
                )
            } else {
                (
                    "tar",
                    vec!["-xf".to_string(), archive_s, "-C".to_string(), dest_s],
                )
            };
            finish(runner.run(program, &args, None), id)
        }
        Step::Run {
            id,
            program,
            args,
            cwd,
        } => finish(runner.run(program, args, cwd.as_deref()), id),
        Step::Verify { id, probe } => finish(runner.run(&probe.program, &probe.args, None), id),
        Step::PathHint { id, .. } => Outcome::Done {
            step_id: id.clone(),
        },
        Step::Handoff { command, .. } => match policy {
            HandoffPolicy::Report => Outcome::AwaitingManual {
                command: command.clone(),
            },
            HandoffPolicy::Prompt => Outcome::AwaitingManual {
                command: command.clone(),
            },
        },
    }
}

fn finish(result: Result<CommandOutput, String>, id: &str) -> Outcome {
    match result {
        Ok(out) if out.status == 0 => Outcome::Done {
            step_id: id.to_string(),
        },
        Ok(out) => Outcome::Failed {
            error: format!("step `{}` exited {}: {}", id, out.status, out.stderr.trim()),
        },
        Err(e) => Outcome::Failed { error: e },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::types::{Plan, Probe, Step};
    use std::path::PathBuf;

    fn download_plan(sha: &str) -> Plan {
        Plan {
            check_id: "flutter".into(),
            steps: vec![
                Step::Download {
                    id: "download".into(),
                    url: "https://example.test/flutter.tar.xz".into(),
                    sha256: sha.into(),
                    dest: PathBuf::from("/tmp/scratch/flutter.tar.xz"),
                },
                Step::Run {
                    id: "verify-runs".into(),
                    program: "flutter".into(),
                    args: vec!["--version".into()],
                    cwd: None,
                },
            ],
        }
    }

    #[test]
    fn an_empty_checksum_is_accepted_but_warned_about() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("whatever");
        let plan = Plan {
            check_id: "android".into(),
            steps: vec![Step::Download {
                id: "download".into(),
                url: "https://dl.google.test/tools.zip".into(),
                sha256: String::new(),
                dest: PathBuf::from("/tmp/scratch/tools.zip"),
            }],
        };
        let out = execute_plan(&plan, &runner, &dl, HandoffPolicy::Report, false);
        assert_eq!(
            out,
            vec![Outcome::Done {
                step_id: "download".into()
            }]
        );
    }

    #[test]
    fn the_unverifiable_download_warning_names_the_url_and_says_why() {
        // `run_step` prints exactly this string to stderr for an empty
        // checksum (see the `eprintln!` in the `Step::Download` arm) — this
        // is the actual user-visible text, not a paraphrase of it, so
        // asserting on it here is equivalent to capturing the real stderr
        // output without needing to redirect the process's own fd.
        let msg = unverifiable_download_warning("https://dl.google.test/tools.zip");
        assert!(
            msg.contains("https://dl.google.test/tools.zip"),
            "warning must name the URL: {}",
            msg
        );
        assert!(
            msg.to_lowercase().contains("integrity")
                || msg.to_lowercase().contains("checksum")
                || msg.to_lowercase().contains("verif"),
            "warning must say the archive is unverifiable: {}",
            msg
        );
    }

    fn extract_plan(dest: &std::path::Path) -> Plan {
        Plan {
            check_id: "flutter".into(),
            steps: vec![Step::Extract {
                id: "extract".into(),
                archive: PathBuf::from("/tmp/scratch/flutter.tar.xz"),
                dest: dest.to_path_buf(),
            }],
        }
    }

    #[test]
    fn extract_creates_a_missing_destination_before_unpacking() {
        // The extract destination is the *parent* of the install dir, so on a
        // fresh machine `$HOME/development` does not exist yet. `tar -C` will
        // not create it, and failing here would waste the whole download.
        let tmp = tempfile::TempDir::new().unwrap();
        let dest = tmp.path().join("development");
        assert!(
            !dest.exists(),
            "precondition: the destination must be missing"
        );

        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("abc123");
        let outcomes = execute_plan(
            &extract_plan(&dest),
            &runner,
            &dl,
            HandoffPolicy::Report,
            false,
        );

        assert!(dest.is_dir(), "the executor must create the destination");
        assert_eq!(
            outcomes,
            vec![Outcome::Done {
                step_id: "extract".into()
            }]
        );
        assert_eq!(
            runner.calls(),
            vec![format!(
                "tar -xf /tmp/scratch/flutter.tar.xz -C {}",
                dest.display()
            )],
            "the extract command must still run"
        );
    }

    #[test]
    fn extract_reports_a_failure_to_create_the_destination() {
        let tmp = tempfile::TempDir::new().unwrap();
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();
        let dest = blocker.join("development");

        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("abc123");
        let outcomes = execute_plan(
            &extract_plan(&dest),
            &runner,
            &dl,
            HandoffPolicy::Report,
            false,
        );

        assert!(
            matches!(outcomes.as_slice(), [Outcome::Failed { .. }]),
            "an undiggable destination must fail loudly, got {:?}",
            outcomes
        );
        assert!(
            runner.calls().is_empty(),
            "nothing should be unpacked when the destination cannot be made"
        );
    }

    #[test]
    fn dry_run_executes_nothing_and_reports_every_step_skipped() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("abc123");
        let out = execute_plan(
            &download_plan("abc123"),
            &runner,
            &dl,
            HandoffPolicy::Report,
            true,
        );
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|o| matches!(o, Outcome::Skipped { .. })));
        assert!(runner.calls().is_empty(), "dry run must not run commands");
        assert_eq!(dl.fetches(), 0, "dry run must not download");
    }

    #[test]
    fn checksum_mismatch_fails_and_stops_the_plan() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("deadbeef");
        let out = execute_plan(
            &download_plan("abc123"),
            &runner,
            &dl,
            HandoffPolicy::Report,
            false,
        );
        assert!(matches!(out.last(), Some(Outcome::Failed { .. })));
        assert_eq!(out.len(), 1, "plan must stop at the failed step");
        assert!(
            runner.calls().is_empty(),
            "must not proceed after a bad checksum"
        );
    }

    #[test]
    fn checksum_mismatch_message_reflects_whether_the_download_was_actually_deleted() {
        // `download_plan`'s dest (`/tmp/scratch/flutter.tar.xz`) never
        // actually exists on disk (the `FakeDownloader` never writes it), so
        // `remove_file` fails — the message must say so rather than blindly
        // claiming "download deleted" regardless of what really happened.
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("deadbeef");
        let out = execute_plan(
            &download_plan("abc123"),
            &runner,
            &dl,
            HandoffPolicy::Report,
            false,
        );
        match out.last() {
            Some(Outcome::Failed { error }) => {
                assert!(
                    error.contains("NOT deleted"),
                    "a delete that failed must not be reported as having \
                     succeeded: {}",
                    error
                );
            }
            other => panic!("expected a failed outcome, got {:?}", other),
        }

        // Now the inverse: a dest that really is on disk really does get
        // removed, and the message must say that instead.
        let tmp = tempfile::TempDir::new().unwrap();
        let dest = tmp.path().join("archive.tar.xz");
        std::fs::write(&dest, b"not the real archive").unwrap();
        let plan = Plan {
            check_id: "flutter".into(),
            steps: vec![Step::Download {
                id: "download".into(),
                url: "https://example.test/flutter.tar.xz".into(),
                sha256: "abc123".into(),
                dest: dest.clone(),
            }],
        };
        let out = execute_plan(&plan, &runner, &dl, HandoffPolicy::Report, false);
        match out.last() {
            Some(Outcome::Failed { error }) => {
                assert!(
                    error.contains("download deleted") && !error.contains("NOT deleted"),
                    "a delete that succeeded must say so: {}",
                    error
                );
            }
            other => panic!("expected a failed outcome, got {:?}", other),
        }
        assert!(!dest.exists(), "the bad download must actually be gone");
    }

    #[test]
    fn successful_plan_runs_each_step_in_order() {
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("abc123");
        let out = execute_plan(
            &download_plan("abc123"),
            &runner,
            &dl,
            HandoffPolicy::Report,
            false,
        );
        assert!(
            out.iter().all(|o| matches!(o, Outcome::Done { .. })),
            "{:?}",
            out
        );
        assert_eq!(runner.calls(), vec!["flutter --version".to_string()]);
    }

    #[test]
    fn handoff_under_report_policy_awaits_manual_action() {
        let plan = Plan {
            check_id: "android".into(),
            steps: vec![Step::Handoff {
                id: "licenses".into(),
                reason: "Licence agreements must be accepted by you".into(),
                command: "flutter doctor --android-licenses".into(),
                docs_url: "https://docs.flutter.dev/get-started/install".into(),
                verify: Probe {
                    program: "flutter".into(),
                    args: vec!["doctor".into()],
                },
            }],
        };
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("");
        let out = execute_plan(&plan, &runner, &dl, HandoffPolicy::Report, false);
        assert_eq!(
            out,
            vec![Outcome::AwaitingManual {
                command: "flutter doctor --android-licenses".into()
            }]
        );
        assert!(
            runner.calls().is_empty(),
            "report policy must not run the verify probe"
        );
    }

    #[test]
    fn failing_command_stops_the_plan() {
        let mut runner = FakeRunner::default();
        runner.fail_on("flutter --version");
        let dl = FakeDownloader::new("abc123");
        let out = execute_plan(
            &download_plan("abc123"),
            &runner,
            &dl,
            HandoffPolicy::Report,
            false,
        );
        assert!(matches!(out.last(), Some(Outcome::Failed { .. })));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn path_hint_is_never_a_command() {
        let plan = Plan {
            check_id: "flutter".into(),
            steps: vec![Step::PathHint {
                id: "path".into(),
                dir: PathBuf::from("/Users/ada/development/flutter"),
                rc_file: Some(PathBuf::from("/Users/ada/.zshrc")),
            }],
        };
        let runner = FakeRunner::default();
        let dl = FakeDownloader::new("");
        let out = execute_plan(&plan, &runner, &dl, HandoffPolicy::Report, false);
        assert_eq!(
            out,
            vec![Outcome::Done {
                step_id: "path".into()
            }]
        );
        assert!(
            runner.calls().is_empty(),
            "PathHint must never touch the shell"
        );
    }
}
