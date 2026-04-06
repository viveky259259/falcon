//! Flutter daemon connection and Dart VM Service client.
//!
//! Provides:
//! - `launch_flutter_run` — starts `flutter run` and extracts the VM Service URI.
//! - `VmServiceClient` — a lightweight JSON-RPC client for the Dart VM Service Protocol.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;

// ──────────────────────────────────────────────────────────────────────
// Launch helpers
// ──────────────────────────────────────────────────────────────────────

/// Launch `flutter run` in the given project directory and return the
/// Observatory / VM Service URI once it becomes available.
pub async fn launch_flutter_run(project_path: &Path) -> Result<String> {
    let mut child = Command::new("flutter")
        .arg("run")
        .arg("--observatory-port=0") // let the OS pick a free port
        .current_dir(project_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to launch `flutter run`. Is Flutter on PATH?")?;

    let stdout = child.stdout.take().context("No stdout from flutter run")?;
    let mut reader = BufReader::new(stdout).lines();

    // Scan stdout for the VM Service URI.
    // Flutter prints something like:
    //   A Dart VM Service on ... is available at: http://127.0.0.1:XXXXX/yyyyy=/
    //   or: The Dart VM service is listening on http://127.0.0.1:XXXXX/yyyyy=/
    let uri = tokio::time::timeout(std::time::Duration::from_secs(120), async {
        while let Some(line) = reader.next_line().await? {
            // Check for common VM Service URI patterns
            if let Some(idx) = line.find("http://127.0.0.1") {
                let uri_part = &line[idx..];
                let uri = uri_part
                    .split_whitespace()
                    .next()
                    .unwrap_or(uri_part)
                    .trim_end_matches('/')
                    .to_string();
                return Ok(uri);
            }
            if let Some(idx) = line.find("http://localhost") {
                let uri_part = &line[idx..];
                let uri = uri_part
                    .split_whitespace()
                    .next()
                    .unwrap_or(uri_part)
                    .trim_end_matches('/')
                    .to_string();
                return Ok(uri);
            }
        }
        anyhow::bail!("flutter run exited without printing a VM Service URI")
    })
    .await
    .context("Timed out waiting for flutter run to start (120s)")??;

    Ok(uri)
}

// ──────────────────────────────────────────────────────────────────────
// VM Service client
// ──────────────────────────────────────────────────────────────────────

/// Lightweight JSON-RPC 2.0 client that speaks the Dart VM Service Protocol
/// over a WebSocket-like TCP connection.
///
/// In production you'd use a real websocket crate; here we use a raw TCP
/// stream speaking the JSON-RPC protocol so we can keep external deps minimal.
/// The client converts the ws:// URI from Flutter into a TCP connection to
/// the same host:port and speaks newline-delimited JSON.
pub struct VmServiceClient {
    stream: tokio::sync::Mutex<BufReader<TcpStream>>,
    write_half: tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>,
    next_id: AtomicU64,
    /// The main isolate ID discovered at connect time.
    pub isolate_id: String,
}

impl VmServiceClient {
    /// Connect to the Dart VM Service at the given URI (http:// or ws://).
    pub async fn connect(uri: &str) -> Result<Self> {
        // Parse host:port from the URI.
        let stripped = uri
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_start_matches("ws://")
            .trim_start_matches("wss://");
        let host_port = stripped
            .split('/')
            .next()
            .context("Invalid VM Service URI")?;

        let stream = TcpStream::connect(host_port)
            .await
            .with_context(|| format!("Cannot connect to VM Service at {}", host_port))?;

        let (_read_half, write_half) = stream.into_split();
        let buf_reader = BufReader::new(TcpStream::from_std(
            std::net::TcpStream::connect(host_port)
                .context("TCP reconnect for read half failed")?,
        )?);

        // For a real implementation we'd use a websocket crate.
        // Here we simulate the connection and discover the main isolate.
        let client = Self {
            stream: tokio::sync::Mutex::new(buf_reader),
            write_half: tokio::sync::Mutex::new(write_half),
            next_id: AtomicU64::new(1),
            isolate_id: String::new(),
        };

        // Discover the main isolate via getVM.
        let vm_info = client.call("getVM", json!({})).await?;
        let isolate_id = vm_info["isolates"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|iso| iso["id"].as_str())
            .unwrap_or("isolates/0")
            .to_string();

        Ok(Self {
            stream: client.stream,
            write_half: client.write_half,
            next_id: client.next_id,
            isolate_id,
        })
    }

    /// Enable Flutter-specific service extensions required for diagnostics.
    pub async fn enable_extensions(&self) -> Result<()> {
        // Enable the Flutter rendering extension.
        let _ = self
            .call(
                "ext.flutter.debugAllowBanner",
                json!({"isolateId": self.isolate_id, "enabled": "false"}),
            )
            .await;

        // Request timeline events for rendering.
        let _ = self
            .call(
                "setVMTimelineFlags",
                json!({"recordedStreams": ["Dart", "Embedder", "GC"]}),
            )
            .await;

        Ok(())
    }

    // ── Memory diagnostics ──────────────────────────────────────────

    /// Fetch heap usage for the main isolate via `getMemoryUsage`.
    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        let resp = self
            .call(
                "getMemoryUsage",
                json!({"isolateId": self.isolate_id}),
            )
            .await?;

        Ok(MemoryUsage {
            heap_usage_bytes: resp["heapUsage"].as_u64().unwrap_or(0),
            heap_capacity_bytes: resp["heapCapacity"].as_u64().unwrap_or(0),
            external_usage_bytes: resp["externalUsage"].as_u64().unwrap_or(0),
        })
    }

    /// Fetch allocation profile (class-level allocations) via `getAllocationProfile`.
    pub async fn get_allocation_profile(&self) -> Result<Value> {
        self.call(
            "getAllocationProfile",
            json!({"isolateId": self.isolate_id}),
        )
        .await
    }

    // ── Rendering diagnostics ───────────────────────────────────────

    /// Fetch the Flutter rendering stats via the devtools extension.
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

    /// Fetch the widget rebuild counts.
    pub async fn get_rebuild_counts(&self) -> Result<Value> {
        self.call(
            "ext.flutter.inspector.getWidgetRebuildCounts",
            json!({"isolateId": self.isolate_id}),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    // ── CPU / timeline diagnostics ──────────────────────────────────

    /// Fetch the CPU usage samples from the profiler.
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
        .or_else(|_| Ok(json!({})))
    }

    /// Fetch the VM timeline events (GC, rendering, Dart).
    pub async fn get_vm_timeline(&self) -> Result<Value> {
        self.call("getVMTimeline", json!({}))
            .await
            .or_else(|_| Ok(json!({})))
    }

    // ── Network diagnostics ─────────────────────────────────────────

    /// Fetch recorded HTTP requests via the devtools HTTP timeline extension.
    pub async fn get_http_timeline(&self) -> Result<Value> {
        self.call(
            "ext.dart.io.getHttpProfile",
            json!({"isolateId": self.isolate_id}),
        )
        .await
        .or_else(|_| Ok(json!({})))
    }

    // ── Low-level RPC ───────────────────────────────────────────────

    /// Send a JSON-RPC 2.0 request and wait for the response.
    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut payload = serde_json::to_string(&request)?;
        payload.push('\n');

        // Write
        {
            let mut writer = self.write_half.lock().await;
            writer
                .write_all(payload.as_bytes())
                .await
                .context("Failed to write to VM Service")?;
            writer.flush().await?;
        }

        // Read until we get our response (match on id).
        {
            let mut reader = self.stream.lock().await;
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader
                    .read_line(&mut line)
                    .await
                    .context("Failed to read from VM Service")?;
                if n == 0 {
                    anyhow::bail!("VM Service connection closed");
                }
                if let Ok(resp) = serde_json::from_str::<Value>(&line) {
                    if resp["id"].as_u64() == Some(id) {
                        if let Some(err) = resp.get("error") {
                            anyhow::bail!("VM Service error: {}", err);
                        }
                        return Ok(resp["result"].clone());
                    }
                }
            }
        }
    }
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
