use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Server(Child, u16);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn server() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let child = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args(["x", "api", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let server = Server(child, port);
    let deadline = Instant::now() + Duration::from_secs(5);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "server startup timeout");
        thread::sleep(Duration::from_millis(10));
    }
    server
}
fn connect(server: &Server) -> TcpStream {
    let stream = TcpStream::connect(("127.0.0.1", server.1)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
}
#[test]
fn split_post_body_is_read_using_content_length() {
    let server = server();
    let mut stream = connect(&server);
    let body = r#"{"source":"void main() {}"}"#;
    write!(
        stream,
        "POST /check-file HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    thread::sleep(Duration::from_millis(50));
    stream.write_all(body.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
}
#[test]
fn idle_client_does_not_block_health_requests() {
    let server = server();
    let _idle = connect(&server);
    thread::sleep(Duration::from_millis(50));
    let mut stream = connect(&server);
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
}
#[test]
fn oversized_content_length_is_rejected_before_reading_body() {
    let server = server();
    let mut stream = connect(&server);
    stream
        .write_all(b"POST /check-file HTTP/1.1\r\nContent-Length: 999999999\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 413"), "{response}");
}

#[test]
fn trickled_headers_have_a_total_request_deadline() {
    let server = server();
    let mut stream = connect(&server);
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut writer = stream.try_clone().unwrap();
    let started = Instant::now();
    let trickle = thread::spawn(move || {
        for _ in 0..15 {
            if writer.write_all(b"G").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
    });
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let elapsed = started.elapsed();
    trickle.join().unwrap();
    assert!(response.starts_with("HTTP/1.1 408"), "{response}");
    assert!(
        elapsed < Duration::from_secs(3),
        "deadline was reset on each read: {elapsed:?}"
    );
}
