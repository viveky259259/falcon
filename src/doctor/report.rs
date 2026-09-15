//! Rendering a diagnosis for humans and for machines.

use crate::doctor::types::{Diagnosis, Plan, Status, Step};
use colored::Colorize;

pub fn render_text(d: &Diagnosis) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Falcon doctor \u{2014} {} {}\n\n",
        d.host.os, d.host.arch
    ));
    for c in &d.checks {
        let (mark, detail) = match &c.status {
            Status::Ok { version } => ("ok".green().to_string(), version.clone()),
            Status::Missing => ("missing".red().to_string(), "not installed".to_string()),
            Status::Outdated { found, needed } => (
                "outdated".yellow().to_string(),
                format!("{} installed, {} required", found, needed),
            ),
            Status::Broken { reason } => ("broken".red().to_string(), reason.clone()),
            Status::Skipped { because } => ("skipped".dimmed().to_string(), because.clone()),
        };
        s.push_str(&format!("  {:<12} {:<10} {}\n", c.id, mark, detail));
        if let Some(fix) = &c.fix {
            for step in &fix.steps {
                s.push_str(&format!("               \u{b7} {}\n", step.describe));
            }
        }
    }
    s
}

pub fn render_json(d: &Diagnosis) -> serde_json::Value {
    serde_json::to_value(d).unwrap_or(serde_json::Value::Null)
}

/// A `--dry-run` preview of the resolved plan.
///
/// `--dry-run` promises to "print the plan without executing anything", so it
/// must show the concrete facts the user is about to commit to — the chosen
/// archive, where it unpacks, which binary gets primed — not the generic step
/// descriptions they would see without the flag.
pub fn render_plan_preview(plan: &Plan) -> String {
    let mut s = String::from("\nDry run — nothing was executed. The plan is:\n");
    for step in &plan.steps {
        let detail = match step {
            Step::Download { url, dest, .. } => {
                format!("download {} into {}", url, dest.display())
            }
            Step::Extract { archive, dest, .. } => {
                format!("extract {} into {}", archive.display(), dest.display())
            }
            Step::Run {
                program, args, cwd, ..
            } => match cwd {
                Some(dir) => format!("{} {} (in {})", program, args.join(" "), dir.display()),
                None => format!("{} {}", program, args.join(" ")),
            },
            Step::Verify { probe, .. } => {
                format!("{} {}", probe.program, probe.args.join(" "))
            }
            Step::PathHint { dir, .. } => {
                format!("print the PATH line for {}", dir.display())
            }
            Step::Handoff { command, .. } => format!("hand off to you: {}", command),
        };
        s.push_str(&format!("  would run: {:<11} {}\n", step.id(), detail));
    }
    s
}

/// Why `--format json` diagnoses but never installs.
pub const FIX_SKIPPED_REASON: &str = "--format json is diagnosis-only; use \
     --format text --fix, or the MCP doctor tool with execute:true";

/// JSON mode never executes a fix. Say so in the payload — a caller that passed
/// `--fix` must be able to see the no-op rather than infer it from an unchanged
/// machine.
pub fn render_json_with_fix_status(d: &Diagnosis, fix_requested: bool) -> serde_json::Value {
    let mut value = render_json(d);
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    obj.insert("fixes_applied".into(), serde_json::Value::Bool(false));
    if fix_requested {
        obj.insert(
            "fixes_skipped_reason".into(),
            serde_json::Value::String(FIX_SKIPPED_REASON.to_string()),
        );
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::host::HostInfo;
    use crate::doctor::types::{CheckResult, Diagnosis, Status};

    fn diagnosis() -> Diagnosis {
        Diagnosis {
            host: HostInfo {
                os: "macos".into(),
                arch: "arm64".into(),
                home: None,
                shell_rc: None,
                package_managers: vec![],
            },
            checks: vec![
                CheckResult {
                    id: "flutter".into(),
                    status: Status::Missing,
                    required_by: vec!["every build command".into()],
                    fix: None,
                },
                CheckResult {
                    id: "xcode".into(),
                    status: Status::Skipped {
                        because: "no ios/ directory".into(),
                    },
                    required_by: vec![],
                    fix: None,
                },
            ],
        }
    }

    #[test]
    fn text_report_names_every_check() {
        let text = render_text(&diagnosis());
        assert!(text.contains("flutter"));
        assert!(text.contains("xcode"));
    }

    #[test]
    fn text_report_explains_why_a_check_was_skipped() {
        assert!(render_text(&diagnosis()).contains("no ios/ directory"));
    }

    #[test]
    fn json_report_is_stable_and_machine_readable() {
        let json = render_json(&diagnosis());
        assert_eq!(json["checks"][0]["id"], "flutter");
        assert_eq!(json["checks"][0]["status"]["state"], "missing");
        assert_eq!(json["host"]["arch"], "arm64");
    }

    fn install_plan() -> crate::doctor::types::Plan {
        use crate::doctor::types::{Plan, Probe, Step};
        use std::path::PathBuf;
        Plan {
            check_id: "flutter".into(),
            steps: vec![
                Step::Download {
                    id: "download".into(),
                    url: "https://storage.googleapis.com/flutter_infra_release/releases/\
                          stable/macos/flutter_macos_arm64_3.24.5-stable.zip"
                        .into(),
                    sha256: "1".repeat(64),
                    dest: PathBuf::from("/tmp/falcon-doctor/flutter_macos_arm64_3.24.5-stable.zip"),
                },
                Step::Extract {
                    id: "extract".into(),
                    archive: PathBuf::from(
                        "/tmp/falcon-doctor/flutter_macos_arm64_3.24.5-stable.zip",
                    ),
                    dest: PathBuf::from("/Users/ada/development"),
                },
                Step::Run {
                    id: "prime".into(),
                    program: "/Users/ada/development/flutter/bin/flutter".into(),
                    args: vec!["--version".into()],
                    cwd: None,
                },
                Step::Verify {
                    id: "verify".into(),
                    probe: Probe {
                        program: "/Users/ada/development/flutter/bin/flutter".into(),
                        args: vec!["--version".into()],
                    },
                },
                Step::PathHint {
                    id: "path-hint".into(),
                    dir: PathBuf::from("/Users/ada/development/flutter"),
                    rc_file: None,
                },
            ],
        }
    }

    #[test]
    fn dry_run_preview_shows_the_resolved_archive_and_install_directory() {
        let preview = render_plan_preview(&install_plan());
        assert!(
            preview.contains("flutter_macos_arm64_3.24.5-stable.zip"),
            "the concrete archive must appear, not a generic description: {}",
            preview
        );
        assert!(
            preview.contains("/Users/ada/development/flutter"),
            "the install directory must appear: {}",
            preview
        );
        assert!(
            preview.contains("/Users/ada/development"),
            "the extract destination must appear: {}",
            preview
        );
        assert!(
            preview.contains("would run"),
            "must read as a preview: {}",
            preview
        );
    }

    #[test]
    fn dry_run_preview_names_every_step() {
        let preview = render_plan_preview(&install_plan());
        for id in ["download", "extract", "prime", "verify", "path-hint"] {
            assert!(preview.contains(id), "step {} missing from preview", id);
        }
    }

    #[test]
    fn json_report_states_plainly_that_it_applied_no_fixes() {
        let json = render_json_with_fix_status(&diagnosis(), false);
        assert_eq!(json["fixes_applied"], serde_json::Value::Bool(false));
    }

    #[test]
    fn json_report_explains_why_a_requested_fix_was_not_applied() {
        let json = render_json_with_fix_status(&diagnosis(), true);
        assert_eq!(json["fixes_applied"], serde_json::Value::Bool(false));
        let reason = json["fixes_skipped_reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains("--format text --fix"),
            "the reason must name the working alternative: {}",
            reason
        );
    }

    #[test]
    fn json_report_omits_the_reason_when_no_fix_was_asked_for() {
        let json = render_json_with_fix_status(&diagnosis(), false);
        assert!(json.get("fixes_skipped_reason").is_none());
    }

    #[test]
    fn json_report_omits_absent_fixes() {
        let json = render_json(&diagnosis());
        assert!(json["checks"][0].get("fix").is_none());
    }
}
