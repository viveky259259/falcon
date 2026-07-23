//! Read and write the `.falcon/plugin-requirements.yaml` cache file.
//!
//! Cache key: SHA-256 of the contents of `pubspec.lock`. When the lock changes,
//! the cache is invalidated and rebuilt.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneratedRequirements {
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub pubspec_lock_sha256: String,
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginRequirements>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginRequirements {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosRequirements>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidRequirements>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IosRequirements {
    #[serde(default)]
    pub info_plist_keys: Vec<IosKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IosKey {
    pub key: String,
    pub api: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AndroidRequirements {
    #[serde(default)]
    pub permissions: Vec<String>,
}

pub fn pubspec_lock_sha256(project_root: &Path) -> String {
    let path = project_root.join("pubspec.lock");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let digest = Sha256::digest(&bytes);
            format!("{digest:x}")
        }
        Err(_) => String::new(),
    }
}

pub fn cache_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".falcon")
        .join("plugin-requirements.yaml")
}

pub fn read_cache(project_root: &Path) -> Option<GeneratedRequirements> {
    let path = cache_path(project_root);
    let text = std::fs::read_to_string(&path).ok()?;
    let parsed: GeneratedRequirements = serde_yaml::from_str(&text).ok()?;
    if parsed.schema_version != SCHEMA_VERSION {
        return None;
    }
    Some(parsed)
}

pub fn write_cache(project_root: &Path, doc: &GeneratedRequirements) -> Result<()> {
    let path = cache_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Creating {}", parent.display()))?;
    }
    let yaml = serde_yaml::to_string(doc)?;
    std::fs::write(&path, yaml).with_context(|| format!("Writing {}", path.display()))?;
    Ok(())
}

pub(crate) fn new_empty_for(project_root: &Path) -> GeneratedRequirements {
    GeneratedRequirements {
        schema_version: SCHEMA_VERSION,
        generated_at: now_iso8601_string(),
        pubspec_lock_sha256: pubspec_lock_sha256(project_root),
        plugins: BTreeMap::new(),
    }
}

/// Format the current UTC time as ISO 8601 (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn now_iso8601_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let sec_of_day = (secs % 86_400) as u32;
    let days_since_epoch = (secs / 86_400) as i64;
    let (year, month, day) = days_to_ymd(days_since_epoch);
    let hour = sec_of_day / 3600;
    let minute = (sec_of_day / 60) % 60;
    let second = sec_of_day % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Convert days-since-1970-01-01 to (year, month, day). Based on the
/// Howard Hinnant civil_from_days algorithm. Valid for all positive day
/// counts representable in i64.
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn pubspec_lock_sha256_empty_when_missing() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(pubspec_lock_sha256(tmp.path()), "");
    }

    #[test]
    fn pubspec_lock_sha256_stable_for_same_contents() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        std::fs::write(tmp1.path().join("pubspec.lock"), "abc\n").unwrap();
        std::fs::write(tmp2.path().join("pubspec.lock"), "abc\n").unwrap();
        let a = pubspec_lock_sha256(tmp1.path());
        let b = pubspec_lock_sha256(tmp2.path());
        assert_eq!(a, b);
        assert_eq!(a.len(), 64, "sha256 hex should be 64 chars");
    }

    #[test]
    fn read_cache_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(read_cache(tmp.path()).is_none());
    }

    #[test]
    fn write_then_read_round_trip() {
        let tmp = TempDir::new().unwrap();
        let mut doc = new_empty_for(tmp.path());
        doc.pubspec_lock_sha256 = "deadbeef".into();
        doc.plugins.insert(
            "location".into(),
            PluginRequirements {
                version: "8.0.0".into(),
                source: "hosted".into(),
                ios: Some(IosRequirements {
                    info_plist_keys: vec![IosKey {
                        key: "NSLocationWhenInUseUsageDescription".into(),
                        api: "requestWhenInUseAuthorization".into(),
                        source: "ios/Classes/Loc.m".into(),
                    }],
                }),
                android: None,
                skipped: None,
            },
        );
        write_cache(tmp.path(), &doc).unwrap();
        let round = read_cache(tmp.path()).unwrap();
        assert_eq!(round.pubspec_lock_sha256, "deadbeef");
        assert_eq!(round.plugins["location"].version, "8.0.0");
        assert_eq!(
            round.plugins["location"]
                .ios
                .as_ref()
                .unwrap()
                .info_plist_keys[0]
                .key,
            "NSLocationWhenInUseUsageDescription"
        );
    }

    #[test]
    fn read_cache_rejects_wrong_schema_version() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".falcon")).unwrap();
        std::fs::write(
            cache_path(tmp.path()),
            "schema_version: 999\ngenerated_at: '2026'\n",
        )
        .unwrap();
        assert!(read_cache(tmp.path()).is_none());
    }

    #[test]
    fn now_iso8601_string_is_valid_format() {
        let s = now_iso8601_string();
        // Must be exactly 20 chars: YYYY-MM-DDTHH:MM:SSZ
        assert_eq!(s.len(), 20, "expected 20 chars, got: {s}");
        // Year must be >= 2026
        let year: i32 = s[..4].parse().expect("year should be numeric");
        assert!(year >= 2026, "year should be >= 2026, got {year}");
        // Separator chars at known positions
        assert_eq!(&s[4..5], "-", "char 4 should be '-'");
        assert_eq!(&s[7..8], "-", "char 7 should be '-'");
        assert_eq!(&s[10..11], "T", "char 10 should be 'T'");
        assert_eq!(&s[13..14], ":", "char 13 should be ':'");
        assert_eq!(&s[16..17], ":", "char 16 should be ':'");
        // Must end with Z
        assert_eq!(&s[19..20], "Z", "last char should be 'Z'");
    }
}
