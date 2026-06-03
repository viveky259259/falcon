/// Extract a window of source lines centered on `line` (1-based), with `radius`
/// lines of context on each side. Clamps to file bounds. Returns the window text.
pub fn extract_code_window(source: &str, line: usize, radius: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let center = line.saturating_sub(1).min(lines.len() - 1);
    let start = center.saturating_sub(radius);
    let end = (center + radius).min(lines.len() - 1);
    lines[start..=end].join("\n")
}

#[cfg(test)]
mod window_tests {
    use super::*;

    const SRC: &str = "l1\nl2\nl3\nl4\nl5";

    #[test]
    fn window_centered_in_middle() {
        assert_eq!(extract_code_window(SRC, 3, 1), "l2\nl3\nl4");
    }

    #[test]
    fn window_clamps_at_start() {
        assert_eq!(extract_code_window(SRC, 1, 2), "l1\nl2\nl3");
    }

    #[test]
    fn window_clamps_at_end() {
        assert_eq!(extract_code_window(SRC, 5, 2), "l3\nl4\nl5");
    }

    #[test]
    fn empty_source_yields_empty() {
        assert_eq!(extract_code_window("", 1, 3), "");
    }

    #[test]
    fn line_beyond_eof_clamps_to_last() {
        assert_eq!(extract_code_window(SRC, 999, 1), "l4\nl5");
    }
}

use serde::Deserialize;

/// Raw shape the model is asked to emit.
#[derive(Debug, Deserialize)]
struct RawVerdict {
    is_real: bool,
    confidence: u8,
    rationale: String,
}

/// Parsed outcome of triaging one finding. `degraded` means we could not parse a
/// model verdict and fell back to "uncertain".
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVerdict {
    pub is_real: bool,
    pub confidence: u8,
    pub rationale: String,
    pub degraded: bool,
}

/// Parse a model completion into a verdict. Extracts the first balanced JSON
/// object even if surrounded by prose. On any failure, returns an "uncertain"
/// degraded verdict (never errors).
pub fn parse_verdict(raw: &str) -> ParsedVerdict {
    if let Some(json) = first_json_object(raw) {
        if let Ok(v) = serde_json::from_str::<RawVerdict>(&json) {
            return ParsedVerdict {
                is_real: v.is_real,
                confidence: v.confidence.min(100),
                rationale: v.rationale,
                degraded: false,
            };
        }
    }
    ParsedVerdict {
        is_real: true, // fail safe: do not silently hide a finding
        confidence: 0,
        rationale: "Model output could not be parsed; treated as uncertain.".to_string(),
        degraded: true,
    }
}

/// Return the first balanced `{...}` substring, or None.
fn first_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let mut depth = 0usize;
    for (i, ch) in s[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(s[start..start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn parses_clean_json() {
        let v = parse_verdict(r#"{"is_real": false, "confidence": 72, "rationale": "barrel re-export"}"#);
        assert!(!v.is_real);
        assert_eq!(v.confidence, 72);
        assert_eq!(v.rationale, "barrel re-export");
        assert!(!v.degraded);
    }

    #[test]
    fn parses_json_wrapped_in_prose() {
        let v = parse_verdict("Sure! Here is my answer:\n{\"is_real\": true, \"confidence\": 90, \"rationale\": \"x\"} Hope that helps.");
        assert!(v.is_real);
        assert_eq!(v.confidence, 90);
        assert!(!v.degraded);
    }

    #[test]
    fn clamps_confidence_over_100() {
        let v = parse_verdict(r#"{"is_real": true, "confidence": 250, "rationale": "x"}"#);
        assert_eq!(v.confidence, 100);
    }

    #[test]
    fn malformed_output_is_degraded_uncertain() {
        let v = parse_verdict("I think this is probably fine, no JSON here");
        assert!(v.degraded);
        assert_eq!(v.confidence, 0);
        assert!(v.is_real, "fail-safe keeps finding visible");
    }

    #[test]
    fn empty_output_is_degraded() {
        let v = parse_verdict("");
        assert!(v.degraded);
    }
}

use crate::ai::config::EmbeddedModelConfig;
use crate::ai::local::completer::{Completer, GenOpts};
use crate::ai::local::prompt::build_triage_prompt;
use crate::reporters::Issue;
use std::path::Path;

/// Final triage result for one finding.
#[derive(Debug, Clone)]
pub struct TriageVerdict {
    pub issue: Issue,
    pub is_real: bool,
    pub confidence: u8,
    pub rationale: String,
    pub degraded: bool,
}

/// Outcome of a triage run: verdicts plus whether the issue list was truncated.
pub struct TriageRun {
    pub verdicts: Vec<TriageVerdict>,
    pub truncated_from: Option<usize>, // Some(original_len) if capped
}

/// Triage findings sequentially through `engine`. Reads code windows relative to
/// `project_root`. Applies `cfg.max_issues` cap (recording truncation). A failing
/// completion degrades that single verdict to "uncertain" and continues.
pub fn triage_issues(
    issues: &[Issue],
    project_root: &Path,
    engine: &mut dyn Completer,
    cfg: &EmbeddedModelConfig,
) -> TriageRun {
    let original_len = issues.len();
    let truncated_from = if original_len > cfg.max_issues { Some(original_len) } else { None };
    let opts = GenOpts { max_tokens: cfg.max_tokens, temperature: 0.0, stop: vec!["\n\n".to_string()] };

    let verdicts = issues
        .iter()
        .take(cfg.max_issues)
        .map(|issue| {
            let abs = project_root.join(&issue.file);
            let source = std::fs::read_to_string(&abs)
                .or_else(|_| std::fs::read_to_string(&issue.file))
                .unwrap_or_default();
            let window = extract_code_window(&source, issue.line, cfg.context_lines);
            let prompt = build_triage_prompt(issue, &window);
            let parsed = match engine.complete(&prompt, &opts) {
                Ok(text) => parse_verdict(&text),
                Err(_) => ParsedVerdict {
                    is_real: true,
                    confidence: 0,
                    rationale: "Inference failed; treated as uncertain.".to_string(),
                    degraded: true,
                },
            };
            TriageVerdict {
                issue: issue.clone(),
                is_real: parsed.is_real,
                confidence: parsed.confidence,
                rationale: parsed.rationale,
                degraded: parsed.degraded,
            }
        })
        .collect();

    TriageRun { verdicts, truncated_from }
}

#[cfg(test)]
mod orchestration_tests {
    use super::*;
    use crate::ai::local::completer::tests::FakeCompleter;
    use crate::config::Severity;
    use std::path::PathBuf;

    fn issue(line: usize) -> Issue {
        Issue {
            rule: "unused-code".to_string(),
            message: "'x' appears to be unused".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("does_not_exist.dart"),
            line,
            column: 1,
        }
    }

    #[test]
    fn maps_each_issue_to_a_verdict() {
        let issues = vec![issue(1), issue(2)];
        let mut fake = FakeCompleter::new(vec![
            r#"{"is_real": true, "confidence": 80, "rationale": "real"}"#,
            r#"{"is_real": false, "confidence": 60, "rationale": "noise"}"#,
        ]);
        let run = triage_issues(&issues, Path::new("."), &mut fake, &EmbeddedModelConfig::default());
        assert_eq!(run.verdicts.len(), 2);
        assert!(run.verdicts[0].is_real);
        assert!(!run.verdicts[1].is_real);
        assert!(run.truncated_from.is_none());
    }

    #[test]
    fn inference_error_degrades_single_verdict() {
        let issues = vec![issue(1)];
        let mut fake = FakeCompleter { responses: vec![], calls: 0, fail_when_empty: true };
        let run = triage_issues(&issues, Path::new("."), &mut fake, &EmbeddedModelConfig::default());
        assert_eq!(run.verdicts.len(), 1);
        assert!(run.verdicts[0].degraded);
    }

    #[test]
    fn applies_max_issues_cap_and_records_truncation() {
        let issues: Vec<Issue> = (1..=5).map(issue).collect();
        let cfg = EmbeddedModelConfig { max_issues: 2, ..EmbeddedModelConfig::default() };
        let mut fake = FakeCompleter::new(vec![
            r#"{"is_real": true, "confidence": 1, "rationale": "a"}"#,
            r#"{"is_real": true, "confidence": 1, "rationale": "b"}"#,
        ]);
        let run = triage_issues(&issues, Path::new("."), &mut fake, &cfg);
        assert_eq!(run.verdicts.len(), 2);
        assert_eq!(run.truncated_from, Some(5));
    }
}
