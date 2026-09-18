use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::version_util::version_cmp;

const GITHUB_REPO: &str = "viveky259259/falcon";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub assets: Vec<AssetInfo>,
    pub body: String,
}

#[derive(Debug)]
pub struct AssetInfo {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

fn platform_archive_label(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Some("macos-arm64"),
        ("macos", "x86_64") => Some("macos-x64"),
        ("linux", "x86_64") => Some("linux-x64"),
        _ => None,
    }
}

fn platform_asset_name(version: &str) -> Option<String> {
    platform_archive_label(env::consts::OS, env::consts::ARCH)
        .map(|label| format!("falcon-{}-{}.tar.gz", version, label))
}

fn fetch_json(url: &str) -> Result<String> {
    if let Ok(output) = std::process::Command::new("gh")
        .args(["api", url.trim_start_matches("https://api.github.com/")])
        .output()
    {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    let output = std::process::Command::new("curl")
        .args(["-sL", "-H", "Accept: application/vnd.github+json", url])
        .output()
        .context("Failed to run curl")?;

    if !output.status.success() {
        bail!(
            "HTTP request failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_release(json: &str) -> Result<ReleaseInfo> {
    let tag = extract_json_string(json, "tag_name").unwrap_or_default();
    let body = extract_json_string(json, "body").unwrap_or_default();
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();

    let mut assets = Vec::new();
    let mut remaining = json;
    while let Some(idx) = remaining.find("\"browser_download_url\"") {
        let after = &remaining[idx..];
        if let Some(url) = extract_json_string(after, "browser_download_url") {
            let name = url.rsplit('/').next().unwrap_or("").to_string();
            let size = extract_json_number(after, "size").unwrap_or(0);
            assets.push(AssetInfo {
                name,
                download_url: url,
                size,
            });
        }
        remaining = &remaining[idx + 24..];
    }

    Ok(ReleaseInfo {
        tag,
        version,
        assets,
        body,
    })
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let idx = json.find(&pattern)?;
    let after_key = &json[idx + pattern.len()..];
    let colon_idx = after_key.find(':')?;
    let after_colon = after_key[colon_idx + 1..].trim_start();

    if !after_colon.starts_with('"') {
        return None;
    }

    let content = &after_colon[1..];
    let mut end = 0;
    let bytes = content.as_bytes();
    while end < bytes.len() {
        if bytes[end] == b'"' && (end == 0 || bytes[end - 1] != b'\\') {
            break;
        }
        end += 1;
    }

    Some(content[..end].replace("\\n", "\n").replace("\\\"", "\""))
}

fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let pattern = format!("\"{}\"", key);
    let idx = json.find(&pattern)?;
    let after_key = &json[idx + pattern.len()..];
    let colon_idx = after_key.find(':')?;
    let after_colon = after_key[colon_idx + 1..].trim_start();

    let end = after_colon
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_colon.len());
    after_colon[..end].parse().ok()
}

pub fn fetch_latest_release() -> Result<ReleaseInfo> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let json = fetch_json(&url)?;

    if json.contains("\"Not Found\"") || json.contains("\"message\"") {
        bail!("No releases found for {}", GITHUB_REPO);
    }

    parse_release(&json)
}

pub fn fetch_release_by_tag(version: &str) -> Result<ReleaseInfo> {
    let tag = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{}", version)
    };

    let url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        GITHUB_REPO, tag
    );
    let json = fetch_json(&url)?;

    if json.contains("\"Not Found\"") {
        bail!("Release {} not found", tag);
    }

    parse_release(&json)
}

pub fn list_available_versions() -> Result<Vec<String>> {
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page=20",
        GITHUB_REPO
    );
    let json = fetch_json(&url)?;

    let mut versions = Vec::new();
    let mut remaining = &json[..];
    while let Some(tag) = extract_json_string(remaining, "tag_name") {
        versions.push(tag.clone());
        if let Some(idx) = remaining.find(&format!("\"tag_name\":\"{}\"", tag)) {
            remaining = &remaining[idx + 10..];
        } else {
            break;
        }
    }

    Ok(versions)
}

fn current_exe_path() -> Result<PathBuf> {
    env::current_exe().context("Cannot determine current executable path")
}

fn download_file(url: &str, dest: &Path) -> Result<()> {
    let dest_str = dest.display().to_string();

    let gh_success = if let Some(api_path) = url.strip_prefix("https://api.github.com/") {
        std::process::Command::new("gh")
            .args([
                "api",
                api_path,
                "--method",
                "GET",
                "-H",
                "Accept: application/octet-stream",
            ])
            .stdout(std::fs::File::create(dest).context("Cannot create temp file")?)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        let token = std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        let mut cmd = std::process::Command::new("curl");
        cmd.args(["-sL", "-o", &dest_str]);
        if let Some(t) = &token {
            cmd.args(["-H", &format!("Authorization: token {}", t)]);
        }
        cmd.arg(url);
        cmd.status().map(|s| s.success()).unwrap_or(false)
    };

    if !gh_success {
        let status = std::process::Command::new("curl")
            .args(["-sL", "-o", &dest_str, url])
            .status()
            .context("Failed to download binary")?;
        if !status.success() {
            bail!("Download failed from {}", url);
        }
    }

    Ok(())
}

fn current_binary_name(exe_path: &Path) -> Result<String> {
    let name = exe_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Cannot determine current executable name")?;
    Ok(name.to_string())
}

fn extract_binary_from_archive(archive: &Path, binary_name: &str, dest: &Path) -> Result<()> {
    let list = std::process::Command::new("tar")
        .arg("-tzf")
        .arg(archive)
        .output()
        .context("Failed to list release archive")?;

    if !list.status.success() {
        bail!(
            "Failed to inspect release archive: {}",
            String::from_utf8_lossy(&list.stderr)
        );
    }

    let suffix = format!("/{}", binary_name);
    let member = String::from_utf8_lossy(&list.stdout)
        .lines()
        .find(|line| line.ends_with(&suffix) || *line == binary_name)
        .map(str::to_string)
        .with_context(|| {
            format!(
                "Release archive did not contain {} in the expected package directory",
                binary_name
            )
        })?;

    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-O")
        .arg(member)
        .stdout(std::fs::File::create(dest).context("Cannot create extracted binary")?)
        .status()
        .context("Failed to extract release archive")?;

    if !status.success() {
        bail!("Failed to extract {} from release archive", binary_name);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dest, fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

pub fn check_for_update() -> Result<Option<ReleaseInfo>> {
    let release = fetch_latest_release()?;
    if version_cmp(&release.version, CURRENT_VERSION) == std::cmp::Ordering::Greater {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

pub fn run_update(target_version: Option<&str>) -> Result<()> {
    println!();
    println!(
        "  🦅 {} {}",
        "falcon".bright_blue().bold(),
        "Self-Update".bold()
    );
    println!(
        "  📌 Current version: {}",
        format!("v{}", CURRENT_VERSION).bright_cyan()
    );
    println!();

    let release = if let Some(ver) = target_version {
        println!("  🔍 Fetching release {}...", ver.bright_cyan());
        fetch_release_by_tag(ver)?
    } else {
        println!("  🔍 Checking for latest release...");
        match fetch_latest_release() {
            Ok(r) => r,
            Err(_) => {
                println!();
                println!("  📭 No releases published yet on GitHub.");
                println!(
                    "     Build from source: {}",
                    "cargo install --git https://github.com/viveky259259/falcon".dimmed()
                );
                println!();
                print_build_from_source_options();
                return Ok(());
            }
        }
    };

    let target_ver = &release.version;
    let current_cmp = version_cmp(target_ver, CURRENT_VERSION);

    match current_cmp {
        std::cmp::Ordering::Equal => {
            println!(
                "  ✅ Already on the latest version: {}",
                format!("v{}", CURRENT_VERSION).green()
            );
            println!();
            return Ok(());
        }
        std::cmp::Ordering::Less => {
            println!(
                "  ⚠️  Target version {} is {} than current {}",
                format!("v{}", target_ver).yellow(),
                "older".yellow(),
                format!("v{}", CURRENT_VERSION).bright_cyan()
            );
            println!("     Proceeding with downgrade...");
            println!();
        }
        std::cmp::Ordering::Greater => {
            println!(
                "  📦 New version available: {} → {}",
                format!("v{}", CURRENT_VERSION).dimmed(),
                format!("v{}", target_ver).green().bold()
            );
            println!();
        }
    }

    let Some(asset_name) = platform_asset_name(target_ver) else {
        println!(
            "  📭 No pre-built binary for {}-{} in release v{}",
            env::consts::OS.yellow(),
            env::consts::ARCH.yellow(),
            target_ver
        );
        println!();
        print_build_from_source_options();
        println!();
        return Ok(());
    };
    let matching_asset = release.assets.iter().find(|a| a.name == asset_name);

    match matching_asset {
        Some(asset) => {
            let exe_path = current_exe_path()?;
            let binary_name = current_binary_name(&exe_path)?;
            let tmp_path = exe_path.with_extension("update-tmp");
            let backup_path = exe_path.with_extension("backup");
            let archive_path = exe_path.with_extension("update.tar.gz");

            println!(
                "  ⬇️  Downloading {} ({})...",
                asset.name.bright_cyan(),
                format_bytes(asset.size).dimmed()
            );

            download_file(&asset.download_url, &archive_path)?;
            extract_binary_from_archive(&archive_path, &binary_name, &tmp_path)?;
            let _ = fs::remove_file(&archive_path);

            if exe_path.exists() {
                fs::rename(&exe_path, &backup_path).context("Failed to backup current binary")?;
            }

            match fs::rename(&tmp_path, &exe_path) {
                Ok(_) => {
                    let _ = fs::remove_file(&backup_path);
                    println!(
                        "  ✅ Updated to {}",
                        format!("v{}", target_ver).green().bold()
                    );
                }
                Err(e) => {
                    if backup_path.exists() {
                        let _ = fs::rename(&backup_path, &exe_path);
                    }
                    let _ = fs::remove_file(&archive_path);
                    bail!("Failed to replace binary: {}. Restored backup.", e);
                }
            }
        }
        None => {
            println!(
                "  📭 No pre-built binary for {} in release v{}",
                asset_name.yellow(),
                target_ver
            );
            println!();
            print_build_from_source_options();
            println!();

            if !release.assets.is_empty() {
                println!("  📋 Available binaries in this release:");
                for asset in &release.assets {
                    println!(
                        "     {} ({})",
                        asset.name.dimmed(),
                        format_bytes(asset.size).dimmed()
                    );
                }
            }
        }
    }

    if !release.body.is_empty() {
        println!();
        println!("  📝 {}", "Release Notes:".bold());
        for line in release.body.lines().take(15) {
            println!("     {}", line);
        }
    }

    println!();
    Ok(())
}

pub fn print_version_info() {
    println!();
    println!(
        "  🦅 {} {}",
        "falcon".bright_blue().bold(),
        "Version Info".bold()
    );
    println!(
        "  📌 Version:  {}",
        format!("v{}", CURRENT_VERSION).bright_cyan()
    );
    println!("  🖥  Platform: {}-{}", env::consts::OS, env::consts::ARCH);
    println!(
        "  📦 Binary:   {}",
        current_exe_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".into())
            .dimmed()
    );
    println!(
        "  🏠 Repo:     {}",
        format!("https://github.com/{}", GITHUB_REPO).dimmed()
    );
    println!();
}

pub fn print_available_versions() -> Result<()> {
    println!();
    println!(
        "  🦅 {} {}",
        "falcon".bright_blue().bold(),
        "Available Versions".bold()
    );
    println!();

    let versions = list_available_versions()?;
    if versions.is_empty() {
        println!("  📭 No releases published yet.");
        println!();
        print_build_from_source_options();
    } else {
        for (i, tag) in versions.iter().enumerate() {
            let ver = tag.strip_prefix('v').unwrap_or(tag);
            let current = ver == CURRENT_VERSION;
            let marker = if current {
                " ◀ current".green().to_string()
            } else {
                String::new()
            };
            let ver_display = if current {
                format!("v{}", ver).green().bold().to_string()
            } else {
                format!("v{}", ver).to_string()
            };
            println!(
                "  {:<4} {}{}",
                format!("#{}", i + 1).dimmed(),
                ver_display,
                marker
            );
        }
    }

    println!();
    Ok(())
}

fn print_build_from_source_options() {
    println!("  🔧 {} Install from source:", "Alternative:".dimmed());
    println!(
        "     {}",
        "cargo install --git https://github.com/viveky259259/falcon".bright_blue()
    );
    println!("     {} Install specific version:", "or".dimmed());
    println!(
        "     {}",
        "cargo install --git https://github.com/viveky259259/falcon --tag v0.2.0".bright_blue()
    );
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_binary_from_archive, platform_archive_label, platform_asset_name, version_cmp,
    };
    use std::fs;
    use std::process::Command;

    #[test]
    fn platform_labels_match_release_archive_names() {
        assert_eq!(
            platform_archive_label("macos", "aarch64"),
            Some("macos-arm64")
        );
        assert_eq!(platform_archive_label("macos", "x86_64"), Some("macos-x64"));
        assert_eq!(platform_archive_label("linux", "x86_64"), Some("linux-x64"));
        assert_eq!(platform_archive_label("linux", "aarch64"), None);
        assert_eq!(platform_archive_label("windows", "x86_64"), None);
    }

    #[test]
    fn platform_asset_name_uses_release_tarball_shape() {
        let asset = platform_asset_name("1.2.3");

        if let Some(asset) = asset {
            assert!(asset.starts_with("falcon-1.2.3-"));
            assert!(asset.ends_with(".tar.gz"));
            assert!(!asset.contains("apple-darwin"));
            assert!(!asset.contains("unknown-linux-gnu"));
        }
    }

    #[test]
    fn extracts_current_binary_from_release_archive() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("falcon-1.2.3-linux-x64");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("falcon"), "binary").unwrap();
        fs::write(package.join("falcon-lsp"), "lsp").unwrap();
        fs::write(package.join("falcon-mcp"), "mcp").unwrap();

        let archive = temp.path().join("falcon-1.2.3-linux-x64.tar.gz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(temp.path())
            .arg("falcon-1.2.3-linux-x64")
            .status()
            .unwrap();
        assert!(status.success());

        let extracted = temp.path().join("falcon-extracted");
        extract_binary_from_archive(&archive, "falcon", &extracted).unwrap();

        assert_eq!(fs::read_to_string(extracted).unwrap(), "binary");
    }

    #[test]
    fn version_compare_handles_v_prefix() {
        assert_eq!(version_cmp("v1.2.3", "1.2.2"), std::cmp::Ordering::Greater);
        assert_eq!(version_cmp("1.2.3", "v1.2.3"), std::cmp::Ordering::Equal);
        assert_eq!(version_cmp("1.2.3", "1.3.0"), std::cmp::Ordering::Less);
    }
}
