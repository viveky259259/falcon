//! Locate the pub-cache root and enumerate installed plugins from
//! `pubspec.lock`. Shared by `check_pods` and `check_platform_deps`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One plugin entry resolved from `pubspec.lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPlugin {
    /// Plugin name as used in `pubspec.yaml`.
    pub name: String,
    /// Version string (e.g. "8.0.0").
    pub version: String,
    /// Source kind: "hosted", "git", "path", "sdk".
    pub source: String,
    /// Absolute path to the plugin's directory in the pub-cache, when source
    /// is `hosted`. None for git/path/sdk sources.
    pub root: Option<PathBuf>,
}

/// Return the pub-cache root the user is using.
/// Precedence: `FALCON_PUB_CACHE` env > `PUB_CACHE` env > platform default.
pub fn locate_pub_cache() -> PathBuf {
    if let Ok(p) = std::env::var("FALCON_PUB_CACHE") {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("PUB_CACHE") {
        return PathBuf::from(p);
    }
    if cfg!(windows) {
        if let Ok(p) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(p).join("Pub").join("Cache");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".pub-cache");
    }
    PathBuf::from(".pub-cache")
}

#[derive(Debug, Deserialize)]
struct PubspecLock {
    #[serde(default)]
    packages: BTreeMap<String, LockedPackage>,
}

#[derive(Debug, Deserialize)]
struct LockedPackage {
    #[serde(default)]
    source: String,
    #[serde(default)]
    version: String,
}

/// Read `<project>/pubspec.lock` and return one `InstalledPlugin` per package.
pub fn iter_installed_plugins(project_root: &Path, pub_cache: &Path) -> Result<Vec<InstalledPlugin>> {
    let lock_path = project_root.join("pubspec.lock");
    if !lock_path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&lock_path)
        .with_context(|| format!("Reading {}", lock_path.display()))?;
    let lock: PubspecLock = serde_yaml::from_str(&text).context("Parsing pubspec.lock")?;

    let mut out = Vec::with_capacity(lock.packages.len());
    for (name, pkg) in lock.packages {
        if pkg.source == "sdk" {
            continue;
        }
        let root = if pkg.source == "hosted" {
            Some(
                pub_cache
                    .join("hosted")
                    .join("pub.dev")
                    .join(format!("{name}-{}", pkg.version)),
            )
        } else {
            None
        };
        out.push(InstalledPlugin {
            name,
            version: pkg.version,
            source: pkg.source,
            root,
        });
    }
    Ok(out)
}

/// Filter to plugins that have an `ios/` or `darwin/` source folder.
pub fn plugins_with_ios(plugins: &[InstalledPlugin]) -> Vec<&InstalledPlugin> {
    plugins
        .iter()
        .filter(|p| match &p.root {
            Some(root) => root.join("ios").is_dir() || root.join("darwin").is_dir(),
            None => false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn locate_pub_cache_honors_falcon_env_first() {
        std::env::set_var("FALCON_PUB_CACHE", "/tmp/fake-falcon-cache");
        let p = locate_pub_cache();
        assert_eq!(p, PathBuf::from("/tmp/fake-falcon-cache"));
        std::env::remove_var("FALCON_PUB_CACHE");
    }

    #[test]
    fn iter_installed_plugins_empty_when_no_lock() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn iter_installed_plugins_parses_hosted_packages() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        write(
            &tmp.path().join("pubspec.lock"),
            r#"packages:
  location:
    source: hosted
    version: "8.0.0"
  http:
    source: hosted
    version: "1.2.0"
sdks:
  dart: ">=3.0.0"
"#,
        );
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert_eq!(plugins.len(), 2);
        let loc = plugins.iter().find(|p| p.name == "location").unwrap();
        assert_eq!(loc.version, "8.0.0");
        assert_eq!(loc.source, "hosted");
        assert_eq!(
            loc.root,
            Some(pub_cache.path().join("hosted/pub.dev/location-8.0.0"))
        );
    }

    #[test]
    fn iter_installed_plugins_skips_sdk_entries() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        write(
            &tmp.path().join("pubspec.lock"),
            r#"packages:
  flutter:
    source: sdk
    version: "0.0.0"
  location:
    source: hosted
    version: "8.0.0"
"#,
        );
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "location");
    }

    #[test]
    fn iter_installed_plugins_keeps_non_pub_sources_without_root() {
        let tmp = TempDir::new().unwrap();
        let pub_cache = TempDir::new().unwrap();
        write(
            &tmp.path().join("pubspec.lock"),
            r#"packages:
  my_git_plugin:
    source: git
    version: "0.0.0"
  my_path_plugin:
    source: path
    version: "0.0.0"
"#,
        );
        let plugins = iter_installed_plugins(tmp.path(), pub_cache.path()).unwrap();
        assert_eq!(plugins.len(), 2);
        for p in &plugins {
            assert!(p.root.is_none(), "non-hosted should have root = None: {p:?}");
        }
    }

    #[test]
    fn plugins_with_ios_filters_by_directory() {
        let pub_cache = TempDir::new().unwrap();
        let plugins = vec![
            InstalledPlugin {
                name: "with_ios".into(),
                version: "1.0.0".into(),
                source: "hosted".into(),
                root: Some(pub_cache.path().join("with_ios-1.0.0")),
            },
            InstalledPlugin {
                name: "no_ios".into(),
                version: "1.0.0".into(),
                source: "hosted".into(),
                root: Some(pub_cache.path().join("no_ios-1.0.0")),
            },
            InstalledPlugin {
                name: "git_plug".into(),
                version: "0.0.0".into(),
                source: "git".into(),
                root: None,
            },
        ];
        std::fs::create_dir_all(pub_cache.path().join("with_ios-1.0.0/ios")).unwrap();
        std::fs::create_dir_all(pub_cache.path().join("no_ios-1.0.0/lib")).unwrap();
        let filtered = plugins_with_ios(&plugins);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "with_ios");
    }
}
