//! Execute app-owned Flutter integration tests and preserve their evidence.
mod events;
mod process;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct VerifyConfig {
    pub project: PathBuf,
    pub target: PathBuf,
    pub device: String,
    pub timeout: Duration,
    pub output_dir: PathBuf,
    pub flutter: PathBuf,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    id: String,
    name: String,
    is_supported: bool,
    target_platform: String,
    emulator: bool,
}

#[derive(Serialize)]
pub struct VerifyReport {
    pub status: String,
    project: PathBuf,
    target: PathBuf,
    target_sha256: String,
    device: Option<Device>,
    environment: String,
    native_claim: &'static str,
    elapsed_ms: u128,
    pub exit_code: Option<i32>,
    tests: events::Tests,
    problems: Vec<String>,
}

pub async fn verify(config: VerifyConfig) -> Result<VerifyReport> {
    let started = Instant::now();
    let project = config
        .project
        .canonicalize()
        .context("Flutter project directory does not exist")?;
    if !project.join("pubspec.yaml").is_file() {
        bail!("Missing pubspec.yaml in {}", project.display());
    }
    let target = project
        .join(&config.target)
        .canonicalize()
        .context("Integration test target does not exist")?;
    let relative = target
        .strip_prefix(project.join("integration_test"))
        .context("Target must be inside the project's integration_test directory")?;
    if !target.is_file() || !relative.to_string_lossy().ends_with("_test.dart") {
        bail!("Target must be an integration_test/*_test.dart file");
    }
    if config.timeout.is_zero() {
        bail!("Timeout must be positive");
    }
    let flutter = if config.flutter.components().count() > 1 {
        config
            .flutter
            .canonicalize()
            .context("Flutter executable path does not exist")?
    } else {
        config.flutter.clone()
    };
    fs::create_dir(&config.output_dir).with_context(|| {
        format!(
            "Evidence directory must be new: {}",
            config.output_dir.display()
        )
    })?;
    let mut report = VerifyReport {
        status: "blocked".into(),
        project: project.clone(),
        target: target.clone(),
        target_sha256: format!("{:x}", Sha256::digest(fs::read(&target)?)),
        device: None,
        environment: "unknown".into(),
        native_claim: "requires_review_of_test_assertions",
        elapsed_ms: 0,
        exit_code: None,
        tests: events::Tests::default(),
        problems: Vec::new(),
    };
    let result = execute(&config, &project, &target, &flutter, &mut report, started).await;
    if let Err(error) = result {
        report.problems.push(format!("{error:#}"));
    }
    report.elapsed_ms = started.elapsed().as_millis();
    fs::write(
        config.output_dir.join("summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )
    .context("Writing verification summary")?;
    Ok(report)
}

async fn execute(
    config: &VerifyConfig,
    project: &std::path::Path,
    target: &std::path::Path,
    flutter: &std::path::Path,
    report: &mut VerifyReport,
    started: Instant,
) -> Result<()> {
    let capture = process::capture(
        flutter,
        &["devices".into(), "--machine".into()],
        project,
        config.timeout,
        1024 * 1024,
    )
    .await?;
    fs::write(config.output_dir.join("devices.json"), &capture.stdout)?;
    fs::write(
        config.output_dir.join("devices-stderr.log"),
        &capture.stderr,
    )?;
    if capture.timed_out {
        report.status = "timed_out".into();
        bail!("Device discovery timed out");
    }
    if let Some(error) = capture.error {
        bail!("Device discovery: {error}");
    }
    if capture.exit_code != Some(0) {
        bail!("Device discovery exited {:?}", capture.exit_code);
    }
    let devices: Vec<Device> =
        serde_json::from_slice(&capture.stdout).context("Invalid Flutter device JSON")?;
    let device = devices
        .into_iter()
        .find(|device| device.id == config.device && device.is_supported)
        .with_context(|| format!("Supported device {:?} was not found", config.device))?;
    report.environment = if device.emulator {
        "emulator"
    } else if device.target_platform.starts_with("android") || device.target_platform == "ios" {
        "physical_mobile"
    } else if device.target_platform == "web-javascript" {
        "browser"
    } else {
        "desktop"
    }
    .into();
    report.device = Some(device);
    report.status = "failed".into();
    let remaining = config
        .timeout
        .checked_sub(started.elapsed())
        .context("Verification deadline elapsed during device discovery")?;
    let args = vec![
        "test".into(),
        target.strip_prefix(project)?.to_string_lossy().into_owned(),
        "-d".into(),
        config.device.clone(),
        "--machine".into(),
    ];
    let capture = process::capture(flutter, &args, project, remaining, 16 * 1024 * 1024).await?;
    fs::write(config.output_dir.join("test-events.jsonl"), &capture.stdout)?;
    fs::write(config.output_dir.join("test-stderr.log"), &capture.stderr)?;
    report.exit_code = capture.exit_code;
    if capture.timed_out {
        report.status = "timed_out".into();
        bail!("Integration test deadline elapsed");
    }
    if let Some(error) = capture.error {
        bail!("Integration test capture: {error}");
    }
    events::summarize(&capture.stdout, &mut report.tests)?;
    if capture.exit_code != Some(0) {
        bail!("Integration test process exited {:?}", capture.exit_code);
    }
    report.status = "passed".into();
    Ok(())
}
