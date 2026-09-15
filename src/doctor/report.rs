//! Rendering a diagnosis for humans and for machines.

use crate::doctor::types::{Diagnosis, Status};
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
