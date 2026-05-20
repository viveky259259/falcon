//! Integration coverage for the project-level resolver.
//!
//! The richer name / import resolver from the council roadmap (EPIC 3.1) does
//! not exist yet — `src/resolver/` today is a project file walker plus dead-
//! code / unused-symbol detectors. These tests exercise the publicly observable
//! `ProjectResolver` API end-to-end against synthesised mini-projects.

use falcon::config::FalconConfig;
use falcon::resolver::ProjectResolver;
use std::fs;
use std::path::Path;

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir -p");
    }
    fs::write(path, body).expect("write");
}

#[test]
fn project_resolver_collects_dart_files_and_flags_unused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("lib/main.dart"), "void main() {}\n");
    write(&root.join("lib/util.dart"), "int answer() => 42;\n");
    write(&root.join("README.md"), "hi\n");

    let cfg = FalconConfig::default();
    let resolver = ProjectResolver::new(root, &cfg).expect("ProjectResolver::new");
    let issues = resolver.find_unused_files().expect("find_unused_files");

    // util.dart is not imported → flagged. main.dart is exempt by name.
    assert!(
        issues
            .iter()
            .any(|i| i.file.to_string_lossy().ends_with("util.dart")),
        "util.dart should be flagged; got: {:#?}",
        issues
    );
    assert!(!issues
        .iter()
        .any(|i| i.file.to_string_lossy().ends_with("main.dart")));
}

#[test]
fn project_resolver_flags_unused_dependencies() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join("pubspec.yaml"),
        "name: demo\ndependencies:\n  used_pkg: ^1.0.0\n  ghost_pkg: ^1.0.0\n",
    );
    write(
        &root.join("lib/main.dart"),
        "import 'package:used_pkg/used_pkg.dart';\nvoid main() {}\n",
    );

    let cfg = FalconConfig::default();
    let resolver = ProjectResolver::new(root, &cfg).unwrap();
    let messages: Vec<String> = resolver
        .find_unused_dependencies()
        .unwrap()
        .into_iter()
        .map(|i| i.message)
        .collect();

    assert!(
        messages.iter().any(|m| m.contains("ghost_pkg")),
        "ghost_pkg should be flagged; got: {:?}",
        messages
    );
    assert!(!messages.iter().any(|m| m.contains("used_pkg")));
}
