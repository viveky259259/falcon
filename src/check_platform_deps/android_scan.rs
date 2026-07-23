//! Read the plugin's own `android/src/main/AndroidManifest.xml` template and
//! extract the `<uses-permission>` block. The plugin's manifest IS the source
//! of truth — no dictionary needed for Android.

use std::path::Path;

fn looks_like_uses_permission_tag_start(rest: &str) -> bool {
    const NEEDLE: &[u8] = b"<uses-permission";
    if !rest.starts_with("<uses-permission") {
        return false;
    }
    matches!(
        rest.as_bytes().get(NEEDLE.len()),
        Some(b' ' | b'\t' | b'\n' | b'/' | b'>')
    )
}

pub fn extract_uses_permissions(manifest_xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut remaining = manifest_xml;
    while let Some(idx) = remaining.find("<uses-permission") {
        let after = &remaining[idx..];
        // Skip <uses-permission-sdk-23 and other extended tags.
        if !looks_like_uses_permission_tag_start(after) {
            // Advance past this non-matching occurrence to avoid infinite loop.
            remaining = &after[1..];
            continue;
        }
        let end = match after.find('>') {
            Some(e) => e,
            None => break,
        };
        let tag = &after[..=end];
        if let Some(name) = extract_attr(tag, "android:name") {
            out.push(name);
        }
        remaining = &after[end + 1..];
    }
    out.sort();
    out.dedup();
    out
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=");
    let idx = tag.find(&needle)?;
    let after = &tag[idx + needle.len()..];
    let first = after.chars().next()?;
    if first != '"' && first != '\'' {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find(first)?;
    Some(rest[..end].to_string())
}

pub fn scan_plugin_manifest(plugin_root: &Path) -> Vec<String> {
    let manifest_path = plugin_root
        .join("android")
        .join("src")
        .join("main")
        .join("AndroidManifest.xml");
    let Ok(text) = std::fs::read_to_string(&manifest_path) else {
        return Vec::new();
    };
    extract_uses_permissions(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_uses_permissions_basic() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<manifest>
    <uses-permission android:name="android.permission.ACCESS_FINE_LOCATION"/>
    <uses-permission android:name="android.permission.ACCESS_COARSE_LOCATION" />
</manifest>"#;
        let perms = extract_uses_permissions(xml);
        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&"android.permission.ACCESS_FINE_LOCATION".to_string()));
        assert!(perms.contains(&"android.permission.ACCESS_COARSE_LOCATION".to_string()));
    }

    #[test]
    fn extract_uses_permissions_handles_single_quotes() {
        let xml = "<uses-permission android:name='android.permission.CAMERA'/>";
        let perms = extract_uses_permissions(xml);
        assert_eq!(perms, vec!["android.permission.CAMERA"]);
    }

    #[test]
    fn extract_uses_permissions_empty_when_no_tags() {
        let xml = "<manifest><application/></manifest>";
        assert!(extract_uses_permissions(xml).is_empty());
    }

    #[test]
    fn extract_uses_permissions_dedupes() {
        let xml = r#"
<uses-permission android:name="android.permission.CAMERA"/>
<uses-permission android:name="android.permission.CAMERA"/>
"#;
        let perms = extract_uses_permissions(xml);
        assert_eq!(perms.len(), 1);
    }

    #[test]
    fn scan_plugin_manifest_returns_permissions() {
        let tmp = TempDir::new().unwrap();
        let manifest = r#"
<manifest>
  <uses-permission android:name="android.permission.RECORD_AUDIO"/>
</manifest>
"#;
        std::fs::create_dir_all(tmp.path().join("android/src/main")).unwrap();
        std::fs::write(
            tmp.path().join("android/src/main/AndroidManifest.xml"),
            manifest,
        )
        .unwrap();
        let perms = scan_plugin_manifest(tmp.path());
        assert_eq!(perms, vec!["android.permission.RECORD_AUDIO"]);
    }

    #[test]
    fn scan_plugin_manifest_no_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(scan_plugin_manifest(tmp.path()).is_empty());
    }

    #[test]
    fn extract_uses_permissions_does_not_match_sdk_23_variant() {
        let xml = r#"<manifest>
  <uses-permission-sdk-23 android:name="android.permission.NOT_THIS_ONE"/>
  <uses-permission android:name="android.permission.CAMERA"/>
</manifest>"#;
        let perms = extract_uses_permissions(xml);
        assert_eq!(perms, vec!["android.permission.CAMERA".to_string()]);
    }
}
