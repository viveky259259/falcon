use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
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
