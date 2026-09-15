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
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(format!("{:x}", hasher.finalize()))
    }
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
            match dl.sha256(dest) {
                Ok(actual) if &actual == sha256 => Outcome::Done {
                    step_id: id.clone(),
                },
                Ok(actual) => {
                    let _ = std::fs::remove_file(dest);
                    Outcome::Failed {
                        error: format!(
                            "checksum mismatch for {} (expected {}, got {}); download deleted",
                            url, sha256, actual
                        ),
                    }
                }
                Err(e) => Outcome::Failed { error: e },
            }
        }
        Step::Extract { id, archive, dest } => {
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
