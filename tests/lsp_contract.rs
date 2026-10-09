use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

struct Session {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Session {
    fn new() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_falcon-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.strip_prefix("Content-Length:") {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                if sender.send(serde_json::from_slice(&body).unwrap()).is_err() {
                    return;
                }
            }
        });
        let mut session = Self {
            child,
            stdin,
            messages,
        };
        session.send(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        );
        session.until(|message| message["id"] == 1);
        session.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
        session
    }
    fn send(&mut self, message: Value) {
        let body = serde_json::to_vec(&message).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }
    fn until(&self, predicate: impl Fn(&Value) -> bool) -> Value {
        loop {
            let message = self.messages.recv_timeout(Duration::from_secs(5)).unwrap();
            if predicate(&message) {
                return message;
            }
        }
    }
    fn open(&mut self, source: &str) -> Value {
        self.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///review.dart","languageId":"dart","version":1,"text":source}}}));
        self.until(|message| message["method"] == "textDocument/publishDiagnostics")
    }
}
#[test]
fn unicode_positions_and_fix_ranges_use_utf16() {
    let mut session = Session::new();
    let source =
        "void main() { final label = '😀'; dynamic value = 1; final xs = [1]; final x = xs[0]; }\n";
    let diagnostics = session.open(source);
    let dynamic = diagnostics["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diag| diag["code"] == "avoid-dynamic")
        .unwrap();
    let expected = source[..source.find("dynamic").unwrap()]
        .encode_utf16()
        .count();
    assert_eq!(dynamic["range"]["start"]["character"], expected);
    session.send(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/codeAction","params":{"textDocument":{"uri":"file:///review.dart"},"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":0}},"context":{"diagnostics":diagnostics["params"]["diagnostics"]}}}));
    let actions = session.until(|message| message["id"] == 2);
    let fix = actions["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["isPreferred"] == true)
        .unwrap();
    let edit = &fix["edit"]["changes"]["file:///review.dart"][0];
    let start = source.find("[0]").unwrap();
    assert_eq!(
        edit["range"]["start"]["character"],
        source[..start].encode_utf16().count()
    );
    assert_eq!(edit["newText"], ".first");
}
#[test]
fn fix_all_requests_non_overlapping_edits_for_open_documents() {
    let mut session = Session::new();
    session.open(
        "void main() { final xs = [1]; final a = xs[0]; final b = xs[0]; final label = '[0]'; }\n",
    );
    session.send(json!({"jsonrpc":"2.0","id":2,"method":"workspace/executeCommand","params":{"command":"falcon.fixAll"}}));
    let edit_request =
        session.until(|message| message["method"] == "workspace/applyEdit" || message["id"] == 2);
    assert_eq!(
        edit_request["method"], "workspace/applyEdit",
        "Fix All returned without edits: {edit_request}"
    );
    let edits = edit_request["params"]["edit"]["changes"]["file:///review.dart"]
        .as_array()
        .unwrap();
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|edit| edit["newText"] == ".first"));
    session.send(json!({"jsonrpc":"2.0","id":edit_request["id"],"result":{"applied":true}}));
    let result = session.until(|message| message["id"] == 2);
    assert!(result.get("error").is_none());
}
