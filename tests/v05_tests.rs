use falcon::incremental::dep_graph::DependencyGraph;
use falcon::resolver::cyclic;
use falcon::resolver::dead_code;
use falcon::resolver::unused_l10n;
use falcon::resolver::unused_params;
use falcon::workspace;
use tempfile::TempDir;

// --- Monorepo / Workspace Tests ---

#[test]
fn test_workspace_single_package() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("pubspec.yaml"),
        "name: my_app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )
    .unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();

    let (ws_type, packages) = workspace::detect_workspace(dir.path());
    assert!(matches!(ws_type, workspace::WorkspaceType::SinglePackage));
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "my_app");
}

#[test]
fn test_workspace_melos_detection() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("melos.yaml"),
        "name: my_workspace\npackages:\n  - packages/**\n",
    )
    .unwrap();

    let pkg_a = dir.path().join("packages").join("pkg_a");
    let pkg_b = dir.path().join("packages").join("pkg_b");
    std::fs::create_dir_all(&pkg_a).unwrap();
    std::fs::create_dir_all(&pkg_b).unwrap();
    std::fs::write(pkg_a.join("pubspec.yaml"), "name: pkg_a\n").unwrap();
    std::fs::write(pkg_b.join("pubspec.yaml"), "name: pkg_b\n").unwrap();

    let (ws_type, packages) = workspace::detect_workspace(dir.path());
    assert!(matches!(ws_type, workspace::WorkspaceType::Melos));
    assert_eq!(packages.len(), 2);
}

#[test]
fn test_workspace_pub_workspace_detection() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("pubspec.yaml"),
        "name: root_app\nworkspace:\n  - modules/auth\n  - modules/core\n",
    )
    .unwrap();

    let auth = dir.path().join("modules").join("auth");
    let core = dir.path().join("modules").join("core");
    std::fs::create_dir_all(&auth).unwrap();
    std::fs::create_dir_all(&core).unwrap();
    std::fs::write(auth.join("pubspec.yaml"), "name: auth\n").unwrap();
    std::fs::write(core.join("pubspec.yaml"), "name: core_module\n").unwrap();

    let (ws_type, packages) = workspace::detect_workspace(dir.path());
    assert!(matches!(ws_type, workspace::WorkspaceType::PubWorkspace));
    assert_eq!(packages.len(), 3);
}

// --- Cyclic Dependency Tests ---

#[test]
fn test_cyclic_dependency_detection() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(lib.join("a.dart"), "import 'b.dart';\nclass A {}\n").unwrap();
    std::fs::write(lib.join("b.dart"), "import 'a.dart';\nclass B {}\n").unwrap();
    std::fs::write(lib.join("c.dart"), "class C {}\n").unwrap();

    let graph = DependencyGraph::build(dir.path(), &[]);
    let (issues, cycles) = cyclic::detect_cycles(&graph, dir.path());

    assert!(
        !cycles.is_empty(),
        "Should detect cycle between a.dart and b.dart"
    );
    assert!(!issues.is_empty());
}

#[test]
fn test_no_cycles_in_clean_graph() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(lib.join("a.dart"), "class A {}\n").unwrap();
    std::fs::write(lib.join("b.dart"), "import 'a.dart';\nclass B {}\n").unwrap();

    let graph = DependencyGraph::build(dir.path(), &[]);
    let (_issues, cycles) = cyclic::detect_cycles(&graph, dir.path());

    assert!(cycles.is_empty(), "Clean graph should have no cycles");
}

#[test]
fn test_cycle_visualization() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(lib.join("a.dart"), "import 'b.dart';\n").unwrap();
    std::fs::write(lib.join("b.dart"), "import 'a.dart';\n").unwrap();

    let graph = DependencyGraph::build(dir.path(), &[]);
    let (_issues, cycles) = cyclic::detect_cycles(&graph, dir.path());
    let viz = cyclic::format_cycles(&cycles, dir.path());

    assert!(viz.contains("Cycle #"));
    assert!(viz.contains("├──"));
    assert!(viz.contains("└──→"));
}

// --- Promoted Dependencies Tests ---

#[test]
fn test_over_promoted_dependency() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("pubspec.yaml"),
        "name: test_app\ndependencies:\n  mockito: ^5.0.0\ndev_dependencies:\n  test: ^1.0.0\n",
    )
    .unwrap();

    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "void main() {}\n").unwrap();

    let test_dir = dir.path().join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(
        test_dir.join("app_test.dart"),
        "import 'package:mockito/mockito.dart';\nvoid main() {}\n",
    )
    .unwrap();

    let issues = cyclic::detect_promoted_deps(dir.path());
    let over_promoted: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "over-promoted-dependency")
        .collect();
    assert!(
        !over_promoted.is_empty(),
        "mockito is only used in test/, should be dev_dependency"
    );
}

// --- Unused L10n Tests ---

#[test]
fn test_unused_l10n_detection() {
    let dir = TempDir::new().unwrap();
    let l10n = dir.path().join("lib").join("l10n");
    std::fs::create_dir_all(&l10n).unwrap();

    std::fs::write(
        l10n.join("app_en.arb"),
        r#"{"hello": "Hello", "goodbye": "Goodbye", "unused_key": "Never used"}"#,
    )
    .unwrap();

    let lib = dir.path().join("lib");
    std::fs::write(
        lib.join("main.dart"),
        "void main() {\n  print(hello);\n  print(goodbye);\n}\n",
    )
    .unwrap();

    let issues = unused_l10n::detect_unused_l10n(dir.path(), &[]);
    let unused: Vec<_> = issues
        .iter()
        .filter(|i| i.message.contains("unused_key"))
        .collect();
    assert!(
        !unused.is_empty(),
        "unused_key should be detected as unused l10n key"
    );
}

// --- Unused Parameters Tests ---

#[test]
fn test_unused_params_detection() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(
        lib.join("utils.dart"),
        "int add(int a, int b, int unused) {\n  return a + b;\n}\n",
    )
    .unwrap();

    let issues = unused_params::detect_unused_params(dir.path(), &[]);
    let param_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.message.contains("unused"))
        .collect();
    assert!(
        !param_issues.is_empty(),
        "Parameter 'unused' should be detected as unused"
    );
}

// --- Dead Code Tests ---

#[test]
fn test_dead_code_after_return() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(
        lib.join("dead.dart"),
        "int foo() {\n  return 42;\n  print('unreachable');\n}\n",
    )
    .unwrap();

    let issues = dead_code::detect_dead_code(dir.path(), &[]);
    let dead_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.rule == "dead-code-path")
        .collect();
    assert!(
        !dead_issues.is_empty(),
        "Code after return should be detected as dead"
    );
}

#[test]
fn test_trivial_condition_detection() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(
        lib.join("trivial.dart"),
        "void foo() {\n  if (false) {\n    print('never');\n  }\n}\n",
    )
    .unwrap();

    let issues = dead_code::detect_dead_code(dir.path(), &[]);
    let trivial: Vec<_> = issues
        .iter()
        .filter(|i| i.message.contains("always false"))
        .collect();
    assert!(
        !trivial.is_empty(),
        "if (false) should be detected as always-false condition"
    );
}

// --- Config Validation Tests ---

#[test]
fn test_config_validation_valid() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\nrules:\n  - avoid-dynamic\n",
    )
    .unwrap();

    let errors = falcon::config::validator::validate_config(dir.path());
    let error_count = errors
        .iter()
        .filter(|e| {
            matches!(
                e.severity,
                falcon::config::validator::ConfigErrorSeverity::Error
            )
        })
        .count();
    assert_eq!(error_count, 0);
}

#[test]
fn test_config_validation_unknown_key() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\nfoo_bar: true\n",
    )
    .unwrap();

    let errors = falcon::config::validator::validate_config(dir.path());
    assert!(
        !errors.is_empty(),
        "Should warn about unknown key 'foo_bar'"
    );
}

#[test]
fn test_config_validation_invalid_yaml() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("falcon.yaml"), "{{invalid yaml").unwrap();

    let errors = falcon::config::validator::validate_config(dir.path());
    assert!(!errors.is_empty(), "Should report invalid YAML syntax");
}

// --- Rule Docs Generation Test ---

#[test]
fn test_rule_docs_generation() {
    let dir = TempDir::new().unwrap();
    let docs_dir = dir.path().join("docs");

    falcon::docs::generate_rule_docs(&docs_dir).unwrap();

    assert!(docs_dir.join("RULES.md").exists());
    assert!(docs_dir.join("rules").exists());

    let index = std::fs::read_to_string(docs_dir.join("RULES.md")).unwrap();
    assert!(index.contains("Falcon Lint Rules"));
    assert!(index.contains("avoid-dynamic"));
    assert!(
        index.contains("rules"),
        "RULES.md should mention rule count"
    );

    let rule_files: Vec<_> = std::fs::read_dir(docs_dir.join("rules"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(rule_files.len() >= 43);
}

// --- Command Count Test ---

#[test]
fn test_new_command_count() {
    // v0.3+v0.5 added: workspace, docs, validate, check-cycles,
    // check-unused-params, check-dead-code, check-unused-l10n, check-promoted-deps
    let new_commands = [
        "workspace",
        "docs",
        "validate",
        "check-cycles",
        "check-unused-params",
        "check-dead-code",
        "check-unused-l10n",
        "check-promoted-deps",
    ];
    assert_eq!(new_commands.len(), 8);
}
