use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimSeverity {
    Error,
    Warning,
    Info,
}

impl AnimSeverity {
    fn score_penalty(&self) -> i32 {
        match self {
            AnimSeverity::Error => -15,
            AnimSeverity::Warning => -8,
            AnimSeverity::Info => -3,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            AnimSeverity::Error => "ERROR",
            AnimSeverity::Warning => "WARNING",
            AnimSeverity::Info => "INFO",
        }
    }

    fn colored_str(&self) -> String {
        match self {
            AnimSeverity::Error => "ERROR".red().bold().to_string(),
            AnimSeverity::Warning => "WARNING".yellow().to_string(),
            AnimSeverity::Info => "INFO".cyan().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnimationIssue {
    pub severity: AnimSeverity,
    pub category: &'static str,
    pub file: PathBuf,
    pub line: usize,
    pub snippet: String,
    pub detail: String,
    pub suggestion: String,
}

#[derive(Debug)]
pub struct AnimationAuditReport {
    pub files_scanned: usize,
    pub total_animation_controllers: usize,
    pub issues: Vec<AnimationIssue>,
    pub score: u32,
}

impl AnimationAuditReport {
    fn calculate_score(issues: &[AnimationIssue]) -> u32 {
        let total_penalty: i32 = issues.iter().map(|i| i.severity.score_penalty()).sum();
        let score = 100 + total_penalty;
        (score.max(0)) as u32
    }
}

/// Audits animation code in Dart files for performance anti-patterns
pub fn audit_animations(path: &Path) -> Result<AnimationAuditReport> {
    let mut files_scanned = 0;
    let mut total_animation_controllers = 0;
    let mut all_issues = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "dart") {
            continue;
        }

        // Skip generated and test files for some checks
        let path_str = path.to_string_lossy();
        if path_str.contains(".g.dart")
            || path_str.contains(".freezed.dart")
            || path_str.contains("/build/")
        {
            continue;
        }

        match std::fs::read_to_string(path) {
            Ok(content) => {
                files_scanned += 1;
                let file_issues = analyze_file(path, &content);
                let controller_count = count_animation_controllers(&content);
                total_animation_controllers += controller_count;
                all_issues.extend(file_issues);
            }
            Err(e) => {
                log::warn!("Failed to read {}: {}", path.display(), e);
            }
        }
    }

    let score = AnimationAuditReport::calculate_score(&all_issues);

    Ok(AnimationAuditReport {
        files_scanned,
        total_animation_controllers,
        issues: all_issues,
        score,
    })
}

fn count_animation_controllers(content: &str) -> usize {
    content.matches("AnimationController(").count()
}

fn analyze_file(path: &Path, content: &str) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    // Check 1: Missing AnimationController disposal
    issues.extend(check_missing_disposal(path, &lines));

    // Check 2: setState inside animation listener
    issues.extend(check_setstate_in_listener(path, &lines));

    // Check 3: Heavy computation in animation callbacks
    issues.extend(check_heavy_computation_in_callbacks(path, &lines));

    // Check 4: Implicit animation overuse (duration issues)
    issues.extend(check_implicit_animation_duration(path, &lines));

    // Check 5: Chained .then() on animations
    issues.extend(check_chained_animation_then(path, &lines));

    // Check 6: Missing vsync
    issues.extend(check_missing_vsync(path, &lines));

    // Check 7: Tween used without animation
    issues.extend(check_tween_without_animate(path, &lines));

    // Check 8: Large widget trees in AnimatedBuilder
    issues.extend(check_large_animated_builder(path, &lines));

    issues
}

fn check_missing_disposal(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("AnimationController(") {
            // Check if we're inside a class definition
            let class_start = find_class_start(lines, idx);
            if let Some(class_end) = find_class_end(lines, class_start) {
                // Exclude comment lines so that comments mentioning dispose() don't suppress the issue
                let class_code: String = lines[class_start..=class_end]
                    .iter()
                    .filter(|l| !l.trim().starts_with("//"))
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n");
                // Flag only when there is NO dispose() call at all in the class
                if !class_code.contains("dispose()") && !class_code.contains(".dispose()") {
                    issues.push(AnimationIssue {
                        severity: AnimSeverity::Error,
                        category: "Missing AnimationController disposal",
                        file: path.to_path_buf(),
                        line: idx + 1,
                        snippet: line.trim().to_string(),
                        detail: "AnimationController created but no dispose() found in class"
                            .to_string(),
                        suggestion: "Add a dispose() method that calls controller.dispose() and super.dispose()"
                            .to_string(),
                    });
                }
            }
        }
    }

    issues
}

fn check_setstate_in_listener(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("addListener(") {
            // Look within next 5 lines for setState
            let search_end = (idx + 5).min(lines.len());
            let has_setstate = lines[idx..search_end]
                .iter()
                .any(|l| l.contains("setState("));

            if has_setstate {
                issues.push(AnimationIssue {
                    severity: AnimSeverity::Error,
                    category: "setState inside animation listener",
                    file: path.to_path_buf(),
                    line: idx + 1,
                    snippet: line.trim().to_string(),
                    detail: "setState called inside addListener causes full widget rebuild on every frame"
                        .to_string(),
                    suggestion: "Use AnimatedBuilder or AnimationBuilder instead of setState in listeners"
                        .to_string(),
                });
            }
        }
    }

    issues
}

fn check_heavy_computation_in_callbacks(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("addListener(") {
            let search_end = (idx + 5).min(lines.len());
            let callback_section = lines[idx..search_end].join("\n");

            if callback_section.contains("http.") || callback_section.contains("http.get") {
                issues.push(AnimationIssue {
                    severity: AnimSeverity::Error,
                    category: "Heavy computation in animation callbacks",
                    file: path.to_path_buf(),
                    line: idx + 1,
                    snippet: line.trim().to_string(),
                    detail: "Network call detected inside animation listener callback".to_string(),
                    suggestion:
                        "Move network calls outside animation listeners; fetch data before animation"
                            .to_string(),
                });
            }

            if callback_section.contains("Future.delayed") {
                issues.push(AnimationIssue {
                    severity: AnimSeverity::Warning,
                    category: "Heavy computation in animation callbacks",
                    file: path.to_path_buf(),
                    line: idx + 1,
                    snippet: line.trim().to_string(),
                    detail: "Future.delayed found in animation callback".to_string(),
                    suggestion: "Avoid async delays inside animation listeners for smooth 60fps"
                        .to_string(),
                });
            }
        }
    }

    issues
}

fn check_implicit_animation_duration(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("AnimatedContainer(") || line.contains("AnimatedOpacity(") {
            // Extract duration from this line or next few lines
            let search_end = (idx + 10).min(lines.len());
            let animation_block = lines[idx..search_end].join("\n");

            if let Some(duration_str) = extract_duration_ms(&animation_block) {
                if duration_str < 100 {
                    issues.push(AnimationIssue {
                        severity: AnimSeverity::Info,
                        category: "Implicit animation overuse",
                        file: path.to_path_buf(),
                        line: idx + 1,
                        snippet: line.trim().to_string(),
                        detail: format!(
                            "Animation duration is {}ms — too fast to see smoothly",
                            duration_str
                        ),
                        suggestion: "Increase duration to at least 100ms for perceptible animation"
                            .to_string(),
                    });
                } else if duration_str > 2000 {
                    issues.push(AnimationIssue {
                        severity: AnimSeverity::Warning,
                        category: "Implicit animation overuse",
                        file: path.to_path_buf(),
                        line: idx + 1,
                        snippet: line.trim().to_string(),
                        detail: format!(
                            "Animation duration is {}ms — too slow, poor UX",
                            duration_str
                        ),
                        suggestion: "Reduce duration to 200-2000ms range for better UX".to_string(),
                    });
                }
            }
        }
    }

    issues
}

fn check_chained_animation_then(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains(".forward().then(") {
            issues.push(AnimationIssue {
                severity: AnimSeverity::Warning,
                category: "Chained .then() on animations",
                file: path.to_path_buf(),
                line: idx + 1,
                snippet: line.trim().to_string(),
                detail: "Using .forward().then(...) instead of AnimationController.repeat()"
                    .to_string(),
                suggestion:
                    "Use controller.repeat(reverse: true) or Sequence for cleaner animation chains"
                        .to_string(),
            });
        }
    }

    issues
}

fn check_missing_vsync(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("AnimationController(") {
            let search_end = (idx + 5).min(lines.len());
            let controller_block = lines[idx..search_end].join("\n");

            if !controller_block.contains("vsync:") {
                issues.push(AnimationIssue {
                    severity: AnimSeverity::Error,
                    category: "Missing vsync",
                    file: path.to_path_buf(),
                    line: idx + 1,
                    snippet: line.trim().to_string(),
                    detail: "AnimationController without vsync leads to battery drain".to_string(),
                    suggestion: "Add 'vsync: this' to AnimationController constructor".to_string(),
                });
            }
        }
    }

    issues
}

fn check_tween_without_animate(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("Tween<") && line.contains("(begin:") {
            // Check if this Tween is followed by .animate() in next few lines
            let search_end = (idx + 3).min(lines.len());
            let tween_block = lines[idx..search_end].join(" ");

            if !tween_block.contains(".animate(") {
                issues.push(AnimationIssue {
                    severity: AnimSeverity::Warning,
                    category: "Tween used without animation",
                    file: path.to_path_buf(),
                    line: idx + 1,
                    snippet: line.trim().to_string(),
                    detail: "Tween created but .animate() not called on it".to_string(),
                    suggestion: "Chain .animate(controller) to the Tween".to_string(),
                });
            }
        }
    }

    issues
}

fn check_large_animated_builder(path: &Path, lines: &[&str]) -> Vec<AnimationIssue> {
    let mut issues = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.contains("AnimatedBuilder(") {
            // Find builder: parameter
            if let Some(builder_start) = find_builder_start(lines, idx) {
                if let Some(builder_end) = find_builder_end(lines, builder_start) {
                    let builder_size = builder_end - builder_start;
                    if builder_size > 20 {
                        issues.push(AnimationIssue {
                            severity: AnimSeverity::Warning,
                            category: "Large widget trees rebuilt by animation",
                            file: path.to_path_buf(),
                            line: idx + 1,
                            snippet: line.trim().to_string(),
                            detail: format!(
                                "AnimatedBuilder builder callback spans {} lines",
                                builder_size
                            ),
                            suggestion:
                                "Extract large builder callbacks into separate widget classes to optimize rebuilds"
                                    .to_string(),
                        });
                    }
                }
            }
        }
    }

    issues
}

fn extract_duration_ms(block: &str) -> Option<u32> {
    for line in block.lines() {
        if line.contains("duration:") {
            // Try to extract milliseconds value
            if let Some(start) = line.find("Duration(milliseconds:") {
                let rest = &line[start + 22..];
                if let Some(end) = rest.find(')') {
                    if let Ok(ms) = rest[..end].trim().parse::<u32>() {
                        return Some(ms);
                    }
                }
            } else if let Some(start) = line.find("milliseconds:") {
                let rest = &line[start + 13..];
                if let Some(end) = rest.find(|c: char| !c.is_numeric() && c != ')') {
                    if let Ok(ms) = rest[..end].trim().parse::<u32>() {
                        return Some(ms);
                    }
                }
            }
        }
    }
    None
}

fn find_class_start(lines: &[&str], from_idx: usize) -> usize {
    for i in (0..=from_idx).rev() {
        if lines[i].contains("class ") {
            return i;
        }
    }
    0
}

fn find_class_end(lines: &[&str], from_idx: usize) -> Option<usize> {
    let mut brace_count = 0;
    let mut found_opening = false;

    for (i, line) in lines.iter().enumerate().skip(from_idx) {
        for ch in line.chars() {
            if ch == '{' {
                found_opening = true;
                brace_count += 1;
            } else if ch == '}' {
                brace_count -= 1;
                if found_opening && brace_count == 0 {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn find_builder_start(lines: &[&str], from_idx: usize) -> Option<usize> {
    (from_idx..lines.len().min(from_idx + 20)).find(|&i| lines[i].contains("builder:"))
}

fn find_builder_end(lines: &[&str], from_idx: usize) -> Option<usize> {
    let mut paren_count = 0;
    let mut found_opening = false;

    for (i, line) in lines.iter().enumerate().skip(from_idx) {
        for ch in line.chars() {
            if ch == '(' {
                found_opening = true;
                paren_count += 1;
            } else if ch == ')' {
                paren_count -= 1;
                if found_opening && paren_count == 0 {
                    return Some(i);
                }
            }
        }
    }
    None
}

pub fn print_animation_report(report: &AnimationAuditReport) {
    println!("\n{}", "=== Animation Audit Report ===".bold().cyan());
    println!("Files scanned: {}", report.files_scanned.to_string().bold());
    println!(
        "Animation Controllers found: {}",
        report.total_animation_controllers.to_string().bold()
    );
    println!(
        "Issues detected: {}",
        report.issues.len().to_string().bold()
    );
    println!("Score: {} / 100", report.score.to_string().bold());

    if !report.issues.is_empty() {
        println!("\n{}", "Issues:".bold().underline());
        for issue in &report.issues {
            println!(
                "\n[{}] {} (line {})",
                issue.severity.colored_str(),
                issue.category.bold(),
                issue.line
            );
            println!("  File: {}", issue.file.display());
            println!("  Code: {}", issue.snippet.italic());
            println!("  Detail: {}", issue.detail);
            println!("  Suggestion: {}", issue.suggestion.green());
        }
    } else {
        println!(
            "\n{}",
            "No animation issues found! Great job.".green().bold()
        );
    }
}

pub fn write_animation_html_report(report: &AnimationAuditReport, path: &Path) -> Result<()> {
    let html = generate_html_report(report);
    std::fs::write(path, html)?;
    Ok(())
}

fn generate_html_report(report: &AnimationAuditReport) -> String {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Animation Audit Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }
        .header { background: #2c3e50; color: white; padding: 20px; border-radius: 8px; }
        .stats { display: flex; gap: 20px; margin: 20px 0; }
        .stat-box { background: white; padding: 15px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .stat-label { color: #7f8c8d; font-size: 12px; text-transform: uppercase; }
        .stat-value { font-size: 24px; font-weight: bold; color: #2c3e50; }
        .score { color: #27ae60; }
        .score.low { color: #e74c3c; }
        .issues { margin-top: 30px; }
        .issue { background: white; padding: 15px; margin: 10px 0; border-left: 4px solid #e74c3c; border-radius: 4px; }
        .issue.warning { border-left-color: #f39c12; }
        .issue.info { border-left-color: #3498db; }
        .severity { display: inline-block; padding: 4px 8px; border-radius: 4px; font-size: 12px; font-weight: bold; }
        .severity.error { background: #e74c3c; color: white; }
        .severity.warning { background: #f39c12; color: white; }
        .severity.info { background: #3498db; color: white; }
        .category { font-weight: bold; margin: 10px 0 5px 0; }
        .code { background: #ecf0f1; padding: 8px; border-radius: 4px; font-family: monospace; font-size: 12px; margin: 5px 0; }
        .suggestion { color: #27ae60; margin-top: 5px; }
        .no-issues { background: white; padding: 20px; border-radius: 8px; color: #27ae60; text-align: center; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Animation Audit Report</h1>
        <p>Static analysis of Dart animation code for performance anti-patterns</p>
    </div>
"#,
    );

    let score_class = if report.score >= 80 {
        "score"
    } else {
        "score low"
    };

    html.push_str(&format!(
        r#"    <div class="stats">
        <div class="stat-box">
            <div class="stat-label">Files Scanned</div>
            <div class="stat-value">{}</div>
        </div>
        <div class="stat-box">
            <div class="stat-label">Animation Controllers</div>
            <div class="stat-value">{}</div>
        </div>
        <div class="stat-box">
            <div class="stat-label">Issues Found</div>
            <div class="stat-value">{}</div>
        </div>
        <div class="stat-box">
            <div class="stat-label">Overall Score</div>
            <div class="stat-value {}">{}/100</div>
        </div>
    </div>
"#,
        report.files_scanned,
        report.total_animation_controllers,
        report.issues.len(),
        score_class,
        report.score
    ));

    if report.issues.is_empty() {
        html.push_str(
            r#"    <div class="no-issues">
        <h2>✓ No animation issues found! Great job.</h2>
    </div>
"#,
        );
    } else {
        html.push_str("    <div class=\"issues\">\n        <h2>Issues Detected</h2>\n");

        for issue in &report.issues {
            let issue_class = match issue.severity {
                AnimSeverity::Error => "error",
                AnimSeverity::Warning => "warning",
                AnimSeverity::Info => "info",
            };

            html.push_str(&format!(
                r#"        <div class="issue {}">
            <div style="display: flex; justify-content: space-between; align-items: center;">
                <span class="severity {}">{}</span>
                <span style="color: #7f8c8d; font-size: 12px;">{} (line {})</span>
            </div>
            <div class="category">{}</div>
            <div style="color: #555; font-size: 14px; margin: 5px 0;">{}</div>
            <div class="code">{}</div>
            <div style="color: #555; margin: 5px 0;"><strong>Issue:</strong> {}</div>
            <div class="suggestion"><strong>Suggestion:</strong> {}</div>
        </div>
"#,
                issue_class,
                issue_class,
                issue.severity.as_str(),
                issue.file.display(),
                issue.line,
                issue.category,
                issue.detail,
                html_escape(&issue.snippet),
                issue.detail,
                issue.suggestion
            ));
        }

        html.push_str("    </div>\n");
    }

    html.push_str("</body>\n</html>\n");
    html
}

fn html_escape(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // -------------------------------------------------------------------------
    // AnimSeverity methods
    // -------------------------------------------------------------------------

    #[test]
    fn test_anim_severity_score_penalty() {
        assert_eq!(AnimSeverity::Error.score_penalty(), -15);
        assert_eq!(AnimSeverity::Warning.score_penalty(), -8);
        assert_eq!(AnimSeverity::Info.score_penalty(), -3);
    }

    #[test]
    fn test_anim_severity_as_str() {
        assert_eq!(AnimSeverity::Error.as_str(), "ERROR");
        assert_eq!(AnimSeverity::Warning.as_str(), "WARNING");
        assert_eq!(AnimSeverity::Info.as_str(), "INFO");
    }

    #[test]
    fn test_anim_severity_colored_str_contains_label() {
        // colored_str may contain ANSI escape codes; just check label text is present
        assert!(AnimSeverity::Error.colored_str().contains("ERROR"));
        assert!(AnimSeverity::Warning.colored_str().contains("WARNING"));
        assert!(AnimSeverity::Info.colored_str().contains("INFO"));
    }

    // -------------------------------------------------------------------------
    // count_animation_controllers
    // -------------------------------------------------------------------------

    #[test]
    fn test_count_animation_controllers_zero() {
        let content = "Widget build(BuildContext ctx) { return Container(); }";
        assert_eq!(count_animation_controllers(content), 0);
    }

    #[test]
    fn test_count_animation_controllers_one() {
        let content = "final ctrl = AnimationController(vsync: this);";
        assert_eq!(count_animation_controllers(content), 1);
    }

    #[test]
    fn test_count_animation_controllers_multiple() {
        let content = "AnimationController(\n) AnimationController( AnimationController(";
        assert_eq!(count_animation_controllers(content), 3);
    }

    // -------------------------------------------------------------------------
    // extract_duration_ms
    // -------------------------------------------------------------------------

    #[test]
    fn test_extract_duration_ms_basic() {
        let block = "duration: Duration(milliseconds: 300),";
        assert_eq!(extract_duration_ms(block), Some(300));
    }

    #[test]
    fn test_extract_duration_ms_no_duration_key() {
        let block = "curve: Curves.easeIn,";
        assert_eq!(extract_duration_ms(block), None);
    }

    #[test]
    fn test_extract_duration_ms_duration_key_no_milliseconds() {
        let block = "duration: Duration(seconds: 2),";
        assert_eq!(extract_duration_ms(block), None);
    }

    #[test]
    fn test_extract_duration_ms_short_milliseconds_key() {
        // Matches the `milliseconds:` branch (without leading `Duration(`)
        let block = "  duration: const Duration(milliseconds: 150)";
        assert_eq!(extract_duration_ms(block), Some(150));
    }

    #[test]
    fn test_extract_duration_ms_zero() {
        let block = "duration: Duration(milliseconds: 0),";
        assert_eq!(extract_duration_ms(block), Some(0));
    }

    #[test]
    fn test_extract_duration_ms_large_value() {
        let block = "duration: Duration(milliseconds: 5000),";
        assert_eq!(extract_duration_ms(block), Some(5000));
    }

    // -------------------------------------------------------------------------
    // find_class_start / find_class_end
    // -------------------------------------------------------------------------

    #[test]
    fn test_find_class_start_found() {
        let lines = vec!["class Foo {", "  void bar() {}", "}"];
        assert_eq!(find_class_start(&lines, 2), 0);
    }

    #[test]
    fn test_find_class_start_no_class_returns_zero() {
        let lines = vec!["void foo() {}", "  int x = 1;", "}"];
        assert_eq!(find_class_start(&lines, 2), 0);
    }

    #[test]
    fn test_find_class_start_nested() {
        let lines = vec!["class Outer {", "  class Inner {", "    int x;", "  }", "}"];
        // From line 3 ("}"), searching back should find "class Inner" at 1
        assert_eq!(find_class_start(&lines, 3), 1);
    }

    #[test]
    fn test_find_class_end_simple() {
        let lines = vec!["class Foo {", "  int x;", "}"];
        assert_eq!(find_class_end(&lines, 0), Some(2));
    }

    #[test]
    fn test_find_class_end_no_closing_brace() {
        let lines = vec!["class Foo {", "  int x;"];
        assert_eq!(find_class_end(&lines, 0), None);
    }

    #[test]
    fn test_find_class_end_nested_braces() {
        let lines = vec![
            "class Foo {",
            "  void bar() {",
            "    if (x) {",
            "    }",
            "  }",
            "}",
        ];
        assert_eq!(find_class_end(&lines, 0), Some(5));
    }

    // -------------------------------------------------------------------------
    // find_builder_start / find_builder_end
    // -------------------------------------------------------------------------

    #[test]
    fn test_find_builder_start_found() {
        let lines = vec!["AnimatedBuilder(", "  animation: ctrl,", "  builder: (ctx, child) {", "  },", ")"];
        assert_eq!(find_builder_start(&lines, 0), Some(2));
    }

    #[test]
    fn test_find_builder_start_not_found() {
        let lines = vec!["AnimatedBuilder(", "  animation: ctrl,", ")"];
        assert_eq!(find_builder_start(&lines, 0), None);
    }

    #[test]
    fn test_find_builder_end_simple() {
        let lines = vec!["  builder: (ctx, child) {", "    return Container();", "  },", ")"];
        assert_eq!(find_builder_end(&lines, 0), Some(0));
    }

    #[test]
    fn test_find_builder_end_multiline() {
        // find_builder_end counts ( and ) chars per line.
        // Line 0 "  builder: (ctx, child) =>" has one ( and one ) → balanced at line 0.
        let lines = vec![
            "  builder: (ctx, child) =>",
            "    Transform.rotate(",
            "      angle: ctrl.value,",
            "      child: child,",
            "    ),",
        ];
        assert_eq!(find_builder_end(&lines, 0), Some(0));
    }

    #[test]
    fn test_find_builder_end_unmatched_multiline() {
        // builder: ( spans multiple lines before closing )
        let lines = vec![
            "  builder: (ctx,",
            "    child) =>",
            "    Container(),",
        ];
        // Line 0: one ( → count=1; line 1: one ) → count=0, found → Some(1)
        assert_eq!(find_builder_end(&lines, 0), Some(1));
    }

    #[test]
    fn test_find_builder_end_no_parens() {
        let lines = vec!["  builder: no_parens_here"];
        assert_eq!(find_builder_end(&lines, 0), None);
    }

    // -------------------------------------------------------------------------
    // check_missing_disposal
    // -------------------------------------------------------------------------

    #[test]
    fn test_missing_disposal_detection() {
        let content = r#"
class MyAnimation extends State {
  late AnimationController controller;

  @override
  void initState() {
    controller = AnimationController(duration: Duration(seconds: 1));
    super.initState();
  }

  // No dispose method!
}
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_missing_disposal(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Error);
        assert!(issues[0]
            .category
            .contains("Missing AnimationController disposal"));
    }

    #[test]
    fn test_missing_disposal_clean_with_dispose() {
        let content = r#"
class MyAnimation extends State {
  late AnimationController controller;

  @override
  void initState() {
    controller = AnimationController(vsync: this, duration: Duration(seconds: 1));
  }

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }
}
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_missing_disposal(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "Should not flag when dispose() is present");
    }

    #[test]
    fn test_missing_disposal_comment_does_not_suppress() {
        // dispose() only appears in a comment — should still flag
        let content = r#"
class BadAnim extends State {
  void init() {
    controller = AnimationController(vsync: this);
    // remember to call dispose()
  }
}
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_missing_disposal(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty(), "Comment-only dispose reference should still flag");
    }

    // -------------------------------------------------------------------------
    // check_setstate_in_listener
    // -------------------------------------------------------------------------

    #[test]
    fn test_setstate_in_listener_detection() {
        let content = r#"
controller.addListener(() {
  setState(() {
    value = controller.value;
  });
});
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_setstate_in_listener(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Error);
        assert!(issues[0]
            .category
            .contains("setState inside animation listener"));
    }

    #[test]
    fn test_setstate_in_listener_clean() {
        let content = r#"
controller.addListener(() {
  // just read the value
  print(controller.value);
});
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_setstate_in_listener(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "Should not flag listener without setState");
    }

    #[test]
    fn test_setstate_in_listener_setState_far_away() {
        // setState appears more than 5 lines after addListener — should NOT flag
        let content = "controller.addListener(() {\n  doA();\n  doB();\n  doC();\n  doD();\n  doE();\n  setState(() {});\n});\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_setstate_in_listener(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "setState beyond 5-line window should not flag");
    }

    // -------------------------------------------------------------------------
    // check_heavy_computation_in_callbacks
    // -------------------------------------------------------------------------

    #[test]
    fn test_network_call_in_listener() {
        let content = r#"
controller.addListener(() {
  http.get(Uri.parse('https://example.com'));
});
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_heavy_computation_in_callbacks(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Error);
        assert!(issues[0]
            .category
            .contains("Heavy computation in animation callbacks"));
    }

    #[test]
    fn test_future_delayed_in_listener() {
        let content = "controller.addListener(() {\n  Future.delayed(Duration(ms: 100), doSomething);\n});\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_heavy_computation_in_callbacks(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Warning);
        assert!(issues[0].category.contains("Heavy computation in animation callbacks"));
    }

    #[test]
    fn test_heavy_computation_clean() {
        let content = "controller.addListener(() {\n  print(ctrl.value);\n});\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_heavy_computation_in_callbacks(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "Clean listener should not flag");
    }

    // -------------------------------------------------------------------------
    // check_implicit_animation_duration
    // -------------------------------------------------------------------------

    #[test]
    fn test_animation_duration_too_fast() {
        let content = r#"
AnimatedContainer(
  duration: Duration(milliseconds: 50),
  curve: Curves.easeIn,
)
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_implicit_animation_duration(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Info);
    }

    #[test]
    fn test_animation_duration_too_slow() {
        let content = r#"
AnimatedOpacity(
  duration: Duration(milliseconds: 3000),
  opacity: 1.0,
)
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_implicit_animation_duration(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Warning);
    }

    #[test]
    fn test_animation_duration_in_range_no_issue() {
        let content = "AnimatedContainer(\n  duration: Duration(milliseconds: 300),\n)\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_implicit_animation_duration(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "Duration 300ms is in acceptable range");
    }

    #[test]
    fn test_animation_duration_no_animated_widget() {
        let content = "Container(\n  duration: Duration(milliseconds: 50),\n)\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_implicit_animation_duration(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "Non-animated widget should not flag");
    }

    #[test]
    fn test_animation_duration_no_duration_present() {
        // AnimatedContainer but no duration: line — no issue
        let content = "AnimatedContainer(\n  color: Colors.red,\n)\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_implicit_animation_duration(Path::new("test.dart"), &lines);
        assert!(issues.is_empty());
    }

    // -------------------------------------------------------------------------
    // check_chained_animation_then
    // -------------------------------------------------------------------------

    #[test]
    fn test_chained_animation_then() {
        let content = r#"
controller.forward().then((_) {
  controller.reverse();
});
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_chained_animation_then(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Warning);
    }

    #[test]
    fn test_chained_animation_then_clean() {
        let content = "controller.forward();\ncontroller.repeat(reverse: true);\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_chained_animation_then(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "No .forward().then( means no issue");
    }

    // -------------------------------------------------------------------------
    // check_missing_vsync
    // -------------------------------------------------------------------------

    #[test]
    fn test_missing_vsync_detection() {
        let content = r#"
controller = AnimationController(
  duration: Duration(seconds: 1),
);
"#;
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_missing_vsync(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Error);
        assert!(issues[0].category.contains("Missing vsync"));
    }

    #[test]
    fn test_missing_vsync_clean() {
        let content = "controller = AnimationController(\n  vsync: this,\n  duration: Duration(seconds: 1),\n);\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_missing_vsync(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "vsync present should not flag");
    }

    // -------------------------------------------------------------------------
    // check_tween_without_animate
    // -------------------------------------------------------------------------

    #[test]
    fn test_tween_without_animate_flags() {
        let content = "final tween = Tween<double>(begin: 0.0, end: 1.0);\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_tween_without_animate(Path::new("test.dart"), &lines);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, AnimSeverity::Warning);
        assert!(issues[0].category.contains("Tween used without animation"));
    }

    #[test]
    fn test_tween_with_animate_clean() {
        let content = "final anim = Tween<double>(begin: 0.0, end: 1.0).animate(controller);\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_tween_without_animate(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), ".animate() present should not flag");
    }

    #[test]
    fn test_tween_without_begin_not_flagged() {
        // Tween without `(begin:` should not match the heuristic
        let content = "final tween = Tween<double>(end: 1.0);\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_tween_without_animate(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "Tween without (begin: should not flag");
    }

    // -------------------------------------------------------------------------
    // check_large_animated_builder
    // -------------------------------------------------------------------------

    #[test]
    fn test_large_animated_builder_flags() {
        // find_builder_end tracks '(' and ')'. To make builder_size > 20, we need
        // the opening '(' at builder_start to not close for > 20 lines.
        // Use "builder: (ctx," on the first line (1 open, 0 close → net +1),
        // then 22 lines of "    int x = 0; // (no parens)",
        // then "  child)" to close the paren.
        let mut lines_vec: Vec<String> = vec![
            "AnimatedBuilder(".to_string(),
            "  animation: ctrl,".to_string(),
            "  builder: (ctx,".to_string(), // builder_start = 2; net parens = +1
        ];
        for i in 0..22usize {
            lines_vec.push(format!("    int x{} = {};", i, i)); // no parens
        }
        lines_vec.push("  child)".to_string()); // closes the builder paren → builder_end = 25
        lines_vec.push(");".to_string());

        let lines_str: Vec<&str> = lines_vec.iter().map(|s| s.as_str()).collect();
        let issues = check_large_animated_builder(Path::new("test.dart"), &lines_str);
        assert!(!issues.is_empty(), "Large builder should flag (builder_size = {})", {
            // builder_start = 2, builder_end = 25 → size = 23 > 20
            23
        });
        assert_eq!(issues[0].severity, AnimSeverity::Warning);
    }

    #[test]
    fn test_small_animated_builder_clean() {
        let content = "AnimatedBuilder(\n  animation: ctrl,\n  builder: (ctx, child) {\n    return Container();\n  },\n);\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_large_animated_builder(Path::new("test.dart"), &lines);
        assert!(issues.is_empty(), "Small builder should not flag");
    }

    #[test]
    fn test_animated_builder_no_builder_param() {
        // AnimatedBuilder present but no builder: within 20 lines — should not crash
        let content = "AnimatedBuilder(\n  animation: ctrl,\n);\n";
        let lines: Vec<&str> = content.lines().collect();
        let issues = check_large_animated_builder(Path::new("test.dart"), &lines);
        assert!(issues.is_empty());
    }

    // -------------------------------------------------------------------------
    // report score calculation
    // -------------------------------------------------------------------------

    #[test]
    fn test_report_score_calculation() {
        let issues = vec![
            AnimationIssue {
                severity: AnimSeverity::Error,
                category: "test",
                file: PathBuf::from("test.dart"),
                line: 1,
                snippet: "test".to_string(),
                detail: "test".to_string(),
                suggestion: "test".to_string(),
            },
            AnimationIssue {
                severity: AnimSeverity::Warning,
                category: "test",
                file: PathBuf::from("test.dart"),
                line: 2,
                snippet: "test".to_string(),
                detail: "test".to_string(),
                suggestion: "test".to_string(),
            },
        ];

        let score = AnimationAuditReport::calculate_score(&issues);
        // 100 + (-15) + (-8) = 77
        assert_eq!(score, 77);
    }

    #[test]
    fn test_report_score_no_issues() {
        let score = AnimationAuditReport::calculate_score(&[]);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_report_score_info_penalty() {
        let issues = vec![AnimationIssue {
            severity: AnimSeverity::Info,
            category: "test",
            file: PathBuf::from("test.dart"),
            line: 1,
            snippet: "".to_string(),
            detail: "".to_string(),
            suggestion: "".to_string(),
        }];
        let score = AnimationAuditReport::calculate_score(&issues);
        assert_eq!(score, 97); // 100 - 3
    }

    #[test]
    fn test_report_score_clamped_to_zero() {
        // Many errors should clamp score to 0
        let issues: Vec<AnimationIssue> = (0..10)
            .map(|i| AnimationIssue {
                severity: AnimSeverity::Error,
                category: "test",
                file: PathBuf::from("test.dart"),
                line: i,
                snippet: "".to_string(),
                detail: "".to_string(),
                suggestion: "".to_string(),
            })
            .collect();
        let score = AnimationAuditReport::calculate_score(&issues);
        assert_eq!(score, 0); // 100 - 150 clamped to 0
    }

    // -------------------------------------------------------------------------
    // analyze_file
    // -------------------------------------------------------------------------

    #[test]
    fn test_analyze_file_clean_dart_no_issues() {
        let content = "void main() { print('hello'); }\n";
        let issues = analyze_file(Path::new("lib/main.dart"), content);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_analyze_file_multiple_issues() {
        // File with chained animation + missing vsync — both should be reported
        let content = "controller = AnimationController(duration: Duration(seconds: 1));\ncontroller.forward().then((_) { controller.reverse(); });\n";
        let issues = analyze_file(Path::new("lib/anim.dart"), content);
        assert!(!issues.is_empty());
    }

    // -------------------------------------------------------------------------
    // html_escape
    // -------------------------------------------------------------------------

    #[test]
    fn test_html_escape_ampersand() {
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_html_escape_less_than() {
        assert_eq!(html_escape("a < b"), "a &lt; b");
    }

    #[test]
    fn test_html_escape_greater_than() {
        assert_eq!(html_escape("a > b"), "a &gt; b");
    }

    #[test]
    fn test_html_escape_double_quote() {
        assert_eq!(html_escape("say \"hi\""), "say &quot;hi&quot;");
    }

    #[test]
    fn test_html_escape_combined() {
        assert_eq!(html_escape("<a href=\"x\">foo & bar</a>"),
            "&lt;a href=&quot;x&quot;&gt;foo &amp; bar&lt;/a&gt;");
    }

    #[test]
    fn test_html_escape_no_special_chars() {
        assert_eq!(html_escape("hello world"), "hello world");
    }

    // -------------------------------------------------------------------------
    // generate_html_report (via write_animation_html_report)
    // -------------------------------------------------------------------------

    #[test]
    fn test_write_animation_html_report_no_issues() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("report.html");

        let report = AnimationAuditReport {
            files_scanned: 5,
            total_animation_controllers: 2,
            issues: vec![],
            score: 100,
        };

        write_animation_html_report(&report, &out).unwrap();
        let html = std::fs::read_to_string(&out).unwrap();
        assert!(html.contains("Animation Audit Report"));
        assert!(html.contains("No animation issues found"));
        assert!(html.contains("100"));
    }

    #[test]
    fn test_write_animation_html_report_with_issues() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("report.html");

        let report = AnimationAuditReport {
            files_scanned: 3,
            total_animation_controllers: 1,
            issues: vec![AnimationIssue {
                severity: AnimSeverity::Error,
                category: "Missing vsync",
                file: PathBuf::from("lib/anim.dart"),
                line: 10,
                snippet: "AnimationController()".to_string(),
                detail: "No vsync".to_string(),
                suggestion: "Add vsync: this".to_string(),
            }],
            score: 85,
        };

        write_animation_html_report(&report, &out).unwrap();
        let html = std::fs::read_to_string(&out).unwrap();
        assert!(html.contains("Missing vsync"));
        assert!(html.contains("ERROR"));
        assert!(html.contains("Issues Detected"));
        assert!(html.contains("85"));
    }

    #[test]
    fn test_html_report_low_score_class() {
        // score < 80 => "score low" class
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("report.html");
        let report = AnimationAuditReport {
            files_scanned: 1,
            total_animation_controllers: 0,
            issues: vec![],
            score: 50,
        };
        write_animation_html_report(&report, &out).unwrap();
        let html = std::fs::read_to_string(&out).unwrap();
        assert!(html.contains("score low"));
    }

    #[test]
    fn test_html_report_high_score_class() {
        // score >= 80 => "score" class (not "score low")
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("report.html");
        let report = AnimationAuditReport {
            files_scanned: 1,
            total_animation_controllers: 0,
            issues: vec![],
            score: 95,
        };
        write_animation_html_report(&report, &out).unwrap();
        let html = std::fs::read_to_string(&out).unwrap();
        // Should contain "score" but NOT "score low"
        assert!(!html.contains("score low"));
    }

    #[test]
    fn test_html_report_escapes_snippet() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("report.html");
        let report = AnimationAuditReport {
            files_scanned: 1,
            total_animation_controllers: 0,
            issues: vec![AnimationIssue {
                severity: AnimSeverity::Warning,
                category: "Test",
                file: PathBuf::from("lib/x.dart"),
                line: 1,
                snippet: "<script>alert(1)</script>".to_string(),
                detail: "test detail".to_string(),
                suggestion: "fix it".to_string(),
            }],
            score: 92,
        };
        write_animation_html_report(&report, &out).unwrap();
        let html = std::fs::read_to_string(&out).unwrap();
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>alert"));
    }

    // -------------------------------------------------------------------------
    // audit_animations (orchestrator, TempDir)
    // -------------------------------------------------------------------------

    #[test]
    fn test_audit_animations_empty_dir() {
        let dir = TempDir::new().unwrap();
        let result = audit_animations(dir.path()).unwrap();
        assert_eq!(result.files_scanned, 0);
        assert_eq!(result.total_animation_controllers, 0);
        assert!(result.issues.is_empty());
        assert_eq!(result.score, 100);
    }

    #[test]
    fn test_audit_animations_skips_generated_files() {
        let dir = TempDir::new().unwrap();
        // .g.dart file should be skipped
        let gen_file = dir.path().join("foo.g.dart");
        std::fs::write(&gen_file,
            "class Foo { AnimationController(\n  duration: Duration(seconds: 1),\n); }\n"
        ).unwrap();
        let result = audit_animations(dir.path()).unwrap();
        assert_eq!(result.files_scanned, 0);
    }

    #[test]
    fn test_audit_animations_skips_non_dart_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# title").unwrap();
        std::fs::write(dir.path().join("config.yaml"), "key: value").unwrap();
        let result = audit_animations(dir.path()).unwrap();
        assert_eq!(result.files_scanned, 0);
    }

    #[test]
    fn test_audit_animations_counts_controllers() {
        let dir = TempDir::new().unwrap();
        let content = r#"
class MyWidget extends StatefulWidget {
  AnimationController ctrl1;
  AnimationController ctrl2;
  void init() {
    ctrl1 = AnimationController(vsync: this, duration: Duration(seconds: 1));
    ctrl2 = AnimationController(vsync: this, duration: Duration(seconds: 2));
  }
  void dispose() { ctrl1.dispose(); ctrl2.dispose(); }
}
"#;
        std::fs::write(dir.path().join("widget.dart"), content).unwrap();
        let result = audit_animations(dir.path()).unwrap();
        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.total_animation_controllers, 2);
    }

    #[test]
    fn test_audit_animations_detects_issues() {
        let dir = TempDir::new().unwrap();
        // This Dart content triggers missing_vsync and chained_then
        let content = "controller = AnimationController(duration: Duration(seconds: 1));\ncontroller.forward().then((_) { controller.reverse(); });\n";
        std::fs::write(dir.path().join("bad.dart"), content).unwrap();
        let result = audit_animations(dir.path()).unwrap();
        assert_eq!(result.files_scanned, 1);
        assert!(!result.issues.is_empty());
        assert!(result.score < 100);
    }

    #[test]
    fn test_audit_animations_clean_file_no_issues() {
        let dir = TempDir::new().unwrap();
        let content = "void main() { runApp(const MyApp()); }\n";
        std::fs::write(dir.path().join("main.dart"), content).unwrap();
        let result = audit_animations(dir.path()).unwrap();
        assert_eq!(result.files_scanned, 1);
        assert!(result.issues.is_empty());
        assert_eq!(result.score, 100);
    }
}
