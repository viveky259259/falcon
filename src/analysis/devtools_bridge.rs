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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dummy_path() -> PathBuf {
        PathBuf::from("test_widget.dart")
    }

    // ── PerfReport::all_issues ──────────────────────────────────────────────

    #[test]
    fn perf_report_all_issues_empty() {
        let report = PerfReport::default();
        assert_eq!(report.all_issues().len(), 0);
    }

    #[test]
    fn perf_report_all_issues_aggregates_all_categories() {
        use crate::config::Severity;
        let make_issue = |rule: &str| crate::reporters::Issue {
            rule: rule.to_string(),
            message: "msg".to_string(),
            severity: Severity::Warning,
            file: dummy_path(),
            line: 1,
            column: 1,
        };
        let mut report = PerfReport::default();
        report.rebuild_issues.push(make_issue("r1"));
        report.memory_issues.push(make_issue("m1"));
        report.render_issues.push(make_issue("re1"));
        report.network_issues.push(make_issue("n1"));
        report.total_files = 4;

        let all = report.all_issues();
        assert_eq!(all.len(), 4);
        assert!(all.iter().any(|i| i.rule == "r1"));
        assert!(all.iter().any(|i| i.rule == "m1"));
        assert!(all.iter().any(|i| i.rule == "re1"));
        assert!(all.iter().any(|i| i.rule == "n1"));
    }

    // ── check_rebuild_issues ────────────────────────────────────────────────

    #[test]
    fn rebuild_no_issue_on_empty_source() {
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), "", &mut issues);
        assert!(issues.is_empty());
    }

    #[test]
    fn rebuild_detects_async_set_state_with_future() {
        let source = "setState(() {\n  Future.delayed(Duration.zero, () {});\n});\n";
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-async-set-state"),
            "expected perf-async-set-state, got {:?}", issues);
    }

    #[test]
    fn rebuild_detects_async_set_state_with_timer() {
        let source = "setState(() {\n  Timer(Duration.zero, () {});\n});\n";
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-async-set-state"));
    }

    #[test]
    fn rebuild_detects_async_set_state_with_http() {
        let source = "setState(() {\n  http.get(url);\n});\n";
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-async-set-state"));
    }

    #[test]
    fn rebuild_no_issue_for_sync_set_state() {
        let source = "setState(() {\n  _count++;\n});\n";
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-async-set-state"));
    }

    #[test]
    fn rebuild_detects_repeated_media_query() {
        // build() method with >2 MediaQuery.of() calls within 30 lines
        let lines: String = std::iter::once("Widget build(BuildContext context) {\n".to_string())
            .chain((0..3).map(|_| "  final w = MediaQuery.of(context).size.width;\n".to_string()))
            .collect();
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), &lines, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-repeated-media-query"),
            "expected perf-repeated-media-query, got {:?}", issues);
    }

    #[test]
    fn rebuild_no_issue_for_two_media_query_calls() {
        let source = "Widget build(BuildContext context) {\n  MediaQuery.of(context);\n  MediaQuery.of(context);\n}\n";
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-repeated-media-query"));
    }

    #[test]
    fn rebuild_detects_repeated_theme_of() {
        // build() method with >3 Theme.of() calls within 30 lines
        let lines: String = std::iter::once("Widget build(BuildContext context) {\n".to_string())
            .chain((0..4).map(|_| "  final c = Theme.of(context).primaryColor;\n".to_string()))
            .collect();
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), &lines, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-repeated-theme-of"),
            "expected perf-repeated-theme-of, got {:?}", issues);
    }

    #[test]
    fn rebuild_no_issue_for_three_theme_of_calls() {
        let source = "Widget build(BuildContext context) {\n  Theme.of(context);\n  Theme.of(context);\n  Theme.of(context);\n}\n";
        let mut issues = Vec::new();
        check_rebuild_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-repeated-theme-of"));
    }

    // ── check_memory_issues ─────────────────────────────────────────────────

    #[test]
    fn memory_no_issue_on_empty_source() {
        let mut issues = Vec::new();
        check_memory_issues(&dummy_path(), "", &mut issues);
        assert!(issues.is_empty());
    }

    #[test]
    fn memory_detects_image_network_without_cache() {
        let source = "Image.network('https://example.com/img.png'),\n";
        let mut issues = Vec::new();
        check_memory_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-uncached-image-size"),
            "expected perf-uncached-image-size, got {:?}", issues);
    }

    #[test]
    fn memory_detects_image_provider_without_cache() {
        let source = "ImageProvider provider = AssetImage('assets/logo.png');\n";
        let mut issues = Vec::new();
        check_memory_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-uncached-image-size"));
    }

    #[test]
    fn memory_no_issue_when_cache_width_present() {
        let source = "Image.network('url',\n  cacheWidth: 200,\n),\n";
        let mut issues = Vec::new();
        check_memory_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-uncached-image-size"));
    }

    #[test]
    fn memory_no_issue_when_resize_image_present() {
        let source = "ResizeImage(AssetImage('assets/a.png'), width: 100),\n";
        let mut issues = Vec::new();
        check_memory_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-uncached-image-size"));
    }

    // ── check_render_issues ─────────────────────────────────────────────────

    #[test]
    fn render_no_issue_on_empty_source() {
        let mut issues = Vec::new();
        check_render_issues(&dummy_path(), "", &mut issues);
        assert!(issues.is_empty());
    }

    #[test]
    fn render_detects_unbounded_list_view() {
        let source = "ListView(\n  children: [\n    Text('a'),\n  ],\n)\n";
        let mut issues = Vec::new();
        check_render_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-unbounded-list"),
            "expected perf-unbounded-list, got {:?}", issues);
    }

    #[test]
    fn render_detects_unbounded_grid_view() {
        let source = "GridView(\n  children: [\n    Text('a'),\n  ],\n)\n";
        let mut issues = Vec::new();
        check_render_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-unbounded-list"));
    }

    #[test]
    fn render_no_issue_for_list_view_builder() {
        let source = "ListView.builder(\n  itemCount: items.length,\n  itemBuilder: (ctx, i) => Text(items[i]),\n)\n";
        let mut issues = Vec::new();
        check_render_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-unbounded-list"));
    }

    #[test]
    fn render_detects_opacity_zero() {
        let source = "Opacity(\n  opacity: 0,\n  child: SomeWidget(),\n)\n";
        let mut issues = Vec::new();
        check_render_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-opacity-zero"),
            "expected perf-opacity-zero, got {:?}", issues);
    }

    #[test]
    fn render_no_issue_for_opacity_nonzero() {
        let source = "Opacity(\n  opacity: 1,\n  child: SomeWidget(),\n)\n";
        let mut issues = Vec::new();
        check_render_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-opacity-zero"));
    }

    // ── check_network_issues ────────────────────────────────────────────────

    #[test]
    fn network_no_issue_on_empty_source() {
        let mut issues = Vec::new();
        check_network_issues(&dummy_path(), "", &mut issues);
        assert!(issues.is_empty());
    }

    #[test]
    fn network_detects_http_get_in_build() {
        let source = "Widget build(BuildContext context) {\n  http.get(url);\n}\n";
        let mut issues = Vec::new();
        check_network_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-network-in-build"),
            "expected perf-network-in-build, got {:?}", issues);
    }

    #[test]
    fn network_detects_http_post_in_init_state() {
        let source = "void initState() {\n  super.initState();\n  http.post(url, body: data);\n}\n";
        let mut issues = Vec::new();
        check_network_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-network-in-build"));
    }

    #[test]
    fn network_detects_dio_in_build() {
        let source = "Widget build(BuildContext context) {\n  final dio = Dio();\n}\n";
        let mut issues = Vec::new();
        check_network_issues(&dummy_path(), source, &mut issues);
        assert!(issues.iter().any(|i| i.rule == "perf-network-in-build"));
    }

    #[test]
    fn network_no_issue_for_http_get_outside_build() {
        let source = "Future<void> fetchData() async {\n  final res = http.get(url);\n}\n";
        let mut issues = Vec::new();
        check_network_issues(&dummy_path(), source, &mut issues);
        assert!(!issues.iter().any(|i| i.rule == "perf-network-in-build"));
    }

    // ── analyze_performance (TempDir orchestrator) ──────────────────────────

    #[test]
    fn analyze_performance_returns_empty_for_empty_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let report = analyze_performance(dir.path());
        assert_eq!(report.total_files, 0);
        assert!(report.all_issues().is_empty());
    }

    #[test]
    fn analyze_performance_skips_generated_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("foo.g.dart"), "Image.network('x'),\n").unwrap();
        std::fs::write(dir.path().join("bar.freezed.dart"), "Image.network('y'),\n").unwrap();
        let report = analyze_performance(dir.path());
        assert_eq!(report.total_files, 0);
    }

    #[test]
    fn analyze_performance_counts_dart_files_and_finds_issues() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Trigger memory issue
        let source = "Image.network('https://example.com/x.png'),\n";
        std::fs::write(dir.path().join("widget.dart"), source).unwrap();
        let report = analyze_performance(dir.path());
        assert_eq!(report.total_files, 1);
        assert!(!report.memory_issues.is_empty());
    }

    #[test]
    fn analyze_performance_ignores_test_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&test_dir).unwrap();
        let source = "Image.network('https://example.com/x.png'),\n";
        std::fs::write(test_dir.join("widget_test.dart"), source).unwrap();
        let report = analyze_performance(dir.path());
        assert_eq!(report.total_files, 0);
    }
}
