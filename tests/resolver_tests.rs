//! Integration coverage for the project-level resolver.
//!
//! The richer name / import resolver from the council roadmap (EPIC 3.1) does
//! not exist yet — `src/resolver/` today is a project file walker plus dead-
//! code / unused-symbol detectors. These tests exercise the publicly observable
//! `ProjectResolver` API end-to-end against synthesised mini-projects.

use falcon::config::FalconConfig;
use falcon::parser::{find_descendants_by_kind, DartParser};
use falcon::resolver::{ProjectResolver, ResolvedSymbolKind};
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

#[test]
fn project_resolver_builds_class_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        &root.join("lib/base.dart"),
        "class BaseState extends State<W> { void dispose() { super.dispose(); } }\n",
    );
    write(
        &root.join("lib/screen.dart"),
        "class _ScreenState extends BaseState with Traceable implements ScreenContract {}\n",
    );

    let resolver = ProjectResolver::new(root, &FalconConfig::default()).unwrap();
    let index = resolver.build_index().unwrap();

    let base = index.class("BaseState").expect("BaseState indexed");
    assert!(base.file.to_string_lossy().ends_with("lib/base.dart"));
    assert_eq!(base.line, 1);
    assert_eq!(base.superclass.as_deref(), Some("State"));
    assert!(base.has_method("dispose"));

    let screen = index.class("_ScreenState").expect("_ScreenState indexed");
    assert_eq!(screen.superclass.as_deref(), Some("BaseState"));
    assert_eq!(screen.mixins, vec!["Traceable"]);
    assert_eq!(screen.interfaces, vec!["ScreenContract"]);
}

#[test]
fn resolver_index_resolves_transitive_subtype() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        &root.join("lib/base.dart"),
        "class BaseState extends State<W> {}\nclass Service {}\n",
    );
    write(
        &root.join("lib/screen.dart"),
        "class _ScreenState extends BaseState {}\n",
    );

    let resolver = ProjectResolver::new(root, &FalconConfig::default()).unwrap();
    let index = resolver.build_index().unwrap();

    assert!(index.is_subtype_of("_ScreenState", "State"));
    assert!(index.is_subtype_of("BaseState", "State"));
    assert!(!index.is_subtype_of("Service", "State"));
    assert!(!index.is_subtype_of("Missing", "State"));
}

#[test]
fn resolver_index_reports_method_presence() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        &root.join("lib/widget.dart"),
        "class _WidgetState extends State<W> { void dispose() {} void build() {} }\n",
    );

    let resolver = ProjectResolver::new(root, &FalconConfig::default()).unwrap();
    let index = resolver.build_index().unwrap();
    let class = index.class("_WidgetState").unwrap();

    assert!(class.has_method("dispose"));
    assert!(class.has_method("build"));
    assert!(!class.has_method("initState"));
}

#[test]
fn resolver_resolves_same_file_class_declaration() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let file = root.join("lib/screen.dart");
    let source = "class _ScreenState extends State<W> {}\n";
    write(&file, source);

    let resolver = ProjectResolver::new(root, &FalconConfig::default()).unwrap();
    let index = resolver.build_index().unwrap();
    let file_resolver = index.resolver_for_file(&file, source);

    let mut parser = DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();
    let class_node = find_descendants_by_kind(tree.root_node(), "class_declaration")
        .into_iter()
        .next()
        .expect("class declaration");

    let symbol = file_resolver
        .resolve(class_node)
        .expect("class declaration should resolve");
    assert_eq!(symbol.name, "_ScreenState");
    assert_eq!(symbol.kind, ResolvedSymbolKind::Class);
    assert_eq!(symbol.declaring_library, file);
    assert_eq!(symbol.type_hint, None);
}

#[test]
fn resolver_marks_cross_file_duplicate_simple_names_ambiguous() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        &root.join("lib/a.dart"),
        "class BaseState extends State<A> {}\n",
    );
    write(
        &root.join("lib/b.dart"),
        "class BaseState extends State<B> {}\n",
    );
    let screen = root.join("lib/screen.dart");
    let source = "class _ScreenState extends BaseState {}\n";
    write(&screen, source);

    let resolver = ProjectResolver::new(root, &FalconConfig::default()).unwrap();
    let index = resolver.build_index().unwrap();
    let file_resolver = index.resolver_for_file(&screen, source);

    assert_eq!(index.classes_named("BaseState").len(), 2);
    assert!(file_resolver.is_ambiguous_class_name("BaseState"));
    assert!(file_resolver.resolve_class_name("BaseState").is_none());
}

#[test]
fn resolver_does_not_claim_unindexed_import_resolution() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let screen = root.join("lib/screen.dart");
    let source = "class _ScreenState extends ImportedState {}\n";
    write(&screen, source);

    let resolver = ProjectResolver::new(root, &FalconConfig::default()).unwrap();
    let index = resolver.build_index().unwrap();
    let file_resolver = index.resolver_for_file(&screen, source);

    assert!(file_resolver.resolve_class_name("ImportedState").is_none());
    assert!(!file_resolver.is_ambiguous_class_name("ImportedState"));
}
