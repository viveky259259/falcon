use crate::reporters::Issue;
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FixSuggestion {
    pub rule: String,
    pub file: PathBuf,
    pub line: usize,
    pub original: String,
    pub replacement: String,
    pub description: String,
    pub auto_fixable: bool,
}

pub fn generate_fixes(issues: &[Issue], project_root: &Path) -> Vec<FixSuggestion> {
    let mut fixes = Vec::new();

    for issue in issues {
        if let Some(fix) = suggest_fix(issue, project_root) {
            fixes.push(fix);
        }
    }

    fixes
}

fn suggest_fix(issue: &Issue, _project_root: &Path) -> Option<FixSuggestion> {
    let source = std::fs::read_to_string(&issue.file).ok()?;
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = issue.line.checked_sub(1)?;
    let line_content = lines.get(line_idx)?;

    match issue.rule.as_str() {
        "prefer-trailing-comma" => {
            let mut fixed = line_content.trim_end().to_string();
            if !fixed.ends_with(',') {
                if fixed.ends_with(')') || fixed.ends_with(']') || fixed.ends_with('}') {
                    let last = fixed.pop()?;
                    fixed.push(',');
                    fixed.push(last);
                }
            }
            Some(FixSuggestion {
                rule: issue.rule.clone(),
                file: issue.file.clone(),
                line: issue.line,
                original: line_content.to_string(),
                replacement: fixed,
                description: "Add trailing comma.".to_string(),
                auto_fixable: true,
            })
        }
        "double-literal-format" => {
            let text = line_content.to_string();
            let fixed = fix_double_literal(&text);
            if fixed != text {
                Some(FixSuggestion {
                    rule: issue.rule.clone(),
                    file: issue.file.clone(),
                    line: issue.line,
                    original: text,
                    replacement: fixed,
                    description: "Fix double literal format.".to_string(),
                    auto_fixable: true,
                })
            } else {
                None
            }
        }
        "avoid-double-negation" => {
            let text = line_content.to_string();
            let fixed = text.replace("!!", "");
            if fixed != text {
                Some(FixSuggestion {
                    rule: issue.rule.clone(),
                    file: issue.file.clone(),
                    line: issue.line,
                    original: text,
                    replacement: fixed,
                    description: "Remove double negation.".to_string(),
                    auto_fixable: true,
                })
            } else {
                None
            }
        }
        "avoid-expanded-as-spacer" => Some(FixSuggestion {
            rule: issue.rule.clone(),
            file: issue.file.clone(),
            line: issue.line,
            original: line_content.to_string(),
            replacement: line_content
                .replace("Expanded(child: SizedBox())", "const Spacer()")
                .replace("Expanded(child: SizedBox.shrink())", "const Spacer()")
                .replace("Expanded(child: Container())", "const Spacer()"),
            description: "Replace Expanded(child: SizedBox()) with Spacer().".to_string(),
            auto_fixable: true,
        }),
        "prefer-first-last" => {
            let text = line_content.to_string();
            let fixed = text
                .replace(".elementAt(0)", ".first")
                .replace("[0]", ".first");
            if fixed != text {
                Some(FixSuggestion {
                    rule: issue.rule.clone(),
                    file: issue.file.clone(),
                    line: issue.line,
                    original: text,
                    replacement: fixed,
                    description: "Use .first/.last instead of index access.".to_string(),
                    auto_fixable: true,
                })
            } else {
                None
            }
        }
        "newline-before-return" => Some(FixSuggestion {
            rule: issue.rule.clone(),
            file: issue.file.clone(),
            line: issue.line,
            original: line_content.to_string(),
            replacement: format!("\n{}", line_content),
            description: "Add blank line before return statement.".to_string(),
            auto_fixable: true,
        }),
        _ => None,
    }
}

/// Fix double literal format issues (e.g. `0.0` → `.0`, remove trailing zeros).
fn fix_double_literal(text: &str) -> String {
    let mut result = text.to_string();
    result = result.replace(" 0.", " .");
    result = result.replace("(0.", "(.");
    result = result.replace("=0.", "=.");
    if result.contains('.') && result.ends_with('0') && !result.ends_with(".0") {
        result = result.trim_end_matches('0').to_string();
    }
    result
}

pub fn preview_fixes(fixes: &[FixSuggestion]) {
    if fixes.is_empty() {
        println!("  {} No auto-fixable issues found.", "✓".green().bold());
        return;
    }

    let fixable: Vec<_> = fixes.iter().filter(|f| f.auto_fixable).collect();
    let manual: Vec<_> = fixes.iter().filter(|f| !f.auto_fixable).collect();

    println!();
    println!(
        "  {} {} auto-fixable issue(s), {} require manual review",
        "falcon fix".bright_cyan().bold(),
        fixable.len(),
        manual.len()
    );
    println!();

    let mut by_file: HashMap<&PathBuf, Vec<&&FixSuggestion>> = HashMap::new();
    for fix in &fixable {
        by_file.entry(&fix.file).or_default().push(fix);
    }

    for (file, file_fixes) in &by_file {
        let rel = file.file_name().and_then(|f| f.to_str()).unwrap_or("?");
        println!(
            "  {} ({} fixes)",
            rel.bright_white().bold(),
            file_fixes.len()
        );

        for fix in file_fixes {
            println!(
                "    {} L{}: {}",
                fix.rule.bright_yellow(),
                fix.line,
                fix.description
            );
            println!("      {} {}", "-".red(), fix.original.trim().dimmed());
            println!(
                "      {} {}",
                "+".green(),
                fix.replacement.trim().bright_green()
            );
        }
        println!();
    }
}

pub fn apply_fixes(fixes: &[FixSuggestion]) -> usize {
    let fixable: Vec<_> = fixes.iter().filter(|f| f.auto_fixable).collect();
    let mut applied = 0;

    let mut by_file: HashMap<&PathBuf, Vec<&&FixSuggestion>> = HashMap::new();
    for fix in &fixable {
        by_file.entry(&fix.file).or_default().push(fix);
    }

    for (file, mut file_fixes) in by_file {
        file_fixes.sort_by(|a, b| b.line.cmp(&a.line));

        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();

        for fix in &file_fixes {
            let idx = fix.line.saturating_sub(1);
            if idx < lines.len() && lines[idx] == fix.original {
                lines[idx] = fix.replacement.clone();
                applied += 1;
            }
        }

        let _ = std::fs::write(file, lines.join("\n") + "\n");
    }

    applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use crate::reporters::Issue;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_issue(rule: &str, file: PathBuf, line: usize) -> Issue {
        Issue {
            rule: rule.into(),
            message: "test issue".into(),
            severity: Severity::Warning,
            file,
            line,
            column: 1,
        }
    }

    fn write_dart(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    // ── fix_double_literal (pure) ──────────────────────────────────────────

    #[test]
    fn fix_double_literal_space_zero_dot() {
        assert_eq!(fix_double_literal("double x = 0.5;"), "double x = .5;");
    }

    #[test]
    fn fix_double_literal_paren_zero_dot() {
        assert_eq!(fix_double_literal("foo(0.5)"), "foo(.5)");
    }

    #[test]
    fn fix_double_literal_equals_zero_dot() {
        assert_eq!(fix_double_literal("x=0.5"), "x=.5");
    }

    #[test]
    fn fix_double_literal_trailing_zeros_trimmed() {
        // ends with '0' but NOT ".0" → strip trailing zeros
        assert_eq!(fix_double_literal("1.500"), "1.5");
    }

    #[test]
    fn fix_double_literal_dot_zero_not_trimmed() {
        // ends with ".0" → no trim
        let s = "1.0";
        assert_eq!(fix_double_literal(s), s);
    }

    #[test]
    fn fix_double_literal_no_change_plain() {
        let s = "hello world";
        assert_eq!(fix_double_literal(s), s);
    }

    #[test]
    fn fix_double_literal_no_dot_ending_zero_untouched() {
        // has no dot, ends with 0 – trim_end_matches branch not reached
        let s = "100";
        assert_eq!(fix_double_literal(s), s);
    }

    // ── suggest_fix – prefer-trailing-comma ────────────────────────────────

    #[test]
    fn suggest_fix_trailing_comma_paren() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "a.dart", "  foo(bar)\n");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.ends_with(",)"), "got: {}", fix[0].replacement);
        assert!(fix[0].auto_fixable);
    }

    #[test]
    fn suggest_fix_trailing_comma_bracket() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "a.dart", "  [1, 2]\n");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.ends_with(",]"), "got: {}", fix[0].replacement);
    }

    #[test]
    fn suggest_fix_trailing_comma_brace() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "a.dart", "  {a: 1}\n");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.ends_with(",}"), "got: {}", fix[0].replacement);
    }

    #[test]
    fn suggest_fix_trailing_comma_already_has_comma() {
        // line already ends with `,)` – no change needed but a fix is still produced
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "a.dart", "  foo(bar,)\n");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        // replacement equals original (comma already there)
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains(','));
    }

    // ── suggest_fix – double-literal-format ───────────────────────────────

    #[test]
    fn suggest_fix_double_literal_changes_line() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "b.dart", "double x = 0.5;\n");
        let issue = make_issue("double-literal-format", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert_eq!(fix[0].replacement, "double x = .5;");
    }

    #[test]
    fn suggest_fix_double_literal_no_change_returns_none() {
        let tmp = TempDir::new().unwrap();
        // no 0. prefix variants – fix_double_literal returns same string
        let path = write_dart(&tmp, "b.dart", "double x = 1.5;\n");
        let issue = make_issue("double-literal-format", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        // fix_double_literal("double x = 1.5;") == "double x = 1.5;" → no fix
        assert_eq!(fix.len(), 0);
    }

    // ── suggest_fix – avoid-double-negation ───────────────────────────────

    #[test]
    fn suggest_fix_double_negation_removed() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "c.dart", "if (!!flag) {}\n");
        let issue = make_issue("avoid-double-negation", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert_eq!(fix[0].replacement, "if (flag) {}");
    }

    #[test]
    fn suggest_fix_double_negation_no_match_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "c.dart", "if (!flag) {}\n");
        let issue = make_issue("avoid-double-negation", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 0);
    }

    // ── suggest_fix – avoid-expanded-as-spacer ────────────────────────────

    #[test]
    fn suggest_fix_expanded_sizebox() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "d.dart", "  Expanded(child: SizedBox())\n");
        let issue = make_issue("avoid-expanded-as-spacer", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains("const Spacer()"));
    }

    #[test]
    fn suggest_fix_expanded_sizebox_shrink() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "d.dart", "  Expanded(child: SizedBox.shrink())\n");
        let issue = make_issue("avoid-expanded-as-spacer", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains("const Spacer()"));
    }

    #[test]
    fn suggest_fix_expanded_container() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "d.dart", "  Expanded(child: Container())\n");
        let issue = make_issue("avoid-expanded-as-spacer", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains("const Spacer()"));
    }

    // ── suggest_fix – prefer-first-last ───────────────────────────────────

    #[test]
    fn suggest_fix_prefer_first_element_at() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "e.dart", "  var x = list.elementAt(0);\n");
        let issue = make_issue("prefer-first-last", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains(".first"));
    }

    #[test]
    fn suggest_fix_prefer_first_bracket_zero() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "e.dart", "  var x = list[0];\n");
        let issue = make_issue("prefer-first-last", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains(".first"));
    }

    #[test]
    fn suggest_fix_prefer_first_no_match_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "e.dart", "  var x = list[1];\n");
        let issue = make_issue("prefer-first-last", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 0);
    }

    // ── suggest_fix – newline-before-return ───────────────────────────────

    #[test]
    fn suggest_fix_newline_before_return() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "f.dart", "  return value;\n");
        let issue = make_issue("newline-before-return", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.starts_with('\n'));
        assert!(fix[0].replacement.contains("return value;"));
    }

    // ── suggest_fix – unknown rule returns None ────────────────────────────

    #[test]
    fn suggest_fix_unknown_rule_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "g.dart", "  someCode();\n");
        let issue = make_issue("not-a-real-rule", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 0);
    }

    // ── suggest_fix – missing file returns None ───────────────────────────

    #[test]
    fn suggest_fix_missing_file_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.dart");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 0);
    }

    // ── suggest_fix – line out of range returns None ──────────────────────

    #[test]
    fn suggest_fix_line_zero_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "h.dart", "  foo(bar)\n");
        let issue = make_issue("prefer-trailing-comma", path, 0);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 0);
    }

    #[test]
    fn suggest_fix_line_beyond_eof_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "h.dart", "  foo(bar)\n");
        let issue = make_issue("prefer-trailing-comma", path, 999);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 0);
    }

    // ── generate_fixes – multiple issues ─────────────────────────────────

    #[test]
    fn generate_fixes_multiple_issues() {
        let tmp = TempDir::new().unwrap();
        let path1 = write_dart(&tmp, "i1.dart", "  foo(bar)\n");
        let path2 = write_dart(&tmp, "i2.dart", "if (!!flag) {}\n");
        let issues = vec![
            make_issue("prefer-trailing-comma", path1, 1),
            make_issue("avoid-double-negation", path2, 1),
        ];
        let fixes = generate_fixes(&issues, tmp.path());
        assert_eq!(fixes.len(), 2);
    }

    #[test]
    fn generate_fixes_empty_issues() {
        let tmp = TempDir::new().unwrap();
        let fixes = generate_fixes(&[], tmp.path());
        assert!(fixes.is_empty());
    }

    // ── apply_fixes ───────────────────────────────────────────────────────

    #[test]
    fn apply_fixes_applies_single_fix() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "j.dart", "  foo(bar)\n  other()\n");
        let fix = FixSuggestion {
            rule: "prefer-trailing-comma".into(),
            file: path.clone(),
            line: 1,
            original: "  foo(bar)".into(),
            replacement: "  foo(bar,)".into(),
            description: "Add trailing comma.".into(),
            auto_fixable: true,
        };
        let count = apply_fixes(&[fix]);
        assert_eq!(count, 1);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("  foo(bar,)"));
    }

    #[test]
    fn apply_fixes_skips_non_auto_fixable() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "k.dart", "  foo(bar)\n");
        let fix = FixSuggestion {
            rule: "prefer-trailing-comma".into(),
            file: path.clone(),
            line: 1,
            original: "  foo(bar)".into(),
            replacement: "  foo(bar,)".into(),
            description: "Add trailing comma.".into(),
            auto_fixable: false,
        };
        let count = apply_fixes(&[fix]);
        assert_eq!(count, 0);
        let content = std::fs::read_to_string(&path).unwrap();
        // file unchanged
        assert!(content.contains("  foo(bar)"));
        assert!(!content.contains("  foo(bar,)"));
    }

    #[test]
    fn apply_fixes_original_mismatch_skipped() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "l.dart", "  foo(bar)\n");
        let fix = FixSuggestion {
            rule: "prefer-trailing-comma".into(),
            file: path.clone(),
            line: 1,
            original: "  WRONG_CONTENT".into(),
            replacement: "  foo(bar,)".into(),
            description: "Add trailing comma.".into(),
            auto_fixable: true,
        };
        let count = apply_fixes(&[fix]);
        assert_eq!(count, 0);
        // file unchanged
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("  foo(bar)"));
    }

    #[test]
    fn apply_fixes_missing_file_skipped() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing.dart");
        let fix = FixSuggestion {
            rule: "prefer-trailing-comma".into(),
            file: path,
            line: 1,
            original: "  foo(bar)".into(),
            replacement: "  foo(bar,)".into(),
            description: "Add trailing comma.".into(),
            auto_fixable: true,
        };
        // should not panic
        let count = apply_fixes(&[fix]);
        assert_eq!(count, 0);
    }

    #[test]
    fn apply_fixes_multiple_fixes_same_file() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "m.dart", "  foo(bar)\n  baz(qux)\n");
        let fixes = vec![
            FixSuggestion {
                rule: "prefer-trailing-comma".into(),
                file: path.clone(),
                line: 1,
                original: "  foo(bar)".into(),
                replacement: "  foo(bar,)".into(),
                description: "trailing comma".into(),
                auto_fixable: true,
            },
            FixSuggestion {
                rule: "prefer-trailing-comma".into(),
                file: path.clone(),
                line: 2,
                original: "  baz(qux)".into(),
                replacement: "  baz(qux,)".into(),
                description: "trailing comma".into(),
                auto_fixable: true,
            },
        ];
        let count = apply_fixes(&fixes);
        assert_eq!(count, 2);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("  foo(bar,)"));
        assert!(content.contains("  baz(qux,)"));
    }

    #[test]
    fn apply_fixes_empty_list() {
        let count = apply_fixes(&[]);
        assert_eq!(count, 0);
    }
}
