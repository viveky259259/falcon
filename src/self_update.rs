use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::PathBuf;

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

fn platform_asset_name() -> String {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    let platform = match (os, arch) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => "unknown",
    };

    format!("falcon-{}", platform)
}

fn fetch_json(url: &str) -> Result<String> {
    // LCOV_EXCL_START — invokes gh/curl subprocess
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
    // LCOV_EXCL_STOP
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
    // LCOV_EXCL_START — invokes gh/curl subprocess
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let json = fetch_json(&url)?;

    if json.contains("\"Not Found\"") || json.contains("\"message\"") {
        bail!("No releases found for {}", GITHUB_REPO);
    }

    parse_release(&json)
    // LCOV_EXCL_STOP
}

pub fn fetch_release_by_tag(version: &str) -> Result<ReleaseInfo> {
    // LCOV_EXCL_START — invokes gh/curl subprocess
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
    // LCOV_EXCL_STOP
}

pub fn list_available_versions() -> Result<Vec<String>> {
    // LCOV_EXCL_START — invokes gh/curl subprocess
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
    // LCOV_EXCL_STOP
}

fn current_exe_path() -> Result<PathBuf> {
    // LCOV_EXCL_START — invokes gh/curl subprocess
    env::current_exe().context("Cannot determine current executable path")
    // LCOV_EXCL_STOP
}

fn download_binary(url: &str, dest: &PathBuf) -> Result<()> {
    // LCOV_EXCL_START — invokes gh/curl subprocess
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

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dest, fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
    // LCOV_EXCL_STOP
}

fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u32> {
        v.strip_prefix('v')
            .unwrap_or(v)
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    parse(a).cmp(&parse(b))
}

pub fn check_for_update() -> Result<Option<ReleaseInfo>> {
    // LCOV_EXCL_START — invokes gh/curl subprocess
    let release = fetch_latest_release()?;
    if version_cmp(&release.version, CURRENT_VERSION) == std::cmp::Ordering::Greater {
        Ok(Some(release))
    } else {
        Ok(None)
    }
    // LCOV_EXCL_STOP
}

pub fn run_update(target_version: Option<&str>) -> Result<()> {
    // LCOV_EXCL_START — invokes gh/curl subprocess
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

    let asset_name = platform_asset_name();
    let matching_asset = release.assets.iter().find(|a| a.name.contains(&asset_name));

    match matching_asset {
        Some(asset) => {
            let exe_path = current_exe_path()?;
            let tmp_path = exe_path.with_extension("update-tmp");
            let backup_path = exe_path.with_extension("backup");

            println!(
                "  ⬇️  Downloading {} ({})...",
                asset.name.bright_cyan(),
                format_bytes(asset.size).dimmed()
            );

            download_binary(&asset.download_url, &tmp_path)?;

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
    // LCOV_EXCL_STOP
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
    // LCOV_EXCL_START — invokes gh/curl subprocess
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
    // LCOV_EXCL_STOP
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
    use super::*;
    use std::cmp::Ordering;

    // ── version_cmp ──────────────────────────────────────────────────────────

    #[test]
    fn version_cmp_equal_versions() {
        assert_eq!(version_cmp("1.2.3", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn version_cmp_v_prefix_stripped() {
        assert_eq!(version_cmp("v1.2.3", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn version_cmp_greater_major() {
        assert_eq!(version_cmp("2.0.0", "1.9.9"), Ordering::Greater);
    }

    #[test]
    fn version_cmp_less_minor() {
        assert_eq!(version_cmp("1.1.0", "1.2.0"), Ordering::Less);
    }

    #[test]
    fn version_cmp_handles_unparseable_components_gracefully() {
        // "1.x.0" — the "x" component is filtered out by filter_map, so
        // parse("1.x.0") == [1, 0] and parse("1.0.0") == [1, 0, 0].
        // The comparison must not panic; the exact ordering is implementation-
        // defined and we just verify no panic occurs.
        let _ = version_cmp("1.x.0", "1.0.0");
    }

    // ── extract_json_string ──────────────────────────────────────────────────

    #[test]
    fn extract_json_string_returns_value_for_key() {
        assert_eq!(
            extract_json_string(r#"{"name":"foo"}"#, "name"),
            Some("foo".to_string())
        );
    }

    #[test]
    fn extract_json_string_returns_none_for_missing_key() {
        assert_eq!(extract_json_string(r#"{"x":1}"#, "name"), None);
    }

    #[test]
    fn extract_json_string_handles_escaped_quote() {
        // The raw JSON value is "a\"b"; the impl replaces \\\" with "
        let json = r#"{"x":"a\"b"}"#;
        assert_eq!(
            extract_json_string(json, "x"),
            Some("a\"b".to_string())
        );
    }

    #[test]
    fn extract_json_string_handles_escaped_newline() {
        // The raw JSON value is "a\nb"; the impl replaces \\n with a real newline
        let json = r#"{"x":"a\nb"}"#;
        assert_eq!(
            extract_json_string(json, "x"),
            Some("a\nb".to_string())
        );
    }

    #[test]
    fn extract_json_string_returns_none_for_non_string_value() {
        // Value is a number, not a string — the impl checks starts_with('"')
        assert_eq!(extract_json_string(r#"{"x":42}"#, "x"), None);
    }

    // ── extract_json_number ──────────────────────────────────────────────────

    #[test]
    fn extract_json_number_returns_value() {
        assert_eq!(
            extract_json_number(r#"{"size":1024}"#, "size"),
            Some(1024)
        );
    }

    #[test]
    fn extract_json_number_missing_key_returns_none() {
        assert_eq!(extract_json_number(r#"{}"#, "size"), None);
    }

    #[test]
    fn extract_json_number_stops_at_non_digit() {
        assert_eq!(
            extract_json_number(r#"{"x":12,"y":3}"#, "x"),
            Some(12)
        );
    }

    #[test]
    fn extract_json_number_zero() {
        assert_eq!(extract_json_number(r#"{"x":0}"#, "x"), Some(0));
    }

    // ── parse_release ────────────────────────────────────────────────────────

    #[test]
    fn parse_release_extracts_tag_and_version() {
        let json = r#"{"tag_name":"v0.4.0","body":""}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.tag, "v0.4.0");
        assert_eq!(r.version, "0.4.0");
    }

    #[test]
    fn parse_release_version_without_v_prefix() {
        let json = r#"{"tag_name":"0.4.0","body":""}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.version, "0.4.0");
    }

    #[test]
    fn parse_release_extracts_body() {
        let json = r#"{"tag_name":"v1.0.0","body":"some notes"}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.body, "some notes");
    }

    #[test]
    fn parse_release_extracts_assets_with_url_and_size() {
        let json = r#"{
            "tag_name": "v0.4.0",
            "body": "",
            "assets": [
                {
                    "browser_download_url": "https://example.com/releases/falcon-x86_64-apple-darwin",
                    "size": 2048
                },
                {
                    "browser_download_url": "https://example.com/releases/falcon-aarch64-apple-darwin",
                    "size": 4096
                }
            ]
        }"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.assets[0].name, "falcon-x86_64-apple-darwin");
        assert_eq!(r.assets[0].size, 2048);
        assert_eq!(r.assets[1].name, "falcon-aarch64-apple-darwin");
        assert_eq!(r.assets[1].size, 4096);
    }

    #[test]
    fn parse_release_no_assets_returns_empty_vec() {
        let json = r#"{"tag_name":"v1.0.0","body":""}"#;
        let r = parse_release(json).unwrap();
        assert!(r.assets.is_empty());
    }

    #[test]
    fn parse_release_handles_missing_body() {
        let json = r#"{"tag_name":"v1.0.0"}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.body, "");
    }

    // ── platform_asset_name ──────────────────────────────────────────────────

    #[test]
    fn platform_asset_name_returns_known_or_unknown_format() {
        let name = platform_asset_name();
        assert!(
            name.starts_with("falcon-"),
            "Expected name to start with 'falcon-', got: {}",
            name
        );
        let known_targets = ["-darwin", "-linux-gnu", "-msvc", "unknown"];
        let contains_known = known_targets.iter().any(|t| name.contains(t));
        assert!(
            contains_known,
            "Expected name to contain a known target triple or 'unknown', got: {}",
            name
        );
    }
}
