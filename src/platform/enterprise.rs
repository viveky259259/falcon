//! Falcon for Enterprise — audit logs, custom policies, compliance reporting.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

const AUDIT_LOG_FILE: &str = ".falcon-data/audit-log.json";
const POLICY_FILE: &str = ".falcon-data/policies.json";

/// An audit log entry recording who did what and when.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub user: String,
    pub action: String,
    pub target: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditLog {
    pub entries: Vec<AuditEntry>,
}

/// Custom policy for enterprise enforcement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub description: String,
    pub conditions: Vec<PolicyCondition>,
    pub action: PolicyAction,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyCondition {
    MinScore(u32),
    MaxErrors(usize),
    RequireRules(Vec<String>),
    ForbidRules(Vec<String>),
    MaxDynamic(usize),
    RequireDispose,
}

impl std::fmt::Display for PolicyCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MinScore(s) => write!(f, "min score {}", s),
            Self::MaxErrors(n) => write!(f, "max {} errors", n),
            Self::RequireRules(r) => write!(f, "require rules: {}", r.join(", ")),
            Self::ForbidRules(r) => write!(f, "forbid rules: {}", r.join(", ")),
            Self::MaxDynamic(n) => write!(f, "max {} dynamic uses", n),
            Self::RequireDispose => write!(f, "all controllers must have dispose()"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyAction {
    Warn,
    Block,
    Audit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicySet {
    pub policies: Vec<Policy>,
}

/// Policy check result.
#[derive(Debug, Clone)]
pub struct PolicyCheckResult {
    pub policy: String,
    pub passed: bool,
    pub message: String,
}

/// Load audit log.
pub fn load_audit_log(root: &Path) -> anyhow::Result<AuditLog> {
    let path = root.join(AUDIT_LOG_FILE);
    if !path.exists() { return Ok(AuditLog::default()); }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Save audit log.
pub fn save_audit_log(root: &Path, log: &AuditLog) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let json = serde_json::to_string_pretty(log)?;
    std::fs::write(root.join(AUDIT_LOG_FILE), json)?;
    Ok(())
}

/// Record an audit entry.
pub fn record_audit(root: &Path, user: &str, action: &str, target: &str, details: &str) -> anyhow::Result<()> {
    let mut log = load_audit_log(root)?;
    let timestamp = get_timestamp();
    log.entries.push(AuditEntry {
        timestamp, user: user.to_string(), action: action.to_string(),
        target: target.to_string(), details: details.to_string(),
    });
    save_audit_log(root, &log)
}

/// Load policies.
pub fn load_policies(root: &Path) -> anyhow::Result<PolicySet> {
    let path = root.join(POLICY_FILE);
    if !path.exists() { return Ok(PolicySet::default()); }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Save policies.
pub fn save_policies(root: &Path, policies: &PolicySet) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let json = serde_json::to_string_pretty(policies)?;
    std::fs::write(root.join(POLICY_FILE), json)?;
    Ok(())
}

/// Generate default enterprise policies.
pub fn default_policies() -> PolicySet {
    PolicySet {
        policies: vec![
            Policy {
                name: "production-readiness".to_string(),
                description: "Code must score 70+ to merge".to_string(),
                conditions: vec![PolicyCondition::MinScore(70)],
                action: PolicyAction::Block,
                enabled: true,
            },
            Policy {
                name: "no-credentials".to_string(),
                description: "No hardcoded credentials allowed".to_string(),
                conditions: vec![PolicyCondition::ForbidRules(vec!["avoid-hardcoded-credentials".to_string()])],
                action: PolicyAction::Block,
                enabled: true,
            },
            Policy {
                name: "resource-safety".to_string(),
                description: "All controllers must be disposed".to_string(),
                conditions: vec![PolicyCondition::RequireDispose],
                action: PolicyAction::Warn,
                enabled: true,
            },
            Policy {
                name: "type-safety".to_string(),
                description: "Max 10 dynamic usages".to_string(),
                conditions: vec![PolicyCondition::MaxDynamic(10)],
                action: PolicyAction::Warn,
                enabled: true,
            },
        ],
    }
}

/// Check policies against analysis results.
pub fn check_policies(root: &Path) -> anyhow::Result<Vec<PolicyCheckResult>> {
    let policies = load_policies(root)?;

    let config = crate::config::FalconConfig::load(root)?;
    let falcon = crate::Falcon::new(config)?;
    let report = falcon.analyze(root)?;
    let score = crate::ai_score::score::score_from_report(&report)?;

    let mut results = Vec::new();

    for policy in &policies.policies {
        if !policy.enabled { continue; }

        for condition in &policy.conditions {
            let (passed, msg) = match condition {
                PolicyCondition::MinScore(min) => {
                    let ok = score.overall >= *min;
                    (ok, format!("Score {}/100 {} minimum {}", score.overall, if ok { "≥" } else { "<" }, min))
                }
                PolicyCondition::MaxErrors(max) => {
                    let errors = report.error_count();
                    let ok = errors <= *max;
                    (ok, format!("{} errors {} maximum {}", errors, if ok { "≤" } else { ">" }, max))
                }
                PolicyCondition::MaxDynamic(max) => {
                    let dynamic_count = report.issues.iter().filter(|i| i.rule == "avoid-dynamic").count();
                    let ok = dynamic_count <= *max;
                    (ok, format!("{} dynamic uses {} maximum {}", dynamic_count, if ok { "≤" } else { ">" }, max))
                }
                PolicyCondition::ForbidRules(rules) => {
                    let violations: Vec<&str> = rules.iter()
                        .filter(|r| report.issues.iter().any(|i| &i.rule == *r))
                        .map(|r| r.as_str())
                        .collect();
                    let ok = violations.is_empty();
                    (ok, if ok { "No forbidden rule violations".to_string() } else { format!("Forbidden rules triggered: {}", violations.join(", ")) })
                }
                PolicyCondition::RequireDispose => {
                    let dispose_issues = report.issues.iter().filter(|i| i.rule == "ensure-dispose-lifecycle").count();
                    let ok = dispose_issues == 0;
                    (ok, format!("{} undisposed controllers", dispose_issues))
                }
                PolicyCondition::RequireRules(required) => {
                    let enabled_rules: Vec<String> = {
                        let cfg = crate::config::FalconConfig::load(root).unwrap_or_default();
                        cfg.rules.iter().map(|r| r.name().to_string()).collect()
                    };
                    let missing: Vec<&str> = required.iter()
                        .filter(|r| !enabled_rules.iter().any(|er| er == *r))
                        .map(|r| r.as_str())
                        .collect();
                    let ok = missing.is_empty();
                    (ok, if ok {
                        "All required rules are enabled".to_string()
                    } else {
                        format!("Missing required rules: {}", missing.join(", "))
                    })
                }
            };

            results.push(PolicyCheckResult {
                policy: policy.name.clone(),
                passed,
                message: msg,
            });
        }
    }

    Ok(results)
}

/// Generate a compliance report.
pub fn generate_compliance_report(root: &Path) -> anyhow::Result<String> {
    let results = check_policies(root)?;
    let audit = load_audit_log(root)?;
    let policies = load_policies(root)?;

    let mut md = String::new();
    md.push_str("# Falcon Compliance Report\n\n");
    md.push_str(&format!("Generated: {}\n\n", get_timestamp()));

    md.push_str("## Policy Compliance\n\n");
    md.push_str("| Policy | Status | Details |\n|---|---|---|\n");
    for r in &results {
        let status = if r.passed { "✅ PASS" } else { "❌ FAIL" };
        md.push_str(&format!("| {} | {} | {} |\n", r.policy, status, r.message));
    }

    md.push_str(&format!("\n## Policies Defined: {}\n\n", policies.policies.len()));
    md.push_str(&format!("## Audit Log: {} entries\n\n", audit.entries.len()));
    if !audit.entries.is_empty() {
        md.push_str("| Time | User | Action | Target |\n|---|---|---|---|\n");
        for entry in audit.entries.iter().rev().take(20) {
            md.push_str(&format!("| {} | {} | {} | {} |\n",
                entry.timestamp, entry.user, entry.action, entry.target));
        }
    }

    Ok(md)
}

/// Print policy check results.
pub fn print_policy_results(results: &[PolicyCheckResult]) {
    println!();
    println!("  {} Enterprise Policy Check", "falcon".bright_cyan().bold());
    println!();

    if results.is_empty() {
        println!("  No policies configured. Run: falcon enterprise init");
        println!();
        return;
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();

    for r in results {
        let icon = if r.passed { "✓".green().bold() } else { "✗".red().bold() };
        println!("  {} {:<25} {}", icon, r.policy.bright_white(), r.message.dimmed());
    }

    println!();
    println!("  {} passed, {} failed", passed.to_string().green(), failed.to_string().red());
    println!();
}

fn get_timestamp() -> String {
    std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| { log::warn!("timestamp: {}", e); "unknown".to_string() })
}
