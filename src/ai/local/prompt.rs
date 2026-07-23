use crate::reporters::Issue;

/// Build the FP-triage prompt for one finding plus its surrounding code.
pub fn build_triage_prompt(issue: &Issue, code_window: &str) -> String {
    format!(
        "You are a senior Flutter/Dart static-analysis reviewer. A linter flagged a \
finding. Decide whether it is a REAL problem worth a developer's attention, or a \
FALSE POSITIVE given the surrounding code.\n\n\
Respond with ONE JSON object and nothing else:\n\
{{\"is_real\": true|false, \"confidence\": 0-100, \"rationale\": \"one short sentence\"}}\n\n\
Few-shot example:\n\
Finding: rule=unused-code at a.dart:1 — '_internal' appears to be unused\n\
Code:\nclass A {{ void _internal() {{}} }}\n\
Answer: {{\"is_real\": true, \"confidence\": 80, \"rationale\": \"Private method has no \
references in this file.\"}}\n\n\
Now the actual finding:\n\
Finding: rule={rule} at {file}:{line} — {message}\n\
Code:\n{code}\n\
Answer:",
        rule = issue.rule,
        file = issue.file.display(),
        line = issue.line,
        message = issue.message,
        code = code_window,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use std::path::PathBuf;

    fn sample_issue() -> Issue {
        Issue {
            rule: "unused-code".to_string(),
            message: "'foo' appears to be unused".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("lib/foo.dart"),
            line: 10,
            column: 3,
        }
    }

    #[test]
    fn prompt_contains_rule_message_location_and_code() {
        let code = "void bar() {}\nvoid foo() {}\n";
        let p = build_triage_prompt(&sample_issue(), code);
        assert!(p.contains("unused-code"));
        assert!(p.contains("appears to be unused"));
        assert!(p.contains("lib/foo.dart"));
        assert!(p.contains("void foo()"));
        assert!(p.contains("is_real"));
        assert!(p.contains("confidence"));
        assert!(p.contains("rationale"));
    }
}
