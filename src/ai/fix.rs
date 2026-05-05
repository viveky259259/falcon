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
