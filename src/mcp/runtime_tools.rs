//! Shared plumbing for MCP tools that act on a running Flutter app, plus the
//! `screenshot` tool.
//!
//! Every runtime tool targets the app started by `run_app`, or any debug app
//! whose VM Service URI the caller passes as `attach`. None of them launches
//! `flutter run` on its own, each call has a deadline, and all of them are
//! refused over `Trust::Remote` (the HTTP bridge): they write files and talk
//! to a local debug port.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::schema::JSON_SCHEMA_DRAFT_07;
use crate::doctor::Trust;
use crate::runtime::connection::VmServiceClient;

/// Key under which a tool result carries images for the MCP server to turn
/// into `{"type":"image"}` content blocks. The server strips it from the text.
pub const MCP_IMAGES_KEY: &str = "_mcp_images";

/// Refuse a runtime tool over an untrusted transport.
pub fn refuse_remote(trust: Trust, tool: &str) -> Result<(), String> {
    if trust == Trust::Remote {
        return Err(format!(
            "{tool} is not permitted over this transport; call it from the MCP stdio server \
             (falcon-mcp)"
        ));
    }
    Ok(())
}

/// The VM Service URI and project directory a runtime call should use.
pub fn resolve_target(
    attach: Option<&str>,
    path: Option<&str>,
) -> Result<(String, PathBuf), String> {
    let uri = match attach.map(str::trim).filter(|a| !a.is_empty()) {
        Some(a) => a.to_string(),
        None => super::app_session::current_uri().ok_or(
            "No running app: call run_app first, or pass `attach` with the VM Service URI \
             printed by `flutter run`",
        )?,
    };
    let project = path
        .map(PathBuf::from)
        .or_else(super::app_session::current_project)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok((uri, project))
}

/// Run `fut` on a fresh runtime with a deadline of `secs` seconds.
pub fn run_async<T, F>(what: &str, secs: u64, fut: F) -> Result<T, String>
where
    F: Future<Output = anyhow::Result<T>>,
{
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to start async runtime: {}", e))?;
    match rt.block_on(async { tokio::time::timeout(Duration::from_secs(secs), fut).await }) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(format!("{what} failed: {:#}", e)),
        Err(_) => Err(format!(
            "{what} timed out after {secs}s; is the app still running?"
        )),
    }
}

/// Connect to the app's VM Service. With a URI, `connect_client` never
/// launches the app, so the project path is unused.
pub async fn connect(uri: &str) -> anyhow::Result<VmServiceClient> {
    let (_, client) = crate::runtime::tools::connect_client(Path::new("."), Some(uri)).await?;
    Ok(client)
}

/// Read PNG files into the value stored under [`MCP_IMAGES_KEY`].
pub fn png_images(paths: &[PathBuf]) -> Result<Value, String> {
    let mut images = Vec::new();
    for path in paths {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read image {}: {}", path.display(), e))?;
        images.push(json!({ "data": STANDARD.encode(bytes), "mimeType": "image/png" }));
    }
    Ok(Value::Array(images))
}

/// A new file path under the temp directory, e.g. for a screenshot.
pub fn temp_output(dir: &str, prefix: &str, ext: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let name = format!("{}-{}-{}", prefix, std::process::id(), stamp);
    let name = if ext.is_empty() {
        name
    } else {
        format!("{name}.{ext}")
    };
    std::env::temp_dir().join(dir).join(name)
}

/// Schema properties every runtime tool accepts.
pub fn target_properties() -> serde_json::Map<String, Value> {
    let mut props = serde_json::Map::new();
    props.insert(
        "attach".into(),
        json!({
            "type": "string",
            "description": "VM Service URI of a running debug app. Omit to use the app started by run_app."
        }),
    );
    props.insert(
        "path".into(),
        json!({
            "type": "string",
            "description": "Flutter project directory. Defaults to the run_app project, else '.'"
        }),
    );
    props
}

/// A strict object schema: the target properties plus `extra`.
pub fn runtime_schema(extra: Value, required: &[&str]) -> Value {
    let mut props = target_properties();
    if let Value::Object(extra) = extra {
        props.extend(extra);
    }
    json!({
        "$schema": JSON_SCHEMA_DRAFT_07,
        "type": "object",
        "additionalProperties": false,
        "properties": props,
        "required": required,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<String>,
    #[serde(default = "default_true")]
    pub include_image: bool,
}

fn default_true() -> bool {
    true
}

pub fn screenshot_input_schema() -> Value {
    runtime_schema(
        json!({
            "device": { "type": "string", "description": "Device id for the `flutter screenshot` fallback" },
            "out": { "type": "string", "description": "Where to write the PNG. Defaults to a new file in the system temp directory." },
            "include_image": { "type": "boolean", "default": true, "description": "Return the PNG inline so you can see it (default true)" }
        }),
        &[],
    )
}

pub const SCREENSHOT_DESCRIPTION: &str = "Capture a PNG of the running Flutter app and return it as an image, so you can see the screen you just changed. Uses the app started by run_app, or `attach`. Flutter web is not supported.";

/// The `screenshot` MCP tool.
pub fn execute_screenshot(args: &Value, trust: Trust) -> Result<Value, String> {
    refuse_remote(trust, "screenshot")?;
    let args: ScreenshotArgs = serde_json::from_value(args.clone())
        .map_err(|e| format!("Invalid screenshot arguments: {}", e))?;
    let (uri, project) = resolve_target(args.attach.as_deref(), args.path.as_deref())?;
    let out = args
        .out
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| temp_output("falcon-mcp-screenshots", "screenshot", "png"));

    let report = capture(&uri, &project, &out, args.device.as_deref())?;
    let mut result =
        serde_json::to_value(&report).map_err(|e| format!("Failed to encode report: {}", e))?;
    if args.include_image {
        result[MCP_IMAGES_KEY] = png_images(&[out])?;
    }
    Ok(result)
}

fn capture(
    uri: &str,
    project: &Path,
    out: &Path,
    device: Option<&str>,
) -> Result<crate::runtime::tools::ScreenshotToolReport, String> {
    run_async("Screenshot", 30, async {
        let client = connect(uri).await?;
        crate::runtime::tools::collect_screenshot(&client, uri, out, project, device).await
    })
}

/// Turn a tool result into MCP `content` blocks: the JSON as text, plus one
/// image block per entry under [`MCP_IMAGES_KEY`].
pub fn into_content_blocks(mut result: Value) -> Vec<Value> {
    let images = result
        .as_object_mut()
        .and_then(|obj| obj.remove(MCP_IMAGES_KEY));
    let mut blocks = vec![json!({
        "type": "text",
        "text": serde_json::to_string_pretty(&result).unwrap_or_default(),
    })];
    for image in images
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(data), Some(mime)) = (image.get("data"), image.get("mimeType")) {
            blocks.push(json!({ "type": "image", "data": data, "mimeType": mime }));
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_trust_is_refused_before_anything_runs() {
        let err = execute_screenshot(&json!({"attach": "http://127.0.0.1:1/x=/"}), Trust::Remote)
            .unwrap_err();
        assert!(err.contains("not permitted"), "{}", err);
    }

    #[test]
    fn without_attach_or_session_the_error_says_what_to_do() {
        let err = execute_screenshot(&json!({}), Trust::Local).unwrap_err();
        assert!(err.contains("run_app"), "{}", err);
    }

    #[test]
    fn unknown_args_are_rejected() {
        let err = execute_screenshot(
            &json!({"attach": "http://127.0.0.1:1/x=/", "launch": true}),
            Trust::Local,
        )
        .unwrap_err();
        assert!(err.contains("unknown field"), "{}", err);
    }

    #[test]
    fn runtime_schema_is_strict_and_has_target_fields() {
        let schema = screenshot_input_schema();
        assert_eq!(schema["additionalProperties"], json!(false));
        assert!(schema["properties"]["attach"].is_object());
        assert!(schema["properties"]["out"].is_object());
    }

    #[test]
    fn temp_output_is_under_temp() {
        let path = temp_output("falcon-mcp-screenshots", "screenshot", "png");
        assert!(path.starts_with(std::env::temp_dir()));
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
    }

    #[test]
    fn content_blocks_split_out_every_image() {
        let result = json!({
            "method": "rasterizer",
            MCP_IMAGES_KEY: [
                {"data": "aGk=", "mimeType": "image/png"},
                {"data": "aGo=", "mimeType": "image/png"}
            ]
        });
        let blocks = into_content_blocks(result);
        assert_eq!(blocks.len(), 3);
        let text = blocks[0]["text"].as_str().unwrap();
        assert!(text.contains("rasterizer") && !text.contains(MCP_IMAGES_KEY));
        assert_eq!(
            blocks[2],
            json!({"type": "image", "data": "aGo=", "mimeType": "image/png"})
        );
    }

    #[test]
    fn content_blocks_without_image_are_text_only() {
        let blocks = into_content_blocks(json!({"ok": true}));
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn unreachable_app_fails_with_a_clear_error() {
        let out = std::env::temp_dir().join("falcon-mcp-screenshot-test-never-written.png");
        let err = execute_screenshot(
            &json!({"attach": "http://127.0.0.1:1/x=/", "out": out.to_string_lossy()}),
            Trust::Local,
        )
        .unwrap_err();
        assert!(err.starts_with("Screenshot failed"), "{}", err);
        assert!(!out.exists());
    }
}
