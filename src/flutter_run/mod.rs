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

use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

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

    // Read stdout
    let stdout = child.stdout.take().expect("stdout was piped");
    let errors_stdout = Arc::clone(&errors);
    let output_dir_stdout = output_dir.clone();
    let stdout_thread = std::thread::spawn(move || {
        stream_lines(
            BufReader::new(stdout),
            &errors_stdout,
            &output_dir_stdout,
            "stdout",
        );
    });

    // Read stderr
    let stderr = child.stderr.take().expect("stderr was piped");
    let errors_stderr = Arc::clone(&errors);
    let output_dir_stderr = output_dir.clone();
    let stderr_thread = std::thread::spawn(move || {
        stream_lines(
            BufReader::new(stderr),
            &errors_stderr,
            &output_dir_stderr,
            "stderr",
        );
    });

    stdout_thread.join().ok();
    stderr_thread.join().ok();

    let exit_status = child.wait().ok();
    let exit_code = exit_status.and_then(|s| s.code());

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
fn stream_lines<R: BufRead>(
    reader: R,
    errors: &Arc<Mutex<Vec<FlutterError>>>,
    output_dir: &Path,
    _source: &str,
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

pub fn print_run_summary(report: &FlutterRunReport) {
    eprintln!();
    if report.errors.is_empty() {
        eprintln!(
            "  {} Flutter run completed with {} errors",
            "✓".green().bold(),
            "no".green().bold()
        );
    } else {
        eprintln!(
            "  {} Flutter run completed with {} error{}",
            "✗".red().bold(),
            report.errors.len().to_string().red().bold(),
            if report.errors.len() == 1 { "" } else { "s" }
        );
        for err in &report.errors {
            eprintln!(
                "      {} {}",
                format!("[error_{}]", err.number).bright_yellow(),
                err.title.trim()
            );
            eprintln!(
                "         → {}",
                err.md_path.display().to_string().bright_white()
            );
        }
    }
    eprintln!();
}

// ─────────────────────────────────────────────────────────────────────────────
// OS notification
// ─────────────────────────────────────────────────────────────────────────────

/// Send a native desktop notification. Tries macOS → Linux → Windows in order.
/// Silently succeeds if no supported notifier is found.
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
