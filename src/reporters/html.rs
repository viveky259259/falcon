use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use std::path::PathBuf;

pub struct HtmlReporter {
    pub output_path: PathBuf,
}

impl Reporter for HtmlReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        let html = build_full_report(report);
        match std::fs::write(&self.output_path, &html) {
            Ok(_) => println!("HTML report written to: {}", self.output_path.display()),
            Err(e) => eprintln!("Failed to write HTML report: {}", e),
        }
    }

    fn report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]) {
        let html = build_metrics_report(metrics);
        match std::fs::write(&self.output_path, &html) {
            Ok(_) => println!("HTML report written to: {}", self.output_path.display()),
            Err(e) => eprintln!("Failed to write HTML report: {}", e),
        }
    }

    fn report_issues(&self, issues: &[Issue]) {
        let html = build_issues_report(issues);
        match std::fs::write(&self.output_path, &html) {
            Ok(_) => println!("HTML report written to: {}", self.output_path.display()),
            Err(e) => eprintln!("Failed to write HTML report: {}", e),
        }
    }
}

fn build_full_report(report: &AnalysisReport) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let infos = report.info_count();

    let mut html = String::new();
    html.push_str(&html_header("Falcon Analysis Report"));

    html.push_str(r#"<div class="summary-cards">"#);
    html.push_str(&summary_card("Files Analyzed", &report.file_count.to_string(), "blue"));
    html.push_str(&summary_card("Errors", &errors.to_string(), if errors > 0 { "red" } else { "green" }));
    html.push_str(&summary_card("Warnings", &warnings.to_string(), if warnings > 0 { "orange" } else { "green" }));
    html.push_str(&summary_card("Info", &infos.to_string(), "blue"));
    html.push_str("</div>");

    if !report.issues.is_empty() {
        html.push_str(&issues_section(&report.issues));
    }

    if !report.metrics.is_empty() {
        html.push_str(&metrics_section(&report.metrics));
    }

    html.push_str(&html_footer());
    html
}

fn build_metrics_report(metrics: &[(PathBuf, MetricsResults)]) -> String {
    let mut html = String::new();
    html.push_str(&html_header("Falcon Metrics Report"));
    html.push_str(&metrics_section(metrics));
    html.push_str(&html_footer());
    html
}

fn build_issues_report(issues: &[Issue]) -> String {
    let mut html = String::new();
    html.push_str(&html_header("Falcon Issues Report"));
    html.push_str(&issues_section(issues));
    html.push_str(&html_footer());
    html
}

fn summary_card(label: &str, value: &str, color: &str) -> String {
    format!(
        r#"<div class="card" style="border-left: 4px solid {};">
            <div class="card-value" style="color: {};">{}</div>
            <div class="card-label">{}</div>
        </div>"#,
        color, color, value, label
    )
}

fn issues_section(issues: &[Issue]) -> String {
    let mut html = String::from(r#"<h2>Issues</h2><table class="data-table"><thead><tr><th>Severity</th><th>File</th><th>Line</th><th>Rule</th><th>Message</th></tr></thead><tbody>"#);

    let mut sorted = issues.to_vec();
    sorted.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    for issue in &sorted {
        let sev_class = match issue.severity {
            Severity::Error => "severity-error",
            Severity::Warning => "severity-warning",
            Severity::Info => "severity-info",
        };
        let file_name = issue.file.file_name().and_then(|f| f.to_str()).unwrap_or("unknown");

        html.push_str(&format!(
            r#"<tr><td><span class="{}">{}</span></td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>"#,
            sev_class, issue.severity, file_name, issue.line, issue.rule,
            html_escape(&issue.message)
        ));
    }

    html.push_str("</tbody></table>");
    html
}

fn metrics_section(metrics: &[(PathBuf, MetricsResults)]) -> String {
    let mut html = String::new();

    html.push_str(r#"<h2>Function Metrics</h2><table class="data-table"><thead><tr><th>File</th><th>Function</th><th>CC</th><th>LOC</th><th>MI</th><th>Halstead Vol</th><th>Params</th><th>Nesting</th><th>Widgets</th></tr></thead><tbody>"#);

    for (file, result) in metrics {
        let file_name = file.file_name().and_then(|f| f.to_str()).unwrap_or("unknown");
        for func in &result.functions {
            let cc_class = metric_class(func.cyclomatic_complexity, 10, 20, 30);
            let mi_class = metric_class_inverted(func.maintainability_index, 20.0, 40.0, 60.0);

            html.push_str(&format!(
                r#"<tr><td>{}</td><td><code>{}</code></td><td class="{}">{}</td><td>{}</td><td class="{}">{:.1}</td><td>{:.0}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                file_name, html_escape(&func.name),
                cc_class, func.cyclomatic_complexity,
                func.lines_of_code,
                mi_class, func.maintainability_index,
                func.halstead_volume,
                func.number_of_parameters,
                func.max_nesting_level,
                func.number_of_used_widgets,
            ));
        }
    }

    html.push_str("</tbody></table>");

    html.push_str(r#"<h2>Class Metrics</h2><table class="data-table"><thead><tr><th>File</th><th>Class</th><th>Methods</th><th>WMC</th><th>CBO</th><th>DIT</th><th>RFC</th><th>LCOM</th><th>TCC</th><th>WOC</th><th>Interfaces</th></tr></thead><tbody>"#);

    for (file, result) in metrics {
        let file_name = file.file_name().and_then(|f| f.to_str()).unwrap_or("unknown");
        for class in &result.classes {
            html.push_str(&format!(
                r#"<tr><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td><td>{}</td></tr>"#,
                file_name, html_escape(&class.name),
                class.number_of_methods,
                class.weighted_methods_per_class,
                class.coupling_between_objects,
                class.depth_of_inheritance,
                class.response_for_class,
                class.lack_of_cohesion,
                class.tight_class_cohesion,
                class.weight_of_class,
                class.number_of_interfaces,
            ));
        }
    }

    html.push_str("</tbody></table>");
    html
}

fn metric_class(value: u32, noted: u32, warning: u32, alarm: u32) -> &'static str {
    if value >= alarm { "metric-alarm" }
    else if value >= warning { "metric-warning" }
    else if value >= noted { "metric-noted" }
    else { "metric-ok" }
}

fn metric_class_inverted(value: f64, alarm: f64, warning: f64, noted: f64) -> &'static str {
    if value <= alarm { "metric-alarm" }
    else if value <= warning { "metric-warning" }
    else if value <= noted { "metric-noted" }
    else { "metric-ok" }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn html_header(title: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
  :root {{ --bg: #1a1b26; --surface: #24283b; --text: #c0caf5; --dim: #565f89; --blue: #7aa2f7; --green: #9ece6a; --yellow: #e0af68; --orange: #ff9e64; --red: #f7768e; --purple: #bb9af7; }}
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif; background: var(--bg); color: var(--text); padding: 2rem; }}
  h1 {{ color: var(--blue); margin-bottom: 1.5rem; font-size: 1.8rem; }}
  h2 {{ color: var(--purple); margin: 2rem 0 1rem; font-size: 1.3rem; border-bottom: 1px solid var(--dim); padding-bottom: 0.5rem; }}
  .summary-cards {{ display: flex; gap: 1rem; margin-bottom: 2rem; flex-wrap: wrap; }}
  .card {{ background: var(--surface); border-radius: 8px; padding: 1.2rem 1.5rem; min-width: 140px; }}
  .card-value {{ font-size: 2rem; font-weight: 700; }}
  .card-label {{ color: var(--dim); font-size: 0.85rem; margin-top: 0.3rem; }}
  .data-table {{ width: 100%; border-collapse: collapse; background: var(--surface); border-radius: 8px; overflow: hidden; margin-bottom: 1rem; }}
  .data-table th {{ background: #1e2030; padding: 0.7rem 1rem; text-align: left; font-size: 0.8rem; color: var(--dim); text-transform: uppercase; letter-spacing: 0.05em; }}
  .data-table td {{ padding: 0.6rem 1rem; border-top: 1px solid #2a2e42; font-size: 0.9rem; }}
  .data-table tr:hover {{ background: #2a2e42; }}
  .data-table code {{ color: var(--blue); background: none; font-size: 0.85rem; }}
  .severity-error {{ background: var(--red); color: #1a1b26; padding: 2px 8px; border-radius: 4px; font-weight: 600; font-size: 0.75rem; }}
  .severity-warning {{ background: var(--yellow); color: #1a1b26; padding: 2px 8px; border-radius: 4px; font-weight: 600; font-size: 0.75rem; }}
  .severity-info {{ background: var(--blue); color: #1a1b26; padding: 2px 8px; border-radius: 4px; font-weight: 600; font-size: 0.75rem; }}
  .metric-ok {{ color: var(--green); font-weight: 600; }}
  .metric-noted {{ color: var(--yellow); font-weight: 600; }}
  .metric-warning {{ color: var(--orange); font-weight: 600; }}
  .metric-alarm {{ color: var(--red); font-weight: 700; }}
</style>
</head>
<body>
<h1>{title}</h1>
"#)
}

fn html_footer() -> String {
    format!(
        r#"<div style="margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #2a2e42; color: #565f89; font-size: 0.8rem;">
  Generated by Falcon v{} &mdash; Rust-powered static analysis for Flutter/Dart
</div>
</body>
</html>"#,
        env!("CARGO_PKG_VERSION")
    )
}
