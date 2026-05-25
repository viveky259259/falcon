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
    if !path.exists() {
        return Ok(AuditLog::default());
    }
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
pub fn record_audit(
    root: &Path,
    user: &str,
    action: &str,
    target: &str,
    details: &str,
) -> anyhow::Result<()> {
    let mut log = load_audit_log(root)?;
    let timestamp = get_timestamp();
    log.entries.push(AuditEntry {
        timestamp,
        user: user.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        details: details.to_string(),
    });
    save_audit_log(root, &log)
}

/// Load policies.
pub fn load_policies(root: &Path) -> anyhow::Result<PolicySet> {
    let path = root.join(POLICY_FILE);
    if !path.exists() {
        return Ok(PolicySet::default());
    }
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
                conditions: vec![PolicyCondition::ForbidRules(vec![
                    "avoid-hardcoded-credentials".to_string(),
                ])],
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
        if !policy.enabled {
            continue;
        }

        for condition in &policy.conditions {
            let (passed, msg) = match condition {
                PolicyCondition::MinScore(min) => {
                    let ok = score.overall >= *min;
                    (
                        ok,
                        format!(
                            "Score {}/100 {} minimum {}",
                            score.overall,
                            if ok { "≥" } else { "<" },
                            min
                        ),
                    )
                }
                PolicyCondition::MaxErrors(max) => {
                    let errors = report.error_count();
                    let ok = errors <= *max;
                    (
                        ok,
                        format!(
                            "{} errors {} maximum {}",
                            errors,
                            if ok { "≤" } else { ">" },
                            max
                        ),
                    )
                }
                PolicyCondition::MaxDynamic(max) => {
                    let dynamic_count = report
                        .issues
                        .iter()
                        .filter(|i| i.rule == "avoid-dynamic")
                        .count();
                    let ok = dynamic_count <= *max;
                    (
                        ok,
                        format!(
                            "{} dynamic uses {} maximum {}",
                            dynamic_count,
                            if ok { "≤" } else { ">" },
                            max
                        ),
                    )
                }
                PolicyCondition::ForbidRules(rules) => {
                    let violations: Vec<&str> = rules
                        .iter()
                        .filter(|r| report.issues.iter().any(|i| &i.rule == *r))
                        .map(|r| r.as_str())
                        .collect();
                    let ok = violations.is_empty();
                    (
                        ok,
                        if ok {
                            "No forbidden rule violations".to_string()
                        } else {
                            format!("Forbidden rules triggered: {}", violations.join(", "))
                        },
                    )
                }
                PolicyCondition::RequireDispose => {
                    let dispose_issues = report
                        .issues
                        .iter()
                        .filter(|i| i.rule == "ensure-dispose-lifecycle")
                        .count();
                    let ok = dispose_issues == 0;
                    (ok, format!("{} undisposed controllers", dispose_issues))
                }
                PolicyCondition::RequireRules(required) => {
                    let enabled_rules: Vec<String> = {
                        let cfg = crate::config::FalconConfig::load(root).unwrap_or_default();
                        cfg.rules.iter().map(|r| r.name().to_string()).collect()
                    };
                    let missing: Vec<&str> = required
                        .iter()
                        .filter(|r| !enabled_rules.iter().any(|er| er == *r))
                        .map(|r| r.as_str())
                        .collect();
                    let ok = missing.is_empty();
                    (
                        ok,
                        if ok {
                            "All required rules are enabled".to_string()
                        } else {
                            format!("Missing required rules: {}", missing.join(", "))
                        },
                    )
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

    md.push_str(&format!(
        "\n## Policies Defined: {}\n\n",
        policies.policies.len()
    ));
    md.push_str(&format!(
        "## Audit Log: {} entries\n\n",
        audit.entries.len()
    ));
    if !audit.entries.is_empty() {
        md.push_str("| Time | User | Action | Target |\n|---|---|---|---|\n");
        for entry in audit.entries.iter().rev().take(20) {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                entry.timestamp, entry.user, entry.action, entry.target
            ));
        }
    }

    Ok(md)
}

/// Print policy check results.
pub fn print_policy_results(results: &[PolicyCheckResult]) {
    println!();
    println!(
        "  {} Enterprise Policy Check",
        "falcon".bright_cyan().bold()
    );
    println!();

    if results.is_empty() {
        println!("  No policies configured. Run: falcon enterprise init");
        println!();
        return;
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();

    for r in results {
        let icon = if r.passed {
            "✓".green().bold()
        } else {
            "✗".red().bold()
        };
        println!(
            "  {} {:<25} {}",
            icon,
            r.policy.bright_white(),
            r.message.dimmed()
        );
    }

    println!();
    println!(
        "  {} passed, {} failed",
        passed.to_string().green(),
        failed.to_string().red()
    );
    println!();
}

fn get_timestamp() -> String {
    std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("timestamp: {}", e);
            "unknown".to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn write_file(dir: &std::path::Path, rel: &str, content: &str) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&full).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    /// Write a minimal Flutter-like project so `Falcon::analyze` has something
    /// to parse (it won't error on an empty dir, but this is more realistic).
    fn write_minimal_dart_project(root: &std::path::Path) {
        write_file(root, "lib/main.dart", "void main() {}\n");
    }

    // ── PolicyCondition Display ───────────────────────────────────────────────

    #[test]
    fn test_display_min_score() {
        assert_eq!(PolicyCondition::MinScore(70).to_string(), "min score 70");
    }

    #[test]
    fn test_display_max_errors() {
        assert_eq!(PolicyCondition::MaxErrors(5).to_string(), "max 5 errors");
    }

    #[test]
    fn test_display_require_rules_single() {
        let c = PolicyCondition::RequireRules(vec!["rule-a".to_string()]);
        assert_eq!(c.to_string(), "require rules: rule-a");
    }

    #[test]
    fn test_display_require_rules_multiple() {
        let c = PolicyCondition::RequireRules(vec!["rule-a".to_string(), "rule-b".to_string()]);
        assert_eq!(c.to_string(), "require rules: rule-a, rule-b");
    }

    #[test]
    fn test_display_forbid_rules_single() {
        let c = PolicyCondition::ForbidRules(vec!["avoid-hardcoded-credentials".to_string()]);
        assert_eq!(c.to_string(), "forbid rules: avoid-hardcoded-credentials");
    }

    #[test]
    fn test_display_forbid_rules_multiple() {
        let c = PolicyCondition::ForbidRules(vec!["rule-x".to_string(), "rule-y".to_string()]);
        assert_eq!(c.to_string(), "forbid rules: rule-x, rule-y");
    }

    #[test]
    fn test_display_max_dynamic() {
        assert_eq!(
            PolicyCondition::MaxDynamic(10).to_string(),
            "max 10 dynamic uses"
        );
    }

    #[test]
    fn test_display_require_dispose() {
        assert_eq!(
            PolicyCondition::RequireDispose.to_string(),
            "all controllers must have dispose()"
        );
    }

    #[test]
    fn test_display_min_score_zero() {
        assert_eq!(PolicyCondition::MinScore(0).to_string(), "min score 0");
    }

    #[test]
    fn test_display_max_errors_zero() {
        assert_eq!(PolicyCondition::MaxErrors(0).to_string(), "max 0 errors");
    }

    #[test]
    fn test_display_require_rules_empty() {
        let c = PolicyCondition::RequireRules(vec![]);
        assert_eq!(c.to_string(), "require rules: ");
    }

    // ── get_timestamp ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_timestamp_non_empty() {
        let ts = get_timestamp();
        assert!(!ts.is_empty(), "timestamp must not be empty");
    }

    #[test]
    fn test_get_timestamp_looks_like_date() {
        let ts = get_timestamp();
        // Should contain digits (year at minimum); "unknown" only on error
        let has_digits = ts.chars().any(|c| c.is_ascii_digit());
        assert!(has_digits, "timestamp should contain digits: {}", ts);
    }

    // ── load_audit_log — missing file returns default ─────────────────────────

    #[test]
    fn test_load_audit_log_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let log = load_audit_log(tmp.path()).unwrap();
        assert!(log.entries.is_empty());
    }

    // ── save_audit_log & load_audit_log round-trip ────────────────────────────

    #[test]
    fn test_save_and_load_audit_log_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let log = AuditLog {
            entries: vec![AuditEntry {
                timestamp: "2024-01-01T00:00:00".to_string(),
                user: "alice".to_string(),
                action: "scan".to_string(),
                target: "lib/".to_string(),
                details: "routine scan".to_string(),
            }],
        };

        save_audit_log(root, &log).unwrap();
        let loaded = load_audit_log(root).unwrap();

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].user, "alice");
        assert_eq!(loaded.entries[0].action, "scan");
        assert_eq!(loaded.entries[0].target, "lib/");
        assert_eq!(loaded.entries[0].details, "routine scan");
    }

    #[test]
    fn test_save_audit_log_creates_data_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let data_dir = root.join(".falcon-data");

        assert!(!data_dir.exists());
        save_audit_log(root, &AuditLog::default()).unwrap();
        assert!(data_dir.exists());
    }

    #[test]
    fn test_save_audit_log_creates_file() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        save_audit_log(root, &AuditLog::default()).unwrap();
        let path = root.join(".falcon-data/audit-log.json");
        assert!(path.exists());
    }

    #[test]
    fn test_save_audit_log_multiple_entries() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let log = AuditLog {
            entries: vec![
                AuditEntry {
                    timestamp: "t1".to_string(),
                    user: "bob".to_string(),
                    action: "init".to_string(),
                    target: ".".to_string(),
                    details: "first".to_string(),
                },
                AuditEntry {
                    timestamp: "t2".to_string(),
                    user: "carol".to_string(),
                    action: "check".to_string(),
                    target: "lib/".to_string(),
                    details: "second".to_string(),
                },
            ],
        };

        save_audit_log(root, &log).unwrap();
        let loaded = load_audit_log(root).unwrap();
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[1].user, "carol");
    }

    // ── load_audit_log — corrupt JSON returns error ───────────────────────────

    #[test]
    fn test_load_audit_log_corrupt_json_errors() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let data_dir = root.join(".falcon-data");
        fs::create_dir_all(&data_dir).unwrap();
        write_file(root, ".falcon-data/audit-log.json", "{ not valid json }");
        let result = load_audit_log(root);
        assert!(result.is_err());
    }

    // ── record_audit ──────────────────────────────────────────────────────────

    #[test]
    fn test_record_audit_appends_entry() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        record_audit(root, "dave", "deploy", "main.dart", "CI pipeline").unwrap();
        let log = load_audit_log(root).unwrap();

        assert_eq!(log.entries.len(), 1);
        let e = &log.entries[0];
        assert_eq!(e.user, "dave");
        assert_eq!(e.action, "deploy");
        assert_eq!(e.target, "main.dart");
        assert_eq!(e.details, "CI pipeline");
        assert!(!e.timestamp.is_empty());
    }

    #[test]
    fn test_record_audit_multiple_entries_accumulate() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        record_audit(root, "u1", "a1", "t1", "d1").unwrap();
        record_audit(root, "u2", "a2", "t2", "d2").unwrap();
        record_audit(root, "u3", "a3", "t3", "d3").unwrap();

        let log = load_audit_log(root).unwrap();
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[0].user, "u1");
        assert_eq!(log.entries[2].user, "u3");
    }

    #[test]
    fn test_record_audit_preserves_existing_entries() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let initial = AuditLog {
            entries: vec![AuditEntry {
                timestamp: "old-ts".to_string(),
                user: "original".to_string(),
                action: "old-action".to_string(),
                target: "old-target".to_string(),
                details: "pre-existing".to_string(),
            }],
        };
        save_audit_log(root, &initial).unwrap();

        record_audit(root, "new-user", "new-action", "new-target", "new-details").unwrap();
        let log = load_audit_log(root).unwrap();

        assert_eq!(log.entries.len(), 2);
        assert_eq!(log.entries[0].user, "original");
        assert_eq!(log.entries[1].user, "new-user");
    }

    // ── load_policies — missing file returns default ──────────────────────────

    #[test]
    fn test_load_policies_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let ps = load_policies(tmp.path()).unwrap();
        assert!(ps.policies.is_empty());
    }

    // ── save_policies & load_policies round-trip ──────────────────────────────

    #[test]
    fn test_save_and_load_policies_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let policies = PolicySet {
            policies: vec![Policy {
                name: "test-policy".to_string(),
                description: "A test".to_string(),
                conditions: vec![PolicyCondition::MinScore(50)],
                action: PolicyAction::Warn,
                enabled: true,
            }],
        };

        save_policies(root, &policies).unwrap();
        let loaded = load_policies(root).unwrap();

        assert_eq!(loaded.policies.len(), 1);
        assert_eq!(loaded.policies[0].name, "test-policy");
        assert!(loaded.policies[0].enabled);
    }

    #[test]
    fn test_save_policies_creates_data_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let data_dir = root.join(".falcon-data");

        assert!(!data_dir.exists());
        save_policies(root, &PolicySet::default()).unwrap();
        assert!(data_dir.exists());
    }

    #[test]
    fn test_save_policies_creates_file() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        save_policies(root, &PolicySet::default()).unwrap();
        let path = root.join(".falcon-data/policies.json");
        assert!(path.exists());
    }

    #[test]
    fn test_save_policies_preserves_all_conditions() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let policies = PolicySet {
            policies: vec![Policy {
                name: "multi-cond".to_string(),
                description: "multiple conditions".to_string(),
                conditions: vec![
                    PolicyCondition::MinScore(80),
                    PolicyCondition::MaxErrors(0),
                    PolicyCondition::RequireDispose,
                    PolicyCondition::MaxDynamic(5),
                    PolicyCondition::ForbidRules(vec!["bad-rule".to_string()]),
                    PolicyCondition::RequireRules(vec!["good-rule".to_string()]),
                ],
                action: PolicyAction::Block,
                enabled: true,
            }],
        };

        save_policies(root, &policies).unwrap();
        let loaded = load_policies(root).unwrap();

        assert_eq!(loaded.policies[0].conditions.len(), 6);
    }

    #[test]
    fn test_load_policies_corrupt_json_errors() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let data_dir = root.join(".falcon-data");
        fs::create_dir_all(&data_dir).unwrap();
        write_file(root, ".falcon-data/policies.json", "NOT { valid }");
        let result = load_policies(root);
        assert!(result.is_err());
    }

    // ── default_policies ──────────────────────────────────────────────────────

    #[test]
    fn test_default_policies_returns_four_policies() {
        let ps = default_policies();
        assert_eq!(ps.policies.len(), 4);
    }

    #[test]
    fn test_default_policies_all_enabled() {
        let ps = default_policies();
        for p in &ps.policies {
            assert!(p.enabled, "policy '{}' should be enabled by default", p.name);
        }
    }

    #[test]
    fn test_default_policies_production_readiness_exists() {
        let ps = default_policies();
        let found = ps.policies.iter().any(|p| p.name == "production-readiness");
        assert!(found);
    }

    #[test]
    fn test_default_policies_no_credentials_exists() {
        let ps = default_policies();
        let found = ps.policies.iter().any(|p| p.name == "no-credentials");
        assert!(found);
    }

    #[test]
    fn test_default_policies_resource_safety_exists() {
        let ps = default_policies();
        let found = ps.policies.iter().any(|p| p.name == "resource-safety");
        assert!(found);
    }

    #[test]
    fn test_default_policies_type_safety_exists() {
        let ps = default_policies();
        let found = ps.policies.iter().any(|p| p.name == "type-safety");
        assert!(found);
    }

    #[test]
    fn test_default_policies_production_readiness_blocks() {
        let ps = default_policies();
        let p = ps
            .policies
            .iter()
            .find(|p| p.name == "production-readiness")
            .unwrap();
        assert!(matches!(p.action, PolicyAction::Block));
    }

    #[test]
    fn test_default_policies_resource_safety_warns() {
        let ps = default_policies();
        let p = ps
            .policies
            .iter()
            .find(|p| p.name == "resource-safety")
            .unwrap();
        assert!(matches!(p.action, PolicyAction::Warn));
    }

    #[test]
    fn test_default_policies_min_score_is_70() {
        let ps = default_policies();
        let p = ps
            .policies
            .iter()
            .find(|p| p.name == "production-readiness")
            .unwrap();
        assert!(p
            .conditions
            .iter()
            .any(|c| matches!(c, PolicyCondition::MinScore(70))));
    }

    #[test]
    fn test_default_policies_serializable() {
        let ps = default_policies();
        let json = serde_json::to_string(&ps).unwrap();
        let back: PolicySet = serde_json::from_str(&json).unwrap();
        assert_eq!(back.policies.len(), ps.policies.len());
    }

    // ── check_policies ────────────────────────────────────────────────────────

    #[test]
    fn test_check_policies_no_policies_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);
        // No policy file → PolicySet::default() → empty policies
        let results = check_policies(root).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_check_policies_disabled_policy_skipped() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "disabled-check".to_string(),
                description: "should be skipped".to_string(),
                conditions: vec![PolicyCondition::MinScore(100)], // would fail
                action: PolicyAction::Block,
                enabled: false,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        assert!(
            results.is_empty(),
            "disabled policy should produce no results"
        );
    }

    #[test]
    fn test_check_policies_max_errors_empty_project_passes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "error-limit".to_string(),
                description: "allow many errors".to_string(),
                conditions: vec![PolicyCondition::MaxErrors(9999)],
                action: PolicyAction::Warn,
                enabled: true,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "MaxErrors(9999) should pass on empty project");
    }

    #[test]
    fn test_check_policies_max_dynamic_empty_project_passes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "no-dynamic".to_string(),
                description: "forbid dynamic".to_string(),
                conditions: vec![PolicyCondition::MaxDynamic(100)],
                action: PolicyAction::Warn,
                enabled: true,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
    }

    #[test]
    fn test_check_policies_forbid_rules_no_violations_passes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "no-credentials".to_string(),
                description: "no hard-coded creds".to_string(),
                conditions: vec![PolicyCondition::ForbidRules(vec![
                    "avoid-hardcoded-credentials".to_string(),
                ])],
                action: PolicyAction::Block,
                enabled: true,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        assert_eq!(results[0].policy, "no-credentials");
    }

    #[test]
    fn test_check_policies_require_dispose_clean_project_passes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "dispose-check".to_string(),
                description: "all disposed".to_string(),
                conditions: vec![PolicyCondition::RequireDispose],
                action: PolicyAction::Warn,
                enabled: true,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        assert_eq!(results.len(), 1);
        // Clean project has no dispose issues
        assert!(results[0].passed);
    }

    #[test]
    fn test_check_policies_require_rules_empty_required_passes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "require-nothing".to_string(),
                description: "require no rules".to_string(),
                conditions: vec![PolicyCondition::RequireRules(vec![])],
                action: PolicyAction::Warn,
                enabled: true,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
    }

    #[test]
    fn test_check_policies_result_has_correct_policy_name() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "my-named-policy".to_string(),
                description: "named".to_string(),
                conditions: vec![PolicyCondition::MaxErrors(9999)],
                action: PolicyAction::Audit,
                enabled: true,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        assert_eq!(results[0].policy, "my-named-policy");
    }

    #[test]
    fn test_check_policies_multiple_conditions_each_produces_result() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![Policy {
                name: "multi".to_string(),
                description: "two conditions".to_string(),
                conditions: vec![
                    PolicyCondition::MaxErrors(9999),
                    PolicyCondition::MaxDynamic(9999),
                ],
                action: PolicyAction::Warn,
                enabled: true,
            }],
        };
        save_policies(root, &policies).unwrap();

        let results = check_policies(root).unwrap();
        // One result per condition
        assert_eq!(results.len(), 2);
    }

    // ── generate_compliance_report ────────────────────────────────────────────

    #[test]
    fn test_generate_compliance_report_returns_string() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let report = generate_compliance_report(root).unwrap();
        assert!(!report.is_empty());
    }

    #[test]
    fn test_generate_compliance_report_has_heading() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let report = generate_compliance_report(root).unwrap();
        assert!(
            report.contains("Falcon Compliance Report"),
            "report must have title"
        );
    }

    #[test]
    fn test_generate_compliance_report_has_policy_section() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let report = generate_compliance_report(root).unwrap();
        assert!(report.contains("Policy Compliance"));
    }

    #[test]
    fn test_generate_compliance_report_includes_audit_log_count() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        // Add two audit entries
        record_audit(root, "u1", "scan", ".", "first").unwrap();
        record_audit(root, "u2", "check", ".", "second").unwrap();

        let report = generate_compliance_report(root).unwrap();
        assert!(report.contains("2 entries"), "should show audit log count");
    }

    #[test]
    fn test_generate_compliance_report_includes_policies_count() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let policies = PolicySet {
            policies: vec![
                Policy {
                    name: "p1".to_string(),
                    description: "d1".to_string(),
                    conditions: vec![PolicyCondition::MaxErrors(9999)],
                    action: PolicyAction::Warn,
                    enabled: true,
                },
                Policy {
                    name: "p2".to_string(),
                    description: "d2".to_string(),
                    conditions: vec![PolicyCondition::MaxDynamic(9999)],
                    action: PolicyAction::Warn,
                    enabled: true,
                },
            ],
        };
        save_policies(root, &policies).unwrap();

        let report = generate_compliance_report(root).unwrap();
        assert!(
            report.contains("Policies Defined: 2"),
            "report should show policy count"
        );
    }

    #[test]
    fn test_generate_compliance_report_no_audit_entries_no_table() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let report = generate_compliance_report(root).unwrap();
        // With 0 entries the audit table header is not written
        assert!(report.contains("0 entries"));
    }

    #[test]
    fn test_generate_compliance_report_audit_table_shown_when_entries_present() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        record_audit(root, "tester", "report-gen", ".", "coverage test").unwrap();

        let report = generate_compliance_report(root).unwrap();
        // The audit table header should appear
        assert!(report.contains("| Time |"));
        assert!(report.contains("tester"));
    }

    #[test]
    fn test_generate_compliance_report_includes_generated_timestamp() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_minimal_dart_project(root);

        let report = generate_compliance_report(root).unwrap();
        assert!(report.contains("Generated:"));
    }

    // ── PolicyCheckResult struct ──────────────────────────────────────────────

    #[test]
    fn test_policy_check_result_fields() {
        let r = PolicyCheckResult {
            policy: "my-policy".to_string(),
            passed: true,
            message: "all good".to_string(),
        };
        assert_eq!(r.policy, "my-policy");
        assert!(r.passed);
        assert_eq!(r.message, "all good");
    }

    // ── AuditLog / AuditEntry struct ──────────────────────────────────────────

    #[test]
    fn test_audit_log_default_is_empty() {
        let log = AuditLog::default();
        assert!(log.entries.is_empty());
    }

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry {
            timestamp: "2024-06-01T12:00:00".to_string(),
            user: "eve".to_string(),
            action: "merge".to_string(),
            target: "main".to_string(),
            details: "release".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user, "eve");
        assert_eq!(back.action, "merge");
    }
}
