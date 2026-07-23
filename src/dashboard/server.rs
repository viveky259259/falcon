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
            "No analysis history found. Run 'falcon x dashboard snapshot' first to capture data."
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
