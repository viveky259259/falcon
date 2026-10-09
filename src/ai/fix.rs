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

pub fn generate_fixes(issues: &[Issue], _project_root: &Path) -> Vec<FixSuggestion> {
    use super::syntax_fixes::{fixes_for_issue, non_overlapping};
    let mut by_file: HashMap<&PathBuf, Vec<&Issue>> = HashMap::new();
    for issue in issues {
        by_file.entry(&issue.file).or_default().push(issue);
    }
    let mut suggestions = Vec::new();
    for (file, issues) in by_file {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let edits = non_overlapping(
            issues
                .iter()
                .flat_map(|issue| fixes_for_issue(&source, &issue.rule, issue.line, issue.column))
                .collect(),
        );
        // Group edits that touch the same source lines so every edit survives.
        let mut groups: Vec<(usize, usize, Vec<super::syntax_fixes::SyntaxFix>)> = Vec::new();
        for edit in edits {
            let start = source[..edit.range.start]
                .rfind('\n')
                .map_or(0, |offset| offset + 1);
            let end = source[edit.range.end..]
                .find('\n')
                .map_or(source.len(), |offset| edit.range.end + offset);
            if let Some(group) = groups.last_mut().filter(|group| group.1 >= start) {
                group.1 = group.1.max(end);
                group.2.push(edit);
            } else {
                groups.push((start, end, vec![edit]));
            }
        }
        for (start, end, edits) in groups {
            let original = source[start..end].to_string();
            let mut replacement = original.clone();
            for edit in edits.iter().rev() {
                replacement.replace_range(
                    edit.range.start - start..edit.range.end - start,
                    &edit.replacement,
                );
            }
            suggestions.push(FixSuggestion {
                rule: edits[0].rule.clone(),
                file: file.clone(),
                line: source[..start]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1,
                original,
                replacement,
                description: "Apply syntax-bounded fixes.".into(),
                auto_fixable: true,
            });
        }
    }
    suggestions
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
    let mut by_file: HashMap<&PathBuf, Vec<&FixSuggestion>> = HashMap::new();
    for fix in fixes.iter().filter(|fix| fix.auto_fixable) {
        by_file.entry(&fix.file).or_default().push(fix);
    }
    let mut applied = 0;
    for (file, mut fixes) in by_file {
        fixes.sort_by_key(|fix| std::cmp::Reverse(fix.line));
        let Ok(mut source) = std::fs::read_to_string(file) else {
            continue;
        };
        let mut changed = 0;
        for fix in fixes {
            let offset = if fix.line == 1 {
                Some(0)
            } else {
                source
                    .match_indices('\n')
                    .nth(fix.line.saturating_sub(2))
                    .map(|(offset, _)| offset + 1)
            };
            let Some(offset) = offset else {
                continue;
            };
            if !source[offset..].starts_with(&fix.original) {
                continue;
            }
            source.replace_range(offset..offset + fix.original.len(), &fix.replacement);
            changed += 1;
        }
        if changed == 0 {
            continue;
        }
        match std::fs::write(file, source) {
            Ok(()) => applied += changed,
            Err(error) => log::error!("Could not apply fixes to {}: {}", file.display(), error),
        }
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

    // Exercise the shared syntax implementation rather than the removed text rewriter.
    fn fix_double_literal(source: &str) -> String {
        let mut result = source.to_string();
        let fixes = crate::ai::syntax_fixes::fixes_for_issue(source, "double-literal-format", 1, 1);
        for fix in fixes.iter().rev() {
            result.replace_range(fix.range.clone(), &fix.replacement);
        }
        result
    }

    // ── fix_double_literal (pure) ──────────────────────────────────────────

    #[test]
    fn fix_double_literal_space_zero_dot() {
        assert_eq!(fix_double_literal("double x = .5;"), "double x = 0.5;");
    }

    #[test]
    fn fix_double_literal_paren_zero_dot() {
        assert_eq!(
            fix_double_literal("void main() { foo(.5); }"),
            "void main() { foo(0.5); }"
        );
    }

    #[test]
    fn fix_double_literal_equals_zero_dot() {
        assert_eq!(fix_double_literal("x=.5"), "x=0.5");
    }

    #[test]
    fn fix_double_literal_preserves_valid_precision() {
        // Valid decimal precision is not a violation of the configured rule.
        assert_eq!(fix_double_literal("1.500"), "1.500");
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
        assert!(
            fix[0].replacement.ends_with(",)"),
            "got: {}",
            fix[0].replacement
        );
        assert!(fix[0].auto_fixable);
    }

    #[test]
    fn suggest_fix_trailing_comma_bracket() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "a.dart", "void main() { final x = [1, 2]; }\n");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(
            fix[0].replacement.contains("[1, 2,]"),
            "got: {}",
            fix[0].replacement
        );
    }

    #[test]
    fn suggest_fix_trailing_comma_brace() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "a.dart", "void main() { final x = {a: 1}; }\n");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(
            fix[0].replacement.contains("{a: 1,}"),
            "got: {}",
            fix[0].replacement
        );
    }

    #[test]
    fn suggest_fix_trailing_comma_already_has_comma() {
        // Already-correct syntax must not produce an edit.
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "a.dart", "  foo(bar,)\n");
        let issue = make_issue("prefer-trailing-comma", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert!(fix.is_empty());
    }

    // ── suggest_fix – double-literal-format ───────────────────────────────

    #[test]
    fn suggest_fix_double_literal_changes_line() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "b.dart", "double x = .5;\n");
        let issue = make_issue("double-literal-format", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert_eq!(fix[0].replacement, "double x = 0.5;");
    }

    #[test]
    fn suggest_fix_double_literal_no_change_returns_none() {
        let tmp = TempDir::new().unwrap();
        // A correctly formatted literal requires no fix.
        let path = write_dart(&tmp, "b.dart", "double x = 1.5;\n");
        let issue = make_issue("double-literal-format", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 0);
    }

    // ── suggest_fix – avoid-double-negation ───────────────────────────────

    #[test]
    fn suggest_fix_double_negation_removed() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(&tmp, "c.dart", "void main() { if (!!flag) {} }\n");
        let issue = make_issue("avoid-double-negation", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert_eq!(fix[0].replacement, "void main() { if (flag) {} }");
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
        let path = write_dart(
            &tmp,
            "d.dart",
            "void main() { Expanded(child: SizedBox()); }\n",
        );
        let issue = make_issue("avoid-expanded-as-spacer", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains("const Spacer()"));
    }

    #[test]
    fn suggest_fix_expanded_sizebox_shrink() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(
            &tmp,
            "d.dart",
            "void main() { Expanded(child: SizedBox.shrink()); }\n",
        );
        let issue = make_issue("avoid-expanded-as-spacer", path, 1);
        let fix = generate_fixes(&[issue], tmp.path());
        assert_eq!(fix.len(), 1);
        assert!(fix[0].replacement.contains("const Spacer()"));
    }

    #[test]
    fn suggest_fix_expanded_container() {
        let tmp = TempDir::new().unwrap();
        let path = write_dart(
            &tmp,
            "d.dart",
            "void main() { Expanded(child: Container()); }\n",
        );
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
        let path = write_dart(&tmp, "f.dart", "void main() {\n  return value;\n}\n");
        let issue = make_issue("newline-before-return", path, 2);
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
        let path2 = write_dart(&tmp, "i2.dart", "void main() { if (!!flag) {} }\n");
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
