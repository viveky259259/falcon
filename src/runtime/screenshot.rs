//! Native screenshots of a running macOS application window.
//!
//! This path intentionally does not use Flutter tooling or the Dart VM service:
//! a user should be able to capture the app they are looking at, regardless of
//! how it was launched.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl WindowBounds {
    fn screencapture_region(self) -> String {
        format!("-R{},{},{},{}", self.x, self.y, self.width, self.height)
    }
}

/// Capture the current project's macOS app window as a PNG.
///
/// This is the explicit native-window path for `falcon screenshot --window`.
///
/// When Falcon cannot find a built macOS app for `project_path`, it captures the
/// frontmost window instead. The first invocation may require macOS
/// Accessibility and Screen Recording permission for the terminal application
/// running Falcon.
pub fn capture_current_app(project_path: &Path, output_path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        ensure_parent_dir(output_path);
        let app_name = built_macos_app_name(project_path);
        let bounds = window_bounds(app_name.as_deref())?;
        let output = Command::new("screencapture")
            .args([
                bounds.screencapture_region(),
                "-o".to_string(),
                "-x".to_string(),
                "-t".to_string(),
                "png".to_string(),
                output_path.display().to_string(),
            ])
            .output()
            .context("Failed to run macOS `screencapture`")?;

        if !output.status.success() {
            bail!(
                "macOS could not capture the app window: {}\n\
                 Grant Screen Recording permission to the terminal app that runs Falcon in \
                 System Settings → Privacy & Security → Screen Recording.",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        if !output_path.exists() {
            bail!("macOS `screencapture` reported success but no file was written");
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (project_path, output_path);
        bail!(
            "`falcon screenshot --window` captures a macOS app window only. \
             Omit `--window` to capture the running Flutter app instead."
        );
    }
}

#[cfg(target_os = "macos")]
fn window_bounds(app_name: Option<&str>) -> Result<WindowBounds> {
    const SCRIPT: &str = r#"
on run argv
  tell application "System Events"
    if (count of argv) is 0 then
      set targetProcess to first application process whose frontmost is true
    else
      set targetProcess to first application process whose name is item 1 of argv
    end if
    tell front window of targetProcess
    set {x, y} to position
    set {w, h} to size
    return (x as integer) & "," & (y as integer) & "," & (w as integer) & "," & (h as integer)
    end tell
  end tell
end run
"#;

    let mut command = Command::new("osascript");
    command.args(["-e", SCRIPT]);
    if let Some(app_name) = app_name {
        command.arg(app_name);
    }
    let output = command
        .output()
        .context("Failed to inspect the frontmost macOS window")?;
    if !output.status.success() {
        let target = app_name
            .map(|name| format!("the running `{name}` window"))
            .unwrap_or_else(|| "the frontmost window".to_string());
        bail!(
            "Falcon could not read {target}: {}\n\
             Grant Accessibility permission to the terminal app that runs Falcon in \
             System Settings → Privacy & Security → Accessibility.",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    parse_window_bounds(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn built_macos_app_name(project_path: &Path) -> Option<String> {
    let products = project_path.join("build/macos/Build/Products/Debug");
    let mut bundles = std::fs::read_dir(products)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .collect::<Vec<PathBuf>>();
    bundles.sort();
    bundles
        .into_iter()
        .next()
        .and_then(|bundle| bundle.file_stem()?.to_str().map(str::to_owned))
}

fn parse_window_bounds(value: &str) -> Result<WindowBounds> {
    let values = value
        .trim()
        .split(',')
        .map(|part| part.trim().parse::<i32>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("macOS returned invalid frontmost-window bounds")?;
    let [x, y, width, height]: [i32; 4] = values
        .try_into()
        .map_err(|_| anyhow::anyhow!("macOS returned incomplete frontmost-window bounds"))?;
    if width <= 0 || height <= 0 {
        bail!("macOS returned a frontmost window with an empty size");
    }

    Ok(WindowBounds {
        x,
        y,
        width: width as u32,
        height: height as u32,
    })
}

fn ensure_parent_dir(output_path: &Path) {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmost_window_bounds() {
        let bounds = parse_window_bounds(" 12, -34, 1280, 720\n").unwrap();
        assert_eq!(
            bounds,
            WindowBounds {
                x: 12,
                y: -34,
                width: 1280,
                height: 720,
            }
        );
        assert_eq!(bounds.screencapture_region(), "-R12,-34,1280,720");
    }

    #[test]
    fn rejects_invalid_or_empty_window_bounds() {
        assert!(parse_window_bounds("1,2,three,4").is_err());
        assert!(parse_window_bounds("1,2,3").is_err());
        assert!(parse_window_bounds("1,2,0,4").is_err());
    }

    #[test]
    fn finds_the_built_macos_app_name() {
        let temp = tempfile::tempdir().unwrap();
        let products = temp.path().join("build/macos/Build/Products/Debug");
        std::fs::create_dir_all(products.join("PDF Notes.app")).unwrap();
        std::fs::create_dir_all(products.join("Ignored")).unwrap();

        assert_eq!(
            built_macos_app_name(temp.path()).as_deref(),
            Some("PDF Notes")
        );
    }
}
