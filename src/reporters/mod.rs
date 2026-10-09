pub mod checkstyle;
pub mod codeclimate;
pub mod console;
pub mod html;
pub mod json;
pub mod sarif;
pub mod sonar;

use crate::config::Severity;
use crate::metrics::MetricsResults;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Issue {
    pub rule: String,
    pub message: String,
    pub severity: Severity,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug)]
pub struct AnalysisReport {
    pub issues: Vec<Issue>,
    pub metrics: Vec<(PathBuf, MetricsResults)>,
    pub file_count: usize,
    pub project_path: Option<PathBuf>,
}

impl AnalysisReport {
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    pub fn info_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}

pub trait Reporter {
    /// Fallible output for automation; existing display methods remain available.
    fn try_report_analysis(&self, report: &AnalysisReport) -> anyhow::Result<()> {
        self.report_analysis(report);
        Ok(())
    }
    fn try_report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]) -> anyhow::Result<()> {
        self.report_metrics(metrics);
        Ok(())
    }
    fn try_report_issues(&self, issues: &[Issue]) -> anyhow::Result<()> {
        self.report_issues(issues);
        Ok(())
    }

    fn report_analysis(&self, report: &AnalysisReport);
    fn report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]);
    fn report_issues(&self, issues: &[Issue]);
}

pub(super) fn write_report(content: &str, path: Option<&std::path::Path>) -> anyhow::Result<()> {
    use anyhow::Context;
    use std::io::Write;
    if let Some(path) = path {
        std::fs::write(path, content)
            .with_context(|| format!("Writing report {}", path.display()))?;
    } else {
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{}", content)?;
        stdout.flush()?;
    }
    Ok(())
}
