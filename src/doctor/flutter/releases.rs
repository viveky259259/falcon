//! Client for Google's Flutter release manifest — the same source
//! flutter.dev's install page is generated from.

use crate::doctor::host::{Arch, Os};
use serde::Deserialize;
use std::collections::BTreeMap;

const RELEASES_BASE: &str = "https://storage.googleapis.com/flutter_infra_release/releases";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub hash: String,
    pub channel: String,
    pub version: String,
    #[serde(default)]
    pub dart_sdk_version: Option<String>,
    /// Absent on older entries, which are x64.
    #[serde(default)]
    pub dart_sdk_arch: Option<String>,
    pub archive: String,
    pub sha256: String,
}

impl Release {
    pub fn arch(&self) -> Arch {
        match self.dart_sdk_arch.as_deref() {
            Some("arm64") => Arch::Arm64,
            _ => Arch::X64,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseManifest {
    pub base_url: String,
    #[serde(default)]
    pub current_release: BTreeMap<String, String>,
    pub releases: Vec<Release>,
}

impl ReleaseManifest {
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("malformed Flutter release manifest: {}", e))
    }

    pub fn archive_url(&self, release: &Release) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            release.archive
        )
    }

    /// Every release on `channel` for `arch`, newest version first.
    pub fn for_channel(&self, channel: &str, arch: Arch) -> Vec<&Release> {
        let mut out: Vec<&Release> = self
            .releases
            .iter()
            .filter(|r| r.channel == channel && r.arch() == arch)
            .collect();
        out.sort_by(|a, b| crate::version_util::version_cmp(&b.version, &a.version));
        out
    }

    /// The channel head, resolved through `current_release` and narrowed to `arch`.
    pub fn latest(&self, channel: &str, arch: Arch) -> Option<&Release> {
        let hash = self.current_release.get(channel)?;
        self.releases
            .iter()
            .find(|r| &r.hash == hash && r.arch() == arch)
            .or_else(|| self.for_channel(channel, arch).into_iter().next())
    }

    pub fn find(&self, channel: &str, version: &str, arch: Arch) -> Option<&Release> {
        self.releases
            .iter()
            .find(|r| r.channel == channel && r.version == version && r.arch() == arch)
    }
}

pub fn manifest_url(os: Os) -> String {
    format!("{}/releases_{}.json", RELEASES_BASE, os.manifest_name())
}

/// Only stable and beta ship prebuilt archives; master must be cloned.
pub fn channel_has_archives(channel: &str) -> bool {
    matches!(channel, "stable" | "beta")
}

pub const CHANNELS: [&str; 3] = ["stable", "beta", "master"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::host::{Arch, Os};

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn manifest() -> ReleaseManifest {
        ReleaseManifest::parse(FIXTURE).expect("fixture must parse")
    }

    #[test]
    fn manifest_url_uses_googles_os_naming() {
        assert_eq!(
            manifest_url(Os::MacOs),
            "https://storage.googleapis.com/flutter_infra_release/releases/releases_macos.json"
        );
        assert!(manifest_url(Os::Linux).ends_with("releases_linux.json"));
    }

    #[test]
    fn for_channel_filters_by_architecture() {
        let m = manifest();
        let arm = m.for_channel("stable", Arch::Arm64);
        assert_eq!(arm.len(), 1, "only one stable arm64 entry in the fixture");
        assert!(arm[0].archive.contains("arm64"));
    }

    #[test]
    fn missing_dart_sdk_arch_is_treated_as_x64() {
        let m = manifest();
        let x64: Vec<_> = m
            .for_channel("stable", Arch::X64)
            .iter()
            .map(|r| r.version.clone())
            .collect();
        assert!(
            x64.contains(&"3.19.0".to_string()),
            "legacy entry must count as x64: {:?}",
            x64
        );
    }

    #[test]
    fn for_channel_returns_newest_first() {
        let m = manifest();
        let versions: Vec<_> = m
            .for_channel("stable", Arch::X64)
            .iter()
            .map(|r| r.version.clone())
            .collect();
        assert_eq!(versions, vec!["3.24.5".to_string(), "3.19.0".to_string()]);
    }

    #[test]
    fn latest_uses_current_release_hash() {
        let m = manifest();
        assert_eq!(m.latest("stable", Arch::Arm64).unwrap().version, "3.24.5");
        assert_eq!(m.latest("beta", Arch::Arm64).unwrap().version, "3.27.0");
    }

    #[test]
    fn find_matches_channel_version_and_arch() {
        let m = manifest();
        let r = m.find("stable", "3.24.5", Arch::X64).unwrap();
        assert_eq!(r.sha256, "2".repeat(64));
        assert!(m.find("stable", "9.9.9", Arch::X64).is_none());
    }

    #[test]
    fn archive_url_joins_base_and_path() {
        let m = manifest();
        let r = m.find("stable", "3.24.5", Arch::Arm64).unwrap();
        assert_eq!(
            m.archive_url(r),
            "https://storage.googleapis.com/flutter_infra_release/releases/stable/macos/flutter_macos_arm64_3.24.5-stable.zip"
        );
    }

    #[test]
    fn master_has_no_published_archives() {
        assert!(channel_has_archives("stable"));
        assert!(channel_has_archives("beta"));
        assert!(!channel_has_archives("master"));
    }

    #[test]
    fn unknown_fields_do_not_break_parsing() {
        let json = r#"{"base_url":"https://x","current_release":{},"releases":[],"surprise":1}"#;
        assert!(ReleaseManifest::parse(json).is_ok());
    }

    #[test]
    fn malformed_json_returns_an_error_not_a_panic() {
        assert!(ReleaseManifest::parse("not json").is_err());
    }
}
