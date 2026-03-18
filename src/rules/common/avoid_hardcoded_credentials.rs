use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidHardcodedCredentials;

const SUSPICIOUS_NAMES: &[&str] = &[
    "apiKey", "api_key", "apikey", "API_KEY",
    "secret", "SECRET", "secretKey", "secret_key",
    "password", "PASSWORD", "passwd",
    "token", "TOKEN", "accessToken", "access_token",
    "authToken", "auth_token",
    "privateKey", "private_key",
    "clientSecret", "client_secret",
];

impl Rule for AvoidHardcodedCredentials {
    fn name(&self) -> &'static str {
        "avoid-hardcoded-credentials"
    }

    fn description(&self) -> &'static str {
        "Hardcoded API keys, tokens, and passwords should use environment variables or secure storage."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, _root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let file_str = file.to_string_lossy();
        if file_str.contains("_test.dart") || file_str.contains("/test/") || file_str.contains(".g.dart") {
            return Vec::new();
        }

        let mut issues = Vec::new();

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }

            for name in SUSPICIOUS_NAMES {
                if trimmed.contains(name) && (trimmed.contains("= '") || trimmed.contains("= \"")) {
                    let has_real_value = !trimmed.contains("''")
                        && !trimmed.contains("\"\"")
                        && !trimmed.contains("= '';")
                        && !trimmed.contains("= \"\";")
                        && !trimmed.contains("TODO")
                        && !trimmed.contains("FIXME")
                        && !trimmed.contains("example")
                        && !trimmed.contains("placeholder");

                    if has_real_value {
                        issues.push(Issue {
                            rule: "avoid-hardcoded-credentials".to_string(),
                            message: format!(
                                "Possible hardcoded credential '{}' — use environment variables or secure storage.",
                                name
                            ),
                            severity: Severity::Error,
                            file: file.to_path_buf(),
                            line: line_num + 1,
                            column: 1,
                        });
                        break;
                    }
                }
            }
        }

        issues
    }
}
