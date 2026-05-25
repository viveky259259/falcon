use super::snapshot::AnalysisSnapshot;
use crate::config::FalconConfig;
use crate::Falcon;
use colored::Colorize;
use std::path::Path;

pub struct ComparisonResult {
    pub run1: AnalysisSnapshot,
    pub run2: AnalysisSnapshot,
    pub deltas: Deltas,
}

pub struct Deltas {
    pub files: Delta<usize>,
    pub lines: Delta<usize>,
    pub health: Delta<f64>,
    pub errors: Delta<usize>,
    pub warnings: Delta<usize>,
    pub info: Delta<usize>,
    pub total_issues: Delta<usize>,
    pub avg_cc: Delta<f64>,
    pub max_cc: Delta<u32>,
    pub avg_mi: Delta<f64>,
    pub god_files: Delta<usize>,
    pub rules_added: Vec<(String, usize)>,
    pub rules_removed: Vec<(String, usize)>,
    pub rules_changed: Vec<(String, i64)>,
}

pub struct Delta<T> {
    pub before: T,
    pub after: T,
}

impl Delta<usize> {
    fn diff(&self) -> i64 {
        self.after as i64 - self.before as i64
    }
}

impl Delta<f64> {
    fn diff(&self) -> f64 {
        self.after - self.before
    }
}

impl Delta<u32> {
    fn diff(&self) -> i64 {
        self.after as i64 - self.before as i64
    }
}

pub fn compare_snapshots(run1: &AnalysisSnapshot, run2: &AnalysisSnapshot) -> ComparisonResult {
    let mut rules_added = Vec::new();
    let mut rules_removed = Vec::new();
    let mut rules_changed = Vec::new();

    for (rule, &count2) in &run2.rule_counts {
        match run1.rule_counts.get(rule) {
            Some(&count1) if count1 != count2 => {
                rules_changed.push((rule.clone(), count2 as i64 - count1 as i64));
            }
            None => {
                rules_added.push((rule.clone(), count2));
            }
            _ => {}
        }
    }
    for (rule, &count1) in &run1.rule_counts {
        if !run2.rule_counts.contains_key(rule) {
            rules_removed.push((rule.clone(), count1));
        }
    }

    rules_changed.sort_by(|a, b| b.1.abs().cmp(&a.1.abs()));
    rules_added.sort_by(|a, b| b.1.cmp(&a.1));
    rules_removed.sort_by(|a, b| b.1.cmp(&a.1));

    ComparisonResult {
        run1: run1.clone(),
        run2: run2.clone(),
        deltas: Deltas {
            files: Delta {
                before: run1.file_count,
                after: run2.file_count,
            },
            lines: Delta {
                before: run1.total_lines,
                after: run2.total_lines,
            },
            health: Delta {
                before: run1.health_score,
                after: run2.health_score,
            },
            errors: Delta {
                before: run1.issues.errors,
                after: run2.issues.errors,
            },
            warnings: Delta {
                before: run1.issues.warnings,
                after: run2.issues.warnings,
            },
            info: Delta {
                before: run1.issues.info,
                after: run2.issues.info,
            },
            total_issues: Delta {
                before: run1.issues.total,
                after: run2.issues.total,
            },
            avg_cc: Delta {
                before: run1.metrics_summary.avg_cyclomatic,
                after: run2.metrics_summary.avg_cyclomatic,
            },
            max_cc: Delta {
                before: run1.metrics_summary.max_cyclomatic,
                after: run2.metrics_summary.max_cyclomatic,
            },
            avg_mi: Delta {
                before: run1.metrics_summary.avg_maintainability,
                after: run2.metrics_summary.avg_maintainability,
            },
            god_files: Delta {
                before: run1.metrics_summary.god_file_count,
                after: run2.metrics_summary.god_file_count,
            },
            rules_added,
            rules_removed,
            rules_changed,
        },
    }
}

pub fn format_comparison(result: &ComparisonResult) -> String {
    use std::fmt::Write;
    let r1 = &result.run1;
    let r2 = &result.run2;
    let d = &result.deltas;

    let mut out = String::new();

    out.push('\n');
    writeln!(
        out,
        "  🦅 {} {}",
        "falcon".bright_blue().bold(),
        "Report Comparison".bold()
    ).unwrap();
    out.push('\n');
    writeln!(
        out,
        "  🏷  {} {} ({} {})",
        "Run 1:".dimmed(),
        r1.timestamp,
        "🌿",
        r1.branch.as_deref().unwrap_or("—").bright_cyan()
    ).unwrap();
    writeln!(
        out,
        "  🏷  {} {} ({} {})",
        "Run 2:".dimmed(),
        r2.timestamp,
        "🌿",
        r2.branch.as_deref().unwrap_or("—").bright_cyan()
    ).unwrap();
    out.push('\n');

    let health_icon = if d.health.diff() > 0.5 {
        "📈"
    } else if d.health.diff() < -0.5 {
        "📉"
    } else {
        "📊"
    };
    writeln!(out, "  {} {}", health_icon, "── Overview ──".dimmed()).unwrap();
    writeln!(out, "{}", format_delta_row(
        "💚 Health Score",
        &format!("{:.0}", d.health.before),
        &format!("{:.0}", d.health.after),
        d.health.diff(),
        true,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "📁 Files",
        &d.files.before.to_string(),
        &d.files.after.to_string(),
        d.files.diff() as f64,
        false,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "📝 Lines",
        &d.lines.before.to_string(),
        &d.lines.after.to_string(),
        d.lines.diff() as f64,
        false,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "⚡ Total Issues",
        &d.total_issues.before.to_string(),
        &d.total_issues.after.to_string(),
        d.total_issues.diff() as f64,
        false,
    )).unwrap();
    out.push('\n');

    writeln!(out, "  🔍 {}", "── Issues ──".dimmed()).unwrap();
    writeln!(out, "{}", format_delta_row(
        "🔴 Errors",
        &d.errors.before.to_string(),
        &d.errors.after.to_string(),
        d.errors.diff() as f64,
        false,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "🟡 Warnings",
        &d.warnings.before.to_string(),
        &d.warnings.after.to_string(),
        d.warnings.diff() as f64,
        false,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "🔵 Info",
        &d.info.before.to_string(),
        &d.info.after.to_string(),
        d.info.diff() as f64,
        false,
    )).unwrap();
    out.push('\n');

    writeln!(out, "  📐 {}", "── Metrics ──".dimmed()).unwrap();
    writeln!(out, "{}", format_delta_row(
        "🔄 Avg CC",
        &format!("{:.1}", d.avg_cc.before),
        &format!("{:.1}", d.avg_cc.after),
        d.avg_cc.diff(),
        false,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "🔺 Max CC",
        &d.max_cc.before.to_string(),
        &d.max_cc.after.to_string(),
        d.max_cc.diff() as f64,
        false,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "🛡  Avg MI",
        &format!("{:.1}", d.avg_mi.before),
        &format!("{:.1}", d.avg_mi.after),
        d.avg_mi.diff(),
        true,
    )).unwrap();
    writeln!(out, "{}", format_delta_row(
        "🏗  God Files",
        &d.god_files.before.to_string(),
        &d.god_files.after.to_string(),
        d.god_files.diff() as f64,
        false,
    )).unwrap();
    out.push('\n');

    if !d.rules_changed.is_empty() {
        writeln!(out, "  📋 {}", "── Rule Changes (top 10) ──".dimmed()).unwrap();
        for (rule, delta) in d.rules_changed.iter().take(10) {
            let (icon, arrow) = if *delta > 0 {
                ("⬆ ", format!("+{}", delta).red().to_string())
            } else {
                ("⬇ ", format!("{}", delta).green().to_string())
            };
            writeln!(out, "    {} {} {}", icon, arrow, rule.dimmed()).unwrap();
        }
        out.push('\n');
    }

    if !d.rules_added.is_empty() {
        writeln!(out, "  🆕 {} new rule(s) detected", d.rules_added.len()).unwrap();
    }
    if !d.rules_removed.is_empty() {
        writeln!(out, "  ✅ {} rule(s) resolved", d.rules_removed.len()).unwrap();
    }
    out.push('\n');

    out
}

pub fn print_comparison(result: &ComparisonResult) {
    print!("{}", format_comparison(result));
}

fn format_delta_row(label: &str, before: &str, after: &str, diff: f64, higher_is_better: bool) -> String {
    let negligible = is_negligible(diff);

    let arrow = if negligible {
        "─".dimmed().to_string()
    } else if diff > 0.0 {
        if higher_is_better {
            "▲".green().to_string()
        } else {
            "▲".red().to_string()
        }
    } else {
        if higher_is_better {
            "▼".red().to_string()
        } else {
            "▼".green().to_string()
        }
    };

    let diff_str = if negligible {
        "0".dimmed().to_string()
    } else if diff > 0.0 {
        let s = format!("+{:.0}", diff);
        if higher_is_better {
            s.green().to_string()
        } else {
            s.red().to_string()
        }
    } else {
        let s = format!("{:.0}", diff);
        if higher_is_better {
            s.red().to_string()
        } else {
            s.green().to_string()
        }
    };

    format!(
        "    {:<18} {:>8} → {:<8}  {} {}",
        label,
        before.dimmed(),
        after,
        arrow,
        diff_str
    )
}

pub fn generate_html_comparison(result: &ComparisonResult, output: &Path) -> anyhow::Result<()> {
    let html = build_comparison_html(result);
    std::fs::write(output, &html)?;
    println!("Comparison report written to: {}", output.display());
    Ok(())
}

fn is_negligible(diff: f64) -> bool {
    diff.abs() < 0.5 || format!("{:.0}", diff) == "0" || format!("{:.0}", diff) == "-0"
}

fn delta_badge_html(diff: f64, higher_is_better: bool) -> String {
    if is_negligible(diff) {
        return r#"<span class="delta delta-neutral">0</span>"#.to_string();
    }
    let (class, sign) = if diff > 0.0 {
        if higher_is_better {
            ("delta-good", "+")
        } else {
            ("delta-bad", "+")
        }
    } else {
        if higher_is_better {
            ("delta-bad", "")
        } else {
            ("delta-good", "")
        }
    };
    format!(
        r#"<span class="delta {}">{}{:.0}</span>"#,
        class, sign, diff
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_comparison_html(result: &ComparisonResult) -> String {
    let r1 = &result.run1;
    let r2 = &result.run2;
    let d = &result.deltas;

    let health_diff = d.health.diff();
    let health_effectively_same = is_negligible(health_diff);
    let health_color = if health_effectively_same {
        "#6366f1"
    } else if health_diff > 0.0 {
        "#10b981"
    } else {
        "#ef4444"
    };
    let health_direction = if health_effectively_same {
        "Unchanged"
    } else if health_diff > 0.0 {
        "Improved"
    } else {
        "Declined"
    };

    let mut html = String::with_capacity(32 * 1024);
    html.push_str(&format!(r##"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Falcon — Report Comparison</title>
<style>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500&display=swap');
:root {{ --font-sans:'Inter',-apple-system,sans-serif; --font-mono:'JetBrains Mono',monospace; --radius:12px; --radius-sm:8px; --accent:#6366f1; --accent-light:#818cf8; --accent-bg:rgba(99,102,241,.08); }}
[data-theme="dark"] {{ --bg:#0f0f14; --card:#1a1a24; --surface:#1e1e2a; --border:#2a2a3a; --text:#e4e4ef; --text2:#8888a0; --text3:#5c5c72; --ring:#2a2a3a; --good:#10b981; --bad:#ef4444; --warn:#f59e0b; --hover:rgba(99,102,241,.06); }}
[data-theme="light"] {{ --bg:#f5f5f7; --card:#fff; --surface:#f0f0f5; --border:#e2e2ea; --text:#1a1a2e; --text2:#6b6b80; --text3:#9999aa; --ring:#e2e2ea; --good:#059669; --bad:#dc2626; --warn:#d97706; --hover:rgba(99,102,241,.04); }}
*{{margin:0;padding:0;box-sizing:border-box}} html{{font-size:15px}} body{{font-family:var(--font-sans);background:var(--bg);color:var(--text);padding:32px;max-width:960px;margin:0 auto;-webkit-font-smoothing:antialiased}}
h1{{font-size:1.5rem;font-weight:800;margin-bottom:4px;letter-spacing:-.02em}}
.sub{{font-size:.8rem;color:var(--text3);margin-bottom:28px}}
.brand{{display:inline;background:linear-gradient(135deg,#6366f1,#a78bfa);-webkit-background-clip:text;-webkit-text-fill-color:transparent}}

.runs-row{{display:grid;grid-template-columns:1fr auto 1fr;gap:16px;margin-bottom:28px;align-items:stretch}}
.run-card{{background:var(--card);border:1px solid var(--border);border-radius:var(--radius);padding:18px 20px}}
.run-label{{font-size:.68rem;font-weight:600;text-transform:uppercase;letter-spacing:.06em;color:var(--text3);margin-bottom:6px}}
.run-time{{font-size:.95rem;font-weight:600}}
.run-meta{{font-size:.78rem;color:var(--text2);margin-top:4px;font-family:var(--font-mono)}}
.vs-badge{{align-self:center;background:var(--accent-bg);color:var(--accent-light);width:40px;height:40px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-weight:800;font-size:.85rem}}

.section{{background:var(--card);border:1px solid var(--border);border-radius:var(--radius);padding:20px 24px;margin-bottom:20px}}
.section h2{{font-size:.9rem;font-weight:600;margin-bottom:14px;color:var(--text2)}}

.compare-table{{width:100%;border-collapse:collapse}}
.compare-table th{{text-align:left;padding:8px 12px;font-size:.7rem;font-weight:600;text-transform:uppercase;letter-spacing:.06em;color:var(--text3);border-bottom:1px solid var(--border)}}
.compare-table td{{padding:10px 12px;border-bottom:1px solid rgba(42,42,58,.5);font-size:.85rem}}
.compare-table tr:hover td{{background:var(--hover)}}
.compare-table .metric-name{{font-weight:500}}
.compare-table .val{{font-family:var(--font-mono);font-weight:600;text-align:right;min-width:70px}}
.compare-table .arrow-cell{{text-align:center;width:40px;font-size:1rem}}

.delta{{padding:2px 8px;border-radius:10px;font-size:.72rem;font-weight:700;font-family:var(--font-mono)}}
.delta-good{{background:rgba(16,185,129,.12);color:var(--good)}}
.delta-bad{{background:rgba(239,68,68,.1);color:var(--bad)}}
.delta-neutral{{background:rgba(99,102,241,.08);color:var(--text3)}}

.health-hero{{display:flex;align-items:center;gap:24px;margin-bottom:28px;background:var(--card);border:1px solid var(--border);border-radius:var(--radius);padding:24px;border-left:4px solid {hc}}}
.health-delta{{font-size:2.4rem;font-weight:800;color:{hc};letter-spacing:-.03em}}
.health-dir{{font-size:.9rem;font-weight:600;color:{hc}}}
.health-detail{{font-size:.78rem;color:var(--text2);margin-top:2px}}

.rule-row{{display:flex;align-items:center;gap:10px;padding:6px 0;font-size:.82rem}}
.rule-name{{font-family:var(--font-mono);font-size:.75rem;color:var(--text2);flex:1}}
.rule-delta{{font-family:var(--font-mono);font-weight:600;min-width:50px;text-align:right}}
.rule-delta.pos{{color:var(--bad)}} .rule-delta.neg{{color:var(--good)}}
.rule-chip{{padding:2px 8px;border-radius:6px;font-size:.68rem;font-weight:600}}
.rule-chip-new{{background:rgba(245,158,11,.1);color:var(--warn)}}
.rule-chip-gone{{background:rgba(16,185,129,.1);color:var(--good)}}

.footer{{margin-top:32px;padding-top:12px;border-top:1px solid var(--border);font-size:.75rem;color:var(--text3)}}
</style>
</head>
<body>
<h1><span class="brand">Falcon</span> Report Comparison</h1>
<p class="sub">Side-by-side analysis delta between two runs</p>
"##, hc = health_color));

    // Runs header
    html.push_str(&format!(r#"<div class="runs-row">
  <div class="run-card"><div class="run-label">Run 1 (Baseline)</div><div class="run-time">{t1}</div><div class="run-meta">{b1} &bull; {f1} files</div></div>
  <div class="vs-badge">vs</div>
  <div class="run-card"><div class="run-label">Run 2 (Latest)</div><div class="run-time">{t2}</div><div class="run-meta">{b2} &bull; {f2} files</div></div>
</div>"#,
        t1 = esc(&r1.timestamp), b1 = esc(r1.branch.as_deref().unwrap_or("—")), f1 = r1.file_count,
        t2 = esc(&r2.timestamp), b2 = esc(r2.branch.as_deref().unwrap_or("—")), f2 = r2.file_count,
    ));

    // Health hero
    let health_display = if health_effectively_same {
        "0".to_string()
    } else {
        let sign = if health_diff > 0.0 { "+" } else { "" };
        format!("{}{:.0}", sign, health_diff)
    };
    let health_icon = if health_effectively_same {
        "═"
    } else if health_diff > 0.0 {
        "▲"
    } else {
        "▼"
    };
    html.push_str(&format!(r#"<div class="health-hero">
  <div class="health-delta">{icon} {display}</div>
  <div><div class="health-dir">{dir}</div><div class="health-detail">{b:.0} → {a:.0} health score</div></div>
</div>"#,
        icon = health_icon, display = health_display, dir = health_direction,
        b = d.health.before, a = d.health.after,
    ));

    // Overview table
    html.push_str(r#"<div class="section"><h2>Overview</h2><table class="compare-table"><thead><tr><th>Metric</th><th class="val">Run 1</th><th class="arrow-cell"></th><th class="val">Run 2</th><th>Delta</th></tr></thead><tbody>"#);
    compare_row(
        &mut html,
        "Files",
        &d.files.before.to_string(),
        &d.files.after.to_string(),
        d.files.diff() as f64,
        false,
    );
    compare_row(
        &mut html,
        "Lines of Code",
        &d.lines.before.to_string(),
        &d.lines.after.to_string(),
        d.lines.diff() as f64,
        false,
    );
    compare_row(
        &mut html,
        "Health Score",
        &format!("{:.0}", d.health.before),
        &format!("{:.0}", d.health.after),
        d.health.diff(),
        true,
    );
    compare_row(
        &mut html,
        "Total Issues",
        &d.total_issues.before.to_string(),
        &d.total_issues.after.to_string(),
        d.total_issues.diff() as f64,
        false,
    );
    html.push_str("</tbody></table></div>");

    // Issues table
    html.push_str(r#"<div class="section"><h2>Issues Breakdown</h2><table class="compare-table"><thead><tr><th>Severity</th><th class="val">Run 1</th><th class="arrow-cell"></th><th class="val">Run 2</th><th>Delta</th></tr></thead><tbody>"#);
    compare_row(
        &mut html,
        "Errors",
        &d.errors.before.to_string(),
        &d.errors.after.to_string(),
        d.errors.diff() as f64,
        false,
    );
    compare_row(
        &mut html,
        "Warnings",
        &d.warnings.before.to_string(),
        &d.warnings.after.to_string(),
        d.warnings.diff() as f64,
        false,
    );
    compare_row(
        &mut html,
        "Info",
        &d.info.before.to_string(),
        &d.info.after.to_string(),
        d.info.diff() as f64,
        false,
    );
    html.push_str("</tbody></table></div>");

    // Metrics table
    html.push_str(r#"<div class="section"><h2>Metrics</h2><table class="compare-table"><thead><tr><th>Metric</th><th class="val">Run 1</th><th class="arrow-cell"></th><th class="val">Run 2</th><th>Delta</th></tr></thead><tbody>"#);
    compare_row(
        &mut html,
        "Avg Cyclomatic Complexity",
        &format!("{:.1}", d.avg_cc.before),
        &format!("{:.1}", d.avg_cc.after),
        d.avg_cc.diff(),
        false,
    );
    compare_row(
        &mut html,
        "Max Cyclomatic Complexity",
        &d.max_cc.before.to_string(),
        &d.max_cc.after.to_string(),
        d.max_cc.diff() as f64,
        false,
    );
    compare_row(
        &mut html,
        "Avg Maintainability Index",
        &format!("{:.1}", d.avg_mi.before),
        &format!("{:.1}", d.avg_mi.after),
        d.avg_mi.diff(),
        true,
    );
    compare_row(
        &mut html,
        "God Files (>500 LOC)",
        &d.god_files.before.to_string(),
        &d.god_files.after.to_string(),
        d.god_files.diff() as f64,
        false,
    );
    html.push_str("</tbody></table></div>");

    // Rule changes
    if !d.rules_changed.is_empty() || !d.rules_added.is_empty() || !d.rules_removed.is_empty() {
        html.push_str(r#"<div class="section"><h2>Rule Changes</h2>"#);

        if !d.rules_changed.is_empty() {
            for (rule, delta) in d.rules_changed.iter().take(15) {
                let cls = if *delta > 0 { "pos" } else { "neg" };
                let sign = if *delta > 0 { "+" } else { "" };
                html.push_str(&format!(
                    r#"<div class="rule-row"><span class="rule-name">{rule}</span><span class="rule-delta {cls}">{sign}{delta}</span></div>"#,
                    rule = esc(rule), cls = cls, sign = sign, delta = delta
                ));
            }
        }

        if !d.rules_added.is_empty() {
            for (rule, count) in d.rules_added.iter().take(10) {
                html.push_str(&format!(
                    r#"<div class="rule-row"><span class="rule-chip rule-chip-new">NEW</span><span class="rule-name">{rule}</span><span class="rule-delta pos">+{count}</span></div>"#,
                    rule = esc(rule), count = count
                ));
            }
        }

        if !d.rules_removed.is_empty() {
            for (rule, count) in d.rules_removed.iter().take(10) {
                html.push_str(&format!(
                    r#"<div class="rule-row"><span class="rule-chip rule-chip-gone">RESOLVED</span><span class="rule-name">{rule}</span><span class="rule-delta neg">-{count}</span></div>"#,
                    rule = esc(rule), count = count
                ));
            }
        }

        html.push_str("</div>");
    }

    html.push_str(&format!(
        r#"<div class="footer">Generated by Falcon v{} &mdash; Rust-powered static analysis for Flutter &amp; Dart</div>"#,
        env!("CARGO_PKG_VERSION")
    ));
    html.push_str("</body></html>");
    html
}

fn compare_row(
    html: &mut String,
    label: &str,
    before: &str,
    after: &str,
    diff: f64,
    higher_is_better: bool,
) {
    let arrow = if is_negligible(diff) { "─" } else { "→" };
    let badge = delta_badge_html(diff, higher_is_better);
    html.push_str(&format!(
        r#"<tr><td class="metric-name">{label}</td><td class="val">{before}</td><td class="arrow-cell">{arrow}</td><td class="val">{after}</td><td>{badge}</td></tr>"#,
        label = label, before = before, after = after, arrow = arrow, badge = badge
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::snapshot::{AnalysisSnapshot, IssueSummary, MetricsSummary};
    use std::collections::HashMap;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn disable_colors() {
        INIT.call_once(|| {
            colored::control::set_override(false);
        });
    }

    fn sample_snapshot() -> AnalysisSnapshot {
        AnalysisSnapshot {
            timestamp: "2026-05-25T00:00:00Z".to_string(),
            commit_hash: Some("abc123".to_string()),
            commit_message: Some("test commit".to_string()),
            branch: Some("main".to_string()),
            file_count: 10,
            total_lines: 1000,
            health_score: 80.0,
            issues: IssueSummary {
                errors: 1,
                warnings: 2,
                info: 3,
                total: 6,
            },
            metrics_summary: MetricsSummary {
                avg_cyclomatic: 3.5,
                max_cyclomatic: 10,
                avg_maintainability: 70.0,
                avg_lines_per_file: 100.0,
                god_file_count: 1,
            },
            rule_counts: HashMap::new(),
            per_package: vec![],
        }
    }

    // --- Delta<usize> tests ---

    #[test]
    fn delta_usize_diff_positive_for_increase() {
        let d = Delta { before: 5usize, after: 10usize };
        assert_eq!(d.diff(), 5i64);
    }

    #[test]
    fn delta_usize_diff_negative_for_decrease() {
        let d = Delta { before: 10usize, after: 3usize };
        assert_eq!(d.diff(), -7i64);
    }

    #[test]
    fn delta_usize_diff_zero_for_unchanged() {
        let d = Delta { before: 7usize, after: 7usize };
        assert_eq!(d.diff(), 0i64);
    }

    // --- Delta<f64> tests ---

    #[test]
    fn delta_f64_diff_returns_difference() {
        let d = Delta { before: 1.0f64, after: 3.5f64 };
        assert!((d.diff() - 2.5).abs() < 1e-10);
    }

    // --- Delta<u32> tests ---

    #[test]
    fn delta_u32_diff_negative_for_decrease() {
        let d = Delta { before: 10u32, after: 4u32 };
        assert_eq!(d.diff(), -6i64);
    }

    // --- compare_snapshots tests ---

    #[test]
    fn compare_snapshots_finds_rules_added() {
        let run1 = sample_snapshot();
        let mut run2 = sample_snapshot();
        run2.rule_counts.insert("rule-a".to_string(), 3);
        let result = compare_snapshots(&run1, &run2);
        assert!(
            result.deltas.rules_added.iter().any(|(r, c)| r == "rule-a" && *c == 3),
            "rules_added should contain (\"rule-a\", 3)"
        );
    }

    #[test]
    fn compare_snapshots_finds_rules_removed() {
        let mut run1 = sample_snapshot();
        run1.rule_counts.insert("rule-x".to_string(), 5);
        let run2 = sample_snapshot();
        let result = compare_snapshots(&run1, &run2);
        assert!(
            result.deltas.rules_removed.iter().any(|(r, c)| r == "rule-x" && *c == 5),
            "rules_removed should contain (\"rule-x\", 5)"
        );
    }

    #[test]
    fn compare_snapshots_finds_rules_changed() {
        let mut run1 = sample_snapshot();
        run1.rule_counts.insert("rule-y".to_string(), 2);
        let mut run2 = sample_snapshot();
        run2.rule_counts.insert("rule-y".to_string(), 7);
        let result = compare_snapshots(&run1, &run2);
        assert!(
            result.deltas.rules_changed.iter().any(|(r, d)| r == "rule-y" && *d == 5),
            "rules_changed should contain (\"rule-y\", 5)"
        );
    }

    #[test]
    fn compare_snapshots_unchanged_rules_omitted() {
        let mut run1 = sample_snapshot();
        run1.rule_counts.insert("rule-z".to_string(), 4);
        let mut run2 = sample_snapshot();
        run2.rule_counts.insert("rule-z".to_string(), 4);
        let result = compare_snapshots(&run1, &run2);
        assert!(
            !result.deltas.rules_added.iter().any(|(r, _)| r == "rule-z"),
            "rule-z should not be in rules_added"
        );
        assert!(
            !result.deltas.rules_removed.iter().any(|(r, _)| r == "rule-z"),
            "rule-z should not be in rules_removed"
        );
        assert!(
            !result.deltas.rules_changed.iter().any(|(r, _)| r == "rule-z"),
            "rule-z should not be in rules_changed"
        );
    }

    #[test]
    fn compare_snapshots_rules_changed_sorted_by_abs_value() {
        let mut run1 = sample_snapshot();
        run1.rule_counts.insert("rule-a".to_string(), 10);
        run1.rule_counts.insert("rule-b".to_string(), 5);
        run1.rule_counts.insert("rule-c".to_string(), 1);
        let mut run2 = sample_snapshot();
        // rule-a: 10 -> 11 => delta +1 (abs=1)
        run2.rule_counts.insert("rule-a".to_string(), 11);
        // rule-b: 5 -> 15 => delta +10 (abs=10)
        run2.rule_counts.insert("rule-b".to_string(), 15);
        // rule-c: 1 -> 6 => delta +5 (abs=5)
        run2.rule_counts.insert("rule-c".to_string(), 6);
        let result = compare_snapshots(&run1, &run2);
        let changed = &result.deltas.rules_changed;
        assert_eq!(changed.len(), 3);
        // sorted by abs descending: rule-b(10), rule-c(5), rule-a(1)
        assert_eq!(changed[0].0, "rule-b");
        assert_eq!(changed[0].1, 10i64);
        assert_eq!(changed[1].0, "rule-c");
        assert_eq!(changed[1].1, 5i64);
        assert_eq!(changed[2].0, "rule-a");
        assert_eq!(changed[2].1, 1i64);
    }

    #[test]
    fn compare_snapshots_carries_through_summary_fields() {
        let run1 = sample_snapshot();
        let mut run2 = sample_snapshot();
        run2.file_count = 20;
        run2.total_lines = 2000;
        run2.health_score = 90.0;
        run2.issues.errors = 5;
        run2.issues.warnings = 6;
        run2.issues.info = 7;
        let result = compare_snapshots(&run1, &run2);
        assert_eq!(result.deltas.files.before, 10);
        assert_eq!(result.deltas.files.after, 20);
        assert_eq!(result.deltas.lines.before, 1000);
        assert_eq!(result.deltas.lines.after, 2000);
        assert!((result.deltas.health.before - 80.0).abs() < 1e-10);
        assert!((result.deltas.health.after - 90.0).abs() < 1e-10);
        assert_eq!(result.deltas.errors.before, 1);
        assert_eq!(result.deltas.errors.after, 5);
        assert_eq!(result.deltas.warnings.before, 2);
        assert_eq!(result.deltas.warnings.after, 6);
        assert_eq!(result.deltas.info.before, 3);
        assert_eq!(result.deltas.info.after, 7);
    }

    #[test]
    fn compare_snapshots_files_delta() {
        let run1 = sample_snapshot();
        let mut run2 = sample_snapshot();
        run2.file_count = 15;
        let result = compare_snapshots(&run1, &run2);
        assert_eq!(result.deltas.files.before, 10);
        assert_eq!(result.deltas.files.after, 15);
    }

    #[test]
    fn compare_snapshots_lines_delta() {
        let run1 = sample_snapshot();
        let mut run2 = sample_snapshot();
        run2.total_lines = 1500;
        let result = compare_snapshots(&run1, &run2);
        assert_eq!(result.deltas.lines.before, 1000);
        assert_eq!(result.deltas.lines.after, 1500);
    }

    #[test]
    fn compare_snapshots_health_delta() {
        let run1 = sample_snapshot();
        let mut run2 = sample_snapshot();
        run2.health_score = 92.5;
        let result = compare_snapshots(&run1, &run2);
        assert!((result.deltas.health.before - 80.0).abs() < 1e-10);
        assert!((result.deltas.health.after - 92.5).abs() < 1e-10);
    }

    #[test]
    fn compare_snapshots_errors_warnings_info_delta() {
        let run1 = sample_snapshot();
        let mut run2 = sample_snapshot();
        run2.issues.errors = 10;
        run2.issues.warnings = 20;
        run2.issues.info = 30;
        let result = compare_snapshots(&run1, &run2);
        assert_eq!(result.deltas.errors.before, 1);
        assert_eq!(result.deltas.errors.after, 10);
        assert_eq!(result.deltas.warnings.before, 2);
        assert_eq!(result.deltas.warnings.after, 20);
        assert_eq!(result.deltas.info.before, 3);
        assert_eq!(result.deltas.info.after, 30);
    }

    #[test]
    fn compare_snapshots_metrics_summary_deltas() {
        let run1 = sample_snapshot();
        let mut run2 = sample_snapshot();
        run2.metrics_summary.avg_cyclomatic = 5.0;
        run2.metrics_summary.max_cyclomatic = 20;
        run2.metrics_summary.avg_maintainability = 85.0;
        run2.metrics_summary.god_file_count = 3;
        let result = compare_snapshots(&run1, &run2);
        assert!((result.deltas.avg_cc.before - 3.5).abs() < 1e-10);
        assert!((result.deltas.avg_cc.after - 5.0).abs() < 1e-10);
        assert_eq!(result.deltas.max_cc.before, 10u32);
        assert_eq!(result.deltas.max_cc.after, 20u32);
        assert!((result.deltas.avg_mi.before - 70.0).abs() < 1e-10);
        assert!((result.deltas.avg_mi.after - 85.0).abs() < 1e-10);
        assert_eq!(result.deltas.god_files.before, 1);
        assert_eq!(result.deltas.god_files.after, 3);
    }

    // --- is_negligible tests ---
    // Implementation: diff.abs() < 0.5 || format!("{:.0}", diff) == "0" || format!("{:.0}", diff) == "-0"
    // Note: 0.05.abs() < 0.5 => true; 1.0.abs() < 0.5 => false; -0.01.abs() < 0.5 => true

    #[test]
    fn is_negligible_returns_true_for_small_diff() {
        // 0.05 < 0.5 => negligible
        assert!(is_negligible(0.05));
    }

    #[test]
    fn is_negligible_returns_false_for_larger_diff() {
        // 1.0 >= 0.5 and format is "1" => not negligible
        assert!(!is_negligible(1.0));
    }

    #[test]
    fn is_negligible_handles_negative() {
        // -0.01.abs() = 0.01 < 0.5 => negligible
        assert!(is_negligible(-0.01));
    }

    // --- delta_badge_html tests ---

    #[test]
    fn delta_badge_html_for_positive_diff_higher_is_better() {
        disable_colors();
        let html = delta_badge_html(5.0, true);
        // diff=5.0 is not negligible; diff>0 and higher_is_better => delta-good
        assert!(html.contains("delta-good"), "Expected delta-good class, got: {}", html);
        assert!(html.contains("+5"), "Expected +5 in output, got: {}", html);
    }

    #[test]
    fn delta_badge_html_for_negative_diff_higher_is_better() {
        disable_colors();
        let html = delta_badge_html(-5.0, true);
        // diff=-5.0 is not negligible; diff<0 and higher_is_better => delta-bad
        assert!(html.contains("delta-bad"), "Expected delta-bad class, got: {}", html);
        assert!(html.contains("-5"), "Expected -5 in output, got: {}", html);
    }

    #[test]
    fn delta_badge_html_for_positive_diff_lower_is_better() {
        disable_colors();
        let html = delta_badge_html(5.0, false);
        // diff=5.0 is not negligible; diff>0 and !higher_is_better => delta-bad
        assert!(html.contains("delta-bad"), "Expected delta-bad class (positive is bad), got: {}", html);
        assert!(html.contains("+5"), "Expected +5 in output, got: {}", html);
    }

    #[test]
    fn delta_badge_html_for_negligible_returns_neutral() {
        disable_colors();
        // 0.0 is negligible (abs < 0.5)
        let html = delta_badge_html(0.0, true);
        assert!(html.contains("delta-neutral"), "Expected delta-neutral class, got: {}", html);
        // The neutral badge shows "0"
        assert!(html.contains(">0<"), "Expected literal 0 in span, got: {}", html);
    }

    // --- esc tests ---

    #[test]
    fn esc_replaces_lt() {
        assert_eq!(esc("a<b"), "a&lt;b");
    }

    #[test]
    fn esc_replaces_gt() {
        assert_eq!(esc("a>b"), "a&gt;b");
    }

    #[test]
    fn esc_replaces_amp() {
        assert_eq!(esc("a&b"), "a&amp;b");
    }

    #[test]
    fn esc_replaces_quote_if_implemented() {
        // Implementation replaces '"' with "&quot;"
        assert_eq!(esc("a\"b"), "a&quot;b");
    }

    #[test]
    fn esc_passes_through_safe_chars() {
        assert_eq!(esc("hello world"), "hello world");
    }

    // --- build_comparison_html tests ---

    #[test]
    fn build_comparison_html_contains_doctype_and_html_tags() {
        let run1 = sample_snapshot();
        let run2 = sample_snapshot();
        let result = compare_snapshots(&run1, &run2);
        let html = build_comparison_html(&result);
        assert!(html.contains("<!DOCTYPE html>"), "Expected DOCTYPE, got beginning: {}", &html[..100.min(html.len())]);
        assert!(html.contains("<html"), "Expected <html tag");
    }

    #[test]
    fn build_comparison_html_includes_run_metadata() {
        let mut run1 = sample_snapshot();
        run1.commit_hash = Some("abc111".to_string());
        run1.branch = Some("feature-x".to_string());
        let mut run2 = sample_snapshot();
        run2.commit_hash = Some("def222".to_string());
        run2.branch = Some("main".to_string());
        let result = compare_snapshots(&run1, &run2);
        let html = build_comparison_html(&result);
        // Branch names appear in the runs-row section
        assert!(html.contains("feature-x"), "Expected run1 branch in HTML");
        assert!(html.contains("main"), "Expected run2 branch in HTML");
    }

    #[test]
    fn build_comparison_html_includes_file_count_delta() {
        let run1 = sample_snapshot(); // file_count=10
        let mut run2 = sample_snapshot();
        run2.file_count = 15;
        let result = compare_snapshots(&run1, &run2);
        let html = build_comparison_html(&result);
        assert!(html.contains("10"), "Expected run1.file_count=10 in HTML");
        assert!(html.contains("15"), "Expected run2.file_count=15 in HTML");
    }

    #[test]
    fn build_comparison_html_includes_health_score_section() {
        let run1 = sample_snapshot();
        let run2 = sample_snapshot();
        let result = compare_snapshots(&run1, &run2);
        let html = build_comparison_html(&result);
        assert!(
            html.contains("Health") || html.contains("health"),
            "Expected 'Health' in output, got snippet: {}",
            &html[..200.min(html.len())]
        );
    }

    #[test]
    fn build_comparison_html_lists_rules_changed() {
        let mut run1 = sample_snapshot();
        run1.rule_counts.insert("my-special-rule".to_string(), 2);
        let mut run2 = sample_snapshot();
        run2.rule_counts.insert("my-special-rule".to_string(), 9);
        let result = compare_snapshots(&run1, &run2);
        let html = build_comparison_html(&result);
        assert!(html.contains("my-special-rule"), "Expected rule name in HTML output");
    }

    // --- compare_row tests ---

    #[test]
    fn compare_row_basic_output() {
        disable_colors();
        let mut html = String::new();
        compare_row(&mut html, "Files", "10", "15", 5.0, false);
        assert!(html.contains("Files"), "Expected label 'Files'");
        assert!(html.contains("10"), "Expected before value '10'");
        assert!(html.contains("15"), "Expected after value '15'");
        // diff=5.0, not negligible => arrow should be "→"
        assert!(html.contains("→"), "Expected → arrow for non-negligible diff");
        // diff=5.0, lower_is_better => delta-bad badge
        assert!(html.contains("delta-bad"), "Expected delta-bad badge for positive diff with lower_is_better");
    }

    #[test]
    fn compare_row_negligible_uses_dash_arrow() {
        disable_colors();
        let mut html = String::new();
        // diff=0.0 is negligible
        compare_row(&mut html, "Health", "80", "80", 0.0, true);
        assert!(html.contains("─"), "Expected ─ arrow for negligible diff");
        assert!(html.contains("delta-neutral"), "Expected delta-neutral badge for negligible diff");
    }

    // --- format_delta_row tests ---

    #[test]
    fn format_delta_row_label_appears_in_output() {
        disable_colors();
        let row = format_delta_row("MyLabel", "10", "20", 10.0, false);
        assert!(row.contains("MyLabel"), "Expected label in row output");
    }

    #[test]
    fn format_delta_row_before_and_after_values_appear() {
        disable_colors();
        let row = format_delta_row("Files", "100", "200", 100.0, false);
        assert!(row.contains("100"), "Expected before value in row");
        assert!(row.contains("200"), "Expected after value in row");
    }

    #[test]
    fn format_delta_row_improvement_higher_is_better_true() {
        disable_colors();
        // positive diff with higher_is_better=true => improvement => diff shown as +N (no color in test)
        let row = format_delta_row("Health", "70", "80", 10.0, true);
        assert!(row.contains("+10"), "Expected +10 in improvement row");
        // Arrow should be ▲ (upward)
        assert!(row.contains("▲"), "Expected ▲ arrow for positive diff");
    }

    #[test]
    fn format_delta_row_regression_higher_is_better_true() {
        disable_colors();
        // negative diff with higher_is_better=true => regression
        let row = format_delta_row("Health", "80", "70", -10.0, true);
        assert!(row.contains("-10"), "Expected -10 in regression row");
        assert!(row.contains("▼"), "Expected ▼ arrow for negative diff");
    }

    #[test]
    fn format_delta_row_regression_higher_is_better_false() {
        disable_colors();
        // positive diff with higher_is_better=false => regression (more issues = bad)
        let row = format_delta_row("Issues", "5", "15", 10.0, false);
        assert!(row.contains("+10"), "Expected +10 in regression row (lower_is_better)");
        assert!(row.contains("▲"), "Expected ▲ arrow for positive diff");
    }

    #[test]
    fn format_delta_row_improvement_higher_is_better_false() {
        disable_colors();
        // negative diff with higher_is_better=false => improvement (fewer issues = good)
        let row = format_delta_row("Issues", "15", "5", -10.0, false);
        assert!(row.contains("-10"), "Expected -10 in improvement row (lower_is_better)");
        assert!(row.contains("▼"), "Expected ▼ arrow for negative diff");
    }

    #[test]
    fn format_delta_row_negligible_diff_renders_dash_arrow() {
        disable_colors();
        let row = format_delta_row("Score", "80", "80", 0.0, true);
        assert!(row.contains("─"), "Expected ─ dash arrow for negligible diff");
        // diff renders as 0 not +0
        assert!(row.contains('0'), "Expected 0 in negligible diff row");
    }

    #[test]
    fn format_delta_row_no_trailing_newline() {
        disable_colors();
        let row = format_delta_row("Test", "1", "2", 1.0, true);
        assert!(!row.ends_with('\n'), "format_delta_row must NOT have trailing newline");
    }

    // --- format_comparison tests ---

    #[test]
    fn format_comparison_empty_deltas_contains_section_headers() {
        disable_colors();
        let run1 = sample_snapshot();
        let run2 = sample_snapshot();
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        assert!(out.contains("Overview"), "Expected Overview section header");
        assert!(out.contains("Issues"), "Expected Issues section header");
        assert!(out.contains("Metrics"), "Expected Metrics section header");
    }

    #[test]
    fn format_comparison_contains_run1_and_run2_metadata() {
        disable_colors();
        let mut run1 = sample_snapshot();
        run1.branch = Some("branch-one".to_string());
        run1.timestamp = "2026-01-01T00:00:00Z".to_string();
        let mut run2 = sample_snapshot();
        run2.branch = Some("branch-two".to_string());
        run2.timestamp = "2026-02-01T00:00:00Z".to_string();
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        assert!(out.contains("branch-one"), "Expected run1 branch in output");
        assert!(out.contains("branch-two"), "Expected run2 branch in output");
        assert!(out.contains("2026-01-01"), "Expected run1 timestamp in output");
        assert!(out.contains("2026-02-01"), "Expected run2 timestamp in output");
    }

    #[test]
    fn format_comparison_with_rule_changes() {
        disable_colors();
        let mut run1 = sample_snapshot();
        run1.rule_counts.insert("avoid_print".to_string(), 3);
        let mut run2 = sample_snapshot();
        run2.rule_counts.insert("avoid_print".to_string(), 8);
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        assert!(out.contains("avoid_print"), "Expected changed rule name in output");
        assert!(out.contains("Rule Changes"), "Expected Rule Changes section header");
    }

    #[test]
    fn format_comparison_with_file_count_changes() {
        disable_colors();
        let run1 = sample_snapshot(); // file_count=10
        let mut run2 = sample_snapshot();
        run2.file_count = 25;
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        assert!(out.contains("10"), "Expected run1 file count in output");
        assert!(out.contains("25"), "Expected run2 file count in output");
    }

    #[test]
    fn format_comparison_large_positive_health_delta() {
        disable_colors();
        let mut run1 = sample_snapshot();
        run1.health_score = 20.0;
        let mut run2 = sample_snapshot();
        run2.health_score = 95.0;
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        // Should show 📈 since health improved > 0.5
        assert!(out.contains("📈"), "Expected 📈 icon for large positive health delta");
    }

    #[test]
    fn format_comparison_large_negative_health_delta() {
        disable_colors();
        let mut run1 = sample_snapshot();
        run1.health_score = 90.0;
        let mut run2 = sample_snapshot();
        run2.health_score = 20.0;
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        // Should show 📉 since health declined > 0.5
        assert!(out.contains("📉"), "Expected 📉 icon for large negative health delta");
    }

    #[test]
    fn format_comparison_with_rules_added_and_removed() {
        disable_colors();
        let mut run1 = sample_snapshot();
        run1.rule_counts.insert("old_rule".to_string(), 5);
        let mut run2 = sample_snapshot();
        run2.rule_counts.insert("new_rule".to_string(), 3);
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        assert!(out.contains("new rule(s) detected"), "Expected new rule count in output");
        assert!(out.contains("rule(s) resolved"), "Expected resolved rule count in output");
    }

    #[test]
    fn format_comparison_ends_with_newline() {
        disable_colors();
        let run1 = sample_snapshot();
        let run2 = sample_snapshot();
        let result = compare_snapshots(&run1, &run2);
        let out = format_comparison(&result);
        assert!(out.ends_with('\n'), "format_comparison output must end with newline");
    }
}

pub fn list_history(root: &Path) -> anyhow::Result<()> {
    let history = super::snapshot::load_history(root)?;
    if history.is_empty() {
        println!(
            "  📭 No analysis history found. Run {} to start recording.",
            "falcon analyze".bright_blue()
        );
        return Ok(());
    }

    println!();
    println!(
        "  🦅 {} {}",
        "falcon".bright_blue().bold(),
        "Analysis History".bold()
    );
    println!("  {} {} run(s) stored", "📦".dimmed(), history.len());
    println!();
    println!(
        "  {:<4} {:<22}  {:<12} {:>7} {:>7} {:>8} {:>8}",
        "#".dimmed(),
        "🕐 Timestamp".dimmed(),
        "🌿 Branch".dimmed(),
        "📁".dimmed(),
        "💚".dimmed(),
        "⚡".dimmed(),
        "🔗".dimmed()
    );
    println!("  {}", "─".repeat(80).dimmed());

    for (i, snap) in history.iter().enumerate().rev() {
        let branch = snap.branch.as_deref().unwrap_or("—");
        let branch_display = if branch.len() > 10 {
            &branch[..10]
        } else {
            branch
        };
        let commit = snap.commit_hash.as_deref().unwrap_or("—");
        let health_colored = if snap.health_score >= 80.0 {
            format!("{:.0}", snap.health_score).green().to_string()
        } else if snap.health_score >= 50.0 {
            format!("{:.0}", snap.health_score).yellow().to_string()
        } else {
            format!("{:.0}", snap.health_score).red().to_string()
        };

        println!(
            "  {:<4} {:<22}  {:<12} {:>7} {:>7} {:>8} {:>8}",
            format!("#{}", i + 1).dimmed(),
            &snap.timestamp,
            branch_display,
            snap.file_count,
            health_colored,
            snap.issues.total,
            commit
        );
    }
    println!();
    println!(
        "  💡 {} Use {} to compare two runs.",
        "tip:".dimmed(),
        "falcon compare-reports <path> --run1 N --run2 M".bright_blue()
    );
    println!(
        "     {} Use {} to compare branches.",
        "   ".dimmed(),
        "falcon compare-branches <path> --base main --branch dev".bright_blue()
    );
    println!();
    Ok(())
}

fn current_branch(root: &Path) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn has_uncommitted(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["diff", "--quiet", "HEAD"])
        .current_dir(root)
        .status()
        .map(|s| !s.success())
        .unwrap_or(false)
}

fn checkout(root: &Path, branch: &str) -> anyhow::Result<()> {
    let status = std::process::Command::new("git")
        .args(["checkout", branch])
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to checkout branch '{}'", branch);
    }
    Ok(())
}

fn stash_save(root: &Path) -> anyhow::Result<bool> {
    if !has_uncommitted(root) {
        return Ok(false);
    }
    let status = std::process::Command::new("git")
        .args(["stash", "push", "-m", "falcon-compare-branches"])
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(status.success())
}

fn stash_pop(root: &Path) {
    let _ = std::process::Command::new("git")
        .args(["stash", "pop"])
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn analyze_current(root: &Path, config: &FalconConfig) -> anyhow::Result<AnalysisSnapshot> {
    let falcon = Falcon::new(config.clone())?;
    let report = falcon.analyze(root)?;
    Ok(AnalysisSnapshot::capture(&report, root))
}

pub fn compare_branches(
    root: &Path,
    base: &str,
    branch: &str,
    config: &FalconConfig,
    html_out: &Path,
) -> anyhow::Result<()> {
    let original_branch = current_branch(root)?;
    let had_stash = stash_save(root)?;

    let restore = |root: &Path, orig: &str, stashed: bool| {
        let _ = checkout(root, orig);
        if stashed {
            stash_pop(root);
        }
    };

    // Analyze base branch
    println!("  🌿 Checking out base branch: {}", base.bright_cyan());
    checkout(root, base).map_err(|e| {
        restore(root, &original_branch, had_stash);
        e
    })?;
    println!("  🔬 Analyzing {}...", base.bright_cyan());
    let snap_base = analyze_current(root, config).map_err(|e| {
        restore(root, &original_branch, had_stash);
        e
    })?;
    println!(
        "    ✅ {} 📁 {} files  ⚡ {} issues  💚 {:.0} health",
        base.bright_cyan(),
        snap_base.file_count,
        snap_base.issues.total,
        snap_base.health_score
    );

    // Save base branch snapshot to history
    if let Err(e) = super::snapshot::save_snapshot(root, &snap_base) {
        log::debug!("Could not save base snapshot: {}", e);
    }

    // Analyze target branch
    println!("  🌿 Checking out branch: {}", branch.bright_cyan());
    checkout(root, branch).map_err(|e| {
        restore(root, &original_branch, had_stash);
        e
    })?;
    println!("  🔬 Analyzing {}...", branch.bright_cyan());
    let snap_branch = analyze_current(root, config).map_err(|e| {
        restore(root, &original_branch, had_stash);
        e
    })?;
    println!(
        "    ✅ {} 📁 {} files  ⚡ {} issues  💚 {:.0} health",
        branch.bright_cyan(),
        snap_branch.file_count,
        snap_branch.issues.total,
        snap_branch.health_score
    );

    // Save target branch snapshot to history
    if let Err(e) = super::snapshot::save_snapshot(root, &snap_branch) {
        log::debug!("Could not save branch snapshot: {}", e);
    }

    // Restore original branch
    restore(root, &original_branch, had_stash);
    println!("  🔄 Restored to branch: {}", original_branch.bright_cyan());
    println!();

    // Compare
    let result = compare_snapshots(&snap_base, &snap_branch);
    print_comparison(&result);
    generate_html_comparison(&result, html_out)?;

    println!(
        "  💾 Both snapshots saved to history. Use {} to view.",
        "falcon history".bright_blue()
    );
    println!();

    Ok(())
}
