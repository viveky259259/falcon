//! Host platform detection, written as pure functions so tests never read
//! the real environment.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
    Windows,
}

impl Os {
    /// The name Google uses in `releases_<name>.json`.
    pub fn manifest_name(&self) -> &'static str {
        match self {
            Os::MacOs => "macos",
            Os::Linux => "linux",
            Os::Windows => "windows",
        }
    }
}

/// Matches the `dart_sdk_arch` field in the release manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    Arm64,
    X64,
}

impl Arch {
    pub fn manifest_name(&self) -> &'static str {
        match self {
            Arch::Arm64 => "arm64",
            Arch::X64 => "x64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub home: Option<PathBuf>,
    pub shell_rc: Option<PathBuf>,
    pub package_managers: Vec<String>,
}

pub fn current_os() -> Os {
    if cfg!(target_os = "macos") {
        Os::MacOs
    } else if cfg!(target_os = "windows") {
        Os::Windows
    } else {
        Os::Linux
    }
}

pub fn current_arch() -> Arch {
    if cfg!(target_arch = "aarch64") {
        Arch::Arm64
    } else {
        Arch::X64
    }
}

/// Resolve the home directory, mirroring `src/main.rs:3103`.
pub fn home_from(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    get("HOME")
        .or_else(|| get("USERPROFILE"))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// The rc file Falcon will *name* (never edit) for a given `$SHELL`.
pub fn shell_rc_for(shell: &str, home: &Path) -> Option<PathBuf> {
    let name = Path::new(shell).file_name()?.to_str()?;
    match name {
        "zsh" => Some(home.join(".zshrc")),
        "bash" => Some(home.join(".bashrc")),
        "fish" => Some(home.join(".config/fish/config.fish")),
        _ => None,
    }
}

/// Which package managers are on PATH — used to pick between `brew` and `gem`.
pub fn package_managers(is_on_path: impl Fn(&str) -> bool) -> Vec<String> {
    [
        "brew", "apt-get", "dnf", "pacman", "choco", "winget", "gem", "xcodes",
    ]
    .iter()
    .filter(|p| is_on_path(p))
    .map(|p| p.to_string())
    .collect()
}

/// Is `program` resolvable on PATH? Uses `which`/`where`, no new dependency.
pub fn on_path(program: &str) -> bool {
    let finder = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    std::process::Command::new(finder)
        .arg(program)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn detect() -> HostInfo {
    let home = home_from(|k| std::env::var(k).ok());
    let shell_rc = home.as_ref().and_then(|h| {
        std::env::var("SHELL")
            .ok()
            .and_then(|s| shell_rc_for(&s, h))
    });
    HostInfo {
        os: current_os().manifest_name().to_string(),
        arch: current_arch().manifest_name().to_string(),
        home,
        shell_rc,
        package_managers: package_managers(on_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn home_prefers_home_over_userprofile() {
        let got = home_from(|k| match k {
            "HOME" => Some("/Users/ada".into()),
            "USERPROFILE" => Some("C:\\Users\\ada".into()),
            _ => None,
        });
        assert_eq!(got, Some(PathBuf::from("/Users/ada")));
    }

    #[test]
    fn home_falls_back_to_userprofile() {
        let got = home_from(|k| (k == "USERPROFILE").then(|| "C:\\Users\\ada".to_string()));
        assert_eq!(got, Some(PathBuf::from("C:\\Users\\ada")));
    }

    #[test]
    fn home_is_none_when_neither_is_set() {
        assert_eq!(home_from(|_| None), None);
    }

    #[test]
    fn shell_rc_maps_known_shells() {
        let home = Path::new("/Users/ada");
        assert_eq!(shell_rc_for("/bin/zsh", home), Some(home.join(".zshrc")));
        assert_eq!(shell_rc_for("/bin/bash", home), Some(home.join(".bashrc")));
        assert_eq!(
            shell_rc_for("/opt/homebrew/bin/fish", home),
            Some(home.join(".config/fish/config.fish"))
        );
    }

    #[test]
    fn shell_rc_is_none_for_unknown_shell() {
        assert_eq!(shell_rc_for("/bin/nonsuch", Path::new("/Users/ada")), None);
    }

    #[test]
    fn manifest_name_matches_google_naming() {
        assert_eq!(Os::MacOs.manifest_name(), "macos");
        assert_eq!(Os::Linux.manifest_name(), "linux");
        assert_eq!(Os::Windows.manifest_name(), "windows");
    }

    #[test]
    fn detect_populates_os_and_arch() {
        let info = detect();
        assert!(!info.os.is_empty());
        assert!(info.arch == "arm64" || info.arch == "x64");
    }
}
