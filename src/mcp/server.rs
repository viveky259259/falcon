use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

use super::tools;
use crate::doctor::Trust;

const SERVER_NAME: &str = "falcon";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

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

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Invalid JSON-RPC request: {}", e);
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                write_response(&mut stdout, &resp)?;
                continue;
            }
        };

        let response = handle_request(&request);
        write_response(&mut stdout, &response)?;
    }

    Ok(())
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
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
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

            match tools::execute_tool(tool_name, &arguments, Trust::Local) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                        }]
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
