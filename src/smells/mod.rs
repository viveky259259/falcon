//! Smell categorization: bucket falcon issues into Dead Code, Code Smells, Security Smells.
//!
//! Used by the `falcon smells` command to give users a SonarQube-style classified view
//! instead of a flat severity-only list.

pub mod dead_folders;

use crate::reporters::Issue;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SmellCategory {
    DeadCode,
    CodeSmell,
    SecuritySmell,
    Other,
}

impl std::fmt::Display for SmellCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmellCategory::DeadCode => write!(f, "dead-code"),
            SmellCategory::CodeSmell => write!(f, "code-smell"),
            SmellCategory::SecuritySmell => write!(f, "security-smell"),
            SmellCategory::Other => write!(f, "other"),
        }
    }
}

/// Classify a falcon rule id into one of the smell buckets.
///
/// Mapping is by rule-id substring. Unknown rules fall to `Other` so they are
/// still surfaced but don't pollute the three primary buckets.
pub fn classify(rule_id: &str) -> SmellCategory {
    let r = rule_id.to_ascii_lowercase();

    // Dead code / unused: prefix-based to avoid catching things like
    // "unused_parameters" which is a code smell, not strictly dead code.
    if r == "dead-code-path"
        || r == "unused-file"
        || r == "unused-folder"
        || r == "unused-code"
        || r == "unused-import"
        || r == "unused-dependency"
        || r == "unused-l10n"
        || r == "unreachable-code"
    {
        return SmellCategory::DeadCode;
    }

    // Security smells: anything in the security/vuln family.
    if r.contains("hardcoded-credential")
        || r.contains("hardcoded_credential")
        || r.contains("vuln")
        || r.contains("insecure")
        || r.contains("unsafe-")
        || r.starts_with("security-")
        || r.contains("sql-injection")
        || r.contains("xss")
        || r.contains("crypto-weak")
    {
        return SmellCategory::SecuritySmell;
    }

    // Code smells: the bulk of falcon's `avoid_*` and `prefer_*` rules,
    // complexity warnings, magic numbers, long functions, etc.
    if r.starts_with("avoid-")
        || r.starts_with("avoid_")
        || r.starts_with("prefer-")
        || r.starts_with("prefer_")
        || r.contains("magic-number")
        || r.contains("long-function")
        || r.contains("long-parameter")
        || r.contains("nested-conditional")
        || r.contains("cognitive-complexity")
        || r.contains("cyclomatic")
        || r.contains("widget-rebuild")
        || r.contains("excessive-widget-nesting")
        || r.contains("global-state")
        || r.contains("dynamic")
        || r.contains("late-keyword")
        || r.contains("empty-catch")
        || r.contains("print-in-production")
        || r.contains("returning-widgets")
        || r.contains("unawaited-future")
        || r.contains("non-ascii")
        || r.contains("double-negation")
        || r.contains("unnecessary-")
        || r.contains("unused-parameter")
        || r.contains("unused_parameter")
    {
        return SmellCategory::CodeSmell;
    }

    SmellCategory::Other
}

/// Group issues by category. Returns a map keyed by category, with stable ordering
/// preserved within each bucket.
pub fn group_by_category(issues: &[Issue]) -> HashMap<SmellCategory, Vec<Issue>> {
    let mut grouped: HashMap<SmellCategory, Vec<Issue>> = HashMap::new();
    for issue in issues {
        let cat = classify(&issue.rule);
        grouped.entry(cat).or_default().push(issue.clone());
    }
    grouped
}

#[derive(Debug, Clone)]
pub struct SmellsSummary {
    pub dead_code: Vec<Issue>,
    pub code_smells: Vec<Issue>,
    pub security_smells: Vec<Issue>,
    pub other: Vec<Issue>,
    pub dead_folders: Vec<PathBuf>,
}

impl SmellsSummary {
    pub fn from_issues(issues: &[Issue], dead_folders: Vec<PathBuf>) -> Self {
        let mut grouped = group_by_category(issues);
        Self {
            dead_code: grouped.remove(&SmellCategory::DeadCode).unwrap_or_default(),
            code_smells: grouped.remove(&SmellCategory::CodeSmell).unwrap_or_default(),
            security_smells: grouped
                .remove(&SmellCategory::SecuritySmell)
                .unwrap_or_default(),
            other: grouped.remove(&SmellCategory::Other).unwrap_or_default(),
            dead_folders,
        }
    }

    pub fn total(&self) -> usize {
        self.dead_code.len()
            + self.code_smells.len()
            + self.security_smells.len()
            + self.other.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use std::path::PathBuf;

    fn issue(rule: &str) -> Issue {
        Issue {
            rule: rule.to_string(),
            message: String::new(),
            severity: Severity::Warning,
            file: PathBuf::from("a.dart"),
            line: 1,
            column: 1,
        }
    }

    #[test]
    fn classifies_dead_code_rules() {
        assert_eq!(classify("dead-code-path"), SmellCategory::DeadCode);
        assert_eq!(classify("unused-file"), SmellCategory::DeadCode);
        assert_eq!(classify("unused-folder"), SmellCategory::DeadCode);
        assert_eq!(classify("unused-import"), SmellCategory::DeadCode);
        assert_eq!(classify("unused-dependency"), SmellCategory::DeadCode);
    }

    #[test]
    fn classifies_security_rules() {
        assert_eq!(
            classify("avoid-hardcoded-credentials"),
            SmellCategory::SecuritySmell
        );
        assert_eq!(classify("vuln-cve-2024-1234"), SmellCategory::SecuritySmell);
        assert_eq!(classify("insecure-random"), SmellCategory::SecuritySmell);
        assert_eq!(classify("crypto-weak-md5"), SmellCategory::SecuritySmell);
    }

    #[test]
    fn classifies_code_smell_rules() {
        assert_eq!(classify("avoid-print-in-production"), SmellCategory::CodeSmell);
        assert_eq!(classify("prefer-const-constructors"), SmellCategory::CodeSmell);
        assert_eq!(classify("no-magic-numbers"), SmellCategory::CodeSmell);
        assert_eq!(classify("cognitive-complexity"), SmellCategory::CodeSmell);
        assert_eq!(classify("avoid-long-functions"), SmellCategory::CodeSmell);
        assert_eq!(classify("avoid-unused-parameters"), SmellCategory::CodeSmell);
    }

    #[test]
    fn unknown_rule_falls_to_other() {
        assert_eq!(classify("custom-internal-rule"), SmellCategory::Other);
        assert_eq!(classify(""), SmellCategory::Other);
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(classify("Avoid-Hardcoded-Credentials"), SmellCategory::SecuritySmell);
        assert_eq!(classify("DEAD-CODE-PATH"), SmellCategory::DeadCode);
    }

    #[test]
    fn group_by_category_buckets_correctly() {
        let issues = vec![
            issue("dead-code-path"),
            issue("avoid-hardcoded-credentials"),
            issue("avoid-print-in-production"),
            issue("custom-rule"),
        ];
        let grouped = group_by_category(&issues);
        assert_eq!(grouped[&SmellCategory::DeadCode].len(), 1);
        assert_eq!(grouped[&SmellCategory::SecuritySmell].len(), 1);
        assert_eq!(grouped[&SmellCategory::CodeSmell].len(), 1);
        assert_eq!(grouped[&SmellCategory::Other].len(), 1);
    }

    #[test]
    fn summary_total_matches_input() {
        let issues = vec![
            issue("dead-code-path"),
            issue("avoid-print-in-production"),
            issue("avoid-hardcoded-credentials"),
        ];
        let summary = SmellsSummary::from_issues(&issues, vec![]);
        assert_eq!(summary.total(), 3);
        assert_eq!(summary.dead_code.len(), 1);
        assert_eq!(summary.code_smells.len(), 1);
        assert_eq!(summary.security_smells.len(), 1);
    }
}
