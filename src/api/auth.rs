//! Access control for the HTTP bridge.
//!
//! The bridge can read and analyze any directory the process can see, so every
//! endpoint except `GET /health` requires `Authorization: Bearer <token>`.
//! POST bodies must declare `Content-Type: application/json`, which together
//! with the bearer header forces a CORS preflight that the server never
//! answers, so a web page in the developer's browser cannot drive the API.

use anyhow::{anyhow, bail, Result};
use std::net::IpAddr;

/// Environment variable that pins the bearer token instead of generating one.
pub const TOKEN_ENV: &str = "FALCON_API_TOKEN";
const MIN_TOKEN_LEN: usize = 16;

/// Where the active token came from, so startup output never echoes a
/// caller-supplied secret.
#[derive(Debug, PartialEq, Eq)]
pub enum TokenSource {
    Environment,
    Generated,
}

/// Resolve the bearer token: `FALCON_API_TOKEN` if set, else 32 random bytes.
pub fn resolve_token() -> Result<(String, TokenSource)> {
    match std::env::var(TOKEN_ENV) {
        Ok(value) => token_from_env(&value).map(|token| (token, TokenSource::Environment)),
        Err(_) => generate_token().map(|token| (token, TokenSource::Generated)),
    }
}

fn token_from_env(value: &str) -> Result<String> {
    let token = value.trim();
    if token.len() < MIN_TOKEN_LEN {
        bail!("{TOKEN_ENV} must be at least {MIN_TOKEN_LEN} characters");
    }
    if token.chars().any(|c| c.is_whitespace() || c.is_control()) {
        bail!("{TOKEN_ENV} must not contain whitespace or control characters");
    }
    Ok(token.to_string())
}

fn generate_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| anyhow!("could not generate an API token: {error}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// True when `host` only accepts connections from this machine.
pub fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    bare.parse::<IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// Check a raw request against the token and body-type rules.
///
/// Returns the HTTP status and error message to send when the request must be
/// refused.
pub fn check_request(
    request: &str,
    method: &str,
    path: &str,
    token: &str,
) -> Result<(), (&'static str, &'static str)> {
    if method == "GET" && path == "/health" {
        return Ok(());
    }
    let presented = header(request, "authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    match presented {
        Some(candidate) if constant_time_eq(candidate.as_bytes(), token.as_bytes()) => {}
        _ => {
            return Err((
                "401 Unauthorized",
                "Missing or invalid bearer token; send `Authorization: Bearer <token>`",
            ))
        }
    }
    if method == "POST" && !is_json_content_type(header(request, "content-type")) {
        return Err((
            "415 Unsupported Media Type",
            "POST bodies must be sent with `Content-Type: application/json`",
        ));
    }
    Ok(())
}

fn is_json_content_type(value: Option<&str>) -> bool {
    value
        .and_then(|v| v.split(';').next())
        .map(|media| media.trim().eq_ignore_ascii_case("application/json"))
        .unwrap_or(false)
}

/// Value of the first header named `name` (case-insensitive) in a raw request.
pub fn header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    let head = request.split("\r\n\r\n").next().unwrap_or("");
    head.split("\r\n").skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn request(method_path: &str, headers: &[&str]) -> String {
        let mut raw = format!("{method_path} HTTP/1.1\r\nHost: localhost\r\n");
        for h in headers {
            raw.push_str(h);
            raw.push_str("\r\n");
        }
        raw.push_str("\r\n{}");
        raw
    }

    #[test]
    fn health_is_open_without_a_token() {
        let raw = request("GET /health", &[]);
        assert!(check_request(&raw, "GET", "/health", TOKEN).is_ok());
    }

    #[test]
    fn missing_token_is_unauthorized() {
        let raw = request("GET /tools", &[]);
        let err = check_request(&raw, "GET", "/tools", TOKEN).unwrap_err();
        assert_eq!(err.0, "401 Unauthorized");
    }

    #[test]
    fn wrong_token_is_unauthorized() {
        let raw = request("GET /tools", &["Authorization: Bearer nope-nope-nope-nope"]);
        let err = check_request(&raw, "GET", "/tools", TOKEN).unwrap_err();
        assert_eq!(err.0, "401 Unauthorized");
    }

    #[test]
    fn non_bearer_scheme_is_unauthorized() {
        let raw = request("GET /tools", &[&format!("Authorization: Basic {TOKEN}")]);
        assert!(check_request(&raw, "GET", "/tools", TOKEN).is_err());
    }

    #[test]
    fn correct_token_passes_for_get() {
        let raw = request("GET /tools", &[&format!("authorization: Bearer {TOKEN}")]);
        assert!(check_request(&raw, "GET", "/tools", TOKEN).is_ok());
    }

    #[test]
    fn post_without_json_content_type_is_rejected() {
        let auth = format!("Authorization: Bearer {TOKEN}");
        let raw = request("POST /analyze", &[&auth, "Content-Type: text/plain"]);
        let err = check_request(&raw, "POST", "/analyze", TOKEN).unwrap_err();
        assert_eq!(err.0, "415 Unsupported Media Type");

        let raw = request("POST /analyze", &[&auth]);
        assert!(check_request(&raw, "POST", "/analyze", TOKEN).is_err());
    }

    #[test]
    fn post_with_json_content_type_and_charset_passes() {
        let auth = format!("Authorization: Bearer {TOKEN}");
        let raw = request(
            "POST /analyze",
            &[&auth, "Content-Type: Application/JSON; charset=utf-8"],
        );
        assert!(check_request(&raw, "POST", "/analyze", TOKEN).is_ok());
    }

    #[test]
    fn auth_is_checked_before_content_type() {
        let raw = request("POST /analyze", &["Content-Type: text/plain"]);
        let err = check_request(&raw, "POST", "/analyze", TOKEN).unwrap_err();
        assert_eq!(err.0, "401 Unauthorized");
    }

    #[test]
    fn header_lookup_ignores_the_body() {
        let raw = "POST /x HTTP/1.1\r\nA: 1\r\n\r\nAuthorization: Bearer smuggled";
        assert_eq!(header(raw, "a"), Some("1"));
        assert_eq!(header(raw, "authorization"), None);
    }

    #[test]
    fn loopback_hosts_are_recognised() {
        for host in [
            "127.0.0.1",
            "127.0.0.2",
            "::1",
            "[::1]",
            "localhost",
            "LOCALHOST",
        ] {
            assert!(is_loopback_host(host), "{host}");
        }
        for host in ["0.0.0.0", "::", "192.168.1.10", "example.com", ""] {
            assert!(!is_loopback_host(host), "{host}");
        }
    }

    #[test]
    fn generated_tokens_are_long_hex_and_distinct() {
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn env_tokens_must_be_long_and_clean() {
        assert!(token_from_env("short").is_err());
        assert!(token_from_env("has space in the middle!").is_err());
        assert_eq!(token_from_env(&format!("  {TOKEN}\n")).unwrap(), TOKEN);
    }

    #[test]
    fn constant_time_eq_matches_only_identical_bytes() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
