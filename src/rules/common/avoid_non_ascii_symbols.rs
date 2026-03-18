use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

pub struct AvoidNonAsciiSymbols;

impl Rule for AvoidNonAsciiSymbols {
    fn name(&self) -> &'static str {
        "avoid-non-ascii-symbols"
    }

    fn description(&self) -> &'static str {
        "Avoid non-ASCII characters in identifiers."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, _root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }

            let mut in_string = false;
            let mut string_char = '"';
            let chars: Vec<char> = line.chars().collect();

            for (i, &ch) in chars.iter().enumerate() {
                if !in_string && (ch == '\'' || ch == '"') {
                    in_string = true;
                    string_char = ch;
                } else if in_string && ch == string_char && (i == 0 || chars[i - 1] != '\\') {
                    in_string = false;
                } else if !in_string && !ch.is_ascii() {
                    issues.push(Issue {
                        rule: self.name().to_string(),
                        message: format!("Non-ASCII character '{}' found outside string literals.", ch),
                        severity: self.default_severity(),
                        file: file.to_path_buf(),
                        line: line_idx + 1,
                        column: i + 1,
                    });
                    break;
                }
            }
        }

        issues
    }
}
