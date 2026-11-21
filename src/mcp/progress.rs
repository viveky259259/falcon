//! MCP progress notifications for long tool calls.
//!
//! When a `tools/call` request carries `_meta.progressToken`, the server sets
//! it here for the duration of the call; tools call [`report`] and the client
//! receives `notifications/progress`. Without a token, `report` does nothing.

use serde_json::{json, Value};
use std::io::Write;
use std::sync::Mutex;

static TOKEN: Mutex<Option<Value>> = Mutex::new(None);

pub fn set_token(token: Option<Value>) {
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = token;
}

pub fn notification(token: &Value, progress: f64, total: Option<f64>, message: &str) -> Value {
    let mut params = json!({ "progressToken": token, "progress": progress, "message": message });
    if let Some(total) = total {
        params["total"] = json!(total);
    }
    json!({ "jsonrpc": "2.0", "method": "notifications/progress", "params": params })
}

/// Send a progress notification if the current call asked for them.
pub fn report(progress: f64, total: Option<f64>, message: &str) {
    let token = TOKEN.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(token) = token {
        let line = notification(&token, progress, total, message).to_string();
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_shape() {
        let n = notification(&json!("t1"), 2.0, Some(4.0), "building");
        assert_eq!(n["method"], "notifications/progress");
        assert_eq!(n["params"]["progressToken"], "t1");
        assert_eq!(n["params"]["total"], 4.0);
        let n = notification(&json!(7), 1.0, None, "x");
        assert!(n["params"].get("total").is_none());
    }
}
