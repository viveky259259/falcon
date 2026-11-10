//! Verify encoded video QR fixtures without claiming physical camera acceptance.
use super::verify::process;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct QrCheckConfig {
    pub video: PathBuf,
    pub manifest: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
    pub ffmpeg: PathBuf,
    pub swiftc: PathBuf,
}

#[derive(Deserialize)]
struct Scene {
    start_seconds: f64,
    duration_seconds: f64,
    payload: Option<String>,
}

#[derive(Serialize)]
struct Sample {
    at_seconds: f64,
    expected: String,
    decoded: Vec<String>,
    passed: bool,
}

#[derive(Deserialize)]
struct Decoded {
    payloads: Vec<String>,
}

#[derive(Serialize)]
pub struct QrReport {
    pub status: String,
    scope: &'static str,
    video: PathBuf,
    video_sha256: String,
    manifest_sha256: String,
    samples: Vec<Sample>,
    problems: Vec<String>,
    elapsed_ms: u128,
}

pub async fn check(config: QrCheckConfig) -> Result<QrReport> {
    if !cfg!(target_os = "macos") {
        bail!("QR fixture decoding requires macOS Vision");
    }
    if config.timeout.is_zero() {
        bail!("Timeout must be positive");
    }
    let video = config
        .video
        .canonicalize()
        .context("Video does not exist")?;
    if !video.is_file() {
        bail!("Video must be a file");
    }
    let manifest = fs::read(&config.manifest).context("Reading QR scene manifest")?;
    let scenes: Vec<Scene> =
        serde_json::from_slice(&manifest).context("Invalid QR scene manifest")?;
    validate(&scenes)?;
    fs::create_dir(&config.output_dir).context("Evidence directory must be new")?;
    let output = config.output_dir.canonicalize()?;
    let started = Instant::now();
    let mut report = QrReport {
        status: "failed".into(),
        scope: "encoded_qr_fixture_only",
        video: video.clone(),
        video_sha256: hash_file(&video)?,
        manifest_sha256: format!("{:x}", Sha256::digest(&manifest)),
        samples: vec![],
        problems: vec![],
        elapsed_ms: 0,
    };
    fs::write(output.join("manifest.json"), manifest)?;
    if let Err(error) = execute(&config, &scenes, &output, &mut report, started).await {
        report.problems.push(format!("{error:#}"));
    }
    report.elapsed_ms = started.elapsed().as_millis();
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}

fn validate(scenes: &[Scene]) -> Result<()> {
    if scenes.is_empty() || scenes.len() > 128 || !scenes.iter().any(|s| s.payload.is_some()) {
        bail!("Manifest requires 1..128 scenes and at least one QR payload");
    }
    for scene in scenes {
        if !scene.start_seconds.is_finite()
            || scene.start_seconds < 0.0
            || !scene.duration_seconds.is_finite()
            || scene.duration_seconds <= 0.0
            || scene
                .payload
                .as_ref()
                .is_some_and(|p| p.is_empty() || p.len() > 8192)
        {
            bail!("Scene requires nonnegative finite start, positive finite duration and nonempty payload of at most 8192 bytes");
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

async fn run_tool(
    config: &QrCheckConfig,
    tool: &Path,
    args: &[String],
    output: &Path,
    label: &str,
    started: Instant,
) -> Result<Vec<u8>> {
    let remaining = config
        .timeout
        .checked_sub(started.elapsed())
        .context("QR verification deadline elapsed")?;
    let capture = process::capture(
        tool,
        args,
        &std::env::current_dir()?,
        remaining,
        1024 * 1024,
    )
    .await?;
    fs::write(output.join(format!("{label}-stdout.log")), &capture.stdout)?;
    fs::write(output.join(format!("{label}-stderr.log")), capture.stderr)?;
    if capture.timed_out {
        bail!("{label} timed out");
    }
    if let Some(error) = capture.error {
        bail!("{label}: {error}");
    }
    if capture.exit_code != Some(0) {
        bail!("{label} exited {:?}", capture.exit_code);
    }
    Ok(capture.stdout)
}

async fn execute(
    config: &QrCheckConfig,
    scenes: &[Scene],
    output: &Path,
    report: &mut QrReport,
    started: Instant,
) -> Result<()> {
    let source = output.join("qr_decoder.swift");
    let decoder = output.join("qr-decoder");
    fs::write(&source, include_str!("qr_decoder.swift"))?;
    run_tool(
        config,
        &config.swiftc,
        &[
            source.to_string_lossy().into(),
            "-o".into(),
            decoder.to_string_lossy().into(),
        ],
        output,
        "compile",
        started,
    )
    .await?;
    for (index, scene) in scenes.iter().enumerate() {
        let Some(payload) = &scene.payload else {
            continue;
        };
        let at = scene.start_seconds + (scene.duration_seconds / 2.0).min(3.0);
        let label = format!("sample-{index:03}");
        let frame = output.join(format!("{label}.png"));
        run_tool(
            config,
            &config.ffmpeg,
            &[
                "-nostdin".into(),
                "-v".into(),
                "error".into(),
                "-ss".into(),
                at.to_string(),
                "-i".into(),
                report.video.to_string_lossy().into(),
                "-frames:v".into(),
                "1".into(),
                frame.to_string_lossy().into(),
            ],
            output,
            &label,
            started,
        )
        .await?;
        let decoded: Decoded = serde_json::from_slice(
            &run_tool(
                config,
                &decoder,
                &[frame.to_string_lossy().into()],
                output,
                &format!("{label}-decode"),
                started,
            )
            .await?,
        )
        .context("Invalid QR decoder output")?;
        let passed = decoded.payloads == [payload.clone()];
        report.samples.push(Sample {
            at_seconds: at,
            expected: payload.clone(),
            decoded: decoded.payloads,
            passed,
        });
    }
    if report.samples.iter().any(|sample| !sample.passed) {
        bail!("Encoded QR payload mismatch; inspect samples and decoder logs");
    }
    report.status = "passed".into();
    Ok(())
}
