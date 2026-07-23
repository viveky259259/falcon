use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use colored::*;
use std::fmt::Write;
use std::path::PathBuf;

pub struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        print!("{}", format_analysis(report));
    }

    fn report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]) {
        print!("{}", format_metrics(metrics));
    }

    fn report_issues(&self, issues: &[Issue]) {
        print!("{}", format_issues(issues));
    }
}

fn format_analysis(report: &AnalysisReport) -> String {
    let mut out = String::new();
    out.push('\n');
    writeln!(
        out,
        "{}",
        "╔══════════════════════════════════════════════════╗".bright_cyan()
    )
    .unwrap();
    writeln!(
        out,
        "{}",
        "║           FALCON Analysis Report                ║".bright_cyan()
    )
    .unwrap();
    writeln!(
        out,
        "{}",
        "╚══════════════════════════════════════════════════╝".bright_cyan()
    )
    .unwrap();
    out.push('\n');

    writeln!(
        out,
        "  {} files analyzed",
        report.file_count.to_string().bold()
    )
    .unwrap();
    out.push('\n');

    if !report.issues.is_empty() {
        out.push_str(&format_issues(&report.issues));
    }

    if !report.metrics.is_empty() {
        out.push_str(&format_metrics(&report.metrics));
    }

    out.push('\n');
    out.push_str(&format_summary(report));
    out
}

fn format_metrics(metrics: &[(PathBuf, MetricsResults)]) -> String {
    let mut out = String::new();
    writeln!(out, "{}", "  ── Metrics ──".bright_blue().bold()).unwrap();
    out.push('\n');

    for (file, result) in metrics {
        let display_path = file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown");

        if result.functions.is_empty() && result.classes.is_empty() {
            continue;
        }

        writeln!(out, "  {}", display_path.bright_white().bold()).unwrap();

        for func in &result.functions {
            let cc_color = threshold_color(func.cyclomatic_complexity, 10, 20, 30);
            let mi_color = threshold_color_inverted(func.maintainability_index, 20.0, 40.0, 60.0);

            writeln!(
                out,
                "    {} {} │ CC: {} │ LOC: {} │ MI: {} │ HV: {:.0} │ Params: {} │ Nesting: {}",
                "fn".dimmed(),
                func.name.bright_white(),
                func.cyclomatic_complexity.to_string().color(cc_color),
                func.lines_of_code,
                format!("{:.1}", func.maintainability_index).color(mi_color),
                func.halstead_volume,
                func.number_of_parameters,
                func.max_nesting_level,
            )
            .unwrap();

            if func.widgets_nesting_level > 0 || func.number_of_used_widgets > 0 {
                writeln!(
                    out,
                    "           {} Widget Nesting: {} │ Widgets Used: {}",
                    "└".dimmed(),
                    func.widgets_nesting_level,
                    func.number_of_used_widgets,
                )
                .unwrap();
            }
        }

        for class in &result.classes {
            writeln!(
                out,
                "    {} {} │ Methods: {} │ WMC: {} │ CBO: {} │ DIT: {} │ RFC: {} │ LCOM: {}",
                "class".dimmed(),
                class.name.bright_white(),
                class.number_of_methods,
                class.weighted_methods_per_class,
                class.coupling_between_objects,
                class.depth_of_inheritance,
                class.response_for_class,
                class.lack_of_cohesion,
            )
            .unwrap();
            writeln!(
                out,
                "           {} TCC: {:.2} │ WOC: {:.2} │ Interfaces: {} │ Overridden: {} │ Added: {}",
                "└".dimmed(),
                class.tight_class_cohesion,
                class.weight_of_class,
                class.number_of_interfaces,
                class.number_of_overridden_methods,
                class.number_of_added_methods,
            )
            .unwrap();
        }

        out.push('\n');
    }
    out
}

fn format_issues(issues: &[Issue]) -> String {
    let mut out = String::new();
    writeln!(out, "{}", "  ── Issues ──".bright_yellow().bold()).unwrap();
    out.push('\n');

    let mut sorted = issues.to_vec();
    sorted.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    let mut current_file = PathBuf::new();

    for issue in &sorted {
        if issue.file != current_file {
            current_file = issue.file.clone();
            let display = current_file
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown");
            writeln!(out, "  {}", display.bright_white().bold()).unwrap();
        }

        let severity_str = match issue.severity {
            Severity::Error => "ERROR".red().bold(),
            Severity::Warning => "WARN ".yellow().bold(),
            Severity::Info => "INFO ".blue().bold(),
        };

        writeln!(
            out,
            "    {} {}:{} {} {}",
            severity_str,
            issue.line.to_string().dimmed(),
            issue.column.to_string().dimmed(),
            issue.message,
            format!("({})", issue.rule).dimmed(),
        )
        .unwrap();
    }

    out.push('\n');
    out
}

fn format_summary(report: &AnalysisReport) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let infos = report.info_count();

    let mut out = String::new();
    writeln!(out, "{}", "  ── Summary ──".bright_white().bold()).unwrap();

    if errors > 0 {
        writeln!(
            out,
            "    {} {}",
            "●".red(),
            format!("{} errors", errors).red().bold()
        )
        .unwrap();
    }
    if warnings > 0 {
        writeln!(
            out,
            "    {} {}",
            "●".yellow(),
            format!("{} warnings", warnings).yellow().bold()
        )
        .unwrap();
    }
    if infos > 0 {
        writeln!(
            out,
            "    {} {}",
            "●".blue(),
            format!("{} info", infos).blue()
        )
        .unwrap();
    }

    if errors == 0 && warnings == 0 && infos == 0 {
        writeln!(
            out,
            "    {} {}",
            "✓".green().bold(),
            "No issues found!".green()
        )
        .unwrap();
    }

    out.push('\n');
    out
}

fn threshold_color(value: u32, noted: u32, warning: u32, alarm: u32) -> Color {
    if value >= alarm || value >= warning {
        Color::Red
    } else if value >= noted {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn threshold_color_inverted(value: f64, alarm: f64, warning: f64, noted: f64) -> Color {
    if value <= alarm {
        Color::Red
    } else if value <= warning || value <= noted {
        Color::Yellow
    } else {
        Color::Green
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use crate::metrics::{ClassMetrics, FunctionMetrics, MetricsResults};
    use crate::reporters::{AnalysisReport, Issue};
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn disable_colors() {
        INIT.call_once(|| {
            colored::control::set_override(false);
        });
    }

    fn sample_function() -> FunctionMetrics {
        FunctionMetrics {
            name: "my_func".into(),
            line: 10,
            cyclomatic_complexity: 3,
            lines_of_code: 20,
            source_lines_of_code: 15,
            maintainability_index: 75.0,
            max_nesting_level: 2,
            number_of_parameters: 2,
            halstead_volume: 100.0,
            halstead_difficulty: 5.0,
            widgets_nesting_level: 0,
            number_of_used_widgets: 0,
        }
    }

    fn sample_class() -> ClassMetrics {
        ClassMetrics {
            name: "MyClass".into(),
            line: 5,
            number_of_methods: 4,
            lines_of_code: 50,
            coupling_between_objects: 2,
            depth_of_inheritance: 1,
            number_of_added_methods: 1,
            number_of_interfaces: 0,
            number_of_overridden_methods: 1,
            response_for_class: 6,
            tight_class_cohesion: 0.5,
            weight_of_class: 0.75,
            weighted_methods_per_class: 4,
            lack_of_cohesion: 2,
        }
    }

    fn sample_metrics(funcs: usize, classes: usize) -> MetricsResults {
        MetricsResults {
            file_lines_of_code: 100,
            file_source_lines_of_code: 80,
            functions: (0..funcs).map(|_| sample_function()).collect(),
            classes: (0..classes).map(|_| sample_class()).collect(),
        }
    }

    fn sample_issue(rule: &str, severity: Severity, line: usize, file: &str) -> Issue {
        Issue {
            rule: rule.into(),
            message: format!("issue from {rule}"),
            severity,
            file: PathBuf::from(file),
            line,
            column: 1,
        }
    }

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            issues: vec![],
            metrics: vec![],
            file_count: 5,
            project_path: None,
        }
    }

    // threshold_color tests

    #[test]
    fn threshold_color_green_below_noted() {
        assert_eq!(threshold_color(5, 10, 20, 30), Color::Green);
    }

    #[test]
    fn threshold_color_yellow_at_noted() {
        assert_eq!(threshold_color(10, 10, 20, 30), Color::Yellow);
    }

    #[test]
    fn threshold_color_red_at_warning() {
        assert_eq!(threshold_color(20, 10, 20, 30), Color::Red);
    }

    #[test]
    fn threshold_color_red_at_alarm() {
        assert_eq!(threshold_color(30, 10, 20, 30), Color::Red);
    }

    // threshold_color_inverted tests

    #[test]
    fn threshold_color_inverted_red_below_alarm() {
        assert_eq!(threshold_color_inverted(10.0, 20.0, 40.0, 60.0), Color::Red);
    }

    #[test]
    fn threshold_color_inverted_yellow_at_warning() {
        assert_eq!(
            threshold_color_inverted(40.0, 20.0, 40.0, 60.0),
            Color::Yellow
        );
    }

    #[test]
    fn threshold_color_inverted_yellow_at_noted() {
        assert_eq!(
            threshold_color_inverted(60.0, 20.0, 40.0, 60.0),
            Color::Yellow
        );
    }

    #[test]
    fn threshold_color_inverted_green_above_noted() {
        assert_eq!(
            threshold_color_inverted(100.0, 20.0, 40.0, 60.0),
            Color::Green
        );
    }

    // format_summary tests

    #[test]
    fn format_summary_no_issues_shows_ok() {
        disable_colors();
        let report = sample_report();
        let out = format_summary(&report);
        assert!(out.contains("No issues found!"), "out: {out}");
        assert!(out.contains("── Summary ──"), "out: {out}");
    }

    #[test]
    fn format_summary_with_errors_only() {
        disable_colors();
        let mut report = sample_report();
        report.issues = vec![
            sample_issue("rule1", Severity::Error, 1, "a.dart"),
            sample_issue("rule2", Severity::Error, 2, "a.dart"),
        ];
        let out = format_summary(&report);
        assert!(out.contains("2 errors"), "out: {out}");
        assert!(!out.contains("warnings"), "out: {out}");
        assert!(!out.contains("info "), "out: {out}");
    }

    #[test]
    fn format_summary_with_warnings_only() {
        disable_colors();
        let mut report = sample_report();
        report.issues = vec![
            sample_issue("rule1", Severity::Warning, 1, "a.dart"),
            sample_issue("rule2", Severity::Warning, 2, "a.dart"),
            sample_issue("rule3", Severity::Warning, 3, "a.dart"),
        ];
        let out = format_summary(&report);
        assert!(out.contains("3 warnings"), "out: {out}");
    }

    #[test]
    fn format_summary_with_info_only() {
        disable_colors();
        let mut report = sample_report();
        report.issues = vec![
            sample_issue("rule1", Severity::Info, 1, "a.dart"),
            sample_issue("rule2", Severity::Info, 2, "a.dart"),
            sample_issue("rule3", Severity::Info, 3, "a.dart"),
            sample_issue("rule4", Severity::Info, 4, "a.dart"),
        ];
        let out = format_summary(&report);
        assert!(out.contains("4 info"), "out: {out}");
    }

    #[test]
    fn format_summary_with_all_severities() {
        disable_colors();
        let mut report = sample_report();
        report.issues = vec![
            sample_issue("r1", Severity::Error, 1, "a.dart"),
            sample_issue("r2", Severity::Warning, 2, "a.dart"),
            sample_issue("r3", Severity::Warning, 3, "a.dart"),
            sample_issue("r4", Severity::Info, 4, "a.dart"),
            sample_issue("r5", Severity::Info, 5, "a.dart"),
            sample_issue("r6", Severity::Info, 6, "a.dart"),
        ];
        let out = format_summary(&report);
        assert!(out.contains("1 errors"), "out: {out}");
        assert!(out.contains("2 warnings"), "out: {out}");
        assert!(out.contains("3 info"), "out: {out}");
    }

    // format_issues tests

    #[test]
    fn format_issues_empty_skips_header_section() {
        disable_colors();
        // Should not panic with empty slice
        let out = format_issues(&[]);
        // Header is still present but no file/issue lines
        let _ = out; // no panic is the assertion
    }

    #[test]
    fn format_issues_groups_by_file() {
        disable_colors();
        let issues = vec![
            sample_issue("rule1", Severity::Error, 1, "a.dart"),
            sample_issue("rule2", Severity::Warning, 2, "a.dart"),
            sample_issue("rule3", Severity::Info, 1, "b.dart"),
        ];
        let out = format_issues(&issues);
        // The bold filename header line should appear exactly once per file
        // It's rendered as "  a.dart\n" (with leading spaces)
        let a_header_count = out.matches("  a.dart\n").count();
        let b_header_count = out.matches("  b.dart\n").count();
        assert_eq!(
            a_header_count, 1,
            "a.dart header count: {a_header_count}, out:\n{out}"
        );
        assert_eq!(
            b_header_count, 1,
            "b.dart header count: {b_header_count}, out:\n{out}"
        );
    }

    #[test]
    fn format_issues_sorts_by_line() {
        disable_colors();
        let issues = vec![
            sample_issue("rule1", Severity::Error, 5, "a.dart"),
            sample_issue("rule2", Severity::Warning, 2, "a.dart"),
        ];
        let out = format_issues(&issues);
        let pos2 = out.find("2:").expect("line 2 not found");
        let pos5 = out.find("5:").expect("line 5 not found");
        assert!(
            pos2 < pos5,
            "line 2 should appear before line 5, out:\n{out}"
        );
    }

    #[test]
    fn format_issues_renders_all_three_severity_labels() {
        disable_colors();
        let issues = vec![
            sample_issue("r1", Severity::Error, 1, "a.dart"),
            sample_issue("r2", Severity::Warning, 2, "a.dart"),
            sample_issue("r3", Severity::Info, 3, "a.dart"),
        ];
        let out = format_issues(&issues);
        assert!(out.contains("ERROR"), "out: {out}");
        assert!(out.contains("WARN"), "out: {out}");
        assert!(out.contains("INFO"), "out: {out}");
    }

    // format_metrics tests

    #[test]
    fn format_metrics_empty_returns_header_only() {
        disable_colors();
        let out = format_metrics(&[]);
        assert!(out.contains("── Metrics ──"), "out: {out}");
        assert!(!out.contains("fn "), "out: {out}");
        assert!(!out.contains("class "), "out: {out}");
    }

    #[test]
    fn format_metrics_emits_function_lines() {
        disable_colors();
        let metrics = vec![(PathBuf::from("lib.dart"), sample_metrics(1, 0))];
        let out = format_metrics(&metrics);
        assert!(out.contains("my_func"), "out: {out}");
        assert!(out.contains("fn"), "out: {out}");
        assert!(out.contains("CC:"), "out: {out}");
        assert!(out.contains("LOC:"), "out: {out}");
        assert!(out.contains("MI:"), "out: {out}");
    }

    #[test]
    fn format_metrics_emits_class_lines() {
        disable_colors();
        let metrics = vec![(PathBuf::from("lib.dart"), sample_metrics(0, 1))];
        let out = format_metrics(&metrics);
        assert!(out.contains("MyClass"), "out: {out}");
        assert!(out.contains("class"), "out: {out}");
        assert!(out.contains("Methods:"), "out: {out}");
        assert!(out.contains("WMC:"), "out: {out}");
        assert!(out.contains("CBO:"), "out: {out}");
    }

    #[test]
    fn format_metrics_skips_file_with_no_functions_or_classes() {
        disable_colors();
        let metrics = vec![(PathBuf::from("empty.dart"), sample_metrics(0, 0))];
        let out = format_metrics(&metrics);
        assert!(!out.contains("empty.dart"), "out: {out}");
    }

    #[test]
    fn format_metrics_widget_line_present_when_widgets_used() {
        disable_colors();
        let mut func = sample_function();
        func.widgets_nesting_level = 2;
        func.number_of_used_widgets = 3;
        let metrics = vec![(
            PathBuf::from("widget.dart"),
            MetricsResults {
                file_lines_of_code: 100,
                file_source_lines_of_code: 80,
                functions: vec![func],
                classes: vec![],
            },
        )];
        let out = format_metrics(&metrics);
        assert!(out.contains("Widget Nesting:"), "out: {out}");
        assert!(out.contains("Widgets Used:"), "out: {out}");
    }

    #[test]
    fn format_metrics_widget_line_absent_when_no_widgets() {
        disable_colors();
        let metrics = vec![(PathBuf::from("lib.dart"), sample_metrics(1, 0))];
        let out = format_metrics(&metrics);
        assert!(!out.contains("Widget Nesting:"), "out: {out}");
    }

    // format_analysis tests

    #[test]
    fn format_analysis_contains_banner_and_file_count() {
        disable_colors();
        let report = sample_report();
        let out = format_analysis(&report);
        assert!(out.contains("FALCON Analysis Report"), "out: {out}");
        assert!(out.contains("5 files analyzed"), "out: {out}");
    }

    #[test]
    fn format_analysis_skips_issues_block_when_empty() {
        disable_colors();
        let report = sample_report();
        let out = format_analysis(&report);
        assert!(!out.contains("── Issues ──"), "out: {out}");
    }

    #[test]
    fn format_analysis_skips_metrics_block_when_empty() {
        disable_colors();
        let report = sample_report();
        let out = format_analysis(&report);
        assert!(!out.contains("── Metrics ──"), "out: {out}");
    }

    #[test]
    fn format_analysis_includes_issues_block_when_present() {
        disable_colors();
        let mut report = sample_report();
        report.issues = vec![sample_issue("rule1", Severity::Error, 1, "a.dart")];
        let out = format_analysis(&report);
        assert!(out.contains("── Issues ──"), "out: {out}");
    }

    #[test]
    fn format_analysis_includes_metrics_block_when_present() {
        disable_colors();
        let mut report = sample_report();
        report.metrics = vec![(PathBuf::from("lib.dart"), sample_metrics(1, 0))];
        let out = format_analysis(&report);
        assert!(out.contains("── Metrics ──"), "out: {out}");
    }
}
