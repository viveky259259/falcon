use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const FILE_COUNT: usize = 200;
const SAMPLE_COUNT: usize = 20;
const MAX_MEDIAN_MS: u128 = 300;

fn main() {
    let bin = locate_falcon_mcp();
    let temp = tempfile::tempdir().expect("create temp repo");
    let target_file = create_repo(temp.path());

    call_lint_file(&bin, &target_file).expect("warm cache call");

    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let start = Instant::now();
        call_lint_file(&bin, &target_file).expect("cached MCP lint_file roundtrip");
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    let median = samples[SAMPLE_COUNT / 2];

    println!(
        "mcp_warm_cache: {} samples, p50={}ms, threshold={}ms",
        SAMPLE_COUNT,
        median.as_millis(),
        MAX_MEDIAN_MS
    );

    if median.as_millis() >= MAX_MEDIAN_MS {
        panic!(
            "MCP warm-cache p50 exceeded threshold: {}ms >= {}ms",
            median.as_millis(),
            MAX_MEDIAN_MS
        );
    }
}

fn locate_falcon_mcp() -> PathBuf {
    if let Ok(path) = std::env::var("FALCON_MCP_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }

    let current = std::env::current_exe().expect("current bench executable path");
    let profile_dir = current
        .parent()
        .and_then(|p| p.parent())
        .expect("bench executable under target/<profile>/deps");

    let binary_name = if cfg!(windows) {
        "falcon-mcp.exe"
    } else {
        "falcon-mcp"
    };

    for candidate in [
        profile_dir.join(binary_name),
        profile_dir
            .parent()
            .unwrap_or(profile_dir)
            .join("release")
            .join(binary_name),
        profile_dir
            .parent()
            .unwrap_or(profile_dir)
            .join("debug")
            .join(binary_name),
    ] {
        if candidate.is_file() {
            return candidate;
        }
    }

    panic!(
        "could not locate falcon-mcp binary near {}; run `cargo build --release --bin falcon-mcp` or set FALCON_MCP_BIN",
        current.display()
    );
}

fn create_repo(root: &Path) -> PathBuf {
    let lib = root.join("lib");
    std::fs::create_dir_all(&lib).expect("create lib dir");
    for idx in 0..FILE_COUNT {
        let path = lib.join(format!("file_{idx:03}.dart"));
        std::fs::write(
            &path,
            format!(
                "class Widget{idx:03} {{\n  void build() {{\n    final value = {idx};\n  }}\n}}\n"
            ),
        )
        .expect("write dart fixture");
    }
    lib.join("file_000.dart")
}

fn call_lint_file(bin: &Path, file: &Path) -> Result<Value, String> {
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;

    {
        let stdin = child.stdin.as_mut().ok_or("missing child stdin")?;
        writeln!(
            stdin,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "lint_file",
                    "arguments": {
                        "file_path": file.to_string_lossy()
                    }
                }
            })
        )
        .map_err(|e| format!("write request: {e}"))?;
    }
    drop(child.stdin.take());

    let stdout = child.stdout.take().ok_or("missing child stdout")?;
    let mut lines = BufReader::new(stdout).lines();
    let line = lines
        .next()
        .ok_or("missing MCP response")?
        .map_err(|e| format!("read response: {e}"))?;

    let status = child
        .wait_timeout(Duration::from_secs(5))
        .map_err(|e| format!("wait for child: {e}"))?
        .ok_or("MCP child did not exit within timeout")?;
    if !status.success() {
        return Err(format!("MCP child exited with status {status}"));
    }

    let response: Value =
        serde_json::from_str(&line).map_err(|e| format!("parse MCP response: {e}: {line}"))?;
    if response.get("error").is_some() || response["result"]["isError"].as_bool() == Some(true) {
        return Err(format!("MCP returned error response: {response}"));
    }

    Ok(response)
}

trait WaitTimeout {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl WaitTimeout for std::process::Child {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        let start = Instant::now();
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(Some(status));
            }
            if start.elapsed() >= timeout {
                let _ = self.kill();
                let _ = self.wait();
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
