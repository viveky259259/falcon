//! Feature discovery for Flutter projects.
//!
//! Heuristics, not perfect parsing:
//! - `lib/features/<name>/`     → feature
//! - `lib/src/features/<name>/` → feature
//! - `lib/<name>/` (top-level)  → feature, but only if it has ≥2 .dart files
//!   AND its name is not a common shared/utility folder.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Names that are conventionally shared/cross-cutting code, not features.
const NON_FEATURE_NAMES: &[&str] = &[
    "src",
    "widgets",
    "components",
    "utils",
    "util",
    "helpers",
    "models",
    "model",
    "constants",
    "const",
    "theme",
    "themes",
    "styles",
    "config",
    "core",
    "shared",
    "common",
    "services",
    "service",
    "data",
    "domain",
    "infrastructure",
    "l10n",
    "localization",
    "generated",
    "gen",
    "assets",
    "router",
    "routes",
    "routing",
    "navigation",
    "extensions",
    "ext",
];

#[derive(Debug, Clone)]
pub struct Feature {
    /// Absolute or project-relative directory of the feature.
    pub dir: PathBuf,
    /// Short name (last path component).
    pub name: String,
    /// How the feature was discovered.
    pub source: FeatureSource,
    /// .dart files immediately under the feature dir (not recursive).
    pub top_level_dart_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureSource {
    /// Found under `lib/features/<name>/`
    LibFeatures,
    /// Found under `lib/src/features/<name>/`
    LibSrcFeatures,
    /// Top-level `lib/<name>/` with ≥2 .dart files and a non-shared name.
    LibTopLevel,
}

/// Walk `project_root` and return discovered features.
pub fn discover_features(project_root: &Path) -> Vec<Feature> {
    let mut out = Vec::new();
    let lib = project_root.join("lib");
    if !lib.is_dir() {
        return out;
    }

    let conventional = [
        (lib.join("features"), FeatureSource::LibFeatures),
        (lib.join("src").join("features"), FeatureSource::LibSrcFeatures),
    ];
    let mut conventional_used = false;
    for (dir, source) in &conventional {
        if dir.is_dir() {
            conventional_used = true;
            for entry in walkdir::WalkDir::new(dir).max_depth(1).into_iter().flatten() {
                if entry.file_type().is_dir() && entry.path() != dir {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let dart_files = top_level_dart_files(entry.path());
                    out.push(Feature {
                        dir: entry.path().to_path_buf(),
                        name,
                        source: *source,
                        top_level_dart_files: dart_files,
                    });
                }
            }
        }
    }

    // Fall back to lib/<name>/ heuristic only when no conventional layout was found,
    // so projects using lib/features/ don't double-emit.
    if !conventional_used {
        for entry in WalkDir::new(&lib).max_depth(1).into_iter().flatten() {
            if !entry.file_type().is_dir() || entry.path() == lib {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if NON_FEATURE_NAMES.contains(&name.as_str()) {
                continue;
            }
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            let dart_files = top_level_dart_files(entry.path());
            if dart_files.len() < 2 {
                continue;
            }
            out.push(Feature {
                dir: entry.path().to_path_buf(),
                name: entry.file_name().to_string_lossy().to_string(),
                source: FeatureSource::LibTopLevel,
                top_level_dart_files: dart_files,
            });
        }
    }

    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out
}

fn top_level_dart_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().map_or(false, |ext| ext == "dart") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "// dart").unwrap();
    }

    #[test]
    fn discovers_lib_features_layout() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("lib/features/auth/login.dart"));
        touch(&root.join("lib/features/profile/profile.dart"));
        let feats = discover_features(root);
        assert_eq!(feats.len(), 2);
        assert!(feats.iter().any(|f| f.name == "auth"));
        assert!(feats.iter().any(|f| f.name == "profile"));
        assert!(feats.iter().all(|f| f.source == FeatureSource::LibFeatures));
    }

    #[test]
    fn discovers_lib_src_features_layout() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("lib/src/features/cart/cart.dart"));
        let feats = discover_features(root);
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].name, "cart");
        assert_eq!(feats[0].source, FeatureSource::LibSrcFeatures);
    }

    #[test]
    fn falls_back_to_top_level_when_no_features_dir() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("lib/dashboard/page.dart"));
        touch(&root.join("lib/dashboard/widget.dart"));
        let feats = discover_features(root);
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].name, "dashboard");
        assert_eq!(feats[0].source, FeatureSource::LibTopLevel);
    }

    #[test]
    fn skips_shared_dirs_in_top_level_mode() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("lib/widgets/btn.dart"));
        touch(&root.join("lib/widgets/card.dart"));
        touch(&root.join("lib/utils/format.dart"));
        touch(&root.join("lib/utils/parse.dart"));
        let feats = discover_features(root);
        assert!(feats.is_empty(), "got {feats:?}");
    }

    #[test]
    fn ignores_top_level_dirs_with_one_file() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("lib/loose/single.dart"));
        let feats = discover_features(root);
        assert!(feats.is_empty());
    }

    #[test]
    fn lib_features_takes_precedence_over_top_level() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("lib/features/auth/login.dart"));
        touch(&root.join("lib/dashboard/a.dart"));
        touch(&root.join("lib/dashboard/b.dart"));
        let feats = discover_features(root);
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].name, "auth");
    }

    #[test]
    fn returns_empty_when_no_lib_dir() {
        let dir = tempdir().unwrap();
        let feats = discover_features(dir.path());
        assert!(feats.is_empty());
    }
}
