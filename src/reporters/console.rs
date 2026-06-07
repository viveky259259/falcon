use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use colored::*;
use std::path::PathBuf;

pub struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        println!();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════╗".bright_cyan()
        );
        println!(
            "{}",
            "║           FALCON Analysis Report                ║".bright_cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════╝".bright_cyan()
        );
        println!();

        println!("  {} files analyzed", report.file_count.to_string().bold());
        println!();

        if !report.issues.is_empty() {
            self.report_issues(&report.issues);
        }

        if !report.metrics.is_empty() {
            self.report_metrics(&report.metrics);
        }

        println!();
        print_summary(report);
    }

    fn report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]) {
        println!("{}", "  ── Metrics ──".bright_blue().bold());
        println!();

        for (file, result) in metrics {
            let display_path = file
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown");

            if result.functions.is_empty() && result.classes.is_empty() {
                continue;
            }

            println!("  {}", display_path.bright_white().bold());

            for func in &result.functions {
                let cc_color = threshold_color(func.cyclomatic_complexity, 10, 20, 30);
                let mi_color =
                    threshold_color_inverted(func.maintainability_index, 20.0, 40.0, 60.0);

                println!(
                    "    {} {} │ CC: {} │ LOC: {} │ MI: {} │ HV: {:.0} │ Params: {} │ Nesting: {}",
                    "fn".dimmed(),
                    func.name.bright_white(),
                    func.cyclomatic_complexity.to_string().color(cc_color),
                    func.lines_of_code,
                    format!("{:.1}", func.maintainability_index).color(mi_color),
                    func.halstead_volume,
                    func.number_of_parameters,
                    func.max_nesting_level,
                );

                if func.widgets_nesting_level > 0 || func.number_of_used_widgets > 0 {
                    println!(
                        "           {} Widget Nesting: {} │ Widgets Used: {}",
                        "└".dimmed(),
                        func.widgets_nesting_level,
                        func.number_of_used_widgets,
                    );
                }
            }

            for class in &result.classes {
                println!(
                    "    {} {} │ Methods: {} │ WMC: {} │ CBO: {} │ DIT: {} │ RFC: {} │ LCOM: {}",
                    "class".dimmed(),
                    class.name.bright_white(),
                    class.number_of_methods,
                    class.weighted_methods_per_class,
                    class.coupling_between_objects,
                    class.depth_of_inheritance,
                    class.response_for_class,
                    class.lack_of_cohesion,
                );
                println!(
                    "           {} TCC: {:.2} │ WOC: {:.2} │ Interfaces: {} │ Overridden: {} │ Added: {}",
                    "└".dimmed(),
                    class.tight_class_cohesion,
                    class.weight_of_class,
                    class.number_of_interfaces,
                    class.number_of_overridden_methods,
                    class.number_of_added_methods,
                );
            }

            println!();
        }
    }

    fn report_issues(&self, issues: &[Issue]) {
        println!("{}", "  ── Issues ──".bright_yellow().bold());
        println!();

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
                println!("  {}", display.bright_white().bold());
            }

            let severity_str = match issue.severity {
                Severity::Error => "ERROR".red().bold(),
                Severity::Warning => "WARN ".yellow().bold(),
                Severity::Info => "INFO ".blue().bold(),
            };

            println!(
                "    {} {}:{} {} {}",
                severity_str,
                issue.line.to_string().dimmed(),
                issue.column.to_string().dimmed(),
                issue.message,
                format!("({})", issue.rule).dimmed(),
            );
        }

        println!();
    }
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

fn print_summary(report: &AnalysisReport) {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let infos = report.info_count();

    println!("{}", "  ── Summary ──".bright_white().bold());

    if errors > 0 {
        println!(
            "    {} {}",
            "●".red(),
            format!("{} errors", errors).red().bold()
        );
    }
    if warnings > 0 {
        println!(
            "    {} {}",
            "●".yellow(),
            format!("{} warnings", warnings).yellow().bold()
        );
    }
    if infos > 0 {
        println!("    {} {}", "●".blue(), format!("{} info", infos).blue());
    }

    if errors == 0 && warnings == 0 && infos == 0 {
        println!("    {} {}", "✓".green().bold(), "No issues found!".green());
    }

    println!();
}
