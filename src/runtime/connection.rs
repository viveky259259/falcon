//! Flutter daemon connection and Dart VM Service client.
//!
//! Provides:
//! - `launch_flutter_run` — starts `flutter run` and extracts the VM Service URI.
//! - `VmServiceClient` — a lightweight JSON-RPC client for the Dart VM Service Protocol.

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use percent_encoding::percent_decode_str;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::{protocol::WebSocketConfig, Message};
use tokio_tungstenite::{connect_async_with_config, MaybeTlsStream, WebSocketStream};
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
const VM_SERVICE_MAX_MESSAGE_SIZE: usize = 256 << 20;
const VM_SERVICE_MAX_FRAME_SIZE: usize = 64 << 20;

// ──────────────────────────────────────────────────────────────────────
// Launch helpers
// ──────────────────────────────────────────────────────────────────────

/// Launch `flutter run` in the given project directory and return the
/// Observatory / VM Service URI once it becomes available.
pub async fn launch_flutter_run(project_path: &Path) -> Result<String> {
    let mut child = Command::new("flutter")
        .arg("run")
        .arg("--observatory-port=0")
        .current_dir(project_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to launch `flutter run`. Is Flutter on PATH?")?;

    let stdout = child.stdout.take().context("No stdout from flutter run")?;
    let mut reader = BufReader::new(stdout).lines();

    let uri = tokio::time::timeout(std::time::Duration::from_secs(120), async {
        while let Some(line) = reader.next_line().await? {
            if let Some(uri) = extract_vm_service_uri(&line) {
                return Ok(uri);
            }
        }
        anyhow::bail!("flutter run exited without printing a VM Service URI")
    })
    .await
    .context("Timed out waiting for flutter run to start (120s)")??;

    Ok(uri)
}

fn extract_vm_service_uri(line: &str) -> Option<String> {
    [
        "http://127.0.0.1",
        "http://localhost",
        "https://127.0.0.1",
        "https://localhost",
    ]
    .iter()
    .find_map(|prefix| {
        line.find(prefix).map(|idx| {
            line[idx..]
                .split_whitespace()
                .next()
                .unwrap_or(&line[idx..])
                .trim_end_matches('/')
                .to_string()
        })
    })
}

// ──────────────────────────────────────────────────────────────────────
// URI normalization
// ──────────────────────────────────────────────────────────────────────

fn sanitize_input(input: &str) -> String {
    input
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .replace(r"\?", "?")
        .replace(r"\=", "=")
        .replace(r"\&", "&")
}

pub fn normalize_vm_service_uri(input: &str) -> Result<Url> {
    let mut value = sanitize_input(input);

    if let Ok(parsed) = Url::parse(&value) {
        if let Some(uri_value) = parsed
            .query_pairs()
            .find(|(key, _)| key == "uri")
            .map(|(_, value)| value.into_owned())
        {
            value = uri_value;
        }
    }

    if value.contains("%3A%2F%2F") || value.contains("%3a%2f%2f") {
        value = percent_decode_str(&value)
            .decode_utf8()
            .context("Failed to decode percent-encoded VM service URI")?
            .into_owned();
    }

    let mut uri =
        Url::parse(value.trim()).with_context(|| format!("Invalid VM Service URI: {value}"))?;
    uri.set_fragment(None);

    Ok(uri)
}

pub fn convert_to_websocket_url(service_protocol_url: &Url) -> Url {
    let mut url = service_protocol_url.clone();
    let secure = matches!(url.scheme(), "https" | "wss");
    let scheme = if secure { "wss" } else { "ws" };
    let path = if url.path().ends_with("/ws") {
        url.path().to_string()
    } else if url.path().ends_with('/') {
        format!("{}ws", url.path())
    } else {
        format!("{}/ws", url.path())
    };

    url.set_scheme(scheme).expect("valid websocket scheme");
    url.set_path(&path);
    url
}

// ──────────────────────────────────────────────────────────────────────
// VM Service client
// ──────────────────────────────────────────────────────────────────────

pub struct VmServiceClient {
    socket: Mutex<WsStream>,
    notification_buffer: Mutex<Vec<Value>>,
    next_id: AtomicU64,
    /// The main isolate ID discovered at connect time.
    pub isolate_id: String,
}

impl VmServiceClient {
    /// Connect to the Dart VM Service at the given URI (http://, ws://, or a
    /// DevTools-prefixed URL containing `?uri=`).
    pub async fn connect(uri: &str) -> Result<Self> {
        let normalized_uri = normalize_vm_service_uri(uri)?;
        let websocket_uri = convert_to_websocket_url(&normalized_uri);

        let config = WebSocketConfig {
            max_message_size: Some(VM_SERVICE_MAX_MESSAGE_SIZE),
            max_frame_size: Some(VM_SERVICE_MAX_FRAME_SIZE),
            ..WebSocketConfig::default()
        };

        let (socket, _) = connect_async_with_config(websocket_uri.as_str(), Some(config), false)
            .await
            .with_context(|| format!("Cannot connect to VM Service at {}", websocket_uri))?;

        let client = Self {
            socket: Mutex::new(socket),
            notification_buffer: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
            isolate_id: String::new(),
        };

        client
            .call("getVersion", json!({}))
            .await
            .context("Connected transport but VM service did not respond to getVersion")?;

        let vm_info = client.call("getVM", json!({})).await?;
        let isolate_id = client.detect_main_isolate(&vm_info).await?;

        Ok(Self {
            socket: client.socket,
            notification_buffer: client.notification_buffer,
            next_id: client.next_id,
            isolate_id,
        })
    }

    async fn detect_main_isolate(&self, vm_info: &Value) -> Result<String> {
        let isolates = vm_info["isolates"]
            .as_array()
            .context("getVM response did not contain isolates")?;

        let first_isolate_id = isolates
            .first()
            .and_then(|iso| iso["id"].as_str())
            .unwrap_or("isolates/0")
            .to_string();

        for isolate in isolates {
            let Some(isolate_id) = isolate["id"].as_str() else {
                continue;
            };
            if let Ok(details) = self
                .call("getIsolate", json!({ "isolateId": isolate_id }))
                .await
            {
                let flutter_isolate = details["extensionRPCs"]
                    .as_array()
                    .map(|exts| {
                        exts.iter().any(|ext| {
                            ext.as_str()
                                .map(|ext| ext.starts_with("ext.flutter"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
                if flutter_isolate {
                    return Ok(isolate_id.to_string());
                }
            }
        }

        if let Some(main_named) = isolates.iter().find_map(|iso| {
            let name = iso["name"].as_str()?;
            let id = iso["id"].as_str()?;
            name.contains(":main(").then_some(id.to_string())
        }) {
            return Ok(main_named);
        }

        Ok(first_isolate_id)
    }

    /// Enable Flutter-specific service extensions required for diagnostics.
    pub async fn enable_extensions(&self) -> Result<()> {
        let _ = self
            .call(
                "ext.flutter.debugAllowBanner",
                json!({"isolateId": self.isolate_id, "enabled": "false"}),
            )
            .await;

        let _ = self
            .call(
                "setVMTimelineFlags",
                json!({"recordedStreams": ["Dart", "Embedder", "GC"]}),
            )
            .await;

        Ok(())
    }

    // ── Memory diagnostics ──────────────────────────────────────────

    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        let resp = self
            .call("getMemoryUsage", json!({"isolateId": self.isolate_id}))
            .await?;

        Ok(MemoryUsage {
            heap_usage_bytes: resp["heapUsage"].as_u64().unwrap_or(0),
            heap_capacity_bytes: resp["heapCapacity"].as_u64().unwrap_or(0),
            external_usage_bytes: resp["externalUsage"].as_u64().unwrap_or(0),
        })
    }

    pub async fn get_allocation_profile(&self) -> Result<Value> {
        self.call(
            "getAllocationProfile",
            json!({"isolateId": self.isolate_id}),
        )
        .await
    }

    pub async fn get_process_memory_usage(&self) -> Result<Value> {
        self.call("getProcessMemoryUsage", json!({})).await
    }

    pub async fn get_isolate(&self) -> Result<Value> {
        self.call("getIsolate", json!({ "isolateId": self.isolate_id }))
            .await
    }

    // ── Rendering diagnostics ───────────────────────────────────────

    pub async fn get_rendering_stats(&self) -> Result<RenderingStats> {
        let resp = self
            .call(
                "ext.flutter.inspector.getRenderStats",
                json!({"isolateId": self.isolate_id}),
            )
            .await
            .unwrap_or_else(|_| json!({}));

        Ok(RenderingStats {
            total_frames: resp["totalFrames"].as_u64().unwrap_or(0),
            dropped_frames: resp["droppedFrames"].as_u64().unwrap_or(0),
            avg_frame_build_time_ms: resp["avgFrameBuildTimeMs"].as_f64().unwrap_or(0.0),
            max_frame_build_time_ms: resp["maxFrameBuildTimeMs"].as_f64().unwrap_or(0.0),
            avg_frame_raster_time_ms: resp["avgFrameRasterTimeMs"].as_f64().unwrap_or(0.0),
            max_frame_raster_time_ms: resp["maxFrameRasterTimeMs"].as_f64().unwrap_or(0.0),
        })
    }

    pub async fn get_rebuild_counts(&self) -> Result<Value> {
        self.call(
            "ext.flutter.inspector.getWidgetRebuildCounts",
            json!({"isolateId": self.isolate_id}),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    /// Fetch the root widget tree from the Flutter inspector.
    /// Tries the new `getRootWidgetTree` first, falling back to `getRootWidgetSummaryTree`
    /// for older Flutter SDKs.
    pub async fn get_root_widget_tree(&self) -> Result<Value> {
        if let Ok(value) = self
            .call(
                "ext.flutter.inspector.getRootWidgetTree",
                json!({
                    "isolateId": self.isolate_id,
                    "groupName": "falcon",
                    "isSummaryTree": true,
                    "withPreviews": false,
                }),
            )
            .await
        {
            return Ok(value);
        }
        self.call(
            "ext.flutter.inspector.getRootWidgetSummaryTree",
            json!({"isolateId": self.isolate_id, "groupName": "falcon"}),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    /// Fetch the currently-selected widget node from the inspector, if any.
    pub async fn get_selected_widget(&self) -> Result<Value> {
        self.call(
            "ext.flutter.inspector.getSelectedWidget",
            json!({"isolateId": self.isolate_id, "groupName": "falcon"}),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    /// Trigger a hot reload via `reloadSources`. When `force` is true, the VM reloads
    /// even if no source changes are detected; `pause` keeps execution paused after reload.
    pub async fn reload_sources(&self, force: bool, pause: bool) -> Result<Value> {
        self.call(
            "reloadSources",
            json!({
                "isolateId": self.isolate_id,
                "force": force,
                "pause": pause,
            }),
        )
        .await
    }

    /// Re-runs `build` for every dirty widget — call after `reload_sources` for a
    /// real hot reload. Implemented as the standard Flutter `ext.flutter.reassemble` extension.
    pub async fn flutter_reassemble(&self) -> Result<Value> {
        self.call(
            "ext.flutter.reassemble",
            json!({"isolateId": self.isolate_id}),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    // ── Screenshot ──────────────────────────────────────────────────

    /// Capture a screenshot of the running app via the engine RPC
    /// `_flutter.screenshot`, which renders the current layer tree to a PNG and
    /// returns it base64-encoded under the `screenshot` field. Not every
    /// embedder registers this RPC (e.g. some headless/test modes), so callers
    /// should treat an error as "unsupported on this device".
    pub async fn capture_screenshot(&self) -> Result<String> {
        let resp = self
            .call("_flutter.screenshot", json!({}))
            .await
            .context("Engine RPC `_flutter.screenshot` failed — the running device may not support screenshots")?;

        resp["screenshot"]
            .as_str()
            .map(ToString::to_string)
            .context("`_flutter.screenshot` response did not contain a `screenshot` field")
    }

    // ── CPU / timeline diagnostics ──────────────────────────────────

    pub async fn get_cpu_samples(&self) -> Result<Value> {
        self.call(
            "getCpuSamples",
            json!({
                "isolateId": self.isolate_id,
                "timeOriginMicros": 0,
                "timeExtentMicros": 999_999_999_999i64,
            }),
        )
        .await
    }

    pub async fn clear_cpu_samples(&self) -> Result<Value> {
        self.call("clearCpuSamples", json!({"isolateId": self.isolate_id}))
            .await
    }

    pub async fn set_flag(&self, name: &str, value: &str) -> Result<Value> {
        self.call("setFlag", json!({ "name": name, "value": value }))
            .await
    }

    pub async fn get_vm_timeline_micros(&self) -> Result<i64> {
        let response = self.call("getVMTimelineMicros", json!({})).await?;
        response["timestamp"]
            .as_i64()
            .or_else(|| response["timestamp"].as_u64().map(|ts| ts as i64))
            .context("VM Service getVMTimelineMicros response missing timestamp")
    }

    pub async fn clear_vm_timeline(&self) -> Result<Value> {
        self.call("clearVMTimeline", json!({})).await
    }

    pub async fn set_vm_timeline_flags(&self, recorded_streams: &[&str]) -> Result<Value> {
        self.call(
            "setVMTimelineFlags",
            json!({ "recordedStreams": recorded_streams }),
        )
        .await
    }

    pub async fn get_vm_timeline_range(
        &self,
        start_micros: i64,
        extent_micros: i64,
    ) -> Result<Value> {
        self.call(
            "getVMTimeline",
            json!({
                "timeOriginMicros": start_micros,
                "timeExtentMicros": extent_micros,
            }),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    pub async fn get_vm_timeline(&self) -> Result<Value> {
        self.call("getVMTimeline", json!({}))
            .await
            .or_else(|_| Ok(json!({})))
    }

    // ── Network diagnostics ─────────────────────────────────────────

    pub async fn get_http_timeline(&self) -> Result<Value> {
        self.call(
            "ext.dart.io.getHttpProfile",
            json!({"isolateId": self.isolate_id}),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    pub async fn clear_http_profile(&self) -> Result<Value> {
        self.call(
            "ext.dart.io.clearHttpProfile",
            json!({ "isolateId": self.isolate_id }),
        )
        .await
    }

    pub async fn enable_http_timeline_logging(&self, enabled: bool) -> Result<Value> {
        self.call(
            "ext.dart.io.httpEnableTimelineLogging",
            json!({
                "isolateId": self.isolate_id,
                "enabled": enabled.to_string(),
            }),
        )
        .await
    }

    pub async fn get_socket_profile(&self) -> Result<Value> {
        self.call(
            "ext.dart.io.getSocketProfile",
            json!({ "isolateId": self.isolate_id }),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    pub async fn clear_socket_profile(&self) -> Result<Value> {
        self.call(
            "ext.dart.io.clearSocketProfile",
            json!({ "isolateId": self.isolate_id }),
        )
        .await
    }

    pub async fn enable_socket_profiling(&self, enabled: bool) -> Result<Value> {
        self.call(
            "ext.dart.io.socketProfilingEnabled",
            json!({
                "isolateId": self.isolate_id,
                "enabled": enabled.to_string(),
            }),
        )
        .await
    }

    pub async fn get_stack(&self, limit: Option<usize>) -> Result<Value> {
        let mut params = json!({ "isolateId": self.isolate_id });
        if let Some(limit) = limit {
            params["limit"] = json!(limit);
        }
        self.call("getStack", params).await
    }

    pub async fn pause(&self) -> Result<Value> {
        self.call("pause", json!({ "isolateId": self.isolate_id }))
            .await
    }

    pub async fn resume(&self, step: Option<&str>) -> Result<Value> {
        let mut params = json!({ "isolateId": self.isolate_id });
        if let Some(step) = step {
            params["step"] = json!(step);
        }
        self.call("resume", params).await
    }

    pub async fn stream_listen(&self, stream_id: &str) -> Result<Value> {
        self.call("streamListen", json!({ "streamId": stream_id }))
            .await
    }

    pub async fn collect_stream_events(
        &self,
        stream_ids: &[&str],
        duration: Duration,
        max_events: usize,
    ) -> Result<Vec<Value>> {
        let deadline = Instant::now() + duration;
        let mut socket = self.socket.lock().await;

        for stream_id in stream_ids {
            let _ = self
                .call_with_socket(
                    &mut socket,
                    "streamListen",
                    json!({ "streamId": stream_id }),
                )
                .await;
        }

        let mut events = self.drain_buffered_notifications(max_events).await;
        while Instant::now() < deadline && events.len() < max_events {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let next = match tokio::time::timeout(remaining, socket.next()).await {
                Ok(value) => value,
                Err(_) => break,
            };

            let Some(message) = next else {
                break;
            };

            match message.context("Failed to read from VM Service websocket")? {
                Message::Text(text) => {
                    if let Some(event) = parse_stream_notification(&text)? {
                        events.push(event);
                    }
                }
                Message::Binary(bytes) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        if let Some(event) = parse_stream_notification(&text)? {
                            events.push(event);
                        }
                    }
                }
                Message::Close(frame) => {
                    if let Some(frame) = frame {
                        bail!("VM Service connection closed: {}", frame.reason);
                    }
                    bail!("VM Service connection closed");
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }

        Ok(events)
    }

    async fn drain_buffered_notifications(&self, max_events: usize) -> Vec<Value> {
        let mut buffered = self.notification_buffer.lock().await;
        if max_events == 0 || buffered.is_empty() {
            return Vec::new();
        }

        let take_count = buffered.len().min(max_events);
        buffered.drain(..take_count).collect()
    }

    // ── Low-level RPC ───────────────────────────────────────────────

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let mut socket = self.socket.lock().await;
        self.call_with_socket(&mut socket, method, params).await
    }

    async fn call_with_socket(
        &self,
        socket: &mut WsStream,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let payload = serde_json::to_string(&request)?;

        socket
            .send(Message::Text(payload))
            .await
            .context("Failed to write to VM Service websocket")?;

        while let Some(message) = socket.next().await {
            match message.context("Failed to read from VM Service websocket")? {
                Message::Text(text) => {
                    if let Some(result) = parse_response(&text, id)? {
                        return Ok(result);
                    }
                    if let Some(event) = parse_stream_notification(&text)? {
                        self.notification_buffer.lock().await.push(event);
                    }
                }
                Message::Binary(bytes) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        if let Some(result) = parse_response(&text, id)? {
                            return Ok(result);
                        }
                        if let Some(event) = parse_stream_notification(&text)? {
                            self.notification_buffer.lock().await.push(event);
                        }
                    }
                }
                Message::Close(frame) => {
                    if let Some(frame) = frame {
                        bail!("VM Service connection closed: {}", frame.reason);
                    }
                    bail!("VM Service connection closed");
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }

        bail!("VM Service connection closed")
    }
}

fn parse_response(message: &str, expected_id: u64) -> Result<Option<Value>> {
    let Ok(resp) = serde_json::from_str::<Value>(message) else {
        return Ok(None);
    };

    if resp.get("id").and_then(|id| id.as_u64()) != Some(expected_id) {
        return Ok(None);
    }

    if let Some(err) = resp.get("error") {
        bail!("VM Service error: {err}");
    }

    Ok(Some(resp["result"].clone()))
}

fn parse_stream_notification(message: &str) -> Result<Option<Value>> {
    let Ok(resp) = serde_json::from_str::<Value>(message) else {
        return Ok(None);
    };

    if resp.get("method").and_then(|method| method.as_str()) != Some("streamNotify") {
        return Ok(None);
    }

    Ok(resp.get("params").cloned())
}

// ──────────────────────────────────────────────────────────────────────
// Data types returned by the VM Service helpers
// ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MemoryUsage {
    pub heap_usage_bytes: u64,
    pub heap_capacity_bytes: u64,
    pub external_usage_bytes: u64,
}

impl MemoryUsage {
    pub fn heap_usage_mb(&self) -> f64 {
        self.heap_usage_bytes as f64 / (1024.0 * 1024.0)
    }
    pub fn heap_capacity_mb(&self) -> f64 {
        self.heap_capacity_bytes as f64 / (1024.0 * 1024.0)
    }
    pub fn external_usage_mb(&self) -> f64 {
        self.external_usage_bytes as f64 / (1024.0 * 1024.0)
    }
}

#[derive(Debug, Clone)]
pub struct RenderingStats {
    pub total_frames: u64,
    pub dropped_frames: u64,
    pub avg_frame_build_time_ms: f64,
    pub max_frame_build_time_ms: f64,
    pub avg_frame_raster_time_ms: f64,
    pub max_frame_raster_time_ms: f64,
}

impl RenderingStats {
    pub fn dropped_frame_pct(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            (self.dropped_frames as f64 / self.total_frames as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        convert_to_websocket_url, extract_vm_service_uri, normalize_vm_service_uri,
        parse_response, parse_stream_notification, sanitize_input, MemoryUsage, RenderingStats,
    };
    use serde_json::json;

    #[test]
    fn normalize_vm_service_uri_handles_devtools_query_param() {
        let uri = normalize_vm_service_uri(
            "http://127.0.0.1:9101?uri=http%3A%2F%2F127.0.0.1%3A56142%2FHOwgrxalK00%3D%2F",
        )
        .unwrap();
        assert_eq!(uri.as_str(), "http://127.0.0.1:56142/HOwgrxalK00=/");
    }

    #[test]
    fn normalize_vm_service_uri_handles_shell_escaped_query() {
        let uri = normalize_vm_service_uri(
            r#"http://127.0.0.1:52231/Y-c3GOrIQCI=/devtools/\?uri\=ws://127.0.0.1:52231/Y-c3GOrIQCI=/ws"#,
        )
        .unwrap();
        assert_eq!(uri.as_str(), "ws://127.0.0.1:52231/Y-c3GOrIQCI=/ws");
    }

    #[test]
    fn convert_to_websocket_url_matches_vm_service_behavior() {
        let input = normalize_vm_service_uri("http://localhost:123/ABCDEF=").unwrap();
        let output = convert_to_websocket_url(&input);
        assert_eq!(output.as_str(), "ws://localhost:123/ABCDEF=/ws");
    }

    #[test]
    fn parse_stream_notification_extracts_stream_notify_payload() {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "streamNotify",
            "params": {
                "streamId": "Stdout",
                "event": {
                    "kind": "WriteEvent",
                    "bytes": "aGVsbG8="
                }
            }
        })
        .to_string();

        let notification = parse_stream_notification(&payload).unwrap().unwrap();
        assert_eq!(notification["streamId"].as_str(), Some("Stdout"));
        assert_eq!(notification["event"]["kind"].as_str(), Some("WriteEvent"));
    }

    // ── extract_vm_service_uri ──────────────────────────────────────────

    #[test]
    fn extract_vm_service_uri_finds_http_127_0_0_1() {
        let result = extract_vm_service_uri("Some log: http://127.0.0.1:8080/abc/ — done");
        assert_eq!(result, Some("http://127.0.0.1:8080/abc".to_string()));
    }

    #[test]
    fn extract_vm_service_uri_finds_http_localhost() {
        let result = extract_vm_service_uri("VM at http://localhost:9101/xy/");
        assert_eq!(result, Some("http://localhost:9101/xy".to_string()));
    }

    #[test]
    fn extract_vm_service_uri_finds_https_variant() {
        let result = extract_vm_service_uri("https://localhost:443/path/");
        assert_eq!(result, Some("https://localhost:443/path".to_string()));
    }

    #[test]
    fn extract_vm_service_uri_no_match_returns_none() {
        let result = extract_vm_service_uri("plain log line");
        assert_eq!(result, None);
    }

    #[test]
    fn extract_vm_service_uri_strips_trailing_slash() {
        let result = extract_vm_service_uri("http://127.0.0.1:1/foo/");
        let uri = result.expect("should match");
        assert!(!uri.ends_with('/'), "trailing slash should be stripped");
    }

    // ── sanitize_input ─────────────────────────────────────────────────

    #[test]
    fn sanitize_input_trims_whitespace() {
        assert_eq!(sanitize_input("  hello  "), "hello");
    }

    #[test]
    fn sanitize_input_strips_surrounding_double_quotes() {
        assert_eq!(sanitize_input(r#""hello""#), "hello");
    }

    #[test]
    fn sanitize_input_strips_surrounding_single_quotes() {
        assert_eq!(sanitize_input("'hello'"), "hello");
    }

    #[test]
    fn sanitize_input_unescapes_shell_escapes() {
        assert_eq!(sanitize_input(r"a\?b\=c\&d"), "a?b=c&d");
    }

    #[test]
    fn sanitize_input_no_changes_for_clean_input() {
        assert_eq!(sanitize_input("clean"), "clean");
    }

    // ── normalize_vm_service_uri additional ───────────────────────────

    #[test]
    fn normalize_vm_service_uri_strips_fragment() {
        let uri = normalize_vm_service_uri("http://127.0.0.1:1/foo#frag").unwrap();
        assert!(uri.fragment().is_none(), "fragment should be stripped");
    }

    #[test]
    fn normalize_vm_service_uri_rejects_invalid_uri() {
        assert!(normalize_vm_service_uri("not a uri").is_err());
    }

    #[test]
    fn normalize_vm_service_uri_handles_plain_http() {
        let uri = normalize_vm_service_uri("http://localhost:1234").unwrap();
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host_str(), Some("localhost"));
    }

    // ── convert_to_websocket_url additional ───────────────────────────

    #[test]
    fn convert_to_websocket_url_https_becomes_wss() {
        let input = normalize_vm_service_uri("https://x/path/").unwrap();
        let output = convert_to_websocket_url(&input);
        assert!(output.as_str().starts_with("wss://"), "scheme should be wss");
        assert!(output.path().ends_with("/path/ws"), "path should end with /path/ws");
    }

    #[test]
    fn convert_to_websocket_url_appends_ws_to_no_trailing_slash_path() {
        let input = normalize_vm_service_uri("http://x/abc").unwrap();
        let output = convert_to_websocket_url(&input);
        assert!(output.path().ends_with("/abc/ws"), "path should end with /abc/ws");
    }

    #[test]
    fn convert_to_websocket_url_preserves_ws_path() {
        let input = normalize_vm_service_uri("http://x/abc/ws").unwrap();
        let output = convert_to_websocket_url(&input);
        assert!(output.path().ends_with("/abc/ws"), "path should end with /abc/ws");
        assert!(!output.path().ends_with("/abc/ws/ws"), "should not double-append ws");
    }

    // ── parse_response ─────────────────────────────────────────────────

    #[test]
    fn parse_response_returns_result_for_matching_id() {
        let msg = json!({"id": 1u64, "result": {"v": 42}}).to_string();
        let result = parse_response(&msg, 1).unwrap();
        assert_eq!(result, Some(json!({"v": 42})));
    }

    #[test]
    fn parse_response_returns_none_for_mismatched_id() {
        let msg = json!({"id": 1u64, "result": {"v": 42}}).to_string();
        let result = parse_response(&msg, 2).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn parse_response_bails_on_error_field() {
        let msg = json!({"id": 1u64, "error": {"code": -1}}).to_string();
        assert!(parse_response(&msg, 1).is_err());
    }

    #[test]
    fn parse_response_invalid_json_returns_none() {
        let result = parse_response("not json", 1).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn parse_response_missing_id_returns_none() {
        let msg = json!({"result": {}}).to_string();
        let result = parse_response(&msg, 1).unwrap();
        assert_eq!(result, None);
    }

    // ── parse_stream_notification additional ──────────────────────────

    #[test]
    fn parse_stream_notification_returns_none_for_non_stream_method() {
        let msg = json!({"jsonrpc": "2.0", "method": "getVM", "params": {}}).to_string();
        let result = parse_stream_notification(&msg).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn parse_stream_notification_returns_none_for_invalid_json() {
        let result = parse_stream_notification("garbage").unwrap();
        assert_eq!(result, None);
    }

    // ── MemoryUsage methods ────────────────────────────────────────────

    #[test]
    fn memory_usage_heap_usage_mb_converts_bytes() {
        let m = MemoryUsage {
            heap_usage_bytes: 1024 * 1024 * 4,
            heap_capacity_bytes: 0,
            external_usage_bytes: 0,
        };
        assert_eq!(m.heap_usage_mb(), 4.0);
    }

    #[test]
    fn memory_usage_heap_capacity_mb_converts() {
        let m = MemoryUsage {
            heap_usage_bytes: 0,
            heap_capacity_bytes: 1024 * 1024,
            external_usage_bytes: 0,
        };
        assert_eq!(m.heap_capacity_mb(), 1.0);
    }

    #[test]
    fn memory_usage_external_usage_mb_zero() {
        let m = MemoryUsage {
            heap_usage_bytes: 0,
            heap_capacity_bytes: 0,
            external_usage_bytes: 0,
        };
        assert_eq!(m.external_usage_mb(), 0.0);
    }

    // ── RenderingStats methods ─────────────────────────────────────────

    #[test]
    fn rendering_stats_dropped_frame_pct_with_zero_total_returns_zero() {
        let s = RenderingStats {
            total_frames: 0,
            dropped_frames: 0,
            avg_frame_build_time_ms: 0.0,
            max_frame_build_time_ms: 0.0,
            avg_frame_raster_time_ms: 0.0,
            max_frame_raster_time_ms: 0.0,
        };
        assert_eq!(s.dropped_frame_pct(), 0.0);
    }

    #[test]
    fn rendering_stats_dropped_frame_pct_correct_percentage() {
        let s = RenderingStats {
            total_frames: 100,
            dropped_frames: 5,
            avg_frame_build_time_ms: 0.0,
            max_frame_build_time_ms: 0.0,
            avg_frame_raster_time_ms: 0.0,
            max_frame_raster_time_ms: 0.0,
        };
        assert_eq!(s.dropped_frame_pct(), 5.0);
    }

    #[test]
    fn rendering_stats_dropped_frame_pct_full_loss() {
        let s = RenderingStats {
            total_frames: 10,
            dropped_frames: 10,
            avg_frame_build_time_ms: 0.0,
            max_frame_build_time_ms: 0.0,
            avg_frame_raster_time_ms: 0.0,
            max_frame_raster_time_ms: 0.0,
        };
        assert_eq!(s.dropped_frame_pct(), 100.0);
    }
}
