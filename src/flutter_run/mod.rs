//! Flutter Run — Launches `flutter run`, captures errors in real-time,
//! writes each distinct error to an incrementally-numbered `error_N.md` file,
//! and sends an OS / webhook notification for every batch of errors found.
//!
//! # Usage (CLI)
//!
//! ```text
//! falcon run [PATH]
//!        [--output-dir <DIR>]        # where to write error_N.md files (default: .)
//!        [--device <DEVICE_ID>]      # passed through to `flutter run -d`
//!        [--flavor <FLAVOR>]         # passed through to `flutter run --flavor`
//!        [--notify]                  # send OS desktop notification on error
//!        [--webhook <URL>]           # POST JSON summary to this URL on error
//! ```

pub mod monitor;

use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the `falcon run` command.
#[derive(Debug, Clone)]
pub struct FlutterRunConfig {
    /// Root of the Flutter project.
    pub project_path: PathBuf,
    /// Directory where `error_N.md` files are written (defaults to project root).
    pub output_dir: PathBuf,
    /// Optional `-d <device>` argument forwarded to `flutter run`.
    pub device: Option<String>,
    /// Optional `--flavor <flavor>` argument forwarded to `flutter run`.
    pub flavor: Option<String>,
    /// Send an OS desktop notification when errors are found.
    pub notify: bool,
    /// Optional webhook URL to POST a JSON summary when errors are found.
    pub webhook: Option<String>,
    /// Interval between live monitor snapshots (memory + issue count).
    /// Defaults to 30 seconds.
    pub monitor_interval: Duration,
    /// Whether the live monitor is enabled. Defaults to `true`. Set to `false`
    /// (e.g. via `--no-monitor`) to disable snapshots entirely.
    pub monitor_enabled: bool,
}

impl FlutterRunConfig {
    /// Create a config with sensible defaults rooted at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            output_dir: path.clone(),
            project_path: path,
            device: None,
            flavor: None,
            notify: false,
            webhook: None,
            monitor_interval: Duration::from_secs(monitor::DEFAULT_MONITOR_INTERVAL_SECS),
            monitor_enabled: true,
        }
    }
}

/// A single captured Flutter error / exception.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlutterError {
    /// Sequential 1-based index within this run.
    pub number: usize,
    /// Short one-line description (first meaningful error line).
    pub title: String,
    /// Full raw lines that make up this error block.
    pub body: Vec<String>,
    /// Path to the `error_N.md` file written for this error.
    pub md_path: PathBuf,
}

/// Summary returned after `flutter run` exits (or is interrupted).
#[derive(Debug)]
pub struct FlutterRunReport {
    /// All errors captured during the run.
    pub errors: Vec<FlutterError>,
    /// The raw exit code of `flutter run` (None if the process was killed).
    pub exit_code: Option<i32>,
}

impl FlutterRunReport {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Core runner
// ─────────────────────────────────────────────────────────────────────────────

/// Launch `flutter run`, stream output line-by-line, collect errors, write
/// markdown files, and notify on completion.
// LCOV_EXCL_START — spawns `flutter` subprocess; integration-tested via `falcon run` smoke; not unit-testable.
pub fn run_flutter_app(config: &FlutterRunConfig) -> Result<FlutterRunReport> {
    std::fs::create_dir_all(&config.output_dir)
        .context("Failed to create output directory for error reports")?;

    // ── Build the command ────────────────────────────────────────────────────
    let mut cmd = Command::new("flutter");
    cmd.arg("run");
    if let Some(device) = &config.device {
        cmd.args(["-d", device]);
    }
    if let Some(flavor) = &config.flavor {
        cmd.args(["--flavor", flavor]);
    }
    cmd.current_dir(&config.project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    eprintln!(
        "\n  {} Launching Flutter app in {} …\n",
        "▸".bright_cyan().bold(),
        config.project_path.display().to_string().bright_white()
    );

    let mut child = cmd
        .spawn()
        .context("Failed to spawn `flutter run`. Is Flutter installed and on PATH?")?;

    // ── Stream stdout + stderr concurrently via a shared error collector ─────
    let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
    let output_dir = config.output_dir.clone();

    // ── Spawn the live monitor (if enabled) — owns its own tokio runtime ────
    // Both pump threads need read-only access to the handle so they can call
    // `notify_vm_uri`. After both pumps exit we'll `try_unwrap` the Arc and
    // call `shutdown` (which joins the monitor thread).
    let monitor_handle = monitor::spawn_monitor(
        config.monitor_enabled,
        config.monitor_interval,
        Arc::clone(&errors),
    );
    let monitor_shared: Arc<Option<monitor::MonitorHandle>> = Arc::new(monitor_handle);

    // Read stdout
    let stdout = child.stdout.take().expect("stdout was piped");
    let errors_stdout = Arc::clone(&errors);
    let output_dir_stdout = output_dir.clone();
    let monitor_stdout = Arc::clone(&monitor_shared);
    let stdout_thread = std::thread::spawn(move || {
        stream_lines(
            BufReader::new(stdout),
            &errors_stdout,
            &output_dir_stdout,
            "stdout",
            monitor_stdout.as_ref().as_ref(),
        );
    });

    // Read stderr
    let stderr = child.stderr.take().expect("stderr was piped");
    let errors_stderr = Arc::clone(&errors);
    let output_dir_stderr = output_dir.clone();
    let monitor_stderr = Arc::clone(&monitor_shared);
    let stderr_thread = std::thread::spawn(move || {
        stream_lines(
            BufReader::new(stderr),
            &errors_stderr,
            &output_dir_stderr,
            "stderr",
            monitor_stderr.as_ref().as_ref(),
        );
    });

    stdout_thread.join().ok();
    stderr_thread.join().ok();

    let exit_status = child.wait().ok();
    let exit_code = exit_status.and_then(|s| s.code());

    // ── Shut down the monitor before printing the run summary ───────────────
    // Both pump threads have already been joined above so the only remaining
    // Arc reference is `monitor_shared`. If `try_unwrap` ever fails (some
    // future refactor adds another clone) we'll fall back to letting Drop
    // shut the monitor down as the Arc goes out of scope.
    if let Ok(Some(handle)) = Arc::try_unwrap(monitor_shared) {
        handle.shutdown();
    }

    let mut collected = errors.lock().unwrap();

    // Re-number sequentially (threads may have raced on numbering)
    for (i, err) in collected.iter_mut().enumerate() {
        err.number = i + 1;
    }

    let report = FlutterRunReport {
        errors: collected.clone(),
        exit_code,
    };

    // ── Console summary ──────────────────────────────────────────────────────
    print_run_summary(&report);

    // ── Notifications ────────────────────────────────────────────────────────
    if report.has_errors() {
        if config.notify {
            let msg = format!(
                "{} error{} found — see {}",
                report.errors.len(),
                if report.errors.len() == 1 { "" } else { "s" },
                config.output_dir.display()
            );
            send_os_notification("Falcon — Flutter Errors", &msg);
        }

        if let Some(webhook_url) = &config.webhook {
            if let Err(e) = send_webhook(&report, webhook_url, &config.project_path) {
                eprintln!("  {} Webhook notification failed: {}", "!".yellow(), e);
            }
        }
    }

    Ok(report)
}
// LCOV_EXCL_STOP

// ─────────────────────────────────────────────────────────────────────────────
// Line streaming + error detection
// ─────────────────────────────────────────────────────────────────────────────

/// Patterns that mark the beginning of a Flutter error block.
const ERROR_TRIGGERS: &[&str] = &[
    "══╡ EXCEPTION CAUGHT",
    "Error: ",
    "error: ",
    "Unhandled Exception:",
    "Exception:",
    "FlutterError(",
    "══╡ EXCEPTION",
    "EXCEPTION CAUGHT",
    "The following ", // "The following assertion was thrown…"
    "dart:core",      // stack frame prefixes in build errors
    "Failed to compile",
    "Compiler message:",
    "[ERROR]",
    "BUILD FAILED",
];

/// Sentinel that marks the end of a Flutter exception block.
const ERROR_END_MARKERS: &[&str] = &[
    "═══════════════════════════════════",
    "════════════════════════",
    "Reloaded",
    "Restarted",
    "Hot reload",
];

fn is_error_start(line: &str) -> bool {
    let trimmed = line.trim();
    ERROR_TRIGGERS.iter().any(|t| trimmed.contains(t))
}

fn is_error_end(line: &str) -> bool {
    let trimmed = line.trim();
    ERROR_END_MARKERS.iter().any(|m| trimmed.contains(m)) || trimmed.starts_with("════")
}

/// Read lines from `reader`, echo them to stderr, accumulate error blocks, and
/// write an `error_N.md` for each one.
///
/// When `monitor` is `Some`, each line is also scanned for a Dart VM Service
/// URI announcement and the monitor is notified so it can begin sampling.
fn stream_lines<R: BufRead>(
    reader: R,
    errors: &Arc<Mutex<Vec<FlutterError>>>,
    output_dir: &Path,
    _source: &str,
    monitor: Option<&monitor::MonitorHandle>,
) {
    let mut in_error = false;
    let mut current_block: Vec<String> = Vec::new();

    for raw in reader.lines() {
        let line = match raw {
            Ok(l) => l,
            Err(_) => break,
        };

        // Mirror all output to the terminal
        eprintln!("{}", line);

        // Forward a VM service URI to the live monitor (no-op after first hit).
        if let Some(m) = monitor {
            if let Some(uri) = monitor::parse_vm_service_uri(&line) {
                m.notify_vm_uri(uri);
            }
        }

        if !in_error && is_error_start(&line) {
            in_error = true;
            current_block.clear();
            current_block.push(line.clone());
        } else if in_error {
            if is_error_end(&line) && !current_block.is_empty() {
                // Flush the accumulated block
                flush_error_block(&current_block, errors, output_dir);
                current_block.clear();
                in_error = false;
            } else {
                current_block.push(line.clone());
            }
        }
    }

    // Flush any unterminated block at EOF
    if in_error && !current_block.is_empty() {
        flush_error_block(&current_block, errors, output_dir);
    }
}

/// Persist `block` as a new `error_N.md` and append it to `errors`.
fn flush_error_block(block: &[String], errors: &Arc<Mutex<Vec<FlutterError>>>, output_dir: &Path) {
    let mut guard = errors.lock().unwrap();
    let number = guard.len() + 1;

    let title = block
        .iter()
        .find(|l| !l.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("Error #{}", number));

    let md_path = output_dir.join(format!("error_{}.md", number));

    let content = render_error_md(number, &title, block);

    if let Err(e) = std::fs::write(&md_path, &content) {
        eprintln!(
            "  {} Could not write {}: {}",
            "!".yellow(),
            md_path.display(),
            e
        );
    } else {
        eprintln!(
            "  {} Error written → {}",
            "✎".bright_yellow().bold(),
            md_path.display().to_string().bright_white()
        );
    }

    guard.push(FlutterError {
        number,
        title: title.trim().to_string(),
        body: block.to_vec(),
        md_path,
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Markdown rendering
// ─────────────────────────────────────────────────────────────────────────────

fn render_error_md(number: usize, title: &str, body: &[String]) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Error #{}\n\n", number));
    md.push_str(&format!("> **{}**\n\n", title.trim().replace('`', "'")));
    md.push_str("## Output\n\n");
    md.push_str("```\n");
    for line in body {
        md.push_str(line);
        md.push('\n');
    }
    md.push_str("```\n\n");
    md.push_str(&format!(
        "---\n*Captured by [Falcon](https://github.com/falcon-lint/falcon) at {}*\n",
        chrono_now()
    ));

    md
}

/// Returns a simple ISO-8601 timestamp without pulling in the `chrono` crate.
fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Format as "YYYY-MM-DD HH:MM:SS UTC" via manual arithmetic
    let s = secs;
    let mins = s / 60;
    let hours = mins / 60;
    let days_total = hours / 24;
    let sec = s % 60;
    let min = mins % 60;
    let hour = hours % 24;
    // Approximate date (good enough for a log timestamp)
    let days_since_epoch = days_total;
    let year = 1970 + days_since_epoch / 365;
    let day_of_year = days_since_epoch % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year,
        month.min(12),
        day.min(31),
        hour,
        min,
        sec
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Console summary
// ─────────────────────────────────────────────────────────────────────────────

/// Format the run summary as a string (pure helper; extracted for testability).
pub fn format_run_summary(report: &FlutterRunReport) -> String {
    let mut out = String::new();
    out.push('\n');
    if report.errors.is_empty() {
        out.push_str(&format!(
            "  {} Flutter run completed with {} errors\n",
            "✓".green().bold(),
            "no".green().bold()
        ));
    } else {
        out.push_str(&format!(
            "  {} Flutter run completed with {} error{}\n",
            "✗".red().bold(),
            report.errors.len().to_string().red().bold(),
            if report.errors.len() == 1 { "" } else { "s" }
        ));
        for err in &report.errors {
            out.push_str(&format!(
                "      {} {}\n",
                format!("[error_{}]", err.number).bright_yellow(),
                err.title.trim()
            ));
            out.push_str(&format!(
                "         → {}\n",
                err.md_path.display().to_string().bright_white()
            ));
        }
    }
    out.push('\n');
    out
}

pub fn print_run_summary(report: &FlutterRunReport) {
    eprint!("{}", format_run_summary(report));
}

// ─────────────────────────────────────────────────────────────────────────────
// OS notification
// ─────────────────────────────────────────────────────────────────────────────

/// Send a native desktop notification. Tries macOS → Linux → Windows in order.
/// Silently succeeds if no supported notifier is found.
// LCOV_EXCL_START — invokes OS-specific notifier subprocess (osascript/notify-send/powershell).
pub fn send_os_notification(title: &str, message: &str) {
    // macOS
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            message.replace('"', "'"),
            title.replace('"', "'")
        );
        let _ = Command::new("osascript")
            .args(["-e", &script])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    // Linux (notify-send)
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send")
            .args([title, message])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        return;
    }

    // Windows (PowerShell toast)
    #[cfg(target_os = "windows")]
    {
        let ps_script = format!(
            r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null;
$template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02);
$template.GetElementsByTagName('text')[0].AppendChild($template.CreateTextNode('{}')) | Out-Null;
$template.GetElementsByTagName('text')[1].AppendChild($template.CreateTextNode('{}')) | Out-Null;
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Falcon').Show([Windows.UI.Notifications.ToastNotification]::new($template));"#,
            title.replace('\'', "`'"),
            message.replace('\'', "`'")
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}
// LCOV_EXCL_STOP

// ─────────────────────────────────────────────────────────────────────────────
// Webhook notification
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct WebhookPayload<'a> {
    event: &'static str,
    project: String,
    error_count: usize,
    errors: Vec<WebhookError<'a>>,
}

#[derive(Serialize)]
struct WebhookError<'a> {
    number: usize,
    title: &'a str,
    md_file: String,
}

// LCOV_EXCL_START — invokes curl subprocess; behavior verified manually.
fn send_webhook(report: &FlutterRunReport, url: &str, project_path: &Path) -> Result<()> {
    let payload = WebhookPayload {
        event: "flutter_run_errors",
        project: project_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into()),
        error_count: report.errors.len(),
        errors: report
            .errors
            .iter()
            .map(|e| WebhookError {
                number: e.number,
                title: &e.title,
                md_file: e.md_path.display().to_string(),
            })
            .collect(),
    };

    let json = serde_json::to_string(&payload)?;

    let status = Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            &json,
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .status()
        .context("Failed to invoke curl for webhook")?;

    if !status.success() {
        anyhow::bail!("curl exited with non-zero status");
    }

    Ok(())
}
// LCOV_EXCL_STOP

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::Once;
    use tempfile::TempDir;

    static INIT: Once = Once::new();
    fn disable_colors() {
        INIT.call_once(|| {
            colored::control::set_override(false);
        });
    }

    fn sample_error(number: usize, title: &str, md_path: PathBuf) -> FlutterError {
        FlutterError {
            number,
            title: title.to_string(),
            body: vec![title.to_string()],
            md_path,
        }
    }

    // ── is_error_start ────────────────────────────────────────────────────────

    #[test]
    fn is_error_start_error_prefix_returns_true() {
        assert!(is_error_start("Error: something went wrong"));
    }

    #[test]
    fn is_error_start_exception_caught_returns_true() {
        assert!(is_error_start("EXCEPTION CAUGHT by framework"));
    }

    #[test]
    fn is_error_start_flutter_error_returns_true() {
        assert!(is_error_start("FlutterError(RenderBox was not laid out)"));
    }

    #[test]
    fn is_error_start_error_tag_returns_true() {
        assert!(is_error_start("[ERROR] some error message"));
    }

    #[test]
    fn is_error_start_normal_line_returns_false() {
        assert!(!is_error_start("I/flutter (12345): normal log output"));
    }

    #[test]
    fn is_error_start_empty_line_returns_false() {
        assert!(!is_error_start(""));
    }

    #[test]
    fn is_error_start_whitespace_line_returns_false() {
        assert!(!is_error_start("   "));
    }

    // ── is_error_end ─────────────────────────────────────────────────────────

    #[test]
    fn is_error_end_long_bar_returns_true() {
        assert!(is_error_end("═══════════════════════════════════"));
    }

    #[test]
    fn is_error_end_medium_bar_returns_true() {
        assert!(is_error_end("════════════════════════"));
    }

    #[test]
    fn is_error_end_reloaded_returns_true() {
        assert!(is_error_end("Reloaded 3 of 400 libraries in 450ms."));
    }

    #[test]
    fn is_error_end_hot_reload_returns_true() {
        assert!(is_error_end("Hot reload performed in 312ms."));
    }

    #[test]
    fn is_error_end_starts_with_quad_bar_returns_true() {
        assert!(is_error_end("════ Exception Details ════"));
    }

    #[test]
    fn is_error_end_normal_line_returns_false() {
        assert!(!is_error_end(
            "The following assertion was thrown building MyWidget"
        ));
    }

    #[test]
    fn is_error_end_empty_line_returns_false() {
        assert!(!is_error_end(""));
    }

    // ── render_error_md ───────────────────────────────────────────────────────

    #[test]
    fn render_error_md_title_appears_in_output() {
        let body = vec!["Error: bad state".to_string()];
        let md = render_error_md(1, "Error: bad state", &body);
        assert!(md.contains("Error: bad state"), "title should appear in md");
    }

    #[test]
    fn render_error_md_backtick_in_title_replaced_with_single_quote() {
        let title = "Error: `foo` is null";
        let body = vec![title.to_string()];
        let md = render_error_md(1, title, &body);
        // The blockquote line should use single quotes, not backticks
        assert!(
            md.contains("> **Error: 'foo' is null**"),
            "backticks in title should be replaced with single quotes"
        );
    }

    #[test]
    fn render_error_md_body_inside_code_block() {
        let body = vec![
            "Error: something".to_string(),
            "  at stack frame 1".to_string(),
        ];
        let md = render_error_md(1, "Error: something", &body);
        assert!(md.contains("```\n"), "should have opening code fence");
        assert!(
            md.contains("  at stack frame 1\n"),
            "body line should be present"
        );
        assert!(md.contains("```\n\n"), "should have closing code fence");
    }

    #[test]
    fn render_error_md_footer_contains_falcon_link() {
        let body = vec!["Error: x".to_string()];
        let md = render_error_md(1, "Error: x", &body);
        assert!(
            md.contains("falcon-lint/falcon"),
            "footer should link to falcon"
        );
    }

    #[test]
    fn render_error_md_header_contains_error_number() {
        let body = vec!["Error: x".to_string()];
        let md = render_error_md(3, "Error: x", &body);
        assert!(
            md.starts_with("# Error #3\n"),
            "header should contain error number"
        );
    }

    // ── chrono_now ────────────────────────────────────────────────────────────

    #[test]
    fn chrono_now_returns_nonempty_string() {
        let ts = chrono_now();
        assert!(!ts.is_empty(), "timestamp should not be empty");
    }

    #[test]
    fn chrono_now_ends_with_utc() {
        let ts = chrono_now();
        assert!(
            ts.ends_with("UTC"),
            "timestamp should end with UTC: got {}",
            ts
        );
    }

    #[test]
    fn chrono_now_has_date_time_format() {
        let ts = chrono_now();
        // Expect format: "YYYY-MM-DD HH:MM:SS UTC"
        // Check length is correct: 4+1+2+1+2+1+2+1+2+1+2+1+3 = 23
        assert_eq!(ts.len(), 23, "timestamp length should be 23: got '{}'", ts);
        // Check the separators are in the right positions
        assert_eq!(&ts[4..5], "-", "year-month separator");
        assert_eq!(&ts[7..8], "-", "month-day separator");
        assert_eq!(&ts[10..11], " ", "date-time separator");
        assert_eq!(&ts[13..14], ":", "hour-min separator");
        assert_eq!(&ts[16..17], ":", "min-sec separator");
        assert_eq!(&ts[19..], " UTC", "UTC suffix");
    }

    // ── FlutterRunConfig::new ─────────────────────────────────────────────────

    #[test]
    fn flutter_run_config_new_sets_output_dir_to_path() {
        let cfg = FlutterRunConfig::new("/tmp/myproject");
        assert_eq!(cfg.output_dir, PathBuf::from("/tmp/myproject"));
    }

    #[test]
    fn flutter_run_config_new_sets_project_path_to_path() {
        let cfg = FlutterRunConfig::new("/tmp/myproject");
        assert_eq!(cfg.project_path, PathBuf::from("/tmp/myproject"));
    }

    #[test]
    fn flutter_run_config_new_device_is_none() {
        let cfg = FlutterRunConfig::new("/tmp/myproject");
        assert!(cfg.device.is_none());
    }

    #[test]
    fn flutter_run_config_new_flavor_is_none() {
        let cfg = FlutterRunConfig::new("/tmp/myproject");
        assert!(cfg.flavor.is_none());
    }

    #[test]
    fn flutter_run_config_new_notify_is_false() {
        let cfg = FlutterRunConfig::new("/tmp/myproject");
        assert!(!cfg.notify);
    }

    #[test]
    fn flutter_run_config_new_webhook_is_none() {
        let cfg = FlutterRunConfig::new("/tmp/myproject");
        assert!(cfg.webhook.is_none());
    }

    // ── FlutterRunReport::has_errors ──────────────────────────────────────────

    #[test]
    fn flutter_run_report_has_errors_empty_vec_returns_false() {
        let report = FlutterRunReport {
            errors: vec![],
            exit_code: Some(0),
        };
        assert!(!report.has_errors());
    }

    #[test]
    fn flutter_run_report_has_errors_nonempty_vec_returns_true() {
        let dir = TempDir::new().unwrap();
        let report = FlutterRunReport {
            errors: vec![sample_error(
                1,
                "Error: test",
                dir.path().join("error_1.md"),
            )],
            exit_code: Some(1),
        };
        assert!(report.has_errors());
    }

    // ── format_run_summary ────────────────────────────────────────────────────

    #[test]
    fn format_run_summary_no_errors_contains_completed_with_no_errors() {
        disable_colors();
        let report = FlutterRunReport {
            errors: vec![],
            exit_code: Some(0),
        };
        let summary = format_run_summary(&report);
        assert!(
            summary.contains("completed with"),
            "should say 'completed with'"
        );
        assert!(summary.contains("no"), "should say 'no'");
        assert!(summary.contains("errors"), "should say 'errors'");
    }

    #[test]
    fn format_run_summary_one_error_contains_singular_error() {
        disable_colors();
        let dir = TempDir::new().unwrap();
        let md_path = dir.path().join("error_1.md");
        let report = FlutterRunReport {
            errors: vec![sample_error(1, "Error: single failure", md_path)],
            exit_code: Some(1),
        };
        let summary = format_run_summary(&report);
        assert!(summary.contains("1 error"), "should say '1 error'");
        assert!(
            !summary.contains("1 errors"),
            "should not say '1 errors' (bad plural)"
        );
        assert!(
            summary.contains("Error: single failure"),
            "should contain error title"
        );
        assert!(summary.contains("error_1.md"), "should contain md path");
    }

    #[test]
    fn format_run_summary_three_errors_contains_plural_errors() {
        disable_colors();
        let dir = TempDir::new().unwrap();
        let errors = vec![
            sample_error(1, "Error: one", dir.path().join("error_1.md")),
            sample_error(2, "Error: two", dir.path().join("error_2.md")),
            sample_error(3, "Error: three", dir.path().join("error_3.md")),
        ];
        let report = FlutterRunReport {
            errors,
            exit_code: Some(1),
        };
        let summary = format_run_summary(&report);
        assert!(summary.contains("3 errors"), "should say '3 errors'");
    }

    // ── stream_lines ──────────────────────────────────────────────────────────

    #[test]
    fn stream_lines_single_error_block_collects_one_error() {
        let dir = TempDir::new().unwrap();
        let input = concat!(
            "normal log line\n",
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞══\n",
            "The following StateError was thrown:\n",
            "Bad state: foo\n",
            "═══════════════════════════════════\n",
            "post-error log line\n",
        );
        let cursor = Cursor::new(input.as_bytes());
        let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
        stream_lines(cursor, &errors, dir.path(), "test", None);

        let collected = errors.lock().unwrap();
        assert_eq!(collected.len(), 1, "should collect exactly one error");
        assert!(
            dir.path().join("error_1.md").exists(),
            "error_1.md should be written"
        );
    }

    #[test]
    fn stream_lines_two_error_blocks_collects_two_errors() {
        let dir = TempDir::new().unwrap();
        let input = concat!(
            "log before first error\n",
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞══\n",
            "First error details\n",
            "═══════════════════════════════════\n",
            "log between errors\n",
            "Error: second problem occurred\n",
            "at some stack frame\n",
            "Reloaded 1 of 400 libraries in 200ms.\n",
            "log after second error\n",
        );
        let cursor = Cursor::new(input.as_bytes());
        let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
        stream_lines(cursor, &errors, dir.path(), "test", None);

        let collected = errors.lock().unwrap();
        assert_eq!(collected.len(), 2, "should collect exactly two errors");
        assert!(
            dir.path().join("error_1.md").exists(),
            "error_1.md should be written"
        );
        assert!(
            dir.path().join("error_2.md").exists(),
            "error_2.md should be written"
        );
    }

    #[test]
    fn stream_lines_unterminated_block_is_flushed_at_eof() {
        let dir = TempDir::new().unwrap();
        // No end marker before EOF
        let input = concat!(
            "normal line\n",
            "Error: something broke\n",
            "stack frame line\n",
        );
        let cursor = Cursor::new(input.as_bytes());
        let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
        stream_lines(cursor, &errors, dir.path(), "test", None);

        let collected = errors.lock().unwrap();
        assert_eq!(
            collected.len(),
            1,
            "unterminated block should be flushed at EOF"
        );
        assert!(
            dir.path().join("error_1.md").exists(),
            "error_1.md should be written for unterminated block"
        );
    }

    // ── flush_error_block ─────────────────────────────────────────────────────

    #[test]
    fn flush_error_block_title_is_first_nonempty_line() {
        let dir = TempDir::new().unwrap();
        let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
        let block = vec![
            "".to_string(),
            "Error: the real title".to_string(),
            "more details".to_string(),
        ];
        flush_error_block(&block, &errors, dir.path());

        let collected = errors.lock().unwrap();
        assert_eq!(collected[0].title, "Error: the real title");
    }

    #[test]
    fn flush_error_block_md_path_points_into_tempdir() {
        let dir = TempDir::new().unwrap();
        let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
        let block = vec!["Error: test".to_string()];
        flush_error_block(&block, &errors, dir.path());

        let collected = errors.lock().unwrap();
        assert!(
            collected[0].md_path.starts_with(dir.path()),
            "md_path should be inside tempdir"
        );
    }

    #[test]
    fn flush_error_block_file_contents_contain_error_header() {
        let dir = TempDir::new().unwrap();
        let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));
        let block = vec!["Error: test flush".to_string()];
        flush_error_block(&block, &errors, dir.path());

        let collected = errors.lock().unwrap();
        let contents = std::fs::read_to_string(&collected[0].md_path).unwrap();
        assert!(
            contents.starts_with("# Error #1\n"),
            "file should start with markdown header"
        );
    }

    #[test]
    fn flush_error_block_sequential_calls_produce_incrementing_numbers() {
        let dir = TempDir::new().unwrap();
        let errors: Arc<Mutex<Vec<FlutterError>>> = Arc::new(Mutex::new(Vec::new()));

        let block1 = vec!["Error: first".to_string()];
        flush_error_block(&block1, &errors, dir.path());

        let block2 = vec!["Error: second".to_string()];
        flush_error_block(&block2, &errors, dir.path());

        let collected = errors.lock().unwrap();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].number, 1, "first call should produce number=1");
        assert_eq!(
            collected[1].number, 2,
            "second call should produce number=2"
        );
        assert!(
            dir.path().join("error_1.md").exists(),
            "error_1.md should exist"
        );
        assert!(
            dir.path().join("error_2.md").exists(),
            "error_2.md should exist"
        );
    }
}
