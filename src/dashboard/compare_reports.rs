use super::snapshot::AnalysisSnapshot;
use crate::config::FalconConfig;
use crate::Falcon;
use colored::Colorize;
use std::cmp::Reverse;
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

    rules_changed.sort_by_key(|(_, count)| Reverse(count.abs()));
    rules_added.sort_by_key(|(_, count)| Reverse(*count));
    rules_removed.sort_by_key(|(_, count)| Reverse(*count));

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

pub fn print_comparison(result: &ComparisonResult) {
    let r1 = &result.run1;
    let r2 = &result.run2;
    let d = &result.deltas;

    println!();
    println!(
        "  🦅 {} {}",
        "falcon".bright_blue().bold(),
        "Report Comparison".bold()
    );
    println!();
    println!(
        "  🏷  {} {} (🌿 {})",
        "Run 1:".dimmed(),
        r1.timestamp,
        r1.branch.as_deref().unwrap_or("—").bright_cyan()
    );
    println!(
        "  🏷  {} {} (🌿 {})",
        "Run 2:".dimmed(),
        r2.timestamp,
        r2.branch.as_deref().unwrap_or("—").bright_cyan()
    );
    println!();

    let health_icon = if d.health.diff() > 0.5 {
        "📈"
    } else if d.health.diff() < -0.5 {
        "📉"
    } else {
        "📊"
    };
    println!("  {} {}", health_icon, "── Overview ──".dimmed());
    print_delta_row(
        "💚 Health Score",
        &format!("{:.0}", d.health.before),
        &format!("{:.0}", d.health.after),
        d.health.diff(),
        true,
    );
    print_delta_row(
        "📁 Files",
        &d.files.before.to_string(),
        &d.files.after.to_string(),
        d.files.diff() as f64,
        false,
    );
    print_delta_row(
        "📝 Lines",
        &d.lines.before.to_string(),
        &d.lines.after.to_string(),
        d.lines.diff() as f64,
        false,
    );
    print_delta_row(
        "⚡ Total Issues",
        &d.total_issues.before.to_string(),
        &d.total_issues.after.to_string(),
        d.total_issues.diff() as f64,
        false,
    );
    println!();

    println!("  🔍 {}", "── Issues ──".dimmed());
    print_delta_row(
        "🔴 Errors",
        &d.errors.before.to_string(),
        &d.errors.after.to_string(),
        d.errors.diff() as f64,
        false,
    );
    print_delta_row(
        "🟡 Warnings",
        &d.warnings.before.to_string(),
        &d.warnings.after.to_string(),
        d.warnings.diff() as f64,
        false,
    );
    print_delta_row(
        "🔵 Info",
        &d.info.before.to_string(),
        &d.info.after.to_string(),
        d.info.diff() as f64,
        false,
    );
    println!();

    println!("  📐 {}", "── Metrics ──".dimmed());
    print_delta_row(
        "🔄 Avg CC",
        &format!("{:.1}", d.avg_cc.before),
        &format!("{:.1}", d.avg_cc.after),
        d.avg_cc.diff(),
        false,
    );
    print_delta_row(
        "🔺 Max CC",
        &d.max_cc.before.to_string(),
        &d.max_cc.after.to_string(),
        d.max_cc.diff() as f64,
        false,
    );
    print_delta_row(
        "🛡  Avg MI",
        &format!("{:.1}", d.avg_mi.before),
        &format!("{:.1}", d.avg_mi.after),
        d.avg_mi.diff(),
        true,
    );
    print_delta_row(
        "🏗  God Files",
        &d.god_files.before.to_string(),
        &d.god_files.after.to_string(),
        d.god_files.diff() as f64,
        false,
    );
    println!();

    if !d.rules_changed.is_empty() {
        println!("  📋 {}", "── Rule Changes (top 10) ──".dimmed());
        for (rule, delta) in d.rules_changed.iter().take(10) {
            let (icon, arrow) = if *delta > 0 {
                ("⬆ ", format!("+{}", delta).red().to_string())
            } else {
                ("⬇ ", format!("{}", delta).green().to_string())
            };
            println!("    {} {} {}", icon, arrow, rule.dimmed());
        }
        println!();
    }

    if !d.rules_added.is_empty() {
        println!("  🆕 {} new rule(s) detected", d.rules_added.len());
    }
    if !d.rules_removed.is_empty() {
        println!("  ✅ {} rule(s) resolved", d.rules_removed.len());
    }
    println!();
}

fn print_delta_row(label: &str, before: &str, after: &str, diff: f64, higher_is_better: bool) {
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

    println!(
        "    {:<18} {:>8} → {:<8}  {} {}",
        label,
        before.dimmed(),
        after,
        arrow,
        diff_str
    );
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

pub fn list_history(root: &Path) -> anyhow::Result<()> {
    let history = super::snapshot::load_history(root)?;
    if history.is_empty() {
        println!(
            "  📭 No analysis history found. Run {} to start recording.",
            "falcon check".bright_blue()
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
        "falcon x compare-reports <path> --run1 N --run2 M".bright_blue()
    );
    println!(
        "     {} Use {} to compare branches.",
        "   ".dimmed(),
        "falcon x compare-branches <path> --base main --branch dev".bright_blue()
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
    checkout(root, base).inspect_err(|_| {
        restore(root, &original_branch, had_stash);
    })?;
    println!("  🔬 Analyzing {}...", base.bright_cyan());
    let snap_base = analyze_current(root, config).inspect_err(|_| {
        restore(root, &original_branch, had_stash);
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
    checkout(root, branch).inspect_err(|_| {
        restore(root, &original_branch, had_stash);
    })?;
    println!("  🔬 Analyzing {}...", branch.bright_cyan());
    let snap_branch = analyze_current(root, config).inspect_err(|_| {
        restore(root, &original_branch, had_stash);
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
