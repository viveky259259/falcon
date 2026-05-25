//! `falcon check-pods` — verify iOS Podfile deployment target.

pub mod podfile;
pub mod podspec;

use crate::config::{FalconConfig, Severity};
use crate::preflight::pub_cache::{iter_installed_plugins, locate_pub_cache};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue};
use anyhow::Result;
use std::path::{Path, PathBuf};

const RULE_ID_PODFILE_MISSING: &str = "pods/podfile-not-found";
const RULE_ID_DEPLOYMENT_TARGET_LOW: &str = "pods/deployment-target-too-low";
const RULE_ID_PLATFORM_NOT_SET: &str = "pods/platform-not-set";

const DEFAULT_IOS_PODFILE_TARGET: &str = "12.0";
const DEFAULT_IOS_PODSPEC_TARGET: &str = "9.0";

pub fn run(
    root: &Path,
    format: OutputFormat,
    config: &FalconConfig,
    _refresh: bool,
) -> Result<i32> {
    let mut issues: Vec<PreflightIssue> = Vec::new();

    let podfile_path = root.join("ios").join("Podfile");
    let Ok(podfile_text) = std::fs::read_to_string(&podfile_path) else {
        issues.push(PreflightIssue {
            rule_id: RULE_ID_PODFILE_MISSING.into(),
            severity: Severity::Info,
            title: "ios/Podfile not found".into(),
            file: Some(PathBuf::from("ios/Podfile")),
            line: None,
            plugin: None,
            message: "No ios/Podfile to check. Skipping pod deployment-target check.".into(),
            suggestion: None,
        });
        issues.retain(|issue| !is_suppressed(issue, config));
        let exit = exit_code_for_issues(&issues);
        print!("{}", reporter::render(&issues, format));
        return Ok(exit);
    };

    let podfile_platform = podfile::parse_ios_platform(&podfile_text);
    let podfile_target_str = podfile_platform
        .version
        .clone()
        .unwrap_or_else(|| DEFAULT_IOS_PODFILE_TARGET.to_string());

    if podfile_platform.version.is_none() {
        issues.push(PreflightIssue {
            rule_id: RULE_ID_PLATFORM_NOT_SET.into(),
            severity: Severity::Warning,
            title: "ios/Podfile does not set platform :ios".into(),
            file: Some(PathBuf::from("ios/Podfile")),
            line: None,
            plugin: None,
            message: format!(
                "No `platform :ios, 'X.Y'` directive found in ios/Podfile. \
CocoaPods will default to {DEFAULT_IOS_PODFILE_TARGET}, which may be lower than your plugins need."
            ),
            suggestion: Some(format!(
                "Add `platform :ios, '{DEFAULT_IOS_PODFILE_TARGET}'` (or a higher version) near the top of ios/Podfile."
            )),
        });
    }

    let pub_cache = locate_pub_cache();
    let plugins = iter_installed_plugins(root, &pub_cache).unwrap_or_default();
    let mut max_target = podfile_target_str.clone();
    let mut binding_plugins: Vec<(String, String)> = Vec::new();

    for plugin in &plugins {
        let Some(plugin_root) = &plugin.root else { continue };
        if !plugin_root.is_dir() {
            continue;
        }
        let podspecs = podspec::scan_plugin_podspecs(&plugin.name, plugin_root);
        for ps in &podspecs {
            let target = ps
                .ios_target
                .clone()
                .unwrap_or_else(|| DEFAULT_IOS_PODSPEC_TARGET.to_string());
            if podfile::cmp_version(&target, &podfile_target_str) > 0 {
                binding_plugins.push((plugin.name.clone(), target.clone()));
                if podfile::cmp_version(&target, &max_target) > 0 {
                    max_target = target;
                }
            }
        }
    }

    if !binding_plugins.is_empty() {
        binding_plugins.sort();
        binding_plugins.dedup();
        let names: Vec<String> = binding_plugins
            .iter()
            .map(|(name, ver)| format!("{name} (requires {ver})"))
            .collect();
        issues.push(PreflightIssue {
            rule_id: RULE_ID_DEPLOYMENT_TARGET_LOW.into(),
            severity: Severity::Error,
            title: "iOS deployment target too low".into(),
            file: Some(PathBuf::from("ios/Podfile")),
            line: None,
            plugin: None,
            message: format!(
                "Podfile platform :ios = {podfile_target_str} but plugins require ≥ {max_target}: {}",
                names.join(", ")
            ),
            suggestion: Some(format!(
                "Set `platform :ios, '{max_target}'` near the top of ios/Podfile, then run `pod install`."
            )),
        });
    }

    issues.retain(|issue| !is_suppressed(issue, config));
    let exit = exit_code_for_issues(&issues);
    print!("{}", reporter::render(&issues, format));
    Ok(exit)
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
    use crate::config::{PreflightConfig, PreflightSuppression};
    use tempfile::TempDir;
    use std::sync::{Mutex, MutexGuard};

    // Mutex to serialize env-var tests so they don't interfere with each other.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Acquire a global env-var lock. Tests that set FALCON_PUB_CACHE must
    /// hold this guard for their entire body to avoid clobbering each other
    /// under `cargo test`'s default parallel execution.
    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn fixture() -> (TempDir, TempDir, MutexGuard<'static, ()>) {
        let guard = env_lock();
        let project = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        std::env::set_var("FALCON_PUB_CACHE", pub_cache.path());
        (project, pub_cache, guard)
    }

    fn write_podfile(project: &Path, version: Option<&str>) {
        let body = match version {
            Some(v) => format!("platform :ios, '{v}'\ntarget 'Runner' do\nend\n"),
            None => "target 'Runner' do\nend\n".to_string(),
        };
        write(&project.join("ios/Podfile"), &body);
    }

    fn write_plugin(
        pub_cache: &Path,
        name: &str,
        version: &str,
        ios_deployment: Option<&str>,
    ) {
        let plugin_root = pub_cache
            .join("hosted/pub.dev")
            .join(format!("{name}-{version}"));
        std::fs::create_dir_all(plugin_root.join("ios")).unwrap();
        let podspec = match ios_deployment {
            Some(d) => format!("Pod::Spec.new do |s|\n  s.ios.deployment_target = '{d}'\nend\n"),
            None => "Pod::Spec.new do |s|\nend\n".to_string(),
        };
        std::fs::write(plugin_root.join("ios").join(format!("{name}.podspec")), podspec).unwrap();
    }

    fn write_lock(project: &Path, packages: &[(&str, &str)]) {
        let mut lock = String::from("packages:\n");
        for (name, version) in packages {
            lock.push_str(&format!("  {name}:\n    source: hosted\n    version: \"{version}\"\n"));
        }
        write(&project.join("pubspec.lock"), &lock);
    }

    #[test]
    fn no_podfile_emits_info_exit_zero() {
        let (project, _cache, _guard) = fixture();
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn missing_platform_directive_emits_warning() {
        let (project, _cache, _guard) = fixture();
        write_podfile(project.path(), None);
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn plugin_requires_higher_than_podfile_emits_error() {
        let (project, cache, _guard) = fixture();
        write_podfile(project.path(), Some("12.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("13.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn plugin_equal_to_podfile_is_clean() {
        let (project, cache, _guard) = fixture();
        write_podfile(project.path(), Some("13.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("13.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn plugin_lower_than_podfile_is_clean() {
        let (project, cache, _guard) = fixture();
        write_podfile(project.path(), Some("14.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("11.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn multiple_plugins_max_target_named_in_error() {
        let (project, cache, _guard) = fixture();
        write_podfile(project.path(), Some("12.0"));
        write_lock(project.path(), &[("a", "1.0.0"), ("b", "2.0.0"), ("c", "3.0.0")]);
        write_plugin(cache.path(), "a", "1.0.0", Some("13.0"));
        write_plugin(cache.path(), "b", "2.0.0", Some("14.0"));
        write_plugin(cache.path(), "c", "3.0.0", Some("12.0"));
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn plugin_without_podspec_target_uses_cocoapods_default() {
        let (project, cache, _guard) = fixture();
        write_podfile(project.path(), Some("10.0"));
        write_lock(project.path(), &[("x", "1.0.0")]);
        write_plugin(cache.path(), "x", "1.0.0", None);
        let code = run(project.path(), OutputFormat::Text, &FalconConfig::default(), false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn suppression_silences_low_target_error() {
        let (project, cache, _guard) = fixture();
        write_podfile(project.path(), Some("12.0"));
        write_lock(project.path(), &[("location", "8.0.0")]);
        write_plugin(cache.path(), "location", "8.0.0", Some("13.0"));
        let cfg = FalconConfig {
            preflight: PreflightConfig {
                suppress: vec![PreflightSuppression {
                    rule_id: RULE_ID_DEPLOYMENT_TARGET_LOW.into(),
                    plugin: None,
                    key: None,
                    reason: "Bumping deferred to release branch.".into(),
                }],
                config: Default::default(),
            },
            ..FalconConfig::default()
        };
        let code = run(project.path(), OutputFormat::Text, &cfg, false).unwrap();
        assert_eq!(code, 0);
    }
}
