//! Performance profiling integration with Flutter DevTools.
//! Analyzes code for performance anti-patterns that would show up in DevTools.

use crate::config::Severity;
use crate::reporters::Issue;
use colored::Colorize;
use std::path::Path;

/// Static performance analysis results mapped to DevTools categories.
#[derive(Debug, Default)]
pub struct PerfReport {
    pub rebuild_issues: Vec<Issue>,
    pub memory_issues: Vec<Issue>,
    pub render_issues: Vec<Issue>,
    pub network_issues: Vec<Issue>,
    pub total_files: usize,
}

impl PerfReport {
    pub fn all_issues(&self) -> Vec<&Issue> {
        self.rebuild_issues
            .iter()
            .chain(self.memory_issues.iter())
            .chain(self.render_issues.iter())
            .chain(self.network_issues.iter())
            .collect()
    }
}

/// Run static performance analysis matching DevTools profiling categories.
pub fn analyze_performance(root: &Path) -> PerfReport {
    let mut report = PerfReport::default();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| {
            let p = e.path().to_string_lossy();
            !p.contains(".g.dart") && !p.contains(".freezed.dart") && !p.contains("/test/")
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        report.total_files += 1;
        check_rebuild_issues(entry.path(), &source, &mut report.rebuild_issues);
        check_memory_issues(entry.path(), &source, &mut report.memory_issues);
        check_render_issues(entry.path(), &source, &mut report.render_issues);
        check_network_issues(entry.path(), &source, &mut report.network_issues);
    }

    report
}

fn check_rebuild_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("setState(") {
            let block: String = source.lines().skip(i).take(5).collect::<Vec<_>>().join(" ");
            if block.contains("Future") || block.contains("Timer") || block.contains("http.") {
                issues.push(Issue {
                    rule: "perf-async-set-state".to_string(),
                    message: "setState called in async context — may cause unnecessary rebuilds. Consider using a state management solution.".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }

        if trimmed.contains("Widget build(") {
            let build_block: String = source
                .lines()
                .skip(i)
                .take(30)
                .collect::<Vec<_>>()
                .join("\n");
            if build_block.matches("MediaQuery.of(").count() > 2 {
                issues.push(Issue {
                    rule: "perf-repeated-media-query".to_string(),
                    message: "Multiple MediaQuery.of() calls in build — cache the result in a local variable".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }

            if build_block.matches("Theme.of(").count() > 3 {
                issues.push(Issue {
                    rule: "perf-repeated-theme-of".to_string(),
                    message: "Multiple Theme.of() calls in build — cache in a local variable"
                        .to_string(),
                    severity: Severity::Info,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }
    }
}

fn check_memory_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("ImageProvider") || trimmed.contains("Image.network(") {
            let block: String = source.lines().skip(i).take(5).collect::<Vec<_>>().join(" ");
            if !block.contains("cacheWidth")
                && !block.contains("cacheHeight")
                && !block.contains("ResizeImage")
            {
                issues.push(Issue {
                    rule: "perf-uncached-image-size".to_string(),
                    message: "Image loaded without cacheWidth/cacheHeight — may consume excessive memory on high-DPI devices".to_string(),
                    severity: Severity::Info,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }
    }
}

fn check_render_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("ListView(") || trimmed.contains("GridView(") {
            let block: String = source.lines().skip(i).take(8).collect::<Vec<_>>().join(" ");
            if !block.contains(".builder")
                && !block.contains(".separated")
                && !block.contains(".custom")
            {
                if block.contains("children:") {
                    issues.push(Issue {
                        rule: "perf-unbounded-list".to_string(),
                        message: "ListView/GridView with children: builds all items eagerly. Use .builder() for large/dynamic lists.".to_string(),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    });
                }
            }
        }

        if trimmed.contains("Opacity(") {
            let block: String = source.lines().skip(i).take(3).collect::<Vec<_>>().join(" ");
            if block.contains("opacity: 0") {
                issues.push(Issue {
                    rule: "perf-opacity-zero".to_string(),
                    message: "Opacity(opacity: 0) still paints the child. Use Visibility or if-else to skip painting entirely.".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }
    }
}

fn check_network_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("http.get(")
            || trimmed.contains("http.post(")
            || trimmed.contains("Dio()")
        {
            let block: String = source
                .lines()
                .skip(i.saturating_sub(5))
                .take(10)
                .collect::<Vec<_>>()
                .join(" ");
            if block.contains("Widget build(") || block.contains("initState(") {
                issues.push(Issue {
                    rule: "perf-network-in-build".to_string(),
                    message: "Network call in build/initState — use a repository/service layer to avoid re-fetching on every rebuild".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }
    }
}

/// Print the performance analysis report.
pub fn print_perf_report(report: &PerfReport) {
    println!();
    println!(
        "  {} DevTools Performance Analysis",
        "falcon".bright_cyan().bold()
    );
    println!();
    println!("  Files analyzed:    {}", report.total_files);
    println!();

    print_category("Widget Rebuilds", &report.rebuild_issues);
    print_category("Memory", &report.memory_issues);
    print_category("Rendering", &report.render_issues);
    print_category("Network", &report.network_issues);

    let total = report.all_issues().len();
    if total == 0 {
        println!("  {} No performance issues detected.", "✓".green().bold());
    } else {
        println!("  {} total performance issue(s)", total);
    }
    println!();
}

fn print_category(name: &str, issues: &[Issue]) {
    if issues.is_empty() {
        println!("  {} {} — {}", "✓".green(), name, "clean".dimmed());
    } else {
        println!(
            "  {} {} — {} issue(s)",
            "⚠".yellow(),
            name.bright_white(),
            issues.len()
        );
        for issue in issues.iter().take(3) {
            let rel = issue
                .file
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("?");
            println!(
                "    {} {}:{} {}",
                "·".dimmed(),
                rel,
                issue.line,
                issue.message.dimmed()
            );
        }
        if issues.len() > 3 {
            println!("    {} ... and {} more", "·".dimmed(), issues.len() - 3);
        }
    }
}
