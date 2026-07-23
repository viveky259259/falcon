//! Read the user's `ios/Runner/Info.plist` and
//! `android/app/src/main/AndroidManifest.xml`.

use super::android_scan::extract_uses_permissions;
use std::collections::HashSet;
use std::path::Path;

pub fn extract_info_plist_keys(plist_xml: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut remaining = plist_xml;
    while let Some(idx) = remaining.find("<key>") {
        let after = &remaining[idx + 5..];
        let end = match after.find("</key>") {
            Some(e) => e,
            None => break,
        };
        out.insert(after[..end].trim().to_string());
        remaining = &after[end + 6..];
    }
    out
}

pub fn read_project_info_plist(project_root: &Path) -> HashSet<String> {
    let path = project_root.join("ios").join("Runner").join("Info.plist");
    match std::fs::read_to_string(&path) {
        Ok(text) => extract_info_plist_keys(&text),
        Err(_) => HashSet::new(),
    }
}

pub fn read_project_android_manifest(project_root: &Path) -> HashSet<String> {
    let path = project_root
        .join("android")
        .join("app")
        .join("src")
        .join("main")
        .join("AndroidManifest.xml");
    match std::fs::read_to_string(&path) {
        Ok(text) => extract_uses_permissions(&text).into_iter().collect(),
        Err(_) => HashSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_info_plist_keys_basic() {
        let plist = r#"<?xml version="1.0"?>
<plist><dict>
  <key>CFBundleName</key><string>App</string>
  <key>NSLocationWhenInUseUsageDescription</key><string>We need it</string>
</dict></plist>
"#;
        let keys = extract_info_plist_keys(plist);
        assert!(keys.contains("CFBundleName"));
        assert!(keys.contains("NSLocationWhenInUseUsageDescription"));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn extract_info_plist_keys_empty_for_empty_dict() {
        let plist = "<plist><dict></dict></plist>";
        assert!(extract_info_plist_keys(plist).is_empty());
    }

    #[test]
    fn read_project_info_plist_finds_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("ios/Runner")).unwrap();
        std::fs::write(
            tmp.path().join("ios/Runner/Info.plist"),
            "<plist><dict><key>NSCameraUsageDescription</key><string>Y</string></dict></plist>",
        )
        .unwrap();
        let keys = read_project_info_plist(tmp.path());
        assert!(keys.contains("NSCameraUsageDescription"));
    }

    #[test]
    fn read_project_info_plist_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(read_project_info_plist(tmp.path()).is_empty());
    }

    #[test]
    fn read_project_android_manifest_finds_permissions() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("android/app/src/main")).unwrap();
        std::fs::write(
            tmp.path().join("android/app/src/main/AndroidManifest.xml"),
            r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#,
        )
        .unwrap();
        let perms = read_project_android_manifest(tmp.path());
        assert!(perms.contains("android.permission.CAMERA"));
    }

    #[test]
    fn read_project_android_manifest_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(read_project_android_manifest(tmp.path()).is_empty());
    }
}
