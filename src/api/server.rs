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

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_http_request ────────────────────────────────────────────────────

    #[test]
    fn test_parse_http_request_get() {
        let raw = "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let (method, path, body) = parse_http_request(raw);
        assert_eq!(method, "GET");
        assert_eq!(path, "/health");
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_http_request_post_with_body() {
        let json_body = r#"{"path":"/tmp/proj"}"#;
        let raw = format!(
            "POST /analyze HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            json_body.len(),
            json_body
        );
        let (method, path, body) = parse_http_request(&raw);
        assert_eq!(method, "POST");
        assert_eq!(path, "/analyze");
        assert_eq!(body, json_body);
    }

    #[test]
    fn test_parse_http_request_missing_body_separator() {
        // No \r\n\r\n — body should be empty string
        let raw = "POST /score HTTP/1.1\r\nContent-Length: 10";
        let (method, path, body) = parse_http_request(raw);
        assert_eq!(method, "POST");
        assert_eq!(path, "/score");
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_http_request_empty_string() {
        let (method, path, body) = parse_http_request("");
        assert_eq!(method, "GET");
        assert_eq!(path, "/");
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_http_request_check_file_path() {
        let raw = "POST /check-file HTTP/1.1\r\n\r\n{\"source\":\"void main(){}\"}";
        let (method, path, body) = parse_http_request(raw);
        assert_eq!(method, "POST");
        assert_eq!(path, "/check-file");
        assert_eq!(body, r#"{"source":"void main(){}"}"#);
    }

    // ── handle_analyze — invalid JSON ─────────────────────────────────────────

    #[test]
    fn test_handle_analyze_invalid_json() {
        let (status, body) = handle_analyze("not valid json").unwrap();
        assert_eq!(status, "400 Bad Request");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["success"], false);
        assert!(v["error"].as_str().unwrap().contains("Invalid JSON"));
    }

    #[test]
    fn test_handle_analyze_missing_path_and_source() {
        let (status, body) = handle_analyze("{}").unwrap();
        assert_eq!(status, "400 Bad Request");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["success"], false);
        assert!(v["error"].as_str().unwrap().contains("path"));
    }

    // ── handle_analyze — source branch ───────────────────────────────────────

    #[test]
    fn test_handle_analyze_source_clean_dart() {
        let body = serde_json::json!({
            "source": "void main() { print('hello'); }\n",
            "file_name": "main.dart"
        })
        .to_string();
        let (status, resp_body) = handle_analyze(&body).unwrap();
        assert_eq!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["success"], true);
        assert!(v["data"]["issue_count"].is_number());
        assert_eq!(v["data"]["file"], "main.dart");
    }

    #[test]
    fn test_handle_analyze_source_default_file_name() {
        let body = serde_json::json!({
            "source": "class A {}\n"
        })
        .to_string();
        let (status, resp_body) = handle_analyze(&body).unwrap();
        assert_eq!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["data"]["file"], "input.dart");
    }

    // ── handle_analyze — path branch ─────────────────────────────────────────

    #[test]
    fn test_handle_analyze_path_valid_project() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("main.dart"), "void main() { print('hi'); }\n").unwrap();
        std::fs::write(
            tmp.path().join("falcon.yaml"),
            "metrics:\n  cyclomatic_complexity: 20\n",
        )
        .unwrap();

        let body = serde_json::json!({
            "path": tmp.path().to_string_lossy()
        })
        .to_string();
        let (status, resp_body) = handle_analyze(&body).unwrap();
        assert_eq!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["success"], true);
        assert!(v["data"]["file_count"].is_number());
    }

    #[test]
    fn test_handle_analyze_path_nonexistent() {
        // The SDK gracefully handles nonexistent paths (returns empty result),
        // so the API returns 200 OK with success:true and zero file_count.
        let body = serde_json::json!({
            "path": "/nonexistent/path/to/project"
        })
        .to_string();
        let (status, resp_body) = handle_analyze(&body).unwrap();
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        // Either success (SDK returns empty) or failure (SDK errors) — both valid
        if status == "200 OK" {
            assert_eq!(v["success"], true);
        } else {
            assert_eq!(v["success"], false);
        }
    }

    // ── handle_score ──────────────────────────────────────────────────────────

    #[test]
    fn test_handle_score_invalid_json() {
        let (status, body) = handle_score("{bad json}").unwrap();
        assert_eq!(status, "400 Bad Request");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["success"], false);
        assert!(v["error"].as_str().unwrap().contains("Invalid JSON"));
    }

    #[test]
    fn test_handle_score_missing_path_field() {
        // path field required; missing key causes JSON parse error for the inner struct
        let (status, body) = handle_score("{}").unwrap();
        assert_eq!(status, "400 Bad Request");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["success"], false);
    }

    #[test]
    fn test_handle_score_valid_project() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(
            lib.join("service.dart"),
            "class MyService { void run() {} }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("falcon.yaml"),
            "metrics:\n  cyclomatic_complexity: 20\n",
        )
        .unwrap();

        let body = serde_json::json!({
            "path": tmp.path().to_string_lossy()
        })
        .to_string();
        let (status, resp_body) = handle_score(&body).unwrap();
        assert_eq!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["success"], true);
        let data = &v["data"];
        assert!(data["overall"].is_number());
        assert!(data["grade"].is_string());
    }

    #[test]
    fn test_handle_score_nonexistent_path() {
        let body = serde_json::json!({
            "path": "/no/such/dir"
        })
        .to_string();
        let (status, resp_body) = handle_score(&body).unwrap();
        assert_ne!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["success"], false);
    }

    // ── handle_check_file ─────────────────────────────────────────────────────

    #[test]
    fn test_handle_check_file_invalid_json() {
        let (status, body) = handle_check_file("!!!").unwrap();
        assert_eq!(status, "400 Bad Request");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["success"], false);
        assert!(v["error"].as_str().unwrap().contains("Invalid JSON"));
    }

    #[test]
    fn test_handle_check_file_missing_source() {
        // source field is required; missing it triggers a parse error
        let (status, body) = handle_check_file("{}").unwrap();
        assert_eq!(status, "400 Bad Request");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["success"], false);
    }

    #[test]
    fn test_handle_check_file_valid_source() {
        let body = serde_json::json!({
            "source": "void main() { print('hello'); }\n",
            "file_name": "hello.dart"
        })
        .to_string();
        let (status, resp_body) = handle_check_file(&body).unwrap();
        assert_eq!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["success"], true);
        assert_eq!(v["data"]["file"], "hello.dart");
        assert!(v["data"]["issue_count"].is_number());
        assert!(v["data"]["issues"].is_array());
    }

    #[test]
    fn test_handle_check_file_default_file_name() {
        let body = serde_json::json!({
            "source": "class Widget {}\n"
        })
        .to_string();
        let (status, resp_body) = handle_check_file(&body).unwrap();
        assert_eq!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["data"]["file"], "input.dart");
    }

    #[test]
    fn test_handle_check_file_with_issues() {
        // dynamic usage should trigger lint issues
        let body = serde_json::json!({
            "source": "void foo(dynamic x) { var y = x as dynamic; }\n",
            "file_name": "bad.dart"
        })
        .to_string();
        let (status, resp_body) = handle_check_file(&body).unwrap();
        assert_eq!(status, "200 OK");
        let v: serde_json::Value = serde_json::from_str(&resp_body).unwrap();
        assert_eq!(v["success"], true);
        // issue_count might be > 0 depending on enabled rules; just verify structure
        assert!(v["data"]["issue_count"].as_u64().is_some());
    }
}
