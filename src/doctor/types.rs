//! Core types shared by every doctor check, fixer and adapter.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The health of one toolchain component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Ok {
        version: String,
    },
    Missing,
    Outdated {
        found: String,
        needed: String,
    },
    Broken {
        reason: String,
    },
    /// Not needed by this project — reported, never failed.
    Skipped {
        because: String,
    },
}

/// How much of a fix Falcon can perform unattended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixKind {
    /// Falcon completes it start to finish.
    Automatic,
    /// Falcon completes part; at least one `Step::Handoff` remains.
    Assisted,
    /// Falcon can only describe the fix.
    Manual,
}

/// One option for a decision, carrying the reason it is suggested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
    pub rationale: String,
    pub recommended: bool,
}

/// A decision only the user (or the driving agent) can make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub prompt: String,
    pub options: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSummary {
    pub id: String,
    pub describe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixOffer {
    pub kind: FixKind,
    pub questions: Vec<Question>,
    pub steps: Vec<StepSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub status: Status,
    /// Why this project needs the component, e.g. "android/ directory present".
    pub required_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub host: crate::doctor::host::HostInfo,
    pub checks: Vec<CheckResult>,
}

/// A command run purely to confirm a step worked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    Download {
        id: String,
        url: String,
        sha256: String,
        dest: PathBuf,
    },
    Extract {
        id: String,
        archive: PathBuf,
        dest: PathBuf,
    },
    Run {
        id: String,
        program: String,
        args: Vec<String>,
        cwd: Option<PathBuf>,
    },
    Verify {
        id: String,
        probe: Probe,
    },
    /// Print the export line. Falcon never edits rc files.
    PathHint {
        id: String,
        dir: PathBuf,
        rc_file: Option<PathBuf>,
    },
    /// A step Falcon will not perform: root, Apple ID auth, or a licence.
    Handoff {
        id: String,
        reason: String,
        command: String,
        docs_url: String,
        /// Reserved for a follow-up: an interactive prompt/verify/retry loop
        /// (re-running this probe after the user says they've done the
        /// handoff, to confirm before continuing) is not implemented in this
        /// wave. Nothing reads this field yet, and Falcon does not currently
        /// re-check a handoff after printing it — see the README.
        verify: Probe,
    },
}

impl Step {
    pub fn id(&self) -> &str {
        match self {
            Step::Download { id, .. }
            | Step::Extract { id, .. }
            | Step::Run { id, .. }
            | Step::Verify { id, .. }
            | Step::PathHint { id, .. }
            | Step::Handoff { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub check_id: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Done { step_id: String },
    Skipped { step_id: String },
    AwaitingManual { command: String },
    Failed { error: String },
}

/// Where a request came from. `Remote` may never execute a fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Local,
    Remote,
}

/// 0 healthy, 1 warnings, 2 a check or fix failed, 3 manual action required.
pub fn exit_code(checks: &[CheckResult], outcomes: &[Outcome]) -> i32 {
    if outcomes
        .iter()
        .any(|o| matches!(o, Outcome::AwaitingManual { .. }))
    {
        return 3;
    }
    if outcomes.iter().any(|o| matches!(o, Outcome::Failed { .. })) {
        return 2;
    }
    let mut code = 0;
    for c in checks {
        match c.status {
            Status::Missing | Status::Broken { .. } => return 2,
            Status::Outdated { .. } => code = code.max(1),
            Status::Ok { .. } | Status::Skipped { .. } => {}
        }
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &'static str, status: Status) -> CheckResult {
        CheckResult {
            id: id.to_string(),
            status,
            required_by: vec![],
            fix: None,
        }
    }

    #[test]
    fn healthy_environment_exits_zero() {
        let checks = vec![check(
            "flutter",
            Status::Ok {
                version: "3.24.5".into(),
            },
        )];
        assert_eq!(exit_code(&checks, &[]), 0);
    }

    #[test]
    fn outdated_is_a_warning() {
        let checks = vec![check(
            "flutter",
            Status::Outdated {
                found: "3.19.0".into(),
                needed: "3.22.0".into(),
            },
        )];
        assert_eq!(exit_code(&checks, &[]), 1);
    }

    #[test]
    fn missing_is_a_failure() {
        let checks = vec![check("flutter", Status::Missing)];
        assert_eq!(exit_code(&checks, &[]), 2);
    }

    #[test]
    fn skipped_checks_do_not_affect_exit_code() {
        let checks = vec![check(
            "xcode",
            Status::Skipped {
                because: "no ios/ directory".into(),
            },
        )];
        assert_eq!(exit_code(&checks, &[]), 0);
    }

    #[test]
    fn awaiting_manual_outranks_failure() {
        let checks = vec![check("android", Status::Missing)];
        let outcomes = vec![Outcome::AwaitingManual {
            command: "flutter doctor --android-licenses".into(),
        }];
        assert_eq!(exit_code(&checks, &outcomes), 3);
    }

    #[test]
    fn failed_outcome_is_exit_two() {
        let checks = vec![check(
            "flutter",
            Status::Ok {
                version: "3.24.5".into(),
            },
        )];
        let outcomes = vec![Outcome::Failed {
            error: "checksum mismatch".into(),
        }];
        assert_eq!(exit_code(&checks, &outcomes), 2);
    }

    #[test]
    fn status_serializes_with_a_state_tag() {
        let json = serde_json::to_value(Status::Missing).unwrap();
        assert_eq!(json, serde_json::json!({ "state": "missing" }));
    }

    #[test]
    fn choice_round_trips_through_json() {
        let c = Choice {
            value: "stable".into(),
            label: "stable".into(),
            rationale: "latest on stable".into(),
            recommended: true,
        };
        let back: Choice = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }
}
