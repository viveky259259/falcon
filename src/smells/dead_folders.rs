//! Dead-folder detection.
//!
//! A folder is "dead" when every `.dart` file under it (recursively) is in the
//! supplied set of unused files, AND it is not under a build/generated/test
//! directory we deliberately ignore. This complements `unused-file` detection
//! by rolling up entire orphan subtrees so the user can delete a directory in
//! one shot instead of file-by-file.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SKIP_FOLDER_NAMES: &[&str] = &[
    ".dart_tool",
    "build",
    ".git",
    ".pub-cache",
    "node_modules",
    "ios",
    "android",
    "macos",
    "windows",
    "linux",
    "web",
];

/// Find folders where every Dart file is unused.
///
/// `root` is the project root. `unused_files` is the set of paths (relative or
/// absolute, matched lexically) that have already been flagged as unused. The
/// result is a sorted, de-duplicated list of folder paths.
///
/// Folders that are wholly contained inside another dead folder are pruned
/// from the output so the user only sees the topmost dead ancestor.
pub fn find_dead_folders(root: &Path, unused_files: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let unused_canonical: HashSet<PathBuf> = unused_files
        .iter()
        .map(|p| canonicalize_or_clone(p))
        .collect();

    let mut folder_files: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "dart") {
            continue;
        }
        if path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|s| SKIP_FOLDER_NAMES.contains(&s))
        }) {
            continue;
        }
        if let Some(parent) = path.parent() {
            folder_files
                .entry(parent.to_path_buf())
                .or_default()
                .push(path.to_path_buf());
        }
    }

    let mut dead: Vec<PathBuf> = folder_files
        .into_iter()
        .filter(|(_, files)| !files.is_empty())
        .filter(|(_, files)| {
            files.iter().all(|f| {
                let c = canonicalize_or_clone(f);
                unused_canonical.contains(&c) || unused_files.contains(f)
            })
        })
        .map(|(folder, _)| folder)
        .collect();

    dead.sort();
    prune_descendants(&mut dead);
    dead
}

fn canonicalize_or_clone(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Given a sorted list of paths, remove any path that is a descendant of an
/// earlier path. So if `lib/dead/` is dead, `lib/dead/sub/` should not also
/// be reported.
fn prune_descendants(paths: &mut Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let mut kept: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for p in paths.drain(..) {
        let is_descendant_of_kept = kept.iter().any(|ancestor| p.starts_with(ancestor));
        if !is_descendant_of_kept {
            kept.push(p);
        }
    }
    *paths = kept;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn folder_with_all_unused_files_is_dead() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("lib/feature_a")).unwrap();
        let f1 = root.join("lib/feature_a/widget.dart");
        let f2 = root.join("lib/feature_a/model.dart");
        fs::write(&f1, "// orphan").unwrap();
        fs::write(&f2, "// orphan").unwrap();

        let mut unused = HashSet::new();
        unused.insert(f1.clone());
        unused.insert(f2.clone());

        let dead = find_dead_folders(root, &unused);
        assert_eq!(dead.len(), 1);
        assert!(dead[0].ends_with("lib/feature_a"));
    }

    #[test]
    fn folder_with_one_used_file_is_not_dead() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("lib/feature_b")).unwrap();
        let f1 = root.join("lib/feature_b/used.dart");
        let f2 = root.join("lib/feature_b/orphan.dart");
        fs::write(&f1, "// used").unwrap();
        fs::write(&f2, "// orphan").unwrap();

        let mut unused = HashSet::new();
        unused.insert(f2);

        let dead = find_dead_folders(root, &unused);
        assert!(dead.is_empty(), "expected no dead folders, got {dead:?}");
    }

    #[test]
    fn nested_dead_folders_are_pruned_to_topmost() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("lib/dead/sub")).unwrap();
        let f1 = root.join("lib/dead/a.dart");
        let f2 = root.join("lib/dead/sub/b.dart");
        fs::write(&f1, "// orphan").unwrap();
        fs::write(&f2, "// orphan").unwrap();

        let mut unused = HashSet::new();
        unused.insert(f1);
        unused.insert(f2);

        let dead = find_dead_folders(root, &unused);
        assert_eq!(dead.len(), 1, "expected only topmost folder, got {dead:?}");
        assert!(dead[0].ends_with("lib/dead"));
    }

    #[test]
    fn build_and_dart_tool_folders_are_skipped() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".dart_tool/foo")).unwrap();
        fs::create_dir_all(root.join("build/foo")).unwrap();
        let f1 = root.join(".dart_tool/foo/x.dart");
        let f2 = root.join("build/foo/y.dart");
        fs::write(&f1, "// orphan").unwrap();
        fs::write(&f2, "// orphan").unwrap();

        let mut unused = HashSet::new();
        unused.insert(f1);
        unused.insert(f2);

        let dead = find_dead_folders(root, &unused);
        assert!(
            dead.is_empty(),
            "build/.dart_tool folders should not be flagged"
        );
    }

    #[test]
    fn empty_folders_are_not_reported() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("lib/empty")).unwrap();

        let unused = HashSet::new();
        let dead = find_dead_folders(root, &unused);
        assert!(dead.is_empty());
    }
}
