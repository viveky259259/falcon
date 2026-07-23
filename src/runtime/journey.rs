//! User-journey recorder.
//!
//! Polls a running Flutter app at a fixed interval while the developer drives it,
//! capturing a screenshot + the current screen label (derived from the widget
//! tree) + memory/frame stats at each tick. Consecutive ticks on the same screen
//! are collapsed; screen changes mark new steps in the journey. Produces a
//! console summary and a self-contained HTML timeline with embedded thumbnails.
//!
//! Capture is cross-platform: it reuses `tools::capture_screenshot_file`, which
//! falls back from the rasterizer RPC (mobile + desktop) to the `flutter
//! screenshot` CLI (device framebuffer). Web targets cannot be captured; the
//! recorder records a step without a thumbnail rather than aborting.

use super::connection::VmServiceClient;
use super::tools::capture_screenshot_file;
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use colored::Colorize;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// One recorded moment in the journey.
#[derive(Debug, Clone, Serialize)]
pub struct JourneyStep {
    pub index: usize,
    pub elapsed_secs: u64,
    pub screen_label: String,
    /// Screenshot filename relative to the output dir, if capture succeeded.
    pub screenshot_file: Option<String>,
    pub heap_mb: f64,
    pub dropped_frame_pct: f64,
    /// True when this step's screen differs from the previous step (a navigation).
    pub is_new_screen: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JourneyReport {
    pub vm_service_uri: String,
    pub isolate_id: String,
    pub duration_secs: u64,
    pub interval_secs: u64,
    pub total_ticks: usize,
    pub distinct_screens: Vec<String>,
    pub steps: Vec<JourneyStep>,
    pub output_dir: String,
    pub html_path: Option<String>,
}

/// Walk a Flutter inspector widget-tree summary and derive a human-readable
/// label for the current screen.
///
/// Heuristic: the first node (depth-first) whose widget type looks like a page
/// (ends with `Page`, `Screen`, `View`, or `Route`) wins; otherwise the root
/// description.
pub fn extract_screen_label(tree: &Value) -> String {
    let root = if tree.get("type").is_some() || tree.get("children").is_some() {
        tree
    } else if let Some(result) = tree.get("result") {
        result
    } else {
        tree
    };

    if let Some(label) = find_page_like(root) {
        return label;
    }
    node_type(root).unwrap_or_else(|| "unknown".to_string())
}

fn find_page_like(node: &Value) -> Option<String> {
    if !node.is_object() {
        return None;
    }
    if let Some(ty) = node_type(node) {
        if is_page_like(&ty) {
            return Some(ty);
        }
    }
    if let Some(children) = node["children"].as_array() {
        for child in children {
            if let Some(found) = find_page_like(child) {
                return Some(found);
            }
        }
    }
    None
}

fn node_type(node: &Value) -> Option<String> {
    node["type"]
        .as_str()
        .or_else(|| node["widgetRuntimeType"].as_str())
        .or_else(|| node["description"].as_str())
        .map(ToString::to_string)
}

fn is_page_like(ty: &str) -> bool {
    // Strip generics/args like "MyPage<Foo>" before matching the suffix.
    let base = ty.split(['<', ' ', '(']).next().unwrap_or(ty);
    ["Page", "Screen", "View", "Route"]
        .iter()
        .any(|suffix| base.ends_with(suffix) && base.len() > suffix.len())
}

/// Decide whether `current_label` represents a new screen relative to the last
/// recorded step. A differing label is always a new screen. When the label is
/// `unknown` (tree unavailable), fall back to a coarse screenshot-size delta so
/// web/headless sessions still segment roughly.
pub fn is_screen_change(
    prev_label: Option<&str>,
    prev_bytes: Option<usize>,
    current_label: &str,
    current_bytes: Option<usize>,
) -> bool {
    match prev_label {
        None => true,
        Some(prev) => {
            if prev != current_label {
                return true;
            }
            if current_label == "unknown" {
                if let (Some(a), Some(b)) = (prev_bytes, current_bytes) {
                    let larger = a.max(b).max(1);
                    let delta = a.abs_diff(b);
                    // >15% size change on an unknown screen ⇒ treat as navigation.
                    return (delta as f64 / larger as f64) > 0.15;
                }
            }
            false
        }
    }
}

/// Configuration for a journey recording session.
#[derive(Debug, Clone)]
pub struct JourneyConfig {
    pub project_path: PathBuf,
    pub device: Option<String>,
    pub duration: Duration,
    pub interval: Duration,
    pub output_dir: PathBuf,
    pub write_html: bool,
}

/// Record a journey by polling the running app for the configured duration.
pub async fn record_journey(
    client: &VmServiceClient,
    vm_service_uri: &str,
    config: &JourneyConfig,
) -> Result<JourneyReport> {
    std::fs::create_dir_all(&config.output_dir).ok();

    let interval_secs = config.interval.as_secs().max(1);
    let total_ticks = (config.duration.as_secs() / interval_secs).max(1);

    let mut steps: Vec<JourneyStep> = Vec::new();
    let mut prev_label: Option<String> = None;
    let mut prev_bytes: Option<usize> = None;

    eprintln!(
        "  {} Recording journey: {} ticks every {}s ({}s total). Drive the app now…",
        "▸".bright_cyan(),
        total_ticks,
        interval_secs,
        config.duration.as_secs()
    );

    for tick in 0..total_ticks {
        let elapsed_secs = tick * interval_secs;

        let tree = client.get_root_widget_tree().await.unwrap_or(Value::Null);
        let screen_label = extract_screen_label(&tree);

        let screenshot_name = format!("step_{tick:03}.png");
        let screenshot_path = config.output_dir.join(&screenshot_name);
        let screenshot_file = match capture_screenshot_file(
            client,
            &screenshot_path,
            &config.project_path,
            config.device.as_deref(),
        )
        .await
        {
            Ok(_) => Some(screenshot_name),
            Err(_) => None,
        };
        let current_bytes = screenshot_file
            .as_ref()
            .and_then(|_| std::fs::metadata(&screenshot_path).ok())
            .map(|m| m.len() as usize);

        let heap_mb = client
            .get_memory_usage()
            .await
            .map(|m| m.heap_usage_mb())
            .unwrap_or(0.0);
        let dropped_frame_pct = client
            .get_rendering_stats()
            .await
            .map(|r| r.dropped_frame_pct())
            .unwrap_or(0.0);

        let is_new_screen = is_screen_change(
            prev_label.as_deref(),
            prev_bytes,
            &screen_label,
            current_bytes,
        );

        steps.push(JourneyStep {
            index: tick as usize,
            elapsed_secs,
            screen_label: screen_label.clone(),
            screenshot_file,
            heap_mb,
            dropped_frame_pct,
            is_new_screen,
        });

        prev_label = Some(screen_label);
        prev_bytes = current_bytes;

        if tick < total_ticks - 1 {
            tokio::time::sleep(config.interval).await;
        }
    }

    let distinct_screens = distinct_screens(&steps);

    let html_path = if config.write_html {
        let path = config.output_dir.join("journey.html");
        write_journey_html(&steps, &distinct_screens, &config.output_dir, &path)?;
        Some(path.display().to_string())
    } else {
        None
    };

    Ok(JourneyReport {
        vm_service_uri: vm_service_uri.to_string(),
        isolate_id: client.isolate_id.clone(),
        duration_secs: config.duration.as_secs(),
        interval_secs,
        total_ticks: steps.len(),
        distinct_screens,
        steps,
        output_dir: config.output_dir.display().to_string(),
        html_path,
    })
}

/// Ordered list of distinct screens in the order they were first visited.
pub fn distinct_screens(steps: &[JourneyStep]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for step in steps {
        if step.is_new_screen && !seen.contains(&step.screen_label) {
            seen.push(step.screen_label.clone());
        }
    }
    // Guarantee the very first screen is present even if it was never re-marked.
    if seen.is_empty() {
        if let Some(first) = steps.first() {
            seen.push(first.screen_label.clone());
        }
    }
    seen
}

fn write_journey_html(
    steps: &[JourneyStep],
    distinct_screens: &[String],
    output_dir: &Path,
    path: &Path,
) -> Result<()> {
    let mut cards = String::new();
    for step in steps {
        let thumb = match &step.screenshot_file {
            Some(file) => match std::fs::read(output_dir.join(file)) {
                // Inline the PNG as a data URI so the report is self-contained.
                Ok(bytes) => format!(
                    "<img src=\"data:image/png;base64,{}\" alt=\"{}\"/>",
                    STANDARD.encode(&bytes),
                    html_escape(&step.screen_label)
                ),
                Err(_) => "<div class=\"noshot\">no image</div>".to_string(),
            },
            None => "<div class=\"noshot\">no capture</div>".to_string(),
        };
        let badge = if step.is_new_screen {
            "<span class=\"badge new\">new screen</span>"
        } else {
            ""
        };
        cards.push_str(&format!(
            "<div class=\"card\">{thumb}<div class=\"meta\"><h3>{label} {badge}</h3>\
             <p>t+{t}s · heap {heap:.1} MB · dropped frames {drop:.1}%</p></div></div>",
            label = html_escape(&step.screen_label),
            t = step.elapsed_secs,
            heap = step.heap_mb,
            drop = step.dropped_frame_pct,
        ));
    }

    let screens_list = distinct_screens
        .iter()
        .map(|s| format!("<li>{}</li>", html_escape(s)))
        .collect::<String>();

    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Falcon Journey</title>\
<style>body{{font-family:-apple-system,Segoe UI,Roboto,sans-serif;margin:0;background:#0d1117;color:#e6edf3}}\
header{{padding:24px 32px;background:#161b22;border-bottom:1px solid #30363d}}h1{{margin:0;font-size:20px}}\
.summary{{padding:16px 32px;color:#8b949e}}.summary ol{{columns:3}}\
.timeline{{display:flex;flex-wrap:wrap;gap:16px;padding:24px 32px}}\
.card{{width:200px;background:#161b22;border:1px solid #30363d;border-radius:8px;overflow:hidden}}\
.card img{{width:100%;display:block;background:#000}}.noshot{{height:120px;display:flex;align-items:center;justify-content:center;color:#8b949e}}\
.meta{{padding:8px 10px}}.meta h3{{margin:0 0 4px;font-size:13px}}.meta p{{margin:0;font-size:11px;color:#8b949e}}\
.badge{{font-size:10px;padding:1px 6px;border-radius:10px;background:#1f6feb;color:#fff}}\
</style></head><body><header><h1>Falcon — User Journey</h1></header>\
<div class=\"summary\"><p>{ticks} ticks · {screens} distinct screens</p><ol>{screens_list}</ol></div>\
<div class=\"timeline\">{cards}</div></body></html>",
        ticks = steps.len(),
        screens = distinct_screens.len(),
    );

    std::fs::write(path, html)
        .map_err(|e| anyhow::anyhow!("Failed to write journey HTML to {}: {e}", path.display()))?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn print_journey_report(report: &JourneyReport) {
    println!("{} {}", "VM Service:".bright_cyan(), report.vm_service_uri);
    println!("{} {}", "Isolate:".bright_cyan(), report.isolate_id);
    println!(
        "{} {}s every {}s",
        "Recorded:".bright_cyan(),
        report.duration_secs,
        report.interval_secs
    );
    println!("{} {}", "Ticks:".bright_cyan(), report.total_ticks);
    println!(
        "{} {}",
        "Distinct screens:".bright_cyan(),
        report.distinct_screens.len()
    );
    if !report.distinct_screens.is_empty() {
        println!();
        println!("{}", "Journey".bright_white().bold());
        for (i, screen) in report.distinct_screens.iter().enumerate() {
            println!("  {}. {}", i + 1, screen);
        }
    }
    println!();
    println!(
        "{} {}",
        "Output dir:".bright_cyan(),
        report.output_dir.bright_white()
    );
    if let Some(html) = &report.html_path {
        println!("{} {}", "HTML report:".bright_cyan(), html.bright_white());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_screen_label_finds_page_like_node() {
        let tree = json!({
            "type": "MaterialApp",
            "children": [
                { "type": "Navigator", "children": [
                    { "type": "HomePage", "children": [] }
                ]}
            ]
        });
        assert_eq!(extract_screen_label(&tree), "HomePage");
    }

    #[test]
    fn extract_screen_label_unwraps_result_envelope_and_falls_back_to_root() {
        let tree = json!({ "result": { "type": "MyCustomRoot", "children": [] } });
        assert_eq!(extract_screen_label(&tree), "MyCustomRoot");
    }

    #[test]
    fn extract_screen_label_strips_generics_before_suffix_match() {
        let tree = json!({
            "type": "App",
            "children": [{ "type": "DetailScreen<Item>", "children": [] }]
        });
        assert_eq!(extract_screen_label(&tree), "DetailScreen<Item>");
    }

    #[test]
    fn extract_screen_label_returns_unknown_for_empty_tree() {
        assert_eq!(extract_screen_label(&Value::Null), "unknown");
    }

    #[test]
    fn is_screen_change_true_on_first_step_and_label_change() {
        assert!(is_screen_change(None, None, "HomePage", Some(100)));
        assert!(is_screen_change(
            Some("HomePage"),
            Some(100),
            "DetailPage",
            Some(100)
        ));
    }

    #[test]
    fn is_screen_change_false_on_same_label() {
        assert!(!is_screen_change(
            Some("HomePage"),
            Some(100),
            "HomePage",
            Some(140)
        ));
    }

    #[test]
    fn is_screen_change_uses_size_delta_only_when_unknown() {
        // Same known label, big size change ⇒ NOT a change.
        assert!(!is_screen_change(
            Some("HomePage"),
            Some(100),
            "HomePage",
            Some(1000)
        ));
        // Unknown label, >15% size change ⇒ change.
        assert!(is_screen_change(
            Some("unknown"),
            Some(100),
            "unknown",
            Some(130)
        ));
        // Unknown label, <15% size change ⇒ no change.
        assert!(!is_screen_change(
            Some("unknown"),
            Some(100),
            "unknown",
            Some(105)
        ));
    }

    #[test]
    fn distinct_screens_preserves_first_visit_order() {
        let steps = vec![
            step("Home", true),
            step("Home", false),
            step("Detail", true),
            step("Home", true),
        ];
        // Home re-appears but is only listed once, in first-visit order.
        assert_eq!(distinct_screens(&steps), vec!["Home", "Detail"]);
    }

    fn step(label: &str, is_new: bool) -> JourneyStep {
        JourneyStep {
            index: 0,
            elapsed_secs: 0,
            screen_label: label.to_string(),
            screenshot_file: None,
            heap_mb: 0.0,
            dropped_frame_pct: 0.0,
            is_new_screen: is_new,
        }
    }
}
