use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

use super::tools;
use crate::doctor::Trust;

const SERVER_NAME: &str = "falcon";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// MCP protocol versions this server speaks, newest first.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// Answer with the client's version when we support it, else our newest.
pub fn negotiate_protocol_version(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|r| {
            SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .copied()
                .find(|v| *v == r)
        })
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0])
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// Run the MCP server on stdio (blocking).
pub fn run_mcp_server() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    log::info!("Falcon MCP server starting (stdio)");

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log::error!("Failed to read stdin: {}", e);
                break;
            }
        };

        if let Some(response) = process_line(&line) {
            write_response(&mut stdout, &response)?;
        }
    }

    // Don't leave a `flutter run` from run_app behind.
    super::app_session::shutdown();
    Ok(())
}

/// Handle one line of input. Returns the response to send, or `None` when the
/// line is blank or a JSON-RPC notification (a message without an `id`), which
/// must never be answered (JSON-RPC 2.0 section 4.1).
fn process_line(line: &str) -> Option<JsonRpcResponse> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let request: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Invalid JSON-RPC request: {}", e);
            return Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Value::Null,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                    data: None,
                }),
            });
        }
    };
    if request.id.is_none() {
        log::debug!("Notification received: {}", request.method);
        return None;
    }
    Some(handle_request(&request))
}

fn write_response(stdout: &mut io::Stdout, response: &JsonRpcResponse) -> io::Result<()> {
    let json = serde_json::to_string(response).unwrap_or_else(|_| "{}".to_string());
    writeln!(stdout, "{}", json)?;
    stdout.flush()
}

fn handle_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    let id = request.id.clone().unwrap_or(Value::Null);

    match request.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "protocolVersion": negotiate_protocol_version(
                    request.params.get("protocolVersion").and_then(|v| v.as_str())
                ),
                "capabilities": {
                    "tools": { "listChanged": false },
                    "prompts": { "listChanged": false },
                    "resources": { "listChanged": false, "subscribe": false }
                },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION
                }
            })),
            error: None,
        },

        "notifications/initialized" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(Value::Null),
            error: None,
        },

        "tools/list" => {
            let tool_defs = tools::list_tools();
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({ "tools": tool_defs })),
                error: None,
            }
        }

        "tools/call" => {
            let tool_name = request
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));

            let progress_token = request
                .params
                .get("_meta")
                .and_then(|m| m.get("progressToken"))
                .cloned();
            super::progress::set_token(progress_token);
            let outcome = tools::execute_tool(tool_name, &arguments, Trust::Local);
            super::progress::set_token(None);

            match outcome {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": super::runtime_tools::into_content_blocks(result)
                    })),
                    error: None,
                },
                Err(err) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Error: {}", err)
                        }],
                        "isError": true
                    })),
                    error: None,
                },
            }
        }

        "prompts/list"
        | "prompts/get"
        | "resources/list"
        | "resources/read"
        | "resources/templates/list" => {
            match super::catalog::handle(&request.method, &request.params) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(result),
                    error: None,
                },
                Err(message) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message,
                        data: None,
                    }),
                },
            }
        }

        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({})),
            error: None,
        },

        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_get_no_response() {
        let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert!(process_line(line).is_none());
        let other = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{}}"#;
        assert!(process_line(other).is_none());
    }

    #[test]
    fn requests_with_an_id_are_answered_with_that_id() {
        let line = r#"{"jsonrpc":"2.0","id":7,"method":"initialize","params":{}}"#;
        let response = process_line(line).expect("initialize must be answered");
        assert_eq!(response.id, serde_json::json!(7));
        assert!(response.result.is_some());
    }

    #[test]
    fn parse_errors_are_answered_with_a_null_id() {
        let response = process_line("{not json").expect("parse errors are answered");
        assert_eq!(response.id, Value::Null);
        assert_eq!(response.error.unwrap().code, -32700);
    }

    #[test]
    fn protocol_version_is_negotiated() {
        assert_eq!(negotiate_protocol_version(Some("2024-11-05")), "2024-11-05");
        assert_eq!(negotiate_protocol_version(Some("1999-01-01")), "2025-06-18");
        assert_eq!(negotiate_protocol_version(None), "2025-06-18");
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#;
        let result = process_line(line).unwrap().result.unwrap();
        assert_eq!(result["protocolVersion"], "2025-03-26");
        assert!(result["capabilities"]["prompts"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
    }

    #[test]
    fn prompts_and_resources_are_served() {
        let list = process_line(r#"{"jsonrpc":"2.0","id":1,"method":"prompts/list"}"#).unwrap();
        assert!(!list.result.unwrap()["prompts"]
            .as_array()
            .unwrap()
            .is_empty());
        let bad = process_line(r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"falcon://nope"}}"#).unwrap();
        assert_eq!(bad.error.unwrap().code, -32602);
    }

    #[test]
    fn blank_lines_are_ignored() {
        assert!(process_line("   ").is_none());
    }
}
