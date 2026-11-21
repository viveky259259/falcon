//! MCP tools that start, reload and observe a running Flutter app:
//! `run_app`, `stop_app`, `reload`, `widget_tree`, `logs`, `tree_diff`,
//! `app_metrics`, `journey` and `verify`.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

use super::app_session;
use super::runtime_tools::{
    connect, png_images, refuse_remote, resolve_target, run_async, temp_output, MCP_IMAGES_KEY,
};
use crate::doctor::Trust;
use crate::runtime::tools as rt;

fn parse<T: for<'de> Deserialize<'de>>(tool: &str, args: &Value) -> Result<T, String> {
    serde_json::from_value(args.clone()).map_err(|e| format!("Invalid {} arguments: {}", tool, e))
}

fn encode<T: serde::Serialize>(v: &T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| format!("Failed to encode report: {}", e))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    #[serde(default)]
    attach: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReloadArgs {
    #[serde(default)]
    attach: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    full_restart: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WidgetTreeArgs {
    #[serde(default)]
    attach: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_mode() -> String {
    "visible".to_string()
}

fn default_limit() -> usize {
    150
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogsArgs {
    #[serde(default)]
    attach: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    since: u64,
    #[serde(default)]
    errors_only: bool,
    #[serde(default = "default_log_limit")]
    limit: usize,
    #[serde(default = "default_listen")]
    listen_secs: u64,
}

fn default_log_limit() -> usize {
    200
}

fn default_listen() -> u64 {
    3
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimedArgs {
    #[serde(default)]
    attach: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default = "default_duration")]
    duration_secs: u64,
    #[serde(default = "default_jank")]
    jank_ms: f64,
}

fn default_duration() -> u64 {
    5
}

fn default_jank() -> f64 {
    16.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JourneyArgs {
    #[serde(default)]
    attach: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    device: Option<String>,
    #[serde(default = "default_journey_duration")]
    duration_secs: u64,
    #[serde(default = "default_interval")]
    interval_secs: u64,
    #[serde(default)]
    output_dir: Option<String>,
    #[serde(default = "default_max_images")]
    max_images: usize,
}

fn default_journey_duration() -> u64 {
    15
}

fn default_interval() -> u64 {
    3
}

fn default_max_images() -> usize {
    4
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyArgs {
    #[serde(default = "dot")]
    path: String,
    target: String,
    device: String,
    #[serde(default = "default_verify_timeout")]
    timeout_secs: u64,
    #[serde(default)]
    output_dir: Option<String>,
    #[serde(default)]
    physical_camera: bool,
}

fn dot() -> String {
    ".".to_string()
}

fn default_verify_timeout() -> u64 {
    180
}

/// Longest recording window any timed tool accepts.
const MAX_WINDOW_SECS: u64 = 120;

pub use super::runtime_defs::{definitions, METRIC_KINDS};

/// Dispatch a tool owned by this module. `None` when the name isn't ours.
pub fn dispatch(name: &str, args: &Value, trust: Trust) -> Option<Result<Value, String>> {
    let run = |f: fn(&Value) -> Result<Value, String>| {
        Some(refuse_remote(trust, name).and_then(|_| f(args)))
    };
    match name {
        "run_app" => run(execute_run_app),
        "stop_app" => run(|args| {
            parse::<serde_json::Map<String, Value>>("stop_app", args).and_then(|m| {
                if m.is_empty() {
                    Ok(app_session::stop_app())
                } else {
                    Err("Invalid stop_app arguments: it takes none".to_string())
                }
            })
        }),
        "reload" => run(execute_reload),
        "widget_tree" => run(execute_widget_tree),
        "logs" => run(execute_logs),
        "tree_diff" => run(execute_tree_diff),
        "app_metrics" => run(execute_app_metrics),
        "journey" => run(execute_journey),
        "verify" => run(execute_verify),
        _ => None,
    }
}

fn execute_run_app(args: &Value) -> Result<Value, String> {
    app_session::run_app(parse("run_app", args)?)
}

fn execute_reload(args: &Value) -> Result<Value, String> {
    let a: ReloadArgs = parse("reload", args)?;
    if a.attach.is_none() {
        if let Some(result) = app_session::hot_reload(a.full_restart) {
            return result;
        }
    }
    let (uri, _) = resolve_target(a.attach.as_deref(), a.path.as_deref())?;
    let mode = if a.full_restart {
        rt::ReloadMode::HotRestart
    } else {
        rt::ReloadMode::HotReload
    };
    let report = run_async("Reload", 150, async {
        let client = connect(&uri).await?;
        rt::collect_reload_report(&client, &uri, mode).await
    })?;
    if !report.success {
        return Err(format!(
            "{} failed (via {}): {}",
            if a.full_restart {
                "Hot restart"
            } else {
                "Hot reload"
            },
            report.via,
            report.reason.as_deref().unwrap_or("no reason given")
        ));
    }
    let mut v = encode(&report)?;
    if report.via == "vm service" {
        v["note"] = json!(
            "Reloaded through the VM Service, which does not recompile edited sources. \
             Start the app with run_app (or attach to its flutter run session) for real hot reload."
        );
    }
    Ok(v)
}

fn execute_widget_tree(args: &Value) -> Result<Value, String> {
    let a: WidgetTreeArgs = parse("widget_tree", args)?;
    let (uri, _) = resolve_target(a.attach.as_deref(), a.path.as_deref())?;
    let limit = a.limit.clamp(1, 1000);
    match a.mode.as_str() {
        "visible" => run_async("Reading the screen", 30, async {
            let client = connect(&uri).await?;
            crate::runtime::interact::describe(&client, limit).await
        }),
        "inspector" => run_async("Reading the widget tree", 30, async {
            let client = connect(&uri).await?;
            Ok(serde_json::to_value(
                rt::collect_inspector_report(&client, &uri).await?,
            )?)
        }),
        other => Err(format!("Unknown mode '{other}': use visible or inspector")),
    }
}

fn execute_logs(args: &Value) -> Result<Value, String> {
    let a: LogsArgs = parse("logs", args)?;
    let limit = a.limit.clamp(1, 2000);
    if a.attach.is_none() {
        if let Some(v) = app_session::session_logs(a.since, a.errors_only, limit) {
            return Ok(v);
        }
    }
    let (uri, _) = resolve_target(a.attach.as_deref(), a.path.as_deref())?;
    let secs = a.listen_secs.clamp(1, MAX_WINDOW_SECS);
    let report = run_async("Collecting logs", secs + 20, async {
        let client = connect(&uri).await?;
        rt::collect_logging_report(&client, &uri, Duration::from_secs(secs)).await
    })?;
    let mut v = encode(&report)?;
    v["source"] = json!("vm service");
    Ok(v)
}

fn execute_tree_diff(args: &Value) -> Result<Value, String> {
    let a: TimedArgs = parse("tree_diff", args)?;
    if a.kind.is_some() {
        return Err("Invalid tree_diff arguments: unknown field `kind`".to_string());
    }
    let (uri, _) = resolve_target(a.attach.as_deref(), a.path.as_deref())?;
    let secs = a.duration_secs.clamp(1, MAX_WINDOW_SECS);
    let report = run_async("Tree diff", secs + 30, async {
        let client = connect(&uri).await?;
        rt::collect_tree_diff(&client, &uri, Duration::from_secs(secs)).await
    })?;
    encode(&report)
}

fn execute_app_metrics(args: &Value) -> Result<Value, String> {
    let a: TimedArgs = parse("app_metrics", args)?;
    let kind = a
        .kind
        .clone()
        .ok_or_else(|| format!("`kind` is required: one of {}", METRIC_KINDS.join(", ")))?;
    if !METRIC_KINDS.contains(&kind.as_str()) {
        return Err(format!(
            "Unknown kind '{kind}': use one of {}",
            METRIC_KINDS.join(", ")
        ));
    }
    let (uri, _) = resolve_target(a.attach.as_deref(), a.path.as_deref())?;
    let secs = a.duration_secs.clamp(1, MAX_WINDOW_SECS);
    let window = Duration::from_secs(secs);
    let jank = a.jank_ms;
    run_async("Measuring the app", secs + 30, async {
        let client = connect(&uri).await?;
        Ok(match kind.as_str() {
            "memory" => serde_json::to_value(rt::collect_memory_report(&client, &uri).await?)?,
            "network" => {
                serde_json::to_value(rt::collect_network_report(&client, &uri, window).await?)?
            }
            "performance" => {
                serde_json::to_value(rt::collect_performance_report(&client, &uri, window).await?)?
            }
            "profiler" => {
                serde_json::to_value(rt::collect_profiler_report(&client, &uri, window).await?)?
            }
            "rebuilds" => {
                serde_json::to_value(rt::collect_rebuilds_report(&client, &uri, window).await?)?
            }
            "trace" => {
                serde_json::to_value(rt::collect_trace_report(&client, &uri, window, jank).await?)?
            }
            _ => serde_json::to_value(rt::collect_route_log(&client, &uri, window).await?)?,
        })
    })
}

fn execute_journey(args: &Value) -> Result<Value, String> {
    let a: JourneyArgs = parse("journey", args)?;
    let (uri, project) = resolve_target(a.attach.as_deref(), a.path.as_deref())?;
    let secs = a.duration_secs.clamp(1, MAX_WINDOW_SECS);
    let output_dir = a
        .output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| temp_output("falcon-mcp-journeys", "journey", ""));
    let config = crate::runtime::journey::JourneyConfig {
        project_path: project,
        device: a.device,
        duration: Duration::from_secs(secs),
        interval: Duration::from_secs(a.interval_secs.max(1)),
        output_dir: output_dir.clone(),
        write_html: true,
    };
    super::progress::report(0.0, Some(secs as f64), "recording journey");
    let report = run_async("Journey", secs + 60, async {
        let client = connect(&uri).await?;
        crate::runtime::journey::record_journey(&client, &uri, &config).await
    })?;
    let images: Vec<PathBuf> = report
        .steps
        .iter()
        .filter(|s| s.is_new_screen)
        .filter_map(|s| s.screenshot_file.as_ref())
        .map(|f| {
            let p = PathBuf::from(f);
            if p.is_absolute() {
                p
            } else {
                output_dir.join(p)
            }
        })
        .filter(|p| p.is_file())
        .take(a.max_images.min(10))
        .collect();
    let mut v = encode(&report)?;
    if !images.is_empty() {
        v[MCP_IMAGES_KEY] = png_images(&images)?;
    }
    Ok(v)
}

fn execute_verify(args: &Value) -> Result<Value, String> {
    let a: VerifyArgs = parse("verify", args)?;
    let output_dir = a
        .output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| temp_output("falcon-mcp-verify", "verify", ""));
    let timeout = a.timeout_secs.clamp(10, 1800);
    let config = crate::runtime::verify::VerifyConfig {
        physical_camera: a.physical_camera,
        project: PathBuf::from(&a.path),
        target: PathBuf::from(&a.target),
        device: a.device,
        timeout: Duration::from_secs(timeout),
        output_dir: output_dir.clone(),
        flutter: PathBuf::from("flutter"),
    };
    super::progress::report(0.0, None, "running integration test");
    let report = run_async(
        "Verify",
        timeout + 30,
        crate::runtime::verify::verify(config),
    )?;
    let mut v = encode(&report)?;
    v["output_dir"] = json!(output_dir.display().to_string());
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_refused_remotely() {
        for def in definitions() {
            let err = dispatch(&def.name, &json!({}), Trust::Remote)
                .expect("owned tool")
                .unwrap_err();
            assert!(err.contains("not permitted"), "{}: {}", def.name, err);
        }
    }

    #[test]
    fn schemas_are_strict_objects() {
        for def in definitions() {
            assert_eq!(def.input_schema["type"], "object", "{}", def.name);
            assert_eq!(
                def.input_schema["additionalProperties"],
                json!(false),
                "{}",
                def.name
            );
        }
    }

    #[test]
    fn unknown_args_are_rejected() {
        for name in [
            "reload",
            "widget_tree",
            "logs",
            "tree_diff",
            "app_metrics",
            "journey",
            "run_app",
            "stop_app",
        ] {
            let err = dispatch(name, &json!({"bogus": 1}), Trust::Local)
                .unwrap()
                .unwrap_err();
            assert!(
                err.contains("bogus")
                    || err.contains("unknown field")
                    || err.contains("takes none"),
                "{name}: {err}"
            );
        }
    }

    #[test]
    fn app_metrics_validates_kind() {
        let err = execute_app_metrics(&json!({})).unwrap_err();
        assert!(err.contains("required"), "{}", err);
        let err = execute_app_metrics(&json!({"kind": "gpu"})).unwrap_err();
        assert!(err.contains("Unknown kind"), "{}", err);
    }

    #[test]
    fn verify_requires_target_and_device() {
        let err = execute_verify(&json!({"path": "."})).unwrap_err();
        assert!(err.contains("missing field"), "{}", err);
    }

    #[test]
    fn widget_tree_rejects_unknown_mode() {
        let err = execute_widget_tree(&json!({"attach": "http://127.0.0.1:1/x=/", "mode": "xml"}))
            .unwrap_err();
        assert!(err.contains("Unknown mode"), "{}", err);
    }

    #[test]
    fn not_ours_returns_none() {
        assert!(dispatch("lint_file", &json!({}), Trust::Local).is_none());
    }
}
