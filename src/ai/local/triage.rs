use crate::ai::config::EmbeddedModelConfig;
use crate::ai::local::completer::{Completer, GenOpts};
use crate::ai::local::prompt::build_triage_prompt;
use crate::reporters::Issue;
use serde::Deserialize;
use std::path::Path;

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

#[derive(Debug, Deserialize)]
struct RawVerdict {
    is_real: bool,
    confidence: u16,
    rationale: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVerdict {
    pub is_real: bool,
    pub confidence: u8,
    pub rationale: String,
    pub degraded: bool,
}

pub fn parse_verdict(raw: &str) -> ParsedVerdict {
    if let Some(json) = first_json_object(raw) {
        if let Ok(verdict) = serde_json::from_str::<RawVerdict>(&json) {
            return ParsedVerdict {
                is_real: verdict.is_real,
                confidence: verdict.confidence.min(100) as u8,
                rationale: verdict.rationale,
                degraded: false,
            };
        }
    }

    ParsedVerdict {
        is_real: true,
        confidence: 0,
        rationale: "Model output could not be parsed; treated as uncertain.".to_string(),
        degraded: true,
    }
}

fn first_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in s[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(s[start..start + offset + 1].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct TriageVerdict {
    pub issue: Issue,
    pub is_real: bool,
    pub confidence: u8,
    pub rationale: String,
    pub degraded: bool,
}

#[derive(Debug, Clone)]
pub struct TriageRun {
    pub verdicts: Vec<TriageVerdict>,
    pub truncated_from: Option<usize>,
}

pub fn triage_issues(
    issues: &[Issue],
    project_root: &Path,
    engine: &mut dyn Completer,
    cfg: &EmbeddedModelConfig,
) -> TriageRun {
    let original_len = issues.len();
    let truncated_from = if original_len > cfg.max_issues {
        Some(original_len)
    } else {
        None
    };
    let opts = GenOpts {
        max_tokens: cfg.max_tokens,
        temperature: 0.0,
        stop: vec!["\n\n".to_string()],
    };

    let verdicts = issues
        .iter()
        .take(cfg.max_issues)
        .map(|issue| {
            let source = read_issue_source(project_root, issue);
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

    TriageRun {
        verdicts,
        truncated_from,
    }
}

fn read_issue_source(project_root: &Path, issue: &Issue) -> String {
    let project_path = project_root.join(&issue.file);
    std::fs::read_to_string(&project_path)
        .or_else(|_| std::fs::read_to_string(&issue.file))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::local::completer::tests::FakeCompleter;
    use crate::config::Severity;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const SRC: &str = "l1\nl2\nl3\nl4\nl5";

    fn issue(file: impl Into<PathBuf>, line: usize) -> Issue {
        Issue {
            rule: "unused-code".to_string(),
            message: "'x' appears to be unused".to_string(),
            severity: Severity::Warning,
            file: file.into(),
            line,
            column: 1,
        }
    }

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

    #[test]
    fn parses_clean_json() {
        let verdict = parse_verdict(
            r#"{"is_real": false, "confidence": 72, "rationale": "barrel re-export"}"#,
        );
        assert!(!verdict.is_real);
        assert_eq!(verdict.confidence, 72);
        assert_eq!(verdict.rationale, "barrel re-export");
        assert!(!verdict.degraded);
    }

    #[test]
    fn parses_json_wrapped_in_prose() {
        let verdict =
            parse_verdict(r#"Sure: {"is_real": true, "confidence": 90, "rationale": "x"} Done."#);
        assert!(verdict.is_real);
        assert_eq!(verdict.confidence, 90);
        assert!(!verdict.degraded);
    }

    #[test]
    fn parses_json_with_braces_inside_string() {
        let verdict = parse_verdict(
            r#"{"is_real": true, "confidence": 85, "rationale": "map literal like {a: b} is fine"}"#,
        );
        assert!(verdict.is_real);
        assert_eq!(verdict.confidence, 85);
        assert!(verdict.rationale.contains("{a: b}"));
    }

    #[test]
    fn clamps_confidence_over_100() {
        let verdict = parse_verdict(r#"{"is_real": true, "confidence": 250, "rationale": "x"}"#);
        assert_eq!(verdict.confidence, 100);
    }

    #[test]
    fn malformed_output_is_degraded_uncertain() {
        let verdict = parse_verdict("I think this is probably fine, no JSON here");
        assert!(verdict.degraded);
        assert_eq!(verdict.confidence, 0);
        assert!(verdict.is_real);
    }

    #[test]
    fn maps_each_issue_to_a_verdict() {
        let issues = vec![
            issue("does_not_exist.dart", 1),
            issue("does_not_exist.dart", 2),
        ];
        let mut fake = FakeCompleter::new(vec![
            r#"{"is_real": true, "confidence": 80, "rationale": "real"}"#,
            r#"{"is_real": false, "confidence": 60, "rationale": "noise"}"#,
        ]);

        let run = triage_issues(
            &issues,
            Path::new("."),
            &mut fake,
            &EmbeddedModelConfig::default(),
        );

        assert_eq!(run.verdicts.len(), 2);
        assert!(run.verdicts[0].is_real);
        assert!(!run.verdicts[1].is_real);
        assert!(run.truncated_from.is_none());
    }

    #[test]
    fn inference_error_degrades_single_verdict() {
        let issues = vec![issue("does_not_exist.dart", 1)];
        let mut fake = FakeCompleter {
            responses: vec![],
            calls: 0,
            fail_when_empty: true,
        };

        let run = triage_issues(
            &issues,
            Path::new("."),
            &mut fake,
            &EmbeddedModelConfig::default(),
        );

        assert_eq!(run.verdicts.len(), 1);
        assert!(run.verdicts[0].degraded);
        assert!(run.verdicts[0].is_real);
    }

    #[test]
    fn applies_max_issues_cap_and_records_truncation() {
        let issues: Vec<Issue> = (1..=5).map(|line| issue("missing.dart", line)).collect();
        let cfg = EmbeddedModelConfig {
            max_issues: 2,
            ..EmbeddedModelConfig::default()
        };
        let mut fake = FakeCompleter::new(vec![
            r#"{"is_real": true, "confidence": 1, "rationale": "a"}"#,
            r#"{"is_real": true, "confidence": 1, "rationale": "b"}"#,
        ]);

        let run = triage_issues(&issues, Path::new("."), &mut fake, &cfg);

        assert_eq!(run.verdicts.len(), 2);
        assert_eq!(run.truncated_from, Some(5));
        assert_eq!(fake.calls, 2);
    }

    #[test]
    fn reads_code_window_relative_to_project_root() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(
            dir.path().join("lib/main.dart"),
            "one\ntwo\nvoid target() {}\nfour\nfive",
        )
        .unwrap();
        let issue = issue("lib/main.dart", 3);
        let mut fake = FakeCompleter::new(vec![
            r#"{"is_real": true, "confidence": 99, "rationale": "saw target"}"#,
        ]);
        let cfg = EmbeddedModelConfig {
            context_lines: 1,
            ..EmbeddedModelConfig::default()
        };

        let run = triage_issues(&[issue], dir.path(), &mut fake, &cfg);

        assert_eq!(run.verdicts.len(), 1);
        assert_eq!(run.verdicts[0].confidence, 99);
    }
}
