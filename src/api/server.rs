//! Falcon HTTP API server.
//!
//! Endpoints:
//! - `POST /analyze` — analyze a project or source code
//! - `POST /score` — calculate AI Code Quality Score
//! - `POST /check-file` — analyze a single file's source
//! - `GET /health` — health check
//! - `GET /tools` — list available analysis capabilities

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;

#[derive(Debug, Deserialize)]
struct AnalyzeRequest {
    path: Option<String>,
    source: Option<String>,
    file_name: Option<String>,
    preset: Option<String>,
    include_score: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Start the Falcon HTTP API server.
pub fn start_api_server(host: &str, port: u16) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr)?;

    eprintln!("  Falcon API server listening on http://{}", addr);
    eprintln!("  Endpoints:");
    eprintln!("    POST /analyze      — analyze a project or source");
    eprintln!("    POST /score        — AI Code Quality Score");
    eprintln!("    POST /check-file   — analyze single file source");
    eprintln!("    GET  /health       — health check");
    eprintln!("    GET  /tools        — list capabilities");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(e) = handle_connection(&mut stream) {
                    log::warn!("Request handling error: {}", e);
                }
            }
            Err(e) => {
                log::warn!("Connection error: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_connection(stream: &mut std::net::TcpStream) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 65536];
    let n = stream.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..n]).to_string();

    let (method, path, body) = parse_http_request(&request);

    let (status, response_body) = match (method.as_str(), path.as_str()) {
        ("GET", "/health") => {
            let resp = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "status": "ok",
                    "version": env!("CARGO_PKG_VERSION"),
                    "name": "falcon"
                })),
                error: None,
            };
            ("200 OK", serde_json::to_string_pretty(&resp)?)
        }

        ("GET", "/tools") => {
            let tools = crate::mcp::tools::list_tools();
            let resp = ApiResponse {
                success: true,
                data: Some(serde_json::json!({ "tools": tools })),
                error: None,
            };
            ("200 OK", serde_json::to_string_pretty(&resp)?)
        }

        ("POST", "/analyze") => handle_analyze(&body)?,

        ("POST", "/score") => handle_score(&body)?,

        ("POST", "/check-file") => handle_check_file(&body)?,

        _ => {
            let resp = ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Not found: {} {}", method, path)),
            };
            ("404 Not Found", serde_json::to_string_pretty(&resp)?)
        }
    };

    let http_response = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
        status,
        response_body.len(),
        response_body
    );
    stream.write_all(http_response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

fn parse_http_request(raw: &str) -> (String, String, String) {
    let lines: Vec<&str> = raw.split("\r\n").collect();
    let first_line = lines.first().unwrap_or(&"");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    let method = parts.first().unwrap_or(&"GET").to_string();
    let path = parts.get(1).unwrap_or(&"/").to_string();

    let body = if let Some(pos) = raw.find("\r\n\r\n") {
        raw[pos + 4..].to_string()
    } else {
        String::new()
    };

    (method, path, body)
}

fn handle_analyze(body: &str) -> anyhow::Result<(&'static str, String)> {
    let req: AnalyzeRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            let resp = ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Invalid JSON: {}", e)),
            };
            return Ok(("400 Bad Request", serde_json::to_string_pretty(&resp)?));
        }
    };

    if let Some(ref source) = req.source {
        let file_name = req.file_name.as_deref().unwrap_or("input.dart");
        let sdk = crate::sdk::FalconSdk::new();
        match sdk.analyze_source(source, file_name) {
            Ok(issues) => {
                let resp = ApiResponse {
                    success: true,
                    data: Some(serde_json::json!({
                        "file": file_name,
                        "issue_count": issues.len(),
                        "issues": issues
                    })),
                    error: None,
                };
                Ok(("200 OK", serde_json::to_string_pretty(&resp)?))
            }
            Err(e) => {
                let resp = ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
                Ok((
                    "500 Internal Server Error",
                    serde_json::to_string_pretty(&resp)?,
                ))
            }
        }
    } else if let Some(ref path) = req.path {
        let sdk = crate::sdk::FalconSdk::new();
        let opts = crate::sdk::AnalysisOptions {
            preset: req.preset,
            include_score: req.include_score.unwrap_or(false),
            ..Default::default()
        };
        match sdk.analyze_project(path, Some(opts)) {
            Ok(result) => {
                let resp = ApiResponse {
                    success: true,
                    data: Some(serde_json::to_value(&result)?),
                    error: None,
                };
                Ok(("200 OK", serde_json::to_string_pretty(&resp)?))
            }
            Err(e) => {
                let resp = ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
                Ok((
                    "500 Internal Server Error",
                    serde_json::to_string_pretty(&resp)?,
                ))
            }
        }
    } else {
        let resp = ApiResponse {
            success: false,
            data: None,
            error: Some("Provide 'path' (project directory) or 'source' (Dart code)".to_string()),
        };
        Ok(("400 Bad Request", serde_json::to_string_pretty(&resp)?))
    }
}

fn handle_score(body: &str) -> anyhow::Result<(&'static str, String)> {
    #[derive(Deserialize)]
    struct ScoreRequest {
        path: String,
    }

    let req: ScoreRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            let resp = ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Invalid JSON: {}", e)),
            };
            return Ok(("400 Bad Request", serde_json::to_string_pretty(&resp)?));
        }
    };

    let sdk = crate::sdk::FalconSdk::new();
    match sdk.score_project(&req.path) {
        Ok(score) => {
            let resp = ApiResponse {
                success: true,
                data: Some(serde_json::to_value(&score)?),
                error: None,
            };
            Ok(("200 OK", serde_json::to_string_pretty(&resp)?))
        }
        Err(e) => {
            let resp = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            Ok((
                "500 Internal Server Error",
                serde_json::to_string_pretty(&resp)?,
            ))
        }
    }
}

fn handle_check_file(body: &str) -> anyhow::Result<(&'static str, String)> {
    #[derive(Deserialize)]
    struct CheckFileRequest {
        source: String,
        #[serde(default = "default_file_name")]
        file_name: String,
    }
    fn default_file_name() -> String {
        "input.dart".to_string()
    }

    let req: CheckFileRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            let resp = ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Invalid JSON: {}", e)),
            };
            return Ok(("400 Bad Request", serde_json::to_string_pretty(&resp)?));
        }
    };

    let sdk = crate::sdk::FalconSdk::new();
    match sdk.analyze_source(&req.source, &req.file_name) {
        Ok(issues) => {
            let resp = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "file": req.file_name,
                    "issue_count": issues.len(),
                    "issues": issues
                })),
                error: None,
            };
            Ok(("200 OK", serde_json::to_string_pretty(&resp)?))
        }
        Err(e) => {
            let resp = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            Ok((
                "500 Internal Server Error",
                serde_json::to_string_pretty(&resp)?,
            ))
        }
    }
}
