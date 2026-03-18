use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Checkstyle XML format — used by Jenkins, many legacy CI systems.
pub struct CheckstyleReporter {
    pub output_path: Option<PathBuf>,
}

impl Reporter for CheckstyleReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        output_xml(&build_xml(&report.issues), &self.output_path);
    }

    fn report_metrics(&self, _metrics: &[(PathBuf, MetricsResults)]) {
        output_xml(&build_xml(&[]), &self.output_path);
    }

    fn report_issues(&self, issues: &[Issue]) {
        output_xml(&build_xml(issues), &self.output_path);
    }
}

fn build_xml(issues: &[Issue]) -> String {
    let mut by_file: BTreeMap<String, Vec<&Issue>> = BTreeMap::new();
    for issue in issues {
        by_file
            .entry(issue.file.to_string_lossy().to_string())
            .or_default()
            .push(issue);
    }

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<checkstyle version=\"8.0\">\n");

    for (file, file_issues) in &by_file {
        xml.push_str(&format!("  <file name=\"{}\">\n", xml_escape(file)));
        for issue in file_issues {
            xml.push_str(&format!(
                "    <error line=\"{}\" column=\"{}\" severity=\"{}\" message=\"{}\" source=\"falcon.{}\"/>\n",
                issue.line,
                issue.column,
                cs_severity(&issue.severity),
                xml_escape(&issue.message),
                xml_escape(&issue.rule),
            ));
        }
        xml.push_str("  </file>\n");
    }

    xml.push_str("</checkstyle>\n");
    xml
}

fn cs_severity(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn output_xml(xml: &str, output_path: &Option<PathBuf>) {
    if let Some(path) = output_path {
        match std::fs::write(path, xml) {
            Ok(_) => eprintln!("Checkstyle report written to: {}", path.display()),
            Err(e) => eprintln!("Failed to write Checkstyle report: {}", e),
        }
    } else {
        println!("{}", xml);
    }
}
