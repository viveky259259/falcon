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
    fn json_report_omits_absent_fixes() {
        let json = render_json(&diagnosis());
        assert!(json["checks"][0].get("fix").is_none());
    }
}
