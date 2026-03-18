use super::snapshot::AnalysisSnapshot;
use colored::Colorize;

#[derive(Debug)]
pub struct TrendReport {
    pub period: String,
    pub snapshots: usize,
    pub health_trend: TrendDirection,
    pub health_current: f64,
    pub health_previous: f64,
    pub issue_trend: TrendDirection,
    pub issues_current: usize,
    pub issues_previous: usize,
    pub complexity_trend: TrendDirection,
    pub complexity_current: f64,
    pub complexity_previous: f64,
    pub top_improving_rules: Vec<(String, i64)>,
    pub top_worsening_rules: Vec<(String, i64)>,
}

#[derive(Debug, PartialEq)]
pub enum TrendDirection {
    Improving,
    Stable,
    Declining,
}

impl TrendDirection {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Improving => "📈",
            Self::Stable => "➡️",
            Self::Declining => "📉",
        }
    }
}

/// Analyze trends from snapshot history.
pub fn analyze_trends(history: &[AnalysisSnapshot], last_n: usize) -> Option<TrendReport> {
    if history.len() < 2 {
        return None;
    }

    let recent: Vec<&AnalysisSnapshot> = history.iter().rev().take(last_n).collect();
    let current = recent.first()?;
    let previous = recent.last()?;

    let health_diff = current.health_score - previous.health_score;
    let health_trend = if health_diff > 2.0 {
        TrendDirection::Improving
    } else if health_diff < -2.0 {
        TrendDirection::Declining
    } else {
        TrendDirection::Stable
    };

    let issue_diff = current.issues.total as i64 - previous.issues.total as i64;
    let issue_trend = if issue_diff < -5 {
        TrendDirection::Improving
    } else if issue_diff > 5 {
        TrendDirection::Declining
    } else {
        TrendDirection::Stable
    };

    let cc_diff = current.metrics_summary.avg_cyclomatic - previous.metrics_summary.avg_cyclomatic;
    let complexity_trend = if cc_diff < -0.5 {
        TrendDirection::Improving
    } else if cc_diff > 0.5 {
        TrendDirection::Declining
    } else {
        TrendDirection::Stable
    };

    let mut rule_changes: Vec<(String, i64)> = Vec::new();
    let mut all_rules = std::collections::HashSet::new();
    for r in current.rule_counts.keys() {
        all_rules.insert(r.clone());
    }
    for r in previous.rule_counts.keys() {
        all_rules.insert(r.clone());
    }
    for rule in all_rules {
        let curr = *current.rule_counts.get(&rule).unwrap_or(&0) as i64;
        let prev = *previous.rule_counts.get(&rule).unwrap_or(&0) as i64;
        let diff = curr - prev;
        if diff != 0 {
            rule_changes.push((rule, diff));
        }
    }

    rule_changes.sort_by(|a, b| a.1.cmp(&b.1));

    let top_improving: Vec<(String, i64)> = rule_changes
        .iter()
        .filter(|(_, d)| *d < 0)
        .take(5)
        .cloned()
        .collect();

    let top_worsening: Vec<(String, i64)> = rule_changes
        .iter()
        .filter(|(_, d)| *d > 0)
        .rev()
        .take(5)
        .cloned()
        .collect();

    Some(TrendReport {
        period: format!("{} snapshots", recent.len()),
        snapshots: recent.len(),
        health_trend,
        health_current: current.health_score,
        health_previous: previous.health_score,
        issue_trend,
        issues_current: current.issues.total,
        issues_previous: previous.issues.total,
        complexity_trend,
        complexity_current: current.metrics_summary.avg_cyclomatic,
        complexity_previous: previous.metrics_summary.avg_cyclomatic,
        top_improving_rules: top_improving,
        top_worsening_rules: top_worsening,
    })
}

pub fn print_trend_report(report: &TrendReport) {
    println!();
    println!(
        "  {} Quality Trends ({} snapshots)",
        "falcon".bright_cyan().bold(),
        report.snapshots
    );
    println!();

    let _health_color = match report.health_trend {
        TrendDirection::Improving => colored::Color::Green,
        TrendDirection::Stable => colored::Color::Yellow,
        TrendDirection::Declining => colored::Color::Red,
    };
    let health_diff = report.health_current - report.health_previous;
    println!(
        "  Health:     {} {:.0} → {:.0} ({}{:.1})",
        report.health_trend.icon(),
        report.health_previous,
        report.health_current,
        if health_diff >= 0.0 { "+" } else { "" },
        health_diff,
    );

    let issue_diff = report.issues_current as i64 - report.issues_previous as i64;
    println!(
        "  Issues:     {} {} → {} ({}{}) ",
        report.issue_trend.icon(),
        report.issues_previous,
        report.issues_current,
        if issue_diff >= 0 { "+" } else { "" },
        issue_diff,
    );

    let cc_diff = report.complexity_current - report.complexity_previous;
    println!(
        "  Complexity: {} {:.1} → {:.1} ({}{:.1})",
        report.complexity_trend.icon(),
        report.complexity_previous,
        report.complexity_current,
        if cc_diff >= 0.0 { "+" } else { "" },
        cc_diff,
    );

    if !report.top_improving_rules.is_empty() {
        println!();
        println!("  {} Improving rules:", "✓".green());
        for (rule, diff) in &report.top_improving_rules {
            println!("    {} {} ({})", "↓".green(), rule, diff);
        }
    }

    if !report.top_worsening_rules.is_empty() {
        println!();
        println!("  {} Worsening rules:", "⚠".yellow());
        for (rule, diff) in &report.top_worsening_rules {
            println!("    {} {} (+{})", "↑".red(), rule, diff);
        }
    }
    println!();
}
