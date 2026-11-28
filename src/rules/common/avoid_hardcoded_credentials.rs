use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;
use tree_sitter::Node;

pub struct AvoidHardcodedCredentials;

const SUSPICIOUS_NAMES: &[&str] = &[
    "apiKey",
    "api_key",
    "apikey",
    "API_KEY",
    "secret",
    "SECRET",
    "secretKey",
    "secret_key",
    "password",
    "PASSWORD",
    "passwd",
    "token",
    "TOKEN",
    "accessToken",
    "access_token",
    "authToken",
    "auth_token",
    "privateKey",
    "private_key",
    "clientSecret",
    "client_secret",
];

/// Key formats that are secrets whatever the variable is called. Each is a
/// provider's fixed prefix plus its key body, inside a string literal.
/// Google `AIza…` keys are left out: Firebase client keys are meant to ship.
static KEY_FORMATS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
        ("Stripe live key", r"\b[sr]k_live_[0-9A-Za-z]{16,}"),
        (
            "GitHub token",
            r"\b(?:gh[pousr]_[0-9A-Za-z]{36,}|github_pat_[0-9A-Za-z_]{40,})",
        ),
        ("Slack token", r"\bxox[abprs]-[0-9A-Za-z-]{20,}"),
        ("private key", r"-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----"),
    ]
    .into_iter()
    .filter_map(|(kind, pattern)| Regex::new(pattern).ok().map(|re| (kind, re)))
    .collect()
});

/// The kind of key a line holds in a string literal, if any.
fn key_format(line: &str) -> Option<&'static str> {
    if !line.contains('\'') && !line.contains('"') {
        return None;
    }
    KEY_FORMATS
        .iter()
        .find(|(_, re)| re.is_match(line))
        .map(|(kind, _)| *kind)
}

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
        if file_str.contains("_test.dart")
            || file_str.contains("/test/")
            || file_str.contains(".g.dart")
        {
            return Vec::new();
        }

        let mut issues = Vec::new();

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }

            let mut named = false;
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
                        named = true;
                        break;
                    }
                }
            }

            if let Some(kind) = key_format(trimmed).filter(|_| !named) {
                issues.push(Issue {
                    rule: "avoid-hardcoded-credentials".to_string(),
                    message: format!("Hardcoded {kind} — move it out of the source and rotate it."),
                    severity: Severity::Error,
                    file: file.to_path_buf(),
                    line: line_num + 1,
                    column: 1,
                });
            }
        }

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    fn run_with_path(source: &str, path: &str) -> Vec<Issue> {
        let rule = AvoidHardcodedCredentials;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from(path))
    }

    fn run(source: &str) -> Vec<Issue> {
        run_with_path(source, "lib/foo.dart")
    }

    #[test]
    fn hardcoded_api_key_is_flagged() {
        let issues = run(r#"
const apiKey = 'sk_live_abcdef1234567890';
"#);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "avoid-hardcoded-credentials");
        assert!(issues[0].message.contains("apiKey"));
    }

    /// Made-up values in each provider's format. They are put together here
    /// so the source holds nothing a secret scanner would stop.
    fn fake_key(prefix: &str, body: &str) -> String {
        format!("{prefix}{body}")
    }

    #[test]
    fn known_key_formats_are_flagged_whatever_the_name() {
        let source = format!(
            "\nclass Config {{\n  static const String awsId = '{}';\n  static const stripe = \"{}\";\n  final gh = '{}';\n  final pem = '{}';\n}}\n",
            fake_key("AKIA", "IOSFODNN7EXAMPLE"),
            fake_key("sk_", "live_0000000000000000000000000000"),
            fake_key("ghp_", "000000000000000000000000000000000000"),
            fake_key("-----BEGIN RSA ", "PRIVATE KEY-----"),
        );
        let issues = run(&source);
        let lines: Vec<usize> = issues.iter().map(|i| i.line).collect();
        assert_eq!(lines, vec![3, 4, 5, 6], "{issues:?}");
        assert!(issues[0].message.contains("AWS access key"));
    }

    #[test]
    fn a_line_is_reported_once_and_lookalikes_are_not_flagged() {
        // Name and value both match: one finding.
        let both = format!(
            "const apiKey = '{}';\n",
            fake_key("AKIA", "IOSFODNN7EXAMPLE")
        );
        assert_eq!(run(&both).len(), 1);
        // Too short, a test-mode key, or not in a string.
        let source = format!(
            "\nconst region = 'AKIA1234';\nconst mode = '{}';\nfinal {} = 1;\n",
            fake_key("sk_", "test_0000000000000000000000000000"),
            fake_key("AKIA", "IOSFODNN7EXAMPLE"),
        );
        let issues = run(&source);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn placeholder_password_is_not_flagged() {
        let issues = run(r#"
const password = 'placeholder';
"#);
        assert!(
            issues.is_empty(),
            "placeholder value must not trip the rule"
        );
    }

    #[test]
    fn empty_token_literal_is_not_flagged() {
        let issues = run(r#"
String token = '';
"#);
        assert!(
            issues.is_empty(),
            "empty string assignments should not be flagged"
        );
    }

    #[test]
    fn test_files_are_skipped() {
        // Files under /test/ or with _test.dart suffix are out of scope.
        let issues = run_with_path(
            "const apiKey = 'sk_live_abcdef1234567890';\n",
            "test/widget_test.dart",
        );
        assert!(
            issues.is_empty(),
            "test files must be skipped by the credential rule"
        );
    }
}
