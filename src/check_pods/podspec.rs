//! Scan a plugin's podspecs for `s.ios.deployment_target` (and macOS sibling).

use std::path::{Path, PathBuf};

/// One podspec's relevant deployment-target info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodspecInfo {
    /// Path to the .podspec file.
    pub podspec_path: PathBuf,
    /// Plugin (parent directory) name.
    pub plugin: String,
    /// Declared iOS deployment target (e.g. "13.0"). None when absent → caller
    /// applies the CocoaPods default (9.0 for iOS, 10.10 for macOS).
    pub ios_target: Option<String>,
    /// Declared macOS deployment target (e.g. "10.15"). None when absent.
    pub osx_target: Option<String>,
}

/// Walk `<plugin_root>/ios/*.podspec` and `<plugin_root>/darwin/*.podspec`
/// and return one PodspecInfo per podspec found.
pub fn scan_plugin_podspecs(plugin_name: &str, plugin_root: &Path) -> Vec<PodspecInfo> {
    let mut out = Vec::new();
    for sub in ["ios", "darwin"] {
        let dir = plugin_root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("podspec") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            out.push(PodspecInfo {
                podspec_path: path.to_path_buf(),
                plugin: plugin_name.to_string(),
                ios_target: extract_deployment_target(&text, "ios"),
                osx_target: extract_deployment_target(&text, "osx"),
            });
        }
    }
    out
}

/// Extract `s.<platform>.deployment_target = '<X.Y>'` from a podspec body.
pub fn extract_deployment_target(podspec: &str, platform: &str) -> Option<String> {
    let needle = format!(".{platform}.deployment_target");
    for line in podspec.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if !trimmed.contains(&needle) {
            continue;
        }
        if let Some(version) = first_quoted(trimmed) {
            return Some(version);
        }
    }
    None
}

fn first_quoted(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\'' || c == b'"' {
            let quote = c;
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j > start {
                return std::str::from_utf8(&bytes[start..j]).ok().map(|s| s.to_string());
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_deployment_target_basic() {
        let s = "Pod::Spec.new do |s|\n  s.ios.deployment_target = '13.0'\nend\n";
        assert_eq!(extract_deployment_target(s, "ios"), Some("13.0".to_string()));
    }

    #[test]
    fn extract_deployment_target_double_quoted() {
        let s = "s.ios.deployment_target = \"14.5\"\n";
        assert_eq!(extract_deployment_target(s, "ios"), Some("14.5".to_string()));
    }

    #[test]
    fn extract_deployment_target_absent() {
        let s = "Pod::Spec.new do |s|\nend\n";
        assert_eq!(extract_deployment_target(s, "ios"), None);
    }

    #[test]
    fn extract_deployment_target_commented_out() {
        let s = "# s.ios.deployment_target = '13.0'\n";
        assert_eq!(extract_deployment_target(s, "ios"), None);
    }

    #[test]
    fn extract_deployment_target_osx_separate() {
        let s = "s.osx.deployment_target = '10.15'\n";
        assert_eq!(extract_deployment_target(s, "osx"), Some("10.15".to_string()));
        assert_eq!(extract_deployment_target(s, "ios"), None);
    }

    #[test]
    fn scan_plugin_podspecs_finds_ios_podspec() {
        let tmp = TempDir::new().unwrap();
        let plugin_root = tmp.path();
        std::fs::create_dir_all(plugin_root.join("ios")).unwrap();
        std::fs::write(
            plugin_root.join("ios/location.podspec"),
            "s.ios.deployment_target = '13.0'\n",
        )
        .unwrap();
        let result = scan_plugin_podspecs("location", plugin_root);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ios_target, Some("13.0".to_string()));
        assert_eq!(result[0].plugin, "location");
    }

    #[test]
    fn scan_plugin_podspecs_finds_multiple_podspecs() {
        let tmp = TempDir::new().unwrap();
        let plugin_root = tmp.path();
        std::fs::create_dir_all(plugin_root.join("ios")).unwrap();
        std::fs::write(plugin_root.join("ios/image_picker.podspec"), "s.ios.deployment_target = '12.0'\n").unwrap();
        std::fs::write(plugin_root.join("ios/image_picker_ios.podspec"), "s.ios.deployment_target = '14.0'\n").unwrap();
        let result = scan_plugin_podspecs("image_picker", plugin_root);
        assert_eq!(result.len(), 2);
        let max = result.iter().filter_map(|p| p.ios_target.as_deref()).max();
        assert_eq!(max, Some("14.0"));
    }

    #[test]
    fn scan_plugin_podspecs_walks_darwin_folder_too() {
        let tmp = TempDir::new().unwrap();
        let plugin_root = tmp.path();
        std::fs::create_dir_all(plugin_root.join("darwin")).unwrap();
        std::fs::write(
            plugin_root.join("darwin/path_provider.podspec"),
            "s.ios.deployment_target = '11.0'\n",
        )
        .unwrap();
        let result = scan_plugin_podspecs("path_provider", plugin_root);
        assert_eq!(result.len(), 1);
        assert!(result[0].podspec_path.to_string_lossy().contains("/darwin/"));
    }

    #[test]
    fn scan_plugin_podspecs_no_ios_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let result = scan_plugin_podspecs("pure_dart", tmp.path());
        assert!(result.is_empty());
    }
}
