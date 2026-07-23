//! Vulnerability and anti-pattern radar — detects security and reliability risks
//! based on common Flutter/Dart vulnerability patterns.

use crate::config::Severity;
use crate::reporters::Issue;
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
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
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

    findings.sort_by_key(|a| risk_priority(&a.risk_level));
    findings
}

fn risk_priority(level: &RiskLevel) -> u8 {
    match level {
        RiskLevel::Critical => 0,
        RiskLevel::High => 1,
        RiskLevel::Medium => 2,
        RiskLevel::Low => 3,
    }
}

fn check_insecure_storage(file: &Path, source: &str, findings: &mut Vec<VulnFinding>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("///") {
            continue;
        }

        if trimmed.contains("SharedPreferences")
            && (trimmed.contains("password")
                || trimmed.contains("token")
                || trimmed.contains("secret"))
        {
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

        if trimmed.starts_with("//") || trimmed.starts_with("///") {
            continue;
        }

        if trimmed.contains("http://")
            && !trimmed.contains("localhost")
            && !trimmed.contains("127.0.0.1")
            && !trimmed.contains("10.0.2.2")
        {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-insecure-http".to_string(),
                    message: "HTTP URL detected — use HTTPS for all network communication"
                        .to_string(),
                    severity: Severity::Error,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                },
                risk_level: RiskLevel::High,
                cwe: Some("CWE-319".to_string()),
            });
        }

        if trimmed.contains("badCertificateCallback")
            && (trimmed.contains("true") || trimmed.contains("=> true"))
        {
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

        if (trimmed.contains("rawQuery(") || trimmed.contains("execute("))
            && (trimmed.contains("$") || trimmed.contains("+ "))
        {
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

        if trimmed.contains("Uri.parse(") && trimmed.contains("$") {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-open-redirect".to_string(),
                    message:
                        "URL built from user input — validate and sanitize to prevent open redirect"
                            .to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
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
                    message: "MD5 is cryptographically broken — use SHA-256 or stronger"
                        .to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                },
                risk_level: RiskLevel::Medium,
                cwe: Some("CWE-328".to_string()),
            });
        }

        if trimmed.contains("sha1") || trimmed.contains("SHA1") {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-weak-hash".to_string(),
                    message: "SHA-1 is deprecated for security — use SHA-256 or stronger"
                        .to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
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

        if (trimmed.contains("debugPrint(") || trimmed.contains("print("))
            && (trimmed.contains("password")
                || trimmed.contains("token")
                || trimmed.contains("secret")
                || trimmed.contains("apiKey"))
        {
            findings.push(VulnFinding {
                issue: Issue {
                    rule: "vuln-sensitive-logging".to_string(),
                    message: "Sensitive data may be logged — remove or mask before production"
                        .to_string(),
                    severity: Severity::Error,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                },
                risk_level: RiskLevel::High,
                cwe: Some("CWE-532".to_string()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_dart_file(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", content).unwrap();
        path
    }

    fn run_check<F>(source: &str, check_fn: F) -> Vec<VulnFinding>
    where
        F: Fn(&Path, &str, &mut Vec<VulnFinding>),
    {
        let mut findings = Vec::new();
        let path = std::path::Path::new("test.dart");
        check_fn(path, source, &mut findings);
        findings
    }

    // ── RiskLevel::Display ────────────────────────────────────────────────────

    #[test]
    fn risk_level_display_critical() {
        assert_eq!(RiskLevel::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn risk_level_display_high() {
        assert_eq!(RiskLevel::High.to_string(), "HIGH");
    }

    #[test]
    fn risk_level_display_medium() {
        assert_eq!(RiskLevel::Medium.to_string(), "MEDIUM");
    }

    #[test]
    fn risk_level_display_low() {
        assert_eq!(RiskLevel::Low.to_string(), "LOW");
    }

    // ── risk_priority ─────────────────────────────────────────────────────────

    #[test]
    fn risk_priority_ordering() {
        assert!(risk_priority(&RiskLevel::Critical) < risk_priority(&RiskLevel::High));
        assert!(risk_priority(&RiskLevel::High) < risk_priority(&RiskLevel::Medium));
        assert!(risk_priority(&RiskLevel::Medium) < risk_priority(&RiskLevel::Low));
    }

    #[test]
    fn risk_priority_values() {
        assert_eq!(risk_priority(&RiskLevel::Critical), 0);
        assert_eq!(risk_priority(&RiskLevel::High), 1);
        assert_eq!(risk_priority(&RiskLevel::Medium), 2);
        assert_eq!(risk_priority(&RiskLevel::Low), 3);
    }

    // ── scan_vulnerabilities (orchestrator) ───────────────────────────────────

    #[test]
    fn scan_empty_dir_returns_no_findings() {
        let dir = TempDir::new().unwrap();
        let results = scan_vulnerabilities(dir.path());
        assert!(results.is_empty());
    }

    #[test]
    fn scan_non_dart_file_ignored() {
        let dir = TempDir::new().unwrap();
        make_dart_file(&dir, "bad.txt", "SharedPreferences password token");
        let results = scan_vulnerabilities(dir.path());
        assert!(results.is_empty());
    }

    #[test]
    fn scan_test_dir_files_are_ignored() {
        let dir = TempDir::new().unwrap();
        let test_subdir = dir.path().join("test");
        std::fs::create_dir_all(&test_subdir).unwrap();
        let path = test_subdir.join("widget_test.dart");
        std::fs::write(&path, "SharedPreferences password token\n").unwrap();
        let results = scan_vulnerabilities(dir.path());
        assert!(results.is_empty());
    }

    #[test]
    fn scan_results_sorted_by_risk_priority() {
        let dir = TempDir::new().unwrap();
        // sha1 → Low, http → High, badCertificateCallback => true → Critical
        let content = "sha1hash\nhttp://example.com/api\nbadCertificateCallback => true\n";
        make_dart_file(&dir, "mixed.dart", content);
        let results = scan_vulnerabilities(dir.path());
        assert!(!results.is_empty());
        // First finding must have the lowest priority number (Critical = 0)
        let first_priority = risk_priority(&results[0].risk_level);
        for f in &results {
            assert!(risk_priority(&f.risk_level) >= first_priority);
        }
    }

    #[test]
    fn scan_dart_file_picks_up_insecure_http() {
        let dir = TempDir::new().unwrap();
        make_dart_file(&dir, "net.dart", "final url = 'http://example.com/api';\n");
        let results = scan_vulnerabilities(dir.path());
        assert!(results.iter().any(|f| f.issue.rule == "vuln-insecure-http"));
    }

    // ── check_insecure_storage ────────────────────────────────────────────────

    #[test]
    fn insecure_storage_password_triggers() {
        let src = "prefs.setString('key', SharedPreferences password);\n";
        let findings = run_check(src, check_insecure_storage);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-insecure-storage");
        assert_eq!(findings[0].risk_level, RiskLevel::High);
        assert_eq!(findings[0].cwe.as_deref(), Some("CWE-312"));
    }

    #[test]
    fn insecure_storage_token_triggers() {
        let src = "SharedPreferences token store\n";
        let findings = run_check(src, check_insecure_storage);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.line, 1);
    }

    #[test]
    fn insecure_storage_secret_triggers() {
        let src = "SharedPreferences secret value\n";
        let findings = run_check(src, check_insecure_storage);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn insecure_storage_no_sensitive_key_clean() {
        let src = "SharedPreferences username;\n";
        let findings = run_check(src, check_insecure_storage);
        assert!(findings.is_empty());
    }

    #[test]
    fn insecure_storage_comment_skipped() {
        let src = "// SharedPreferences password\n";
        let findings = run_check(src, check_insecure_storage);
        assert!(findings.is_empty());
    }

    #[test]
    fn insecure_storage_doc_comment_skipped() {
        let src = "/// SharedPreferences token secret\n";
        let findings = run_check(src, check_insecure_storage);
        assert!(findings.is_empty());
    }

    // ── check_insecure_network ────────────────────────────────────────────────

    #[test]
    fn insecure_network_http_triggers() {
        let src = "final url = 'http://api.example.com/data';\n";
        let findings = run_check(src, check_insecure_network);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-insecure-http");
        assert_eq!(findings[0].risk_level, RiskLevel::High);
        assert_eq!(findings[0].cwe.as_deref(), Some("CWE-319"));
    }

    #[test]
    fn insecure_network_localhost_is_clean() {
        let src = "final url = 'http://localhost:8080/debug';\n";
        let findings = run_check(src, check_insecure_network);
        assert!(findings.is_empty());
    }

    #[test]
    fn insecure_network_loopback_is_clean() {
        let src = "final url = 'http://127.0.0.1:3000/api';\n";
        let findings = run_check(src, check_insecure_network);
        assert!(findings.is_empty());
    }

    #[test]
    fn insecure_network_android_emulator_is_clean() {
        let src = "final url = 'http://10.0.2.2:8080/api';\n";
        let findings = run_check(src, check_insecure_network);
        assert!(findings.is_empty());
    }

    #[test]
    fn insecure_network_https_is_clean() {
        let src = "final url = 'https://api.example.com/data';\n";
        let findings = run_check(src, check_insecure_network);
        assert!(findings.is_empty());
    }

    #[test]
    fn insecure_network_bad_cert_true_triggers() {
        let src = "..badCertificateCallback: (cert, host, port) => true\n";
        let findings = run_check(src, check_insecure_network);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-cert-pinning-bypass");
        assert_eq!(findings[0].risk_level, RiskLevel::Critical);
        assert_eq!(findings[0].cwe.as_deref(), Some("CWE-295"));
    }

    #[test]
    fn insecure_network_bad_cert_arrow_true_triggers() {
        let src = "badCertificateCallback => true\n";
        let findings = run_check(src, check_insecure_network);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-cert-pinning-bypass");
    }

    #[test]
    fn insecure_network_comment_line_skipped() {
        let src = "// http://example.com bad cert badCertificateCallback true\n";
        let findings = run_check(src, check_insecure_network);
        assert!(findings.is_empty());
    }

    // ── check_injection_risks ─────────────────────────────────────────────────

    #[test]
    fn injection_raw_query_interpolation_triggers_sql() {
        let src = "db.rawQuery('SELECT * FROM users WHERE id = $userId');\n";
        let findings = run_check(src, check_injection_risks);
        assert!(findings
            .iter()
            .any(|f| f.issue.rule == "vuln-sql-injection"));
        let sql = findings
            .iter()
            .find(|f| f.issue.rule == "vuln-sql-injection")
            .unwrap();
        assert_eq!(sql.risk_level, RiskLevel::Critical);
        assert_eq!(sql.cwe.as_deref(), Some("CWE-89"));
    }

    #[test]
    fn injection_execute_with_concat_triggers_sql() {
        let src = "db.execute('DELETE FROM ' + tableName);\n";
        let findings = run_check(src, check_injection_risks);
        assert!(findings
            .iter()
            .any(|f| f.issue.rule == "vuln-sql-injection"));
    }

    #[test]
    fn injection_raw_query_no_interpolation_clean() {
        let src = "db.rawQuery('SELECT * FROM users WHERE id = ?', [userId]);\n";
        let findings = run_check(src, check_injection_risks);
        assert!(!findings
            .iter()
            .any(|f| f.issue.rule == "vuln-sql-injection"));
    }

    #[test]
    fn injection_uri_parse_interpolation_triggers_redirect() {
        let src = "Uri.parse('https://example.com/$userInput');\n";
        let findings = run_check(src, check_injection_risks);
        assert!(findings
            .iter()
            .any(|f| f.issue.rule == "vuln-open-redirect"));
        let redirect = findings
            .iter()
            .find(|f| f.issue.rule == "vuln-open-redirect")
            .unwrap();
        assert_eq!(redirect.risk_level, RiskLevel::Medium);
        assert_eq!(redirect.cwe.as_deref(), Some("CWE-601"));
    }

    #[test]
    fn injection_uri_parse_no_interpolation_clean() {
        let src = "Uri.parse('https://example.com/fixed/path');\n";
        let findings = run_check(src, check_injection_risks);
        assert!(!findings
            .iter()
            .any(|f| f.issue.rule == "vuln-open-redirect"));
    }

    // ── check_crypto_issues ───────────────────────────────────────────────────

    #[test]
    fn crypto_md5_lowercase_triggers() {
        let src = "final hash = md5.convert(bytes);\n";
        let findings = run_check(src, check_crypto_issues);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-weak-hash");
        assert_eq!(findings[0].risk_level, RiskLevel::Medium);
        assert_eq!(findings[0].cwe.as_deref(), Some("CWE-328"));
    }

    #[test]
    fn crypto_md5_uppercase_triggers() {
        let src = "import 'package:crypto/MD5.dart';\n";
        let findings = run_check(src, check_crypto_issues);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-weak-hash");
        assert_eq!(findings[0].risk_level, RiskLevel::Medium);
    }

    #[test]
    fn crypto_sha1_lowercase_triggers() {
        let src = "final h = sha1.convert(bytes);\n";
        let findings = run_check(src, check_crypto_issues);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-weak-hash");
        assert_eq!(findings[0].risk_level, RiskLevel::Low);
        assert_eq!(findings[0].cwe.as_deref(), Some("CWE-328"));
    }

    #[test]
    fn crypto_sha1_uppercase_triggers() {
        let src = "var hasher = SHA1();\n";
        let findings = run_check(src, check_crypto_issues);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].risk_level, RiskLevel::Low);
    }

    #[test]
    fn crypto_sha256_is_clean() {
        let src = "final hash = sha256.convert(bytes);\n";
        let findings = run_check(src, check_crypto_issues);
        assert!(findings.is_empty());
    }

    #[test]
    fn crypto_both_md5_and_sha1_triggers_two() {
        let src = "md5.convert(a);\nsha1.convert(b);\n";
        let findings = run_check(src, check_crypto_issues);
        assert_eq!(findings.len(), 2);
    }

    // ── check_data_exposure ───────────────────────────────────────────────────

    #[test]
    fn data_exposure_print_password_triggers() {
        let src = "print('user password: $pw');\n";
        let findings = run_check(src, check_data_exposure);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-sensitive-logging");
        assert_eq!(findings[0].risk_level, RiskLevel::High);
        assert_eq!(findings[0].cwe.as_deref(), Some("CWE-532"));
    }

    #[test]
    fn data_exposure_debug_print_token_triggers() {
        let src = "debugPrint('token: $tok');\n";
        let findings = run_check(src, check_data_exposure);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.rule, "vuln-sensitive-logging");
    }

    #[test]
    fn data_exposure_print_secret_triggers() {
        let src = "print(secret);\n";
        let findings = run_check(src, check_data_exposure);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn data_exposure_print_api_key_triggers() {
        let src = "print('apiKey=$key');\n";
        let findings = run_check(src, check_data_exposure);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn data_exposure_print_safe_value_clean() {
        let src = "print('user logged in');\n";
        let findings = run_check(src, check_data_exposure);
        assert!(findings.is_empty());
    }

    #[test]
    fn data_exposure_debug_print_safe_clean() {
        let src = "debugPrint('loading complete');\n";
        let findings = run_check(src, check_data_exposure);
        assert!(findings.is_empty());
    }

    #[test]
    fn data_exposure_line_number_reported_correctly() {
        let src = "// safe\nprint(password);\n";
        let findings = run_check(src, check_data_exposure);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue.line, 2);
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

    let critical = findings
        .iter()
        .filter(|f| f.risk_level == RiskLevel::Critical)
        .count();
    let high = findings
        .iter()
        .filter(|f| f.risk_level == RiskLevel::High)
        .count();
    let medium = findings
        .iter()
        .filter(|f| f.risk_level == RiskLevel::Medium)
        .count();
    let low = findings
        .iter()
        .filter(|f| f.risk_level == RiskLevel::Low)
        .count();

    if critical > 0 {
        println!("  {} CRITICAL: {}", "●".bright_red().bold(), critical);
    }
    if high > 0 {
        println!("  {} HIGH:     {}", "●".red(), high);
    }
    if medium > 0 {
        println!("  {} MEDIUM:   {}", "●".yellow(), medium);
    }
    if low > 0 {
        println!("  {} LOW:      {}", "●".dimmed(), low);
    }

    println!();
    for f in findings.iter().take(20) {
        let level_color = match f.risk_level {
            RiskLevel::Critical => f.risk_level.to_string().bright_red().bold(),
            RiskLevel::High => f.risk_level.to_string().red(),
            RiskLevel::Medium => f.risk_level.to_string().yellow(),
            RiskLevel::Low => f.risk_level.to_string().dimmed(),
        };
        let cwe = f.cwe.as_deref().unwrap_or("");
        let rel = f
            .issue
            .file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        println!(
            "  {} {:<10} {}:{} {} {}",
            level_color,
            cwe,
            rel,
            f.issue.line,
            f.issue.rule.bright_white(),
            f.issue.message.dimmed()
        );
    }
    if findings.len() > 20 {
        println!("  ... and {} more findings", findings.len() - 20);
    }
    println!();
}
