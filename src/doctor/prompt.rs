//! A numbered-choice prompt over `Question`/`Choice`. Reader and writer are
//! injected so the prompt is testable without a terminal, and so Falcon adds
//! no interactive dependency.

use crate::doctor::types::Question;
use std::io::{BufRead, Write};

/// Resolve one decision.
///
/// `preset` (a CLI flag) wins outright; `yes` takes the recommended option;
/// otherwise the user picks by number. Returns `None` on EOF.
pub fn answer(
    question: &Question,
    preset: Option<&str>,
    yes: bool,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Option<String> {
    if let Some(p) = preset {
        return Some(p.to_string());
    }
    if yes {
        return question
            .options
            .iter()
            .find(|o| o.recommended)
            .map(|o| o.value.clone())
            .or_else(|| question.default.clone());
    }
    if question.options.is_empty() {
        return question.default.clone();
    }

    loop {
        let _ = writeln!(out, "\n{}", question.prompt);
        for (i, opt) in question.options.iter().enumerate() {
            let marker = if opt.recommended {
                " (recommended)"
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "  {}. {} \u{2014} {}{}",
                i + 1,
                opt.label,
                opt.rationale,
                marker
            );
        }
        if let Some(d) = &question.default {
            let _ = write!(out, "Choice [{}]: ", d);
        } else {
            let _ = write!(out, "Choice: ");
        }
        let _ = out.flush();

        let mut line = String::new();
        match input.read_line(&mut line) {
            // End of input: the user is done (Ctrl-D, or a piped script that
            // ran out). Distinct from a reader that is broken, which must say
            // so rather than pass silently as a decision not taken.
            Ok(0) => return None,
            Err(e) => {
                let _ = writeln!(out, "\ncould not read your answer: {}", e);
                return None;
            }
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(d) = &question.default {
                return Some(d.clone());
            }
            continue;
        }
        match trimmed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= question.options.len() => {
                return Some(question.options[n - 1].value.clone())
            }
            _ => {
                let _ = writeln!(out, "Enter 1-{}.", question.options.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::types::{Choice, Question};

    fn question() -> Question {
        Question {
            id: "flutter.channel".into(),
            prompt: "Which Flutter channel?".into(),
            options: vec![
                Choice {
                    value: "stable".into(),
                    label: "stable".into(),
                    rationale: "production-ready".into(),
                    recommended: true,
                },
                Choice {
                    value: "beta".into(),
                    label: "beta".into(),
                    rationale: "next release".into(),
                    recommended: false,
                },
            ],
            default: Some("stable".into()),
        }
    }

    fn ask(input: &str, preset: Option<&str>, yes: bool) -> (Option<String>, String) {
        let mut reader = std::io::BufReader::new(input.as_bytes());
        let mut out: Vec<u8> = Vec::new();
        let got = answer(&question(), preset, yes, &mut reader, &mut out);
        (got, String::from_utf8(out).unwrap())
    }

    #[test]
    fn a_preset_answer_skips_the_prompt_entirely() {
        let (got, printed) = ask("", Some("beta"), false);
        assert_eq!(got.as_deref(), Some("beta"));
        assert!(printed.is_empty(), "a preset must not print a prompt");
    }

    #[test]
    fn yes_mode_takes_the_recommended_option_without_reading_input() {
        let (got, printed) = ask("", None, true);
        assert_eq!(got.as_deref(), Some("stable"));
        assert!(printed.is_empty());
    }

    #[test]
    fn selecting_by_number_returns_that_option() {
        let (got, _) = ask("2\n", None, false);
        assert_eq!(got.as_deref(), Some("beta"));
    }

    #[test]
    fn empty_input_takes_the_default() {
        let (got, _) = ask("\n", None, false);
        assert_eq!(got.as_deref(), Some("stable"));
    }

    #[test]
    fn out_of_range_input_reprompts_then_accepts() {
        let (got, printed) = ask("9\n1\n", None, false);
        assert_eq!(got.as_deref(), Some("stable"));
        assert!(
            printed.contains("Enter 1"),
            "must explain the valid range: {}",
            printed
        );
    }

    #[test]
    fn eof_returns_none_rather_than_looping_forever() {
        let (got, _) = ask("", None, false);
        assert_eq!(got, None);
    }

    /// stdin that is broken rather than merely finished.
    struct BrokenReader;

    impl std::io::Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("stdin exploded"))
        }
    }

    impl std::io::BufRead for BrokenReader {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::other("stdin exploded"))
        }
        fn consume(&mut self, _: usize) {}
    }

    #[test]
    fn a_broken_reader_says_so_instead_of_passing_as_end_of_input() {
        let mut out: Vec<u8> = Vec::new();
        let got = answer(&question(), None, false, &mut BrokenReader, &mut out);
        assert_eq!(got, None);
        let printed = String::from_utf8(out).unwrap();
        assert!(
            printed.contains("stdin exploded"),
            "a read error must be reported, not silently treated as EOF: {}",
            printed
        );
    }

    #[test]
    fn the_prompt_shows_each_rationale() {
        let (_, printed) = ask("1\n", None, false);
        assert!(printed.contains("production-ready"));
        assert!(printed.contains("next release"));
    }
}
