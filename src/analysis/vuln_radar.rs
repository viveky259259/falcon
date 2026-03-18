//! Vulnerability and anti-pattern radar — detects security and reliability risks
//! based on common Flutter/Dart vulnerability patterns.

use crate::reporters::Issue;
use crate::config::Severity;
use colored::Colorize;
use std::path::Path;

/// Vulnerability finding with risk classification.
#[derive(Debug, Clone)]
pub struct VulnFinding {
    pub issue: Issue,
    pub risk_level: RiskLevel,
    pub cwe: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Critical => write!(f, "CRITICAL"),
            RiskLevel::High => write!(f, "HIGH"),
            RiskLevel::Medium => write!(f, "MEDIUM"),
            RiskLevel::Low => write!(f, "LOW"),
        }
    }
}

/// Scan a project for security vulnerabilities and anti-patterns.
pub fn scan_vulnerabilities(root: &Path) -> Vec<VulnFinding> {
    let mut findings = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| !e.path().to_string_lossy().contains("/test/"))
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        check_insecure_storage(entry.path(), &source, &mut findings);
        check_insecure_network(entry.path(), &source, &mut findings);
        check_injection_risks(entry.path(), &source, &mut findings);
        check_crypto_issues(entry.path(), &source, &mut findings);
        check_data_exposure(entry.path(), &source, &mut findings);
    }

    findings.sort_by(|a, b| risk_priority(&a.risk_level).cmp(&risk_priority(&b.risk_level)));
    findings
}

fn risk_priority(level: &RiskLevel) -> u8 {
    match level { RiskLevel::Critical => 0, RiskLevel::High => 1, RiskLevel::Medium => 2, RiskLevel::Low => 3 }
}

fn check_insecure_storage(file: &Path, source: &str, findings: &mut Vec<VulnFinding>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("///") { continue; }

        if trimmed.contains("SharedPreferences") && (trimmed.contains("password") || trimmed.contains("token") || trimmed.contains("secret")) {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-insecure-storage".to_string(),
                    message: "Sensitive data (password/token) stored in SharedPreferences — use flutter_secure_storage instead".to_string(),
                    severity: Severity::Error,
                    file: file.to_path_buf(), line: i + 1, column: 1,
                },
                risk_level: RiskLevel::High,
                cwe: Some("CWE-312".to_string()),
            });
        }
    }
}

fn check_insecure_network(file: &Path, source: &str, findings: &mut Vec<VulnFinding>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("//") || trimmed.starts_with("///") { continue; }

        if trimmed.contains("http://") && !trimmed.contains("localhost") && !trimmed.contains("127.0.0.1") && !trimmed.contains("10.0.2.2") {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-insecure-http".to_string(),
                    message: "HTTP URL detected — use HTTPS for all network communication".to_string(),
                    severity: Severity::Error,
                    file: file.to_path_buf(), line: i + 1, column: 1,
                },
                risk_level: RiskLevel::High,
                cwe: Some("CWE-319".to_string()),
            });
        }

        if trimmed.contains("badCertificateCallback") && (trimmed.contains("true") || trimmed.contains("=> true")) {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-cert-pinning-bypass".to_string(),
                    message: "SSL certificate validation disabled — vulnerable to man-in-the-middle attacks".to_string(),
                    severity: Severity::Error,
                    file: file.to_path_buf(), line: i + 1, column: 1,
                },
                risk_level: RiskLevel::Critical,
                cwe: Some("CWE-295".to_string()),
            });
        }
    }
}

fn check_injection_risks(file: &Path, source: &str, findings: &mut Vec<VulnFinding>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("rawQuery(") || trimmed.contains("execute(") {
            if trimmed.contains("$") || trimmed.contains("+ ") {
                findings.push(VulnFinding {
                    issue: Issue {
                        rule: "vuln-sql-injection".to_string(),
                        message: "SQL query with string interpolation — use parameterized queries to prevent SQL injection".to_string(),
                        severity: Severity::Error,
                        file: file.to_path_buf(), line: i + 1, column: 1,
                    },
                    risk_level: RiskLevel::Critical,
                    cwe: Some("CWE-89".to_string()),
                });
            }
        }

        if trimmed.contains("Uri.parse(") && trimmed.contains("$") {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-open-redirect".to_string(),
                    message: "URL built from user input — validate and sanitize to prevent open redirect".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(), line: i + 1, column: 1,
                },
                risk_level: RiskLevel::Medium,
                cwe: Some("CWE-601".to_string()),
            });
        }
    }
}

fn check_crypto_issues(file: &Path, source: &str, findings: &mut Vec<VulnFinding>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("md5") || trimmed.contains("MD5") {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-weak-hash".to_string(),
                    message: "MD5 is cryptographically broken — use SHA-256 or stronger".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(), line: i + 1, column: 1,
                },
                risk_level: RiskLevel::Medium,
                cwe: Some("CWE-328".to_string()),
            });
        }

        if trimmed.contains("sha1") || trimmed.contains("SHA1") {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-weak-hash".to_string(),
                    message: "SHA-1 is deprecated for security — use SHA-256 or stronger".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(), line: i + 1, column: 1,
                },
                risk_level: RiskLevel::Low,
                cwe: Some("CWE-328".to_string()),
            });
        }
    }
}

fn check_data_exposure(file: &Path, source: &str, findings: &mut Vec<VulnFinding>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("debugPrint(") || trimmed.contains("print(") {
            if trimmed.contains("password") || trimmed.contains("token") || trimmed.contains("secret") || trimmed.contains("apiKey") {
                findings.push(VulnFinding {
                    issue: Issue {
                        rule: "vuln-sensitive-logging".to_string(),
                        message: "Sensitive data may be logged — remove or mask before production".to_string(),
                        severity: Severity::Error,
                        file: file.to_path_buf(), line: i + 1, column: 1,
                    },
                    risk_level: RiskLevel::High,
                    cwe: Some("CWE-532".to_string()),
                });
            }
        }
    }
}

/// Print vulnerability scan report.
pub fn print_vuln_report(findings: &[VulnFinding]) {
    println!();
    println!(
        "  {} Vulnerability & Anti-Pattern Radar",
        "falcon".bright_cyan().bold()
    );
    println!();

    if findings.is_empty() {
        println!("  {} No vulnerabilities detected.", "✓".green().bold());
        println!();
        return;
    }

    let critical = findings.iter().filter(|f| f.risk_level == RiskLevel::Critical).count();
    let high = findings.iter().filter(|f| f.risk_level == RiskLevel::High).count();
    let medium = findings.iter().filter(|f| f.risk_level == RiskLevel::Medium).count();
    let low = findings.iter().filter(|f| f.risk_level == RiskLevel::Low).count();

    if critical > 0 { println!("  {} CRITICAL: {}", "●".bright_red().bold(), critical); }
    if high > 0 { println!("  {} HIGH:     {}", "●".red(), high); }
    if medium > 0 { println!("  {} MEDIUM:   {}", "●".yellow(), medium); }
    if low > 0 { println!("  {} LOW:      {}", "●".dimmed(), low); }

    println!();
    for f in findings.iter().take(20) {
        let level_color = match f.risk_level {
            RiskLevel::Critical => f.risk_level.to_string().bright_red().bold(),
            RiskLevel::High => f.risk_level.to_string().red(),
            RiskLevel::Medium => f.risk_level.to_string().yellow(),
            RiskLevel::Low => f.risk_level.to_string().dimmed(),
        };
        let cwe = f.cwe.as_deref().unwrap_or("");
        let rel = f.issue.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        println!(
            "  {} {:<10} {}:{} {} {}",
            level_color, cwe, rel, f.issue.line, f.issue.rule.bright_white(), f.issue.message.dimmed()
        );
    }
    if findings.len() > 20 {
        println!("  ... and {} more findings", findings.len() - 20);
    }
    println!();
}
