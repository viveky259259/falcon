use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const FILE_COUNT: usize = 200;
const SAMPLE_COUNT: usize = 8;
const DEFAULT_MAX_COLD_P95_MS: u128 = 10_000;
const DEFAULT_MAX_WARM_P95_MS: u128 = 3_000;

fn main() {
    let max_cold_p95_ms = threshold_ms("FALCON_REVIEW_COLD_P95_MS", DEFAULT_MAX_COLD_P95_MS);
    let max_warm_p95_ms = threshold_ms("FALCON_REVIEW_WARM_P95_MS", DEFAULT_MAX_WARM_P95_MS);
    let bin = locate_falcon();
    let temp = tempfile::tempdir().expect("create temp repo");
    create_changed_repo(temp.path());

    let cold = run_review(&bin, temp.path()).expect("cold falcon review");
    let mut warm_samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        warm_samples.push(run_review(&bin, temp.path()).expect("warm falcon review"));
    }
    warm_samples.sort_unstable();
    let warm_p95 = percentile(&warm_samples, 95);

    println!(
        "review_p95: files={}, cold={}ms, warm_p95={}ms, cold_threshold={}ms, warm_threshold={}ms",
        FILE_COUNT,
        cold.as_millis(),
        warm_p95.as_millis(),
        max_cold_p95_ms,
        max_warm_p95_ms
    );

    if cold.as_millis() >= max_cold_p95_ms {
        panic!(
            "review cold latency exceeded threshold: {}ms >= {}ms",
            cold.as_millis(),
            max_cold_p95_ms
        );
    }
    if warm_p95.as_millis() >= max_warm_p95_ms {
        panic!(
            "review warm p95 exceeded threshold: {}ms >= {}ms",
            warm_p95.as_millis(),
            max_warm_p95_ms
        );
    }
}

fn threshold_ms(name: &str, default: u128) -> u128 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn locate_falcon() -> PathBuf {
    if let Ok(path) = std::env::var("FALCON_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }

    let current = std::env::current_exe().expect("current bench executable path");
    let profile_dir = current
        .parent()
        .and_then(|p| p.parent())
        .expect("bench executable under target/<profile>/deps");

    let binary_name = if cfg!(windows) {
        "falcon.exe"
    } else {
        "falcon"
    };

    for candidate in [
        profile_dir.join(binary_name),
        profile_dir
            .parent()
            .unwrap_or(profile_dir)
            .join("release")
            .join(binary_name),
        profile_dir
            .parent()
            .unwrap_or(profile_dir)
            .join("debug")
            .join(binary_name),
    ] {
        if candidate.is_file() {
            return candidate;
        }
    }

    panic!(
        "could not locate falcon binary near {}; run `cargo build --release --bin falcon` or set FALCON_BIN",
        current.display()
    );
}

fn create_changed_repo(root: &Path) {
    command(root, "git", &["init"]);
    command(root, "git", &["config", "user.email", "bench@example.com"]);
    command(root, "git", &["config", "user.name", "Falcon Bench"]);

    let lib = root.join("lib");
    std::fs::create_dir_all(&lib).expect("create lib dir");
    for idx in 0..FILE_COUNT {
        std::fs::write(lib.join(format!("file_{idx:03}.dart")), baseline_file(idx))
            .expect("write baseline dart fixture");
    }
    command(root, "git", &["add", "."]);
    command(root, "git", &["commit", "-m", "baseline"]);

    for idx in 0..FILE_COUNT {
        std::fs::write(lib.join(format!("file_{idx:03}.dart")), changed_file(idx))
            .expect("write changed dart fixture");
    }
    command(root, "git", &["add", "."]);
    command(root, "git", &["commit", "-m", "change 200 dart files"]);
}

fn baseline_file(idx: usize) -> String {
    format!("class Widget{idx:03} {{\n  const Widget{idx:03}();\n\n  int value() => {idx};\n}}\n")
}

fn changed_file(idx: usize) -> String {
    format!(
        "class Widget{idx:03} {{\n  const Widget{idx:03}();\n\n  int value() {{\n    print('debug {idx}');\n    return {idx};\n  }}\n}}\n"
    )
}

fn run_review(bin: &Path, root: &Path) -> Result<Duration, String> {
    let start = Instant::now();
    let output = Command::new(bin)
        .args([
            "review",
            root.to_str().ok_or("non-utf8 repo path")?,
            "--base-ref",
            "HEAD~1",
            "--format",
            "json",
            "--strictness",
            "quick",
            "--no-defer-to-analyzer",
        ])
        .output()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    let elapsed = start.elapsed();

    if !output.status.success() {
        return Err(format_output("falcon review failed", &output));
    }

    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("invalid review JSON: {e}\n{}", stdout(&output)))?;
    let files = json["summary"]["files_analyzed"]
        .as_u64()
        .ok_or_else(|| format!("missing files_analyzed in review JSON: {json}"))?;
    if files != FILE_COUNT as u64 {
        return Err(format!(
            "expected {FILE_COUNT} files analyzed, got {files}: {json}"
        ));
    }

    Ok(elapsed)
}

fn percentile(samples: &[Duration], pct: usize) -> Duration {
    assert!(!samples.is_empty());
    let idx = ((samples.len() * pct).div_ceil(100)).saturating_sub(1);
    samples[idx.min(samples.len() - 1)]
}

fn command(cwd: &Path, program: &str, args: &[&str]) {
    let output = Command::new(program)
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn {program} {args:?}: {e}"));
    if !output.status.success() {
        panic!(
            "{}",
            format_output(&format!("{program} {args:?} failed"), &output)
        );
    }
}

fn format_output(label: &str, output: &Output) -> String {
    format!(
        "{label}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout(output),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
