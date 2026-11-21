//! One `flutter run --machine` process kept alive across MCP calls.
//!
//! `run_app` starts it and waits for the VM Service URI; later runtime tools
//! default to that URI, `reload` sends real hot reloads through the flutter
//! tool's daemon protocol, and `logs` reads what the app printed. `stop_app`
//! (or the MCP server exiting) stops it.

use serde::Deserialize;
use serde_json::{json, Value};

use super::session_state::{handle_stdout_line, lock, logs_since, tail_text, Shared};
pub use super::session_state::{looks_like_error, LogLine};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

struct AppSession {
    child: Child,
    stdin: ChildStdin,
    project: PathBuf,
    device: Option<String>,
    shared: Arc<Mutex<Shared>>,
    next_request: u64,
    started_at: Instant,
}

static SESSION: Mutex<Option<AppSession>> = Mutex::new(None);

fn session() -> MutexGuard<'static, Option<AppSession>> {
    SESSION.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAppArgs {
    #[serde(default = "dot")]
    pub path: String,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub flavor: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    #[serde(default = "default_start_timeout")]
    pub timeout_secs: u64,
}

fn dot() -> String {
    ".".to_string()
}

fn default_start_timeout() -> u64 {
    300
}

/// The VM Service URI of the running session, if any.
pub fn current_uri() -> Option<String> {
    let mut guard = session();
    let s = guard.as_mut()?;
    if !alive(s) {
        return None;
    }
    let uri = lock(&s.shared).ws_uri.clone();
    uri
}

/// The project directory of the running session, if any.
pub fn current_project() -> Option<PathBuf> {
    session().as_ref().map(|s| s.project.clone())
}

fn alive(s: &mut AppSession) -> bool {
    matches!(s.child.try_wait(), Ok(None)) && !lock(&s.shared).stopped
}

fn status_json(s: &mut AppSession) -> Value {
    let running = alive(s);
    let shared = lock(&s.shared);
    json!({
        "running": running,
        "project": s.project.display().to_string(),
        "device": s.device,
        "app_id": shared.app_id,
        "vm_service_uri": shared.ws_uri,
        "uptime_secs": s.started_at.elapsed().as_secs(),
        "log_cursor": shared.next_seq,
    })
}

/// Current session status, for the `falcon://session` resource.
pub fn status() -> Value {
    match session().as_mut() {
        Some(s) => status_json(s),
        None => json!({ "running": false }),
    }
}

/// Start the app, or return the session already running for this project.
pub fn run_app(args: RunAppArgs) -> Result<Value, String> {
    let project = PathBuf::from(&args.path);
    if !project.join("pubspec.yaml").is_file() {
        return Err(format!("No pubspec.yaml in {}", project.display()));
    }
    let mut guard = session();
    if let Some(s) = guard.as_mut() {
        if alive(s) {
            if same_dir(&s.project, &project) {
                let mut status = status_json(s);
                status["already_running"] = json!(true);
                return Ok(status);
            }
            return Err(format!(
                "An app from {} is already running; call stop_app first",
                s.project.display()
            ));
        }
    }
    // A session can look stopped (app.stop, closed stdout) while its
    // `flutter run` is still alive; end it before starting another.
    if let Some(mut old) = guard.take() {
        let _ = old.child.kill();
        let _ = old.child.wait();
    }

    let mut cmd = Command::new("flutter");
    cmd.arg("run").arg("--machine").current_dir(&project);
    if let Some(device) = &args.device {
        cmd.arg("-d").arg(device);
        if device == "flutter-tester" {
            cmd.arg("--show-test-device");
        }
    }
    if let Some(target) = &args.target {
        cmd.arg("-t").arg(target);
    }
    if let Some(flavor) = &args.flavor {
        cmd.arg("--flavor").arg(flavor);
    }
    cmd.args(&args.extra_args);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start `flutter run` (is Flutter on PATH?): {e}"))?;

    let shared = Arc::new(Mutex::new(Shared::default()));
    spawn_readers(&mut child, &shared);
    let stdin = child.stdin.take().ok_or("No stdin for flutter run")?;
    let mut s = AppSession {
        child,
        stdin,
        project,
        device: args.device.clone(),
        shared,
        next_request: 1,
        started_at: Instant::now(),
    };

    let timeout = Duration::from_secs(args.timeout_secs.clamp(10, 1800));
    if let Err(e) = wait_until_started(&mut s, timeout) {
        let tail = tail_text(&s.shared, 25);
        let _ = s.child.kill();
        let _ = s.child.wait();
        return Err(format!("{e}\nLast output from flutter run:\n{tail}"));
    }
    print_framework_errors(&s.shared);
    let status = status_json(&mut s);
    *guard = Some(s);
    Ok(status)
}

/// `flutter run --machine` turns on structured errors, which sends framework
/// exceptions to IDEs as VM Service events instead of printing them. Turn it
/// off so exceptions land in the app output that `logs` reads. Best effort.
fn print_framework_errors(shared: &Mutex<Shared>) {
    let Some(uri) = lock(shared).ws_uri.clone() else {
        return;
    };
    let outcome = super::runtime_tools::run_async("Enabling error output", 10, async {
        let client = super::runtime_tools::connect(&uri).await?;
        client
            .call(
                "ext.flutter.inspector.structuredErrors",
                json!({ "isolateId": client.isolate_id, "enabled": "false" }),
            )
            .await
    });
    if let Err(e) = outcome {
        lock(shared).push(
            "falcon",
            false,
            format!("Could not switch on printed errors: {e}"),
        );
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn wait_until_started(s: &mut AppSession, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    let mut last_progress: Option<String> = None;
    loop {
        {
            let shared = lock(&s.shared);
            if shared.started && shared.ws_uri.is_some() {
                return Ok(());
            }
            if shared.stopped {
                return Err("flutter run stopped before the app started".to_string());
            }
            if shared.progress != last_progress {
                last_progress = shared.progress.clone();
                if let Some(msg) = &last_progress {
                    super::progress::report(started.elapsed().as_secs_f64(), None, msg);
                }
            }
        }
        if let Ok(Some(status)) = s.child.try_wait() {
            return Err(format!(
                "flutter run exited with {status} before the app started"
            ));
        }
        if started.elapsed() > timeout {
            return Err(format!(
                "The app did not start within {}s",
                timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn spawn_readers(child: &mut Child, shared: &Arc<Mutex<Shared>>) {
    if let Some(out) = child.stdout.take() {
        let shared = Arc::clone(shared);
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                handle_stdout_line(&mut lock(&shared), &line);
            }
            lock(&shared).stopped = true;
        });
    }
    if let Some(err) = child.stderr.take() {
        let shared = Arc::clone(shared);
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                lock(&shared).push("stderr", true, line);
            }
        });
    }
}

/// Send a daemon request and wait for its response.
fn request(
    s: &mut AppSession,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let id = s.next_request;
    s.next_request += 1;
    let payload = json!([{ "id": id, "method": method, "params": params }]);
    writeln!(s.stdin, "{payload}")
        .and_then(|_| s.stdin.flush())
        .map_err(|e| format!("Could not talk to flutter run: {e}"))?;
    let started = Instant::now();
    loop {
        if let Some(resp) = lock(&s.shared).responses.remove(&id) {
            return Ok(resp);
        }
        if !alive(s) {
            return Err("flutter run exited".to_string());
        }
        if started.elapsed() > timeout {
            return Err(format!(
                "{method} got no answer within {}s",
                timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Hot reload (or restart) through the flutter tool, which recompiles the
/// changed sources first. `None` when no session is running.
pub fn hot_reload(full_restart: bool) -> Option<Result<Value, String>> {
    let mut guard = session();
    let s = guard.as_mut()?;
    if !alive(s) {
        return None;
    }
    let app_id = lock(&s.shared).app_id.clone()?;
    let cursor = lock(&s.shared).next_seq;
    let started = Instant::now();
    let params = json!({ "appId": app_id, "fullRestart": full_restart, "pause": false, "reason": "falcon-mcp" });
    Some(
        request(s, "app.restart", params, Duration::from_secs(120)).map(|resp| {
            let result = &resp["result"];
            let success = resp.get("error").is_none() && result["code"].as_i64().unwrap_or(0) == 0;
            let errors: Vec<String> = logs_since(&s.shared, cursor, true, 50)
                .into_iter()
                .map(|l| l.text)
                .collect();
            json!({
                "mode": if full_restart { "restart" } else { "reload" },
                "via": "flutter run",
                "success": success,
                "message": result["message"].as_str().or_else(|| resp["error"].as_str()),
                "elapsed_ms": started.elapsed().as_millis() as u64,
                "errors_since_reload": errors,
            })
        }),
    )
}

/// App output since `since` (a cursor from a previous call). `None` when no
/// session exists.
pub fn session_logs(since: u64, errors_only: bool, limit: usize) -> Option<Value> {
    let guard = session();
    let s = guard.as_ref()?;
    let lines = logs_since(&s.shared, since, errors_only, limit);
    let cursor = lock(&s.shared).next_seq;
    Some(json!({ "source": "flutter run", "lines": lines, "next_cursor": cursor }))
}

/// Stop the app. Returns what happened.
pub fn stop_app() -> Value {
    let Some(mut s) = session().take() else {
        return json!({ "stopped": false, "reason": "no app is running" });
    };
    if alive(&mut s) {
        let app_id = lock(&s.shared).app_id.clone();
        if let Some(app_id) = app_id {
            let _ = request(
                &mut s,
                "app.stop",
                json!({ "appId": app_id }),
                Duration::from_secs(10),
            );
        }
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && matches!(s.child.try_wait(), Ok(None)) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = s.child.kill();
    let _ = s.child.wait();
    json!({ "stopped": true, "project": s.project.display().to_string() })
}

/// Called when the MCP server exits so no `flutter run` is left behind.
pub fn shutdown() {
    if session().is_some() {
        stop_app();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_app_rejects_a_directory_without_pubspec() {
        let dir = tempfile::tempdir().unwrap();
        let args = RunAppArgs {
            path: dir.path().display().to_string(),
            device: None,
            target: None,
            flavor: None,
            extra_args: vec![],
            timeout_secs: 10,
        };
        assert!(run_app(args).unwrap_err().contains("pubspec.yaml"));
    }
}
