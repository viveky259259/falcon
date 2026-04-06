//! HTML dashboard report for runtime analysis.

use super::{RuntimeReport, RuntimeSeverity};
use anyhow::Result;
use std::path::Path;

/// Write a self-contained HTML dashboard to `path`.
pub fn write_report(report: &RuntimeReport, path: &Path) -> Result<()> {
    let html = build_html(report);
    std::fs::write(path, html)?;
    Ok(())
}

fn severity_class(s: RuntimeSeverity) -> &'static str {
    match s {
        RuntimeSeverity::Error => "error",
        RuntimeSeverity::Warning => "warning",
        RuntimeSeverity::Info => "info",
    }
}

fn severity_icon(s: RuntimeSeverity) -> &'static str {
    match s {
        RuntimeSeverity::Error => "&#10007;",
        RuntimeSeverity::Warning => "&#9888;",
        RuntimeSeverity::Info => "&#8505;",
    }
}

fn score_color(score: u32) -> &'static str {
    match score {
        90..=100 => "#22c55e",
        70..=89 => "#eab308",
        _ => "#ef4444",
    }
}

fn build_html(report: &RuntimeReport) -> String {
    // Build the memory chart data (JSON array of [elapsed, heap_mb]).
    let mem_data: String = report
        .memory
        .samples
        .iter()
        .map(|(t, v)| format!("[{:.1},{:.2}]", t, v))
        .collect::<Vec<_>>()
        .join(",");

    // Build the frame time chart data.
    let frame_data: String = report
        .rendering
        .frame_times
        .iter()
        .map(|(t, v)| format!("[{:.1},{:.2}]", t, v))
        .collect::<Vec<_>>()
        .join(",");

    // Build the issues HTML.
    let issues_html: String = report
        .issues
        .iter()
        .map(|issue| {
            format!(
                r#"<div class="issue {cls}">
                    <span class="icon">{icon}</span>
                    <div>
                        <strong>[{cat}] {title}</strong>
                        <p>{detail}</p>
                        <p class="suggestion">&rarr; {suggestion}</p>
                    </div>
                </div>"#,
                cls = severity_class(issue.severity),
                icon = severity_icon(issue.severity),
                cat = issue.category,
                title = html_escape(&issue.title),
                detail = html_escape(&issue.detail),
                suggestion = html_escape(&issue.suggestion),
            )
        })
        .collect();

    // Top CPU functions table rows.
    let cpu_rows: String = report
        .cpu
        .top_functions
        .iter()
        .take(10)
        .map(|(name, count)| {
            format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                html_escape(name),
                count
            )
        })
        .collect();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Falcon Runtime Report</title>
<script src="https://cdnjs.cloudflare.com/ajax/libs/Chart.js/4.4.1/chart.umd.min.js"></script>
<style>
  :root {{
    --bg: #0f172a; --surface: #1e293b; --border: #334155;
    --text: #e2e8f0; --muted: #94a3b8;
    --green: #22c55e; --yellow: #eab308; --red: #ef4444; --cyan: #06b6d4;
  }}
  * {{ margin:0; padding:0; box-sizing:border-box; }}
  body {{ font-family: 'Inter', system-ui, sans-serif; background: var(--bg); color: var(--text); padding: 2rem; }}
  h1 {{ font-size: 1.5rem; margin-bottom: 0.5rem; }}
  h2 {{ font-size: 1.1rem; color: var(--cyan); margin: 1.5rem 0 0.75rem; }}
  .header {{ display:flex; align-items:center; gap:1.5rem; margin-bottom:2rem; }}
  .score-ring {{ width:100px; height:100px; position:relative; }}
  .score-ring svg {{ transform: rotate(-90deg); }}
  .score-ring .value {{ position:absolute; inset:0; display:flex; align-items:center; justify-content:center; font-size:1.8rem; font-weight:700; }}
  .subtitle {{ color: var(--muted); font-size: 0.85rem; }}

  .grid {{ display:grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 1rem; margin-bottom: 1.5rem; }}
  .card {{ background: var(--surface); border: 1px solid var(--border); border-radius: 0.75rem; padding: 1rem; }}
  .card .label {{ color: var(--muted); font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; }}
  .card .val {{ font-size: 1.4rem; font-weight: 600; margin-top: 0.25rem; }}
  .card .bar {{ height: 6px; border-radius: 3px; background: var(--border); margin-top: 0.5rem; overflow: hidden; }}
  .card .bar .fill {{ height: 100%; border-radius: 3px; }}

  .chart-container {{ background: var(--surface); border: 1px solid var(--border); border-radius: 0.75rem; padding: 1rem; margin-bottom: 1.5rem; }}
  canvas {{ max-height: 250px; }}

  .issue {{ display:flex; gap:0.75rem; padding:0.75rem 1rem; border-radius:0.5rem; margin-bottom:0.5rem; background: var(--surface); border-left: 3px solid; }}
  .issue.error {{ border-color: var(--red); }}
  .issue.warning {{ border-color: var(--yellow); }}
  .issue.info {{ border-color: var(--cyan); }}
  .issue .icon {{ font-size:1.2rem; min-width:1.5rem; text-align:center; }}
  .issue.error .icon {{ color: var(--red); }}
  .issue.warning .icon {{ color: var(--yellow); }}
  .issue.info .icon {{ color: var(--cyan); }}
  .issue p {{ color: var(--muted); font-size: 0.85rem; margin-top:0.25rem; }}
  .issue .suggestion {{ color: var(--green); }}

  table {{ width:100%; border-collapse:collapse; margin-top:0.5rem; }}
  th, td {{ text-align:left; padding:0.4rem 0.75rem; border-bottom:1px solid var(--border); font-size:0.85rem; }}
  th {{ color: var(--muted); font-weight: 500; }}

  .footer {{ color: var(--muted); font-size: 0.75rem; margin-top: 2rem; text-align: center; }}
</style>
</head>
<body>

<div class="header">
  <div class="score-ring">
    <svg width="100" height="100" viewBox="0 0 100 100">
      <circle cx="50" cy="50" r="42" fill="none" stroke="var(--border)" stroke-width="8"/>
      <circle cx="50" cy="50" r="42" fill="none" stroke="{overall_color}" stroke-width="8"
        stroke-dasharray="{dash} {gap}" stroke-linecap="round"/>
    </svg>
    <div class="value" style="color:{overall_color}">{score}</div>
  </div>
  <div>
    <h1>Falcon Runtime Report</h1>
    <div class="subtitle">Grade {grade} &middot; {snapshots} snapshots over {duration:.1}s</div>
  </div>
</div>

<!-- Dimension scores -->
<div class="grid">
  {dim_cards}
</div>

<!-- Memory chart -->
<h2>Memory (Heap)</h2>
<div class="chart-container"><canvas id="memChart"></canvas></div>

<!-- Memory stats -->
<div class="grid">
  <div class="card"><div class="label">Avg Heap</div><div class="val">{avg_heap:.1} MB</div></div>
  <div class="card"><div class="label">Peak Heap</div><div class="val">{max_heap:.1} MB</div></div>
  <div class="card"><div class="label">Heap Growth</div><div class="val">{growth:.1} MB</div></div>
  <div class="card"><div class="label">External Peak</div><div class="val">{ext:.1} MB</div></div>
</div>

<!-- Rendering chart -->
<h2>Frame Times</h2>
<div class="chart-container"><canvas id="frameChart"></canvas></div>

<!-- Rendering stats -->
<div class="grid">
  <div class="card"><div class="label">Total Frames</div><div class="val">{total_frames}</div></div>
  <div class="card"><div class="label">Dropped</div><div class="val">{dropped} ({dropped_pct:.1}%)</div></div>
  <div class="card"><div class="label">Avg Build</div><div class="val">{avg_build:.1} ms</div></div>
  <div class="card"><div class="label">Max Build</div><div class="val">{max_build:.0} ms</div></div>
</div>

<!-- Network -->
<h2>Network</h2>
<div class="grid">
  <div class="card"><div class="label">Requests</div><div class="val">{net_total}</div></div>
  <div class="card"><div class="label">Failed</div><div class="val">{net_failed}</div></div>
  <div class="card"><div class="label">Avg Latency</div><div class="val">{net_avg_lat:.0} ms</div></div>
  <div class="card"><div class="label">Data Received</div><div class="val">{net_bytes:.1} KB</div></div>
</div>

<!-- CPU -->
<h2>CPU</h2>
<div class="grid">
  <div class="card"><div class="label">Est. Usage</div><div class="val">{cpu_pct:.0}%</div></div>
  <div class="card"><div class="label">Samples</div><div class="val">{cpu_samples}</div></div>
</div>
{cpu_table}

<!-- Issues -->
<h2>Issues ({issue_count})</h2>
{issues_html}

<div class="footer">Generated by Falcon &middot; Runtime Analysis</div>

<script>
const memData = [{mem_data}];
const frameData = [{frame_data}];

function makeChart(id, data, label, color, yLabel) {{
  new Chart(document.getElementById(id), {{
    type: 'line',
    data: {{
      labels: data.map(d => d[0].toFixed(0) + 's'),
      datasets: [{{ label, data: data.map(d => d[1]), borderColor: color, backgroundColor: color + '22', fill: true, tension: 0.3, pointRadius: 2 }}]
    }},
    options: {{
      responsive: true,
      plugins: {{ legend: {{ display: false }} }},
      scales: {{
        x: {{ ticks: {{ color: '#94a3b8' }}, grid: {{ color: '#334155' }} }},
        y: {{ title: {{ display: true, text: yLabel, color: '#94a3b8' }}, ticks: {{ color: '#94a3b8' }}, grid: {{ color: '#334155' }} }}
      }}
    }}
  }});
}}

if (memData.length > 0) makeChart('memChart', memData, 'Heap MB', '#06b6d4', 'MB');
if (frameData.length > 0) makeChart('frameChart', frameData, 'Build Time', '#eab308', 'ms');
</script>
</body>
</html>"##,
        overall_color = score_color(report.overall_score),
        dash = (report.overall_score as f64 / 100.0) * 264.0,
        gap = 264.0 - (report.overall_score as f64 / 100.0) * 264.0,
        score = report.overall_score,
        grade = report.grade,
        snapshots = report.snapshot_count,
        duration = report.duration_secs,
        dim_cards = build_dimension_cards(report),
        avg_heap = report.memory.avg_heap_mb,
        max_heap = report.memory.max_heap_mb,
        growth = report.memory.heap_growth_mb,
        ext = report.memory.peak_external_mb,
        total_frames = report.rendering.total_frames,
        dropped = report.rendering.dropped_frames,
        dropped_pct = report.rendering.dropped_pct,
        avg_build = report.rendering.avg_build_ms,
        max_build = report.rendering.max_build_ms,
        net_total = report.network.total_requests,
        net_failed = report.network.failed_requests,
        net_avg_lat = report.network.avg_latency_ms,
        net_bytes = report.network.total_bytes_received as f64 / 1024.0,
        cpu_pct = report.cpu.estimated_usage_pct,
        cpu_samples = report.cpu.total_samples,
        cpu_table = if cpu_rows.is_empty() {
            String::new()
        } else {
            format!(
                "<table><thead><tr><th>Function</th><th>Samples</th></tr></thead><tbody>{}</tbody></table>",
                cpu_rows
            )
        },
        issue_count = report.issues.len(),
        issues_html = issues_html,
        mem_data = mem_data,
        frame_data = frame_data,
    )
}

fn build_dimension_cards(report: &RuntimeReport) -> String {
    let dims = [
        ("Memory", report.scores.memory),
        ("Rendering", report.scores.rendering),
        ("Network", report.scores.network),
        ("CPU", report.scores.cpu),
        ("Stability", report.scores.stability),
    ];

    dims.iter()
        .map(|(name, score)| {
            let color = score_color(*score);
            format!(
                r#"<div class="card">
                    <div class="label">{name}</div>
                    <div class="val" style="color:{color}">{score}/100</div>
                    <div class="bar"><div class="fill" style="width:{score}%;background:{color}"></div></div>
                </div>"#,
                name = name,
                score = score,
                color = color,
            )
        })
        .collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
