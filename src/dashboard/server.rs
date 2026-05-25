use super::snapshot::{self, AnalysisSnapshot};
use colored::Colorize;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;

/// Start a local web dashboard server.
pub fn start_dashboard(root: &Path, port: u16) -> anyhow::Result<()> {
    let history = snapshot::load_history(root)?;

    if history.is_empty() {
        anyhow::bail!(
            "No analysis history found. Run 'falcon dashboard snapshot' first to capture data."
        );
    }

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;

    println!();
    println!(
        "  {} Dashboard running at {}",
        "falcon".bright_cyan().bold(),
        format!("http://{}", addr).bright_green().underline()
    );
    println!(
        "  {} snapshots loaded. Press Ctrl+C to stop.",
        history.len()
    );
    println!();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let history = snapshot::load_history(root).unwrap_or_default();
                let html = generate_dashboard_html(&history);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = stream.write_all(response.as_bytes());
            }
            Err(e) => {
                log::error!("Connection error: {}", e);
            }
        }
    }

    Ok(())
}

fn generate_dashboard_html(history: &[AnalysisSnapshot]) -> String {
    let latest = history.last();

    let health_data: Vec<String> = history
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{{x:{},y:{:.1}}}", i, s.health_score))
        .collect();

    let issues_data: Vec<String> = history
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{{x:{},y:{}}}", i, s.issues.total))
        .collect();

    let complexity_data: Vec<String> = history
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{{x:{},y:{:.1}}}", i, s.metrics_summary.avg_cyclomatic))
        .collect();

    let labels: Vec<String> = history
        .iter()
        .map(|s| {
            let label = s
                .commit_hash
                .as_deref()
                .unwrap_or(&s.timestamp[..10.min(s.timestamp.len())]);
            format!("\"{}\"", label)
        })
        .collect();

    let (current_health, current_issues, current_files, current_lines) = match latest {
        Some(s) => (
            format!("{:.0}", s.health_score),
            s.issues.total.to_string(),
            s.file_count.to_string(),
            s.total_lines.to_string(),
        ),
        None => ("--".into(), "--".into(), "--".into(), "--".into()),
    };

    let (errors, warnings, info_count) = match latest {
        Some(s) => (s.issues.errors, s.issues.warnings, s.issues.info),
        None => (0, 0, 0),
    };

    let treemap_data = match latest {
        Some(s) => {
            let mut items: Vec<String> = s
                .rule_counts
                .iter()
                .filter(|(_, c)| **c > 0)
                .map(|(rule, count)| format!("{{name:\"{}\",value:{}}}", rule, count))
                .collect();
            items.sort();
            items.join(",")
        }
        None => String::new(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Falcon Dashboard</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4"></script>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#0f1117;color:#e1e4e8}}
.header{{background:linear-gradient(135deg,#1a1f36,#2d1b69);padding:24px 32px;border-bottom:1px solid #30363d}}
.header h1{{font-size:24px;color:#58a6ff}} .header span{{color:#8b949e;font-size:14px}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:16px;padding:24px 32px}}
.card{{background:#161b22;border:1px solid #30363d;border-radius:12px;padding:20px;text-align:center}}
.card .value{{font-size:32px;font-weight:700;margin:8px 0}}
.card .label{{font-size:12px;color:#8b949e;text-transform:uppercase;letter-spacing:1px}}
.health .value{{color:#3fb950}} .issues .value{{color:#f85149}} .files .value{{color:#58a6ff}} .lines .value{{color:#d2a8ff}}
.charts{{display:grid;grid-template-columns:1fr 1fr;gap:24px;padding:0 32px 32px}}
.chart-box{{background:#161b22;border:1px solid #30363d;border-radius:12px;padding:20px}}
.chart-box h3{{font-size:14px;color:#8b949e;margin-bottom:12px;text-transform:uppercase;letter-spacing:1px}}
.treemap{{padding:0 32px 32px}}
.treemap-container{{background:#161b22;border:1px solid #30363d;border-radius:12px;padding:20px}}
.treemap-grid{{display:flex;flex-wrap:wrap;gap:4px;margin-top:12px}}
.treemap-item{{border-radius:6px;padding:8px 12px;font-size:11px;color:#fff;min-width:60px;text-align:center}}
.issue-bar{{display:flex;gap:16px;padding:0 32px 24px}}
.issue-badge{{background:#161b22;border:1px solid #30363d;border-radius:8px;padding:12px 20px;flex:1;text-align:center}}
.issue-badge .count{{font-size:24px;font-weight:700}} .err{{color:#f85149}} .warn{{color:#d29922}} .inf{{color:#58a6ff}}
@media(max-width:768px){{.charts{{grid-template-columns:1fr}}.cards{{grid-template-columns:repeat(2,1fr)}}}}
</style>
</head>
<body>
<div class="header"><h1>🦅 Falcon Dashboard</h1><span>{snapshots} snapshots · last updated {timestamp}</span></div>

<div class="cards">
<div class="card health"><div class="label">Health Score</div><div class="value">{health}</div></div>
<div class="card issues"><div class="label">Total Issues</div><div class="value">{issues}</div></div>
<div class="card files"><div class="label">Files</div><div class="value">{files}</div></div>
<div class="card lines"><div class="label">Lines of Code</div><div class="value">{lines}</div></div>
</div>

<div class="issue-bar">
<div class="issue-badge"><div class="count err">{errors}</div><div class="label">Errors</div></div>
<div class="issue-badge"><div class="count warn">{warnings}</div><div class="label">Warnings</div></div>
<div class="issue-badge"><div class="count inf">{info_count}</div><div class="label">Info</div></div>
</div>

<div class="charts">
<div class="chart-box"><h3>Health Score Over Time</h3><canvas id="healthChart"></canvas></div>
<div class="chart-box"><h3>Issues Over Time</h3><canvas id="issuesChart"></canvas></div>
<div class="chart-box"><h3>Avg Cyclomatic Complexity</h3><canvas id="complexityChart"></canvas></div>
<div class="chart-box"><h3>Issue Breakdown</h3><canvas id="breakdownChart"></canvas></div>
</div>

<div class="treemap">
<div class="treemap-container">
<h3 style="font-size:14px;color:#8b949e;text-transform:uppercase;letter-spacing:1px">Rule Violations Treemap</h3>
<div class="treemap-grid" id="treemap"></div>
</div>
</div>

<script>
const labels=[{labels}];
const chartOpts={{responsive:true,plugins:{{legend:{{display:false}}}},scales:{{x:{{ticks:{{color:'#8b949e'}},grid:{{color:'#21262d'}}}},y:{{ticks:{{color:'#8b949e'}},grid:{{color:'#21262d'}}}}}}}};
new Chart(document.getElementById('healthChart'),{{type:'line',data:{{labels,datasets:[{{data:[{health_data}].map(d=>d.y),borderColor:'#3fb950',backgroundColor:'rgba(63,185,80,0.1)',fill:true,tension:0.3}}]}},options:chartOpts}});
new Chart(document.getElementById('issuesChart'),{{type:'line',data:{{labels,datasets:[{{data:[{issues_data}].map(d=>d.y),borderColor:'#f85149',backgroundColor:'rgba(248,81,73,0.1)',fill:true,tension:0.3}}]}},options:chartOpts}});
new Chart(document.getElementById('complexityChart'),{{type:'line',data:{{labels,datasets:[{{data:[{complexity_data}].map(d=>d.y),borderColor:'#d2a8ff',backgroundColor:'rgba(210,168,255,0.1)',fill:true,tension:0.3}}]}},options:chartOpts}});
new Chart(document.getElementById('breakdownChart'),{{type:'doughnut',data:{{labels:['Errors','Warnings','Info'],datasets:[{{data:[{errors},{warnings},{info_count}],backgroundColor:['#f85149','#d29922','#58a6ff']}}]}},options:{{responsive:true,plugins:{{legend:{{labels:{{color:'#8b949e'}}}}}}}}}});
const treemapData=[{treemap_data}];
const colors=['#f85149','#d29922','#58a6ff','#3fb950','#d2a8ff','#f0883e','#79c0ff','#56d364','#ff7b72','#e3b341'];
const tEl=document.getElementById('treemap');
treemapData.sort((a,b)=>b.value-a.value);
treemapData.forEach((d,i)=>{{const el=document.createElement('div');el.className='treemap-item';el.style.background=colors[i%colors.length];el.style.flexGrow=Math.max(d.value,1);el.innerHTML=d.name+'<br><b>'+d.value+'</b>';tEl.appendChild(el)}});
</script>
</body></html>"#,
        snapshots = history.len(),
        timestamp = history
            .last()
            .map(|s| s.timestamp.as_str())
            .unwrap_or("never"),
        health = current_health,
        issues = current_issues,
        files = current_files,
        lines = current_lines,
        errors = errors,
        warnings = warnings,
        info_count = info_count,
        labels = labels.join(","),
        health_data = health_data.join(","),
        issues_data = issues_data.join(","),
        complexity_data = complexity_data.join(","),
        treemap_data = treemap_data,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::snapshot::{AnalysisSnapshot, IssueSummary, MetricsSummary};
    use std::collections::HashMap;

    fn make_snapshot(
        timestamp: &str,
        commit_hash: Option<&str>,
        branch: Option<&str>,
        health_score: f64,
        errors: usize,
        warnings: usize,
        info: usize,
        file_count: usize,
        total_lines: usize,
        avg_cyclomatic: f64,
        rule_counts: HashMap<String, usize>,
    ) -> AnalysisSnapshot {
        let total = errors + warnings + info;
        AnalysisSnapshot {
            timestamp: timestamp.to_string(),
            commit_hash: commit_hash.map(|s| s.to_string()),
            commit_message: None,
            branch: branch.map(|s| s.to_string()),
            file_count,
            total_lines,
            health_score,
            issues: IssueSummary {
                errors,
                warnings,
                info,
                total,
            },
            metrics_summary: MetricsSummary {
                avg_cyclomatic,
                max_cyclomatic: 0,
                avg_maintainability: 80.0,
                avg_lines_per_file: 100.0,
                god_file_count: 0,
            },
            rule_counts,
            per_package: Vec::new(),
        }
    }

    fn default_snapshot() -> AnalysisSnapshot {
        make_snapshot(
            "2024-01-15T10:00:00",
            Some("abc1234"),
            Some("main"),
            85.5,
            2,
            10,
            5,
            42,
            3000,
            3.2,
            HashMap::new(),
        )
    }

    // ── 1. Empty history returns valid HTML ──────────────────────────────────

    #[test]
    fn empty_history_returns_html_doctype() {
        let html = generate_dashboard_html(&[]);
        assert!(html.starts_with("<!DOCTYPE html>"), "Should start with DOCTYPE");
    }

    #[test]
    fn empty_history_contains_title() {
        let html = generate_dashboard_html(&[]);
        assert!(html.contains("<title>Falcon Dashboard</title>"));
    }

    #[test]
    fn empty_history_shows_zero_snapshots() {
        let html = generate_dashboard_html(&[]);
        assert!(html.contains("0 snapshots"), "Empty history should show 0 snapshots");
    }

    #[test]
    fn empty_history_shows_placeholder_values() {
        let html = generate_dashboard_html(&[]);
        // All four stat cards should show "--" when there is no data
        let dash_count = html.matches(">--<").count();
        assert!(
            dash_count >= 4,
            "Should have at least 4 '--' placeholders, got {}",
            dash_count
        );
    }

    #[test]
    fn empty_history_timestamp_is_never() {
        let html = generate_dashboard_html(&[]);
        assert!(html.contains("last updated never"));
    }

    #[test]
    fn empty_history_treemap_data_is_empty() {
        let html = generate_dashboard_html(&[]);
        // treemap_data should be empty, so the array literal is []
        assert!(html.contains("const treemapData=[];"));
    }

    // ── 2. Single snapshot — commit_hash / branch / health_score present ─────

    #[test]
    fn single_snapshot_commit_hash_in_labels() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        assert!(
            html.contains("\"abc1234\""),
            "Commit hash should appear in chart labels"
        );
    }

    #[test]
    fn single_snapshot_health_score_in_cards() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        // health_score 85.5 → formatted as "86" (format!("{:.0}", 85.5))
        assert!(
            html.contains(">86<"),
            "Rounded health score should appear in health card"
        );
    }

    #[test]
    fn single_snapshot_issue_counts_displayed() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        // total issues = 2+10+5 = 17
        assert!(html.contains(">17<"), "Total issue count should be rendered");
    }

    #[test]
    fn single_snapshot_file_count_displayed() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        assert!(html.contains(">42<"), "File count should appear in files card");
    }

    #[test]
    fn single_snapshot_total_lines_displayed() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        assert!(html.contains(">3000<"), "Total lines should appear in lines card");
    }

    #[test]
    fn single_snapshot_errors_warnings_info_in_issue_bar() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        assert!(html.contains(">2<"), "Error count should be rendered");
        assert!(html.contains(">10<"), "Warning count should be rendered");
        assert!(html.contains(">5<"), "Info count should be rendered");
    }

    #[test]
    fn single_snapshot_timestamp_in_header() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        assert!(
            html.contains("last updated 2024-01-15T10:00:00"),
            "Timestamp should appear in header"
        );
    }

    #[test]
    fn single_snapshot_shows_one_snapshot_count() {
        let s = default_snapshot();
        let html = generate_dashboard_html(&[s]);
        assert!(html.contains("1 snapshots"));
    }

    // ── 3. Multiple snapshots ────────────────────────────────────────────────

    #[test]
    fn multiple_snapshots_all_commit_hashes_in_labels() {
        let s1 = make_snapshot(
            "2024-01-10T00:00:00",
            Some("aaa0001"),
            Some("main"),
            70.0,
            1, 2, 3,
            10, 1000, 2.0,
            HashMap::new(),
        );
        let s2 = make_snapshot(
            "2024-01-11T00:00:00",
            Some("bbb0002"),
            Some("develop"),
            80.0,
            0, 5, 2,
            20, 2000, 3.5,
            HashMap::new(),
        );
        let s3 = make_snapshot(
            "2024-01-12T00:00:00",
            Some("ccc0003"),
            Some("feature/x"),
            90.0,
            0, 1, 0,
            30, 3000, 1.5,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s1, s2, s3]);
        assert!(html.contains("\"aaa0001\""));
        assert!(html.contains("\"bbb0002\""));
        assert!(html.contains("\"ccc0003\""));
    }

    #[test]
    fn multiple_snapshots_uses_last_for_stats() {
        let s1 = make_snapshot(
            "2024-01-10T00:00:00",
            Some("old1234"),
            Some("main"),
            50.0,
            5, 20, 10,
            5, 500, 5.0,
            HashMap::new(),
        );
        let s2 = make_snapshot(
            "2024-01-11T00:00:00",
            Some("new5678"),
            Some("main"),
            95.0,
            0, 1, 1,
            100, 10000, 1.2,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s1, s2]);
        // Latest snapshot's health (95 → "95") and file count (100) should show
        assert!(html.contains(">95<"), "Latest health score should be displayed");
        assert!(html.contains(">100<"), "Latest file count should be displayed");
        // Oldest snapshot's file count (5) should NOT appear in the cards context
        // but may appear in chart data — just verify latest timestamp is shown
        assert!(html.contains("last updated 2024-01-11T00:00:00"));
    }

    #[test]
    fn multiple_snapshots_snapshot_count_correct() {
        let snaps: Vec<AnalysisSnapshot> = (0..5)
            .map(|i| make_snapshot(
                &format!("2024-01-{:02}T00:00:00", i + 1),
                Some(&format!("hash{:04}", i)),
                Some("main"),
                75.0 + i as f64,
                0, i, 0,
                10, 1000, 2.0,
                HashMap::new(),
            ))
            .collect();
        let html = generate_dashboard_html(&snaps);
        assert!(html.contains("5 snapshots"));
    }

    // ── 4. Health score sections / chart data shape ──────────────────────────

    #[test]
    fn health_chart_data_contains_y_values() {
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("deadbeef"),
            Some("main"),
            72.3,
            0, 0, 0,
            1, 100, 1.0,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s]);
        assert!(
            html.contains("{x:0,y:72.3}"),
            "Health chart data point should use x/y object notation"
        );
    }

    #[test]
    fn issues_chart_data_contains_y_values() {
        let mut rc = HashMap::new();
        rc.insert("missing_required".to_string(), 3usize);
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("cafebabe"),
            Some("main"),
            60.0,
            1, 4, 2,
            5, 500, 2.5,
            rc,
        );
        let html = generate_dashboard_html(&[s]);
        // total issues = 1+4+2 = 7
        assert!(
            html.contains("{x:0,y:7}"),
            "Issues chart data point should reflect total issue count"
        );
    }

    #[test]
    fn complexity_chart_data_correct_format() {
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("f00df00d"),
            Some("main"),
            80.0,
            0, 0, 0,
            10, 800, 4.7,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s]);
        assert!(
            html.contains("{x:0,y:4.7}"),
            "Complexity chart data point should use avg_cyclomatic value"
        );
    }

    #[test]
    fn html_contains_four_chart_canvases() {
        let html = generate_dashboard_html(&[]);
        assert!(html.contains("id=\"healthChart\""));
        assert!(html.contains("id=\"issuesChart\""));
        assert!(html.contains("id=\"complexityChart\""));
        assert!(html.contains("id=\"breakdownChart\""));
    }

    // ── 5. Rule counts / treemap data ────────────────────────────────────────

    #[test]
    fn treemap_data_includes_rule_counts() {
        let mut rc = HashMap::new();
        rc.insert("avoid_print".to_string(), 7usize);
        rc.insert("missing_key".to_string(), 3usize);
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("1a2b3c4d"),
            Some("main"),
            70.0,
            0, 10, 0,
            5, 500, 2.0,
            rc,
        );
        let html = generate_dashboard_html(&[s]);
        assert!(html.contains("{name:\"avoid_print\",value:7}"));
        assert!(html.contains("{name:\"missing_key\",value:3}"));
    }

    #[test]
    fn treemap_data_excludes_zero_count_rules() {
        let mut rc = HashMap::new();
        rc.insert("active_rule".to_string(), 5usize);
        rc.insert("zero_rule".to_string(), 0usize);
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("deadc0de"),
            Some("main"),
            80.0,
            0, 5, 0,
            5, 500, 2.0,
            rc,
        );
        let html = generate_dashboard_html(&[s]);
        assert!(
            html.contains("{name:\"active_rule\",value:5}"),
            "Active rule should be in treemap"
        );
        assert!(
            !html.contains("\"zero_rule\""),
            "Zero-count rule should be excluded from treemap"
        );
    }

    // ── 6. Snapshot without commit hash uses timestamp prefix for label ───────

    #[test]
    fn no_commit_hash_uses_timestamp_prefix_as_label() {
        let s = make_snapshot(
            "2024-06-20T12:34:56",
            None, // no commit hash
            None,
            60.0,
            0, 0, 0,
            1, 100, 1.0,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s]);
        // Should use first 10 chars of timestamp: "2024-06-20"
        assert!(
            html.contains("\"2024-06-20\""),
            "Label should fall back to first 10 chars of timestamp when no commit hash"
        );
    }

    // ── 7. Extreme / edge values ─────────────────────────────────────────────

    #[test]
    fn perfect_health_score_displays_100() {
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("perfect1"),
            Some("main"),
            100.0,
            0, 0, 0,
            1, 100, 1.0,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s]);
        assert!(html.contains(">100<"), "Perfect health score should display as 100");
    }

    #[test]
    fn zero_health_score_displays_0() {
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("zero0000"),
            Some("main"),
            0.0,
            100, 200, 50,
            1, 100, 20.0,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s]);
        assert!(html.contains(">0<"), "Zero health score should display as 0");
    }

    #[test]
    fn large_issue_counts_render_correctly() {
        let s = make_snapshot(
            "2024-01-15T00:00:00",
            Some("bigcount"),
            Some("main"),
            10.0,
            500, 1000, 250,
            200, 50000, 15.0,
            HashMap::new(),
        );
        let html = generate_dashboard_html(&[s]);
        // total = 500+1000+250 = 1750
        assert!(html.contains(">1750<"));
        assert!(html.contains(">500<"));
        assert!(html.contains(">1000<"));
    }

    #[test]
    fn html_closes_body_and_html_tags() {
        let html = generate_dashboard_html(&[]);
        assert!(html.contains("</body></html>"), "HTML should be properly closed");
    }
}
