use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
pub(super) const CLIENT_TIMEOUT: Duration = Duration::from_secs(2);
type RequestResult = Result<String, (&'static str, String)>;

/// Read one bounded HTTP request; TCP read boundaries are not message boundaries.
pub(super) fn read_request(stream: &mut TcpStream) -> RequestResult {
    let deadline = Instant::now() + CLIENT_TIMEOUT;
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(position) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            if position + 4 > MAX_HEADER_BYTES {
                return Err((
                    "413 Payload Too Large",
                    "Request headers exceed limit".into(),
                ));
            }
            break position + 4;
        }
        if bytes.len() >= MAX_HEADER_BYTES {
            return Err((
                "413 Payload Too Large",
                "Request headers exceed limit".into(),
            ));
        }
        read_more(stream, &mut bytes, deadline)?;
    };
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| ("400 Bad Request", "Headers must be UTF-8".into()))?;
    let mut content_length = None;
    for line in headers
        .split("\r\n")
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ("400 Bad Request", "Malformed header".into()))?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err((
                "400 Bad Request",
                "Transfer-Encoding is unsupported; provide Content-Length".into(),
            ));
        }
        if !name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if content_length.is_some() {
            return Err(("400 Bad Request", "Duplicate Content-Length".into()));
        }
        let length = value
            .trim()
            .parse::<usize>()
            .map_err(|_| ("400 Bad Request", "Invalid Content-Length".into()))?;
        if length > MAX_BODY_BYTES {
            return Err(("413 Payload Too Large", "Request body exceeds limit".into()));
        }
        content_length = Some(length);
    }
    let total = header_end + content_length.unwrap_or(0);
    while bytes.len() < total {
        read_more(stream, &mut bytes, deadline)?;
    }
    bytes.truncate(total);
    String::from_utf8(bytes).map_err(|_| ("400 Bad Request", "Request must be UTF-8".into()))
}

fn read_more(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
    deadline: Instant,
) -> Result<(), (&'static str, String)> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(("408 Request Timeout", "Request deadline exceeded".into()));
    }
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|error| ("408 Request Timeout", error.to_string()))?;
    let mut buffer = [0; 4096];
    let count = stream.read(&mut buffer).map_err(|error| {
        let status = if matches!(
            error.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) {
            "408 Request Timeout"
        } else {
            "400 Bad Request"
        };
        (status, error.to_string())
    })?;
    if count == 0 {
        return Err(("400 Bad Request", "Incomplete request".into()));
    }
    bytes.extend_from_slice(&buffer[..count]);
    Ok(())
}

pub(super) fn write_response(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
) -> std::io::Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())?;
    stream.flush()
}
