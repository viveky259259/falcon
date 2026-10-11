use anyhow::{Context, Result};
use std::{io, path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

pub(super) struct Capture {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub error: Option<String>,
}

async fn read_bounded(
    mut stream: impl tokio::io::AsyncRead + Unpin,
    bytes: &mut Vec<u8>,
    limit: usize,
) -> io::Result<()> {
    let mut chunk = [0u8; 8192];
    loop {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Ok(());
        }
        let available = limit.saturating_sub(bytes.len());
        bytes.extend_from_slice(&chunk[..count.min(available)]);
        if count > available {
            return Err(io::Error::other(format!("Output exceeded {limit} bytes")));
        }
    }
}

/// Each subprocess gets its own group so timeout cleanup includes its children.
pub(super) async fn capture(
    program: &Path,
    args: &[String],
    directory: &Path,
    duration: Duration,
    stdout_limit: usize,
) -> Result<Capture> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .with_context(|| format!("Launching {}", program.display()))?;
    let pid = child.id().context("Launched process has no ID")?;
    let stdout = child.stdout.take().context("Missing stdout pipe")?;
    let stderr = child.stderr.take().context("Missing stderr pipe")?;
    let mut result = Capture {
        stdout: Vec::new(),
        stderr: Vec::new(),
        exit_code: None,
        timed_out: false,
        error: None,
    };
    let outcome = tokio::time::timeout(duration, async {
        tokio::try_join!(
            read_bounded(stdout, &mut result.stdout, stdout_limit),
            read_bounded(stderr, &mut result.stderr, 2 * 1024 * 1024),
            child.wait()
        )
    })
    .await;
    match outcome {
        Ok(Ok(((), (), status))) => result.exit_code = status.code(),
        Ok(Err(error)) => result.error = Some(error.to_string()),
        Err(_) => result.timed_out = true,
    }
    if result.error.is_some() || result.timed_out {
        terminate_group(pid).await?;
        let _ = child.start_kill();
        tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .context("Timed out reaping verification process")??;
    }
    Ok(result)
}

async fn terminate_group(pid: u32) -> Result<()> {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("/bin/kill");
        command.args(["-KILL", "--", &format!("-{pid}")]);
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/T", "/F"]);
        command
    };
    #[cfg(not(any(unix, windows)))]
    anyhow::bail!("Process-tree cleanup is unsupported on this platform");
    // A group may already have exited after hitting a pipe limit. Reaping the
    // direct child remains mandatory regardless of the cleanup command status.
    #[cfg(any(unix, windows))]
    tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .context("Process-tree cleanup timed out")?
        .context("Launching process-tree cleanup")?;
    Ok(())
}
