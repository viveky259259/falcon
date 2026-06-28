use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TELEMETRY_FILE: &str = "mcp-events.jsonl";

#[derive(Debug, Clone, PartialEq, Eq)]
struct TelemetryConfig {
    enabled: bool,
    output_dir: PathBuf,
    source: String,
}

#[derive(Debug, Serialize)]
struct McpToolInvocationEvent<'a> {
    schema_version: u32,
    event: &'static str,
    timestamp_unix_ms: u128,
    source: &'a str,
    tool: &'a str,
    canonical_tool: Option<&'a str>,
    success: bool,
    duration_ms: u128,
    crate_version: &'static str,
}

pub fn record_mcp_tool_invocation(
    tool: &str,
    canonical_tool: Option<&str>,
    success: bool,
    duration: Duration,
) {
    let Some(config) = TelemetryConfig::from_env() else {
        return;
    };

    let event = McpToolInvocationEvent {
        schema_version: 1,
        event: "mcp_tool_invocation",
        timestamp_unix_ms: unix_timestamp_ms(),
        source: &config.source,
        tool,
        canonical_tool,
        success,
        duration_ms: duration.as_millis(),
        crate_version: env!("CARGO_PKG_VERSION"),
    };

    if let Err(error) = write_event(&config.output_dir, &event) {
        log::warn!("failed to write Falcon telemetry event: {}", error);
    }
}

impl TelemetryConfig {
    fn from_env() -> Option<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Option<Self> {
        let enabled_value = lookup("FALCON_TELEMETRY")?;
        if !is_enabled_value(&enabled_value) {
            return None;
        }

        let output_dir = lookup("FALCON_TELEMETRY_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_telemetry_dir);
        let source = classify_source(&mut lookup);

        Some(Self {
            enabled: true,
            output_dir,
            source,
        })
    }
}

fn write_event<T: Serialize>(output_dir: &PathBuf, event: &T) -> anyhow::Result<()> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join(TELEMETRY_FILE);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn classify_source(lookup: &mut impl FnMut(&str) -> Option<String>) -> String {
    if let Some(source) = lookup("FALCON_INVOCATION_SOURCE").filter(|value| !value.is_empty()) {
        return source;
    }
    if let Some(agent) = lookup("FALCON_MCP_AGENT_ID").filter(|value| !value.is_empty()) {
        return format!("mcp:{agent}");
    }
    if lookup("CI").is_some() || lookup("GITHUB_ACTIONS").is_some() {
        return "cli:ci".to_string();
    }
    if lookup("MCP_CLIENT").is_some() || lookup("MCP_CLIENT_NAME").is_some() {
        return "mcp:unknown".to_string();
    }
    "mcp:unknown".to_string()
}

fn is_enabled_value(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "local"
    )
}

fn default_telemetry_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".falcon-data")
        .join("telemetry")
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::HashMap;

    #[test]
    fn telemetry_is_off_by_default() {
        let env = HashMap::<String, String>::new();
        assert!(TelemetryConfig::from_lookup(|key| env.get(key).cloned()).is_none());
    }

    #[test]
    fn telemetry_zero_disables_recording() {
        let env = HashMap::from([("FALCON_TELEMETRY".to_string(), "0".to_string())]);
        assert!(TelemetryConfig::from_lookup(|key| env.get(key).cloned()).is_none());
    }

    #[test]
    fn telemetry_classifies_mcp_agent_source() {
        let env = HashMap::from([
            ("FALCON_TELEMETRY".to_string(), "1".to_string()),
            (
                "FALCON_TELEMETRY_DIR".to_string(),
                "/tmp/falcon-telemetry".to_string(),
            ),
            ("FALCON_MCP_AGENT_ID".to_string(), "cursor".to_string()),
        ]);

        let config = TelemetryConfig::from_lookup(|key| env.get(key).cloned()).unwrap();

        assert!(config.enabled);
        assert_eq!(config.output_dir, PathBuf::from("/tmp/falcon-telemetry"));
        assert_eq!(config.source, "mcp:cursor");
    }

    #[test]
    fn telemetry_writes_jsonl_event() {
        let temp = tempfile::tempdir().unwrap();
        let event = McpToolInvocationEvent {
            schema_version: 1,
            event: "mcp_tool_invocation",
            timestamp_unix_ms: 42,
            source: "mcp:test",
            tool: "falcon_analyze",
            canonical_tool: Some("review"),
            success: true,
            duration_ms: 7,
            crate_version: "test",
        };

        write_event(&temp.path().to_path_buf(), &event).unwrap();

        let contents = fs::read_to_string(temp.path().join(TELEMETRY_FILE)).unwrap();
        let value: Value = serde_json::from_str(contents.trim()).unwrap();
        assert_eq!(value["event"], "mcp_tool_invocation");
        assert_eq!(value["source"], "mcp:test");
        assert_eq!(value["tool"], "falcon_analyze");
        assert_eq!(value["canonical_tool"], "review");
    }
}
