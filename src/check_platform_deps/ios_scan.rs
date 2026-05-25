//! Scan a plugin's iOS source files for Apple-SDK API substrings and report
//! the required Info.plist keys.

use super::apple_api_map::find_apis_in_source;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredInfoPlistKey {
    pub api: &'static str,
    pub key: &'static str,
    pub framework: &'static str,
    pub source_file: PathBuf,
}

pub fn scan_ios_sources(plugin_root: &Path) -> Vec<RequiredInfoPlistKey> {
    let mut out = Vec::new();
    for sub in ["ios", "darwin"] {
        let dir = plugin_root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "m" && ext != "swift" && ext != "mm" && ext != "h" {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            for api in find_apis_in_source(&text) {
                if api.info_plist_key.is_empty() {
                    continue;
                }
                out.push(RequiredInfoPlistKey {
                    api: api.api,
                    key: api.info_plist_key,
                    framework: api.framework,
                    source_file: path.to_path_buf(),
                });
            }
        }
    }
    dedupe(out)
}

fn dedupe(mut v: Vec<RequiredInfoPlistKey>) -> Vec<RequiredInfoPlistKey> {
    v.sort_by(|a, b| a.key.cmp(b.key).then(a.api.cmp(b.api)));
    v.dedup_by(|a, b| a.api == b.api && a.key == b.key);
    v
}

pub(crate) fn unique_keys(reqs: &[RequiredInfoPlistKey]) -> Vec<&'static str> {
    let mut set: Vec<&'static str> = reqs.iter().map(|r| r.key).collect();
    set.sort();
    set.dedup();
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn scan_ios_sources_no_ios_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(scan_ios_sources(tmp.path()).is_empty());
    }

    #[test]
    fn scan_ios_sources_finds_location_requirement() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/LocationPlugin.m"),
            "[CLLocationManager.shared requestWhenInUseAuthorization];",
        );
        let result = scan_ios_sources(tmp.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "NSLocationWhenInUseUsageDescription");
        assert!(result[0].source_file.to_string_lossy().ends_with("LocationPlugin.m"));
    }

    #[test]
    fn scan_ios_sources_walks_darwin_folder() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("darwin/Classes/CameraPlugin.swift"),
            "AVCaptureDevice.requestAccess(for: .video) { _ in }",
        );
        let result = scan_ios_sources(tmp.path());
        assert!(result.iter().any(|r| r.key == "NSCameraUsageDescription"));
    }

    #[test]
    fn scan_ios_sources_deduplicates_same_api_in_multiple_files() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/A.swift"),
            "AVCaptureDevice.requestAccess(for: .video) { _ in }",
        );
        write(
            &tmp.path().join("ios/Classes/B.swift"),
            "AVCaptureDevice.requestAccess(for: .audio) { _ in }",
        );
        let result = scan_ios_sources(tmp.path());
        let cam = result
            .iter()
            .filter(|r| r.key == "NSCameraUsageDescription")
            .count();
        assert_eq!(cam, 1);
    }

    #[test]
    fn scan_ios_sources_collects_multiple_distinct_keys() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/Multi.swift"),
            r#"
[CLLocationManager.shared requestWhenInUseAuthorization];
AVCaptureDevice.requestAccess(for: .video) { _ in }
PHPhotoLibrary.requestAuthorization { _ in }
"#,
        );
        let keys = unique_keys(&scan_ios_sources(tmp.path()));
        assert!(keys.contains(&"NSLocationWhenInUseUsageDescription"));
        assert!(keys.contains(&"NSCameraUsageDescription"));
        assert!(keys.contains(&"NSPhotoLibraryUsageDescription"));
    }

    #[test]
    fn scan_ios_sources_ignores_non_source_extensions() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("ios/README.md"), "AVCaptureDevice.requestAccess");
        assert!(scan_ios_sources(tmp.path()).is_empty());
    }

    #[test]
    fn scan_ios_sources_picks_up_h_and_mm_files_too() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("ios/Classes/Cam.h"), "AVCaptureDevice.requestAccess");
        write(
            &tmp.path().join("ios/Classes/Cam.mm"),
            "[CLLocationManager.shared requestWhenInUseAuthorization];",
        );
        let keys = unique_keys(&scan_ios_sources(tmp.path()));
        assert!(keys.contains(&"NSCameraUsageDescription"));
        assert!(keys.contains(&"NSLocationWhenInUseUsageDescription"));
    }

    #[test]
    fn scan_ios_sources_capability_only_api_filtered_out() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp.path().join("ios/Classes/Sms.swift"),
            "MFMessageComposeViewController.canSendText()",
        );
        assert!(scan_ios_sources(tmp.path()).is_empty());
    }
}
