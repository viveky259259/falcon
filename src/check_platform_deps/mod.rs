//! `falcon check-platform-deps` — verify Info.plist / AndroidManifest.xml
//! declare every key/permission the project's plugins require.

pub mod android_scan;
pub mod apple_api_map;
pub mod generated;
pub mod ios_scan;
pub mod project_manifest;

use crate::config::{FalconConfig, Severity};
use crate::preflight::pub_cache::{iter_installed_plugins, locate_pub_cache, InstalledPlugin};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue, TargetPlatform};
use anyhow::Result;
use std::path::{Path, PathBuf};

const RULE_ID_MISSING_INFO_PLIST_KEY: &str = "platform-deps/missing-info-plist-key";
const RULE_ID_MISSING_ANDROID_PERMISSION: &str = "platform-deps/missing-android-permission";
const RULE_ID_NON_PUB_DEV_SKIPPED: &str = "platform-deps/non-pub-dev-skipped";

pub fn run(
    root: &Path,
    format: OutputFormat,
    config: &FalconConfig,
    refresh: bool,
    platform: Option<TargetPlatform>,
) -> Result<i32> {
    let pub_cache = locate_pub_cache();
    let installed = iter_installed_plugins(root, &pub_cache).unwrap_or_default();

    let lock_sha = generated::pubspec_lock_sha256(root);
    let cached = if refresh {
        None
    } else {
        generated::read_cache(root).filter(|c| c.pubspec_lock_sha256 == lock_sha && !lock_sha.is_empty())
    };

    let doc = match cached {
        Some(c) => c,
        None => {
            let fresh = scan_all(&installed, root);
            let _ = generated::write_cache(root, &fresh);
            fresh
        }
    };

    let project_keys = project_manifest::read_project_info_plist(root);
    let project_perms = project_manifest::read_project_android_manifest(root);

    let mut issues: Vec<PreflightIssue> = Vec::new();
    let want_ios = matches!(platform, None | Some(TargetPlatform::Ios) | Some(TargetPlatform::Both));
    let want_android = matches!(platform, None | Some(TargetPlatform::Android) | Some(TargetPlatform::Both));

    for (name, plugin) in &doc.plugins {
        if let Some(reason) = &plugin.skipped {
            issues.push(PreflightIssue {
                rule_id: RULE_ID_NON_PUB_DEV_SKIPPED.into(),
                severity: Severity::Info,
                title: "Plugin skipped (non-pub.dev source)".into(),
                file: None,
                line: None,
                plugin: Some(name.clone()),
                message: format!("Plugin `{name}` skipped: {reason}"),
                suggestion: None,
            });
            continue;
        }

        if want_ios {
            if let Some(ios) = &plugin.ios {
                for key in &ios.info_plist_keys {
                    if !project_keys.contains(&key.key) {
                        issues.push(PreflightIssue {
                            rule_id: RULE_ID_MISSING_INFO_PLIST_KEY.into(),
                            severity: Severity::Error,
                            title: "Missing Required Info.plist Key".into(),
                            file: Some(PathBuf::from("ios/Runner/Info.plist")),
                            line: None,
                            plugin: Some(format!("{} {}", name, plugin.version)),
                            message: format!(
                                "Plugin `{name}` calls `{}` which requires <key>{}</key> in Info.plist (found in {}).",
                                key.api, key.key, key.source
                            ),
                            suggestion: Some(format!(
                                "Add <key>{}</key><string>...</string> to ios/Runner/Info.plist.",
                                key.key
                            )),
                        });
                    }
                }
            }
        }
        if want_android {
            if let Some(android) = &plugin.android {
                for perm in &android.permissions {
                    if !project_perms.contains(perm) {
                        issues.push(PreflightIssue {
                            rule_id: RULE_ID_MISSING_ANDROID_PERMISSION.into(),
                            severity: Severity::Error,
                            title: "Missing Android Permission".into(),
                            file: Some(PathBuf::from("android/app/src/main/AndroidManifest.xml")),
                            line: None,
                            plugin: Some(format!("{} {}", name, plugin.version)),
                            message: format!(
                                "Plugin `{name}` declares <uses-permission android:name=\"{perm}\"/> in its own manifest, but the project's manifest does not."
                            ),
                            suggestion: Some(format!(
                                "Add <uses-permission android:name=\"{perm}\"/> to android/app/src/main/AndroidManifest.xml."
                            )),
                        });
                    }
                }
            }
        }
    }

    issues.retain(|issue| !is_suppressed(issue, config));
    let exit = exit_code_for_issues(&issues);
    print!("{}", reporter::render(&issues, format));
    Ok(exit)
}

fn scan_all(installed: &[InstalledPlugin], _root: &Path) -> generated::GeneratedRequirements {
    let mut doc = generated::GeneratedRequirements {
        schema_version: 1,
        generated_at: "generated".into(),
        pubspec_lock_sha256: String::new(),
        plugins: Default::default(),
    };
    for plugin in installed {
        if plugin.source != "hosted" {
            doc.plugins.insert(
                plugin.name.clone(),
                generated::PluginRequirements {
                    version: plugin.version.clone(),
                    source: plugin.source.clone(),
                    ios: None,
                    android: None,
                    skipped: Some("non-pub.dev source — Falcon cannot infer requirements".into()),
                },
            );
            continue;
        }
        let Some(plugin_root) = &plugin.root else { continue };
        let ios_reqs = ios_scan::scan_ios_sources(plugin_root);
        let ios = if ios_reqs.is_empty() {
            None
        } else {
            Some(generated::IosRequirements {
                info_plist_keys: ios_reqs
                    .iter()
                    .map(|r| generated::IosKey {
                        key: r.key.to_string(),
                        api: r.api.to_string(),
                        source: r
                            .source_file
                            .strip_prefix(plugin_root)
                            .unwrap_or(&r.source_file)
                            .display()
                            .to_string(),
                    })
                    .collect(),
            })
        };
        let android_perms = android_scan::scan_plugin_manifest(plugin_root);
        let android = if android_perms.is_empty() {
            None
        } else {
            Some(generated::AndroidRequirements {
                permissions: android_perms,
            })
        };
        doc.plugins.insert(
            plugin.name.clone(),
            generated::PluginRequirements {
                version: plugin.version.clone(),
                source: "hosted".into(),
                ios,
                android,
                skipped: None,
            },
        );
    }
    doc
}

fn is_suppressed(issue: &PreflightIssue, config: &FalconConfig) -> bool {
    config
        .preflight
        .suppress
        .iter()
        .any(|s| s.rule_id == issue.rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preflight::pub_cache::FALCON_ENV_MUTEX;
    use std::sync::MutexGuard;
    use tempfile::TempDir;

    fn env_lock() -> MutexGuard<'static, ()> {
        FALCON_ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fixture() -> (TempDir, TempDir, MutexGuard<'static, ()>) {
        let guard = env_lock();
        let project = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        std::env::set_var("FALCON_PUB_CACHE", pub_cache.path());
        (project, pub_cache, guard)
    }

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn write_plugin(
        pub_cache: &Path,
        name: &str,
        version: &str,
        ios_source: Option<(&str, &str)>,
        android_manifest_xml: Option<&str>,
    ) {
        let plugin_root = pub_cache
            .join("hosted/pub.dev")
            .join(format!("{name}-{version}"));
        if let Some((filename, src)) = ios_source {
            write(&plugin_root.join("ios/Classes").join(filename), src);
        }
        if let Some(xml) = android_manifest_xml {
            write(&plugin_root.join("android/src/main/AndroidManifest.xml"), xml);
        }
    }

    fn write_lock(project: &Path, packages: &[(&str, &str, &str)]) {
        let mut lock = String::from("packages:\n");
        for (name, version, source) in packages {
            lock.push_str(&format!("  {name}:\n    source: {source}\n    version: \"{version}\"\n"));
        }
        write(&project.join("pubspec.lock"), &lock);
    }

    #[test]
    fn empty_project_exits_zero() {
        let (project, _cache, _guard) = fixture();
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn ios_missing_key_exits_two() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("location", "8.0.0", "hosted")]);
        write_plugin(
            cache.path(),
            "location",
            "8.0.0",
            Some(("Location.m", "[CLLocationManager.shared requestWhenInUseAuthorization];")),
            None,
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn ios_key_present_in_project_exits_zero() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("location", "8.0.0", "hosted")]);
        write_plugin(
            cache.path(),
            "location",
            "8.0.0",
            Some(("Location.m", "[CLLocationManager.shared requestWhenInUseAuthorization];")),
            None,
        );
        write(
            &project.path().join("ios/Runner/Info.plist"),
            r#"<plist><dict>
  <key>NSLocationWhenInUseUsageDescription</key><string>Y</string>
</dict></plist>"#,
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn android_missing_permission_exits_two() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("camera", "0.10.0", "hosted")]);
        write_plugin(
            cache.path(),
            "camera",
            "0.10.0",
            None,
            Some(r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#),
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn android_permission_present_in_project_exits_zero() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("camera", "0.10.0", "hosted")]);
        write_plugin(
            cache.path(),
            "camera",
            "0.10.0",
            None,
            Some(r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#),
        );
        write(
            &project.path().join("android/app/src/main/AndroidManifest.xml"),
            r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#,
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn non_pub_dev_plugin_emits_info_only() {
        let (project, _cache, _guard) = fixture();
        write_lock(project.path(), &[("git_plugin", "0.0.0", "git")]);
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn platform_filter_ios_only_skips_android_checks() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("camera", "0.10.0", "hosted")]);
        write_plugin(
            cache.path(),
            "camera",
            "0.10.0",
            None,
            Some(r#"<manifest><uses-permission android:name="android.permission.CAMERA"/></manifest>"#),
        );
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, Some(TargetPlatform::Ios)).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn cache_file_written_to_falcon_dir() {
        let (project, cache, _guard) = fixture();
        write_lock(project.path(), &[("location", "8.0.0", "hosted")]);
        write_plugin(
            cache.path(),
            "location",
            "8.0.0",
            Some(("Location.m", "no-api-here")),
            None,
        );
        let _ = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false, None).unwrap();
        let cache_file = project.path().join(".falcon/plugin-requirements.yaml");
        assert!(cache_file.exists(), "expected .falcon/plugin-requirements.yaml");
    }
}
