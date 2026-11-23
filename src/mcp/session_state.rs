//! State shared between the MCP thread and the threads reading a
//! `flutter run --machine` session: daemon events, request responses and a
//! bounded log buffer.

use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

const LOG_CAPACITY: usize = 2_000;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogLine {
    pub seq: u64,
    pub error: bool,
    pub source: &'static str,
    pub text: String,
}

#[derive(Default)]
pub(super) struct Shared {
    pub(super) logs: VecDeque<LogLine>,
    pub(super) next_seq: u64,
    pub(super) app_id: Option<String>,
    pub(super) ws_uri: Option<String>,
    pub(super) started: bool,
    pub(super) stopped: bool,
    pub(super) progress: Option<String>,
    pub(super) responses: HashMap<u64, Value>,
}

impl Shared {
    pub(super) fn push(&mut self, source: &'static str, error: bool, text: String) {
        let seq = self.next_seq;
        self.next_seq += 1;
        if self.logs.len() == LOG_CAPACITY {
            self.logs.pop_front();
        }
        self.logs.push_back(LogLine {
            seq,
            error,
            source,
            text,
        });
    }
}

pub(super) fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// Apply one line of `flutter run --machine` output to the session state.
pub(super) fn handle_stdout_line(shared: &mut Shared, line: &str) {
    let messages: Vec<Value> = match serde_json::from_str::<Value>(line.trim()) {
        Ok(Value::Array(items)) => items,
        _ => {
            if !line.trim().is_empty() {
                shared.push("flutter", false, line.to_string());
            }
            return;
        }
    };
    for msg in messages {
        if let Some(id) = msg["id"].as_u64() {
            shared.responses.insert(id, msg);
            continue;
        }
        let params = &msg["params"];
        match msg["event"].as_str().unwrap_or("") {
            "app.start" => shared.app_id = params["appId"].as_str().map(str::to_string),
            "app.debugPort" => shared.ws_uri = params["wsUri"].as_str().map(str::to_string),
            "app.started" => shared.started = true,
            "app.stop" => shared.stopped = true,
            "app.progress" => {
                if let Some(m) = params["message"].as_str() {
                    shared.progress = Some(m.to_string());
                }
            }
            "app.log" => {
                let error = params["error"].as_bool().unwrap_or(false);
                let text = params["log"].as_str().unwrap_or("").to_string();
                shared.push("app", error, text);
            }
            "daemon.logMessage" => {
                let error = params["level"].as_str() == Some("error");
                let text = params["message"].as_str().unwrap_or("").to_string();
                shared.push("flutter", error, text);
            }
            _ => {}
        }
    }
}

pub(super) fn tail_text(shared: &Mutex<Shared>, n: usize) -> String {
    let shared = lock(shared);
    let skip = shared.logs.len().saturating_sub(n);
    shared
        .logs
        .iter()
        .skip(skip)
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn logs_since(
    shared: &Mutex<Shared>,
    since: u64,
    errors_only: bool,
    limit: usize,
) -> Vec<LogLine> {
    let shared = lock(shared);
    // Flutter frames each framework error between a `══╡ … ╞══` header and
    // a closing `════` rule; keep the whole block, not just the header.
    let mut in_block = false;
    let mut out: Vec<LogLine> = Vec::new();
    for l in shared.logs.iter().filter(|l| l.seq >= since) {
        let text = app_text(&l.text);
        let starts = text.starts_with("══╡");
        let keep = !errors_only || in_block || starts || l.error || looks_like_error(text);
        if starts {
            in_block = true;
        } else if in_block && text.starts_with("════") {
            in_block = false;
        }
        if keep {
            out.push(l.clone());
        }
    }
    let skip = out.len().saturating_sub(limit);
    out.drain(..skip);
    out
}

/// A log line without the `flutter: ` prefix that macOS and iOS put on
/// everything the app prints.
fn app_text(line: &str) -> &str {
    line.strip_prefix("flutter: ").unwrap_or(line)
}

/// Flutter prints framework errors to stdout, not as `error: true` logs.
pub fn looks_like_error(text: &str) -> bool {
    text.contains("EXCEPTION CAUGHT")
        || text.contains("Exception:")
        || text.contains("Error:")
        || text.contains("RenderFlex overflowed")
        || app_text(text).starts_with("══╡")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daemon_events_update_state() {
        let mut s = Shared::default();
        handle_stdout_line(&mut s, r#"[{"event":"app.start","params":{"appId":"a1"}}]"#);
        handle_stdout_line(
            &mut s,
            r#"[{"event":"app.debugPort","params":{"wsUri":"ws://x/ws"}}]"#,
        );
        handle_stdout_line(&mut s, r#"[{"event":"app.started","params":{}}]"#);
        handle_stdout_line(
            &mut s,
            r#"[{"event":"app.log","params":{"log":"hi","error":false}}]"#,
        );
        handle_stdout_line(&mut s, r#"[{"id":3,"result":{"code":0}}]"#);
        handle_stdout_line(&mut s, "Launching lib/main.dart");
        assert_eq!(s.app_id.as_deref(), Some("a1"));
        assert_eq!(s.ws_uri.as_deref(), Some("ws://x/ws"));
        assert!(s.started);
        assert!(s.responses.contains_key(&3));
        assert_eq!(s.logs.len(), 2);
    }

    #[test]
    fn log_buffer_is_bounded() {
        let mut s = Shared::default();
        for i in 0..(LOG_CAPACITY + 5) {
            s.push("app", false, i.to_string());
        }
        assert_eq!(s.logs.len(), LOG_CAPACITY);
        assert_eq!(s.logs.front().unwrap().seq, 5);
    }

    #[test]
    fn errors_only_keeps_flutter_framework_errors() {
        let shared = Mutex::new(Shared::default());
        lock(&shared).push("app", false, "plain".into());
        lock(&shared).push(
            "app",
            false,
            "══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞══".into(),
        );
        lock(&shared).push("stderr", true, "boom".into());
        let lines = logs_since(&shared, 0, true, 10);
        assert_eq!(lines.len(), 2);
        lock(&shared).push("app", false, "Bad state: boom".into());
        lock(&shared).push("app", false, "═════════════════".into());
        lock(&shared).push("app", false, "after".into());
        let texts: Vec<String> = logs_since(&shared, 0, true, 10)
            .into_iter()
            .map(|l| l.text)
            .collect();
        assert!(texts.contains(&"Bad state: boom".to_string()));
        assert!(!texts.contains(&"after".to_string()));
        assert_eq!(logs_since(&shared, 2, false, 10).len(), 4);
    }

    /// macOS and iOS prefix app output with `flutter: `; the block must
    /// still be kept whole and must still end at its closing rule.
    #[test]
    fn errors_only_keeps_whole_blocks_on_apple_platforms() {
        let shared = Mutex::new(Shared::default());
        for line in [
            "flutter: before",
            "flutter: ══╡ EXCEPTION CAUGHT BY GESTURE ╞══════",
            "flutter: The following RangeError was thrown while handling a gesture:",
            "flutter: #2      _ListState._removeLast (package:app/main.dart:29:27)",
            "flutter: ════════════════════════════",
            "flutter: after",
        ] {
            lock(&shared).push("flutter", false, line.into());
        }
        let texts: Vec<String> = logs_since(&shared, 0, true, 10)
            .into_iter()
            .map(|l| l.text)
            .collect();
        assert_eq!(texts.len(), 4, "{texts:?}");
        assert!(texts[1].contains("RangeError"));
        assert!(texts[2].contains("main.dart:29:27"));
        assert!(texts[3].ends_with("════"));
    }
}
