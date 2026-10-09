use serde_json::Value;
use std::{fs, process::Command};

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("lib")).unwrap();
    fs::write(
        root.path().join("falcon.yaml"),
        "rules: []\nunused:\n  enabled: false\nexclude: [lib/ignored.dart]\n",
    )
    .unwrap();
    fs::write(root.path().join("pubspec.yaml"), "name: sample\n").unwrap();
    fs::write(
        root.path().join("lib/base.dart"),
        "class Base {}\nmixin Feature {}\nabstract class Contract {}\nvoid helper() {}\n",
    )
    .unwrap();
    fs::write(root.path().join("lib/main.dart"), "import 'package:sample/base.dart';\nimport 'dart:async';\nclass Screen extends Base with Feature implements Contract {\n void run() { helper(); }\n}\nvoid main() { helper(); }\n").unwrap();
    root
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_falcon"))
}
fn graph(root: &std::path::Path) -> Value {
    let output = cli().arg("graph").arg(root).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn nodes(g: &Value) -> &[Value] {
    g["nodes"].as_array().unwrap()
}
fn edges(g: &Value) -> &[Value] {
    g["edges"].as_array().unwrap()
}
fn has_relation(g: &Value, kind: &str, from: &str, to: &str) -> bool {
    let labels = |id: &Value| {
        nodes(g).iter().find(|n| n["id"] == *id).unwrap()["name"]
            .as_str()
            .unwrap()
    };
    edges(g)
        .iter()
        .any(|e| e["kind"] == kind && labels(&e["from"]) == from && labels(&e["to"]) == to)
}
#[test]
fn check_automatically_saves_graph_without_polluting_json() {
    let root = project();
    let out = cli()
        .arg("check")
        .arg(root.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let _: Value = serde_json::from_slice(&out.stdout).unwrap();
    let saved: Value = serde_json::from_slice(
        &fs::read(root.path().join(".falcon-data/project-graph.json"))
            .expect("graph saved by check"),
    )
    .unwrap();
    assert_eq!(saved["schema_version"], 1);
    assert!(has_relation(&saved, "extends", "Screen", "Base"));
}
#[test]
fn graph_exports_source_relationships_and_provenance() {
    let root = project();
    let g = graph(root.path());
    assert!(has_relation(
        &g,
        "imports",
        "lib/main.dart",
        "lib/base.dart"
    ));
    assert!(has_relation(&g, "extends", "Screen", "Base"));
    assert!(has_relation(&g, "mixes_in", "Screen", "Feature"));
    assert!(has_relation(&g, "implements", "Screen", "Contract"));
    assert!(has_relation(&g, "contains", "Screen", "run"));
    assert!(has_relation(&g, "calls", "main", "helper"));
    assert!(has_relation(&g, "calls", "run", "helper"));
    let class = nodes(&g).iter().find(|n| n["name"] == "Screen").unwrap();
    assert_eq!(class["span"]["line"], 3);
    assert_eq!(class["path"], "lib/main.dart");
    assert_eq!(class["source_digest"].as_str().unwrap().len(), 64);
    assert!(g["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["target"] == "dart:async" && d["kind"] == "external_dependency"));
    assert!(
        !root.path().join(".falcon-data").exists(),
        "stdout export is read-only"
    );
}
#[test]
fn graph_bytes_are_deterministic_and_checkout_independent() {
    let a = project();
    let b = project();
    let export = |path: &std::path::Path| cli().arg("graph").arg(path).output().unwrap();
    let first = export(a.path());
    assert!(first.status.success());
    assert_eq!(first.stdout, export(a.path()).stdout);
    assert_eq!(first.stdout, export(b.path()).stdout);
    let old: Value = serde_json::from_slice(&first.stdout).unwrap();
    fs::write(
        a.path().join("lib/base.dart"),
        "\nclass Base {}\nmixin Feature {}\nabstract class Contract {}\nvoid helper() {}\n",
    )
    .unwrap();
    let changed = graph(a.path());
    assert_ne!(old["snapshot_digest"], changed["snapshot_digest"]);
    let base_id = |g: &Value| nodes(g).iter().find(|n| n["name"] == "Base").unwrap()["id"].clone();
    assert_eq!(base_id(&old), base_id(&changed));
}
#[test]
fn ambiguous_and_unimported_types_are_not_linked() {
    let root = project();
    fs::write(
        root.path().join("lib/other.dart"),
        "class Base {}\nclass Hidden {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("lib/main.dart"),
        "class Screen extends Base {}\nclass Other extends Hidden {}\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(!edges(&g).iter().any(|e| e["kind"] == "extends"));
    assert!(g["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["target"] == "Base"));
}
#[test]
fn import_scope_resolves_duplicate_names_without_guessing() {
    let root = project();
    fs::write(
        root.path().join("lib/other.dart"),
        "class Base {}\nvoid helper() {}\n",
    )
    .unwrap();
    let g = graph(root.path());
    let edge = edges(&g).iter().find(|e| e["kind"] == "extends").unwrap();
    let base = nodes(&g).iter().find(|n| n["id"] == edge["to"]).unwrap();
    assert_eq!(base["path"], "lib/base.dart");
    assert!(has_relation(&g, "calls", "main", "helper"));
}
#[test]
fn syntax_errors_exclusions_and_noncode_text_are_explicit() {
    let root = project();
    fs::write(root.path().join("lib/ignored.dart"), "class Ignored {}\n").unwrap();
    fs::write(
        root.path().join("lib/main.dart"),
        "// import 'other.dart';\nvoid main() { final text = 'helper()'; }\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(!nodes(&g).iter().any(|n| n["name"] == "Ignored"));
    assert!(!edges(&g)
        .iter()
        .any(|e| e["kind"] == "calls" || e["kind"] == "imports"));
    fs::write(root.path().join("lib/main.dart"), "class Broken {\n").unwrap();
    let broken = graph(root.path());
    assert!(broken["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["kind"] == "syntax_error"));
}
#[test]
fn opt_out_and_custom_output_respect_cli_contracts() {
    let root = project();
    let out = cli()
        .arg("check")
        .arg(root.path())
        .arg("--no-graph")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!root.path().join(".falcon-data/project-graph.json").exists());
    let path = root.path().join("custom.json");
    assert!(cli()
        .arg("check")
        .arg(root.path())
        .arg("--graph-output")
        .arg(&path)
        .output()
        .unwrap()
        .status
        .success());
    let _: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let out = cli()
        .arg("graph")
        .arg(root.path())
        .arg("--output")
        .arg(root.path().join("missing/out.json"))
        .output()
        .unwrap();
    assert!(!out.status.success());
}
#[test]
fn failed_input_does_not_replace_last_graph() {
    let root = project();
    assert!(cli()
        .arg("check")
        .arg(root.path())
        .output()
        .unwrap()
        .status
        .success());
    let path = root.path().join(".falcon-data/project-graph.json");
    let previous = fs::read(&path).unwrap();
    fs::write(root.path().join("lib/main.dart"), [0xff]).unwrap();
    assert!(!cli()
        .arg("check")
        .arg(root.path())
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(previous, fs::read(path).unwrap());
    assert!(!cli()
        .arg("graph")
        .arg(root.path().join("absent"))
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn callback_parameters_shadow_imported_functions() {
    let root = project();
    fs::write(
        root.path().join("lib/main.dart"),
        "import 'base.dart';\nvoid main(void Function() helper) { helper(); }\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(!edges(&g).iter().any(|e| e["kind"] == "calls"));
    assert!(g["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["kind"] == "unresolved_symbol" && d["target"] == "helper"));
}
#[test]
fn prefix_visibility_and_generic_arguments_are_not_guessed() {
    let root = project();
    fs::write(root.path().join("lib/main.dart"), "import 'base.dart' as b show Base, Feature;\nclass Screen extends b.Base with b.Feature<Contract> implements Contract {}\n").unwrap();
    let g = graph(root.path());
    assert!(has_relation(&g, "extends", "Screen", "Base"));
    assert!(has_relation(&g, "mixes_in", "Screen", "Feature"));
    assert!(!edges(&g).iter().any(|e| e["kind"] == "implements"));
}
#[test]
fn exports_cycles_and_part_members_share_library_visibility() {
    let root = project();
    fs::write(
        root.path().join("lib/barrel.dart"),
        "export 'base.dart' show Base;\nexport 'main.dart';\n",
    )
    .unwrap();
    fs::write(root.path().join("lib/main.dart"), "import 'barrel.dart';\npart 'piece.dart';\nvoid helperLocal() {}\nclass Screen extends Base {}\n").unwrap();
    fs::write(
        root.path().join("lib/piece.dart"),
        "part of 'main.dart';\nvoid partFunction() { helperLocal(); }\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(has_relation(&g, "extends", "Screen", "Base"));
    assert!(has_relation(&g, "part", "lib/main.dart", "lib/piece.dart"));
    assert!(has_relation(&g, "calls", "partFunction", "helperLocal"));
}
#[test]
fn single_file_checks_preserve_the_project_artifact() {
    let root = project();
    assert!(cli()
        .arg("check")
        .arg(root.path())
        .output()
        .unwrap()
        .status
        .success());
    let path = root.path().join(".falcon-data/project-graph.json");
    let previous = fs::read(&path).unwrap();
    assert!(cli()
        .arg("check")
        .arg(root.path().join("lib/main.dart"))
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(previous, fs::read(path).unwrap());
    let sdk = falcon::sdk::FalconSdk::new()
        .generate_project_graph(root.path().to_str().unwrap())
        .unwrap();
    assert_eq!(sdk.schema_version, 1);
}
#[test]
fn incremental_check_retains_a_complete_graph() {
    let root = project();
    for args in [
        vec!["init"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.test",
            "commit",
            "-m",
            "fixture",
        ],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(root.path())
            .output()
            .unwrap()
            .status
            .success());
    }
    fs::write(root.path().join("lib/main.dart"), "void main() {}\n").unwrap();
    let out = cli()
        .current_dir(root.path())
        .args(["check", ".", "--since", "HEAD", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let saved: Value = serde_json::from_slice(
        &fs::read(root.path().join(".falcon-data/project-graph.json")).unwrap(),
    )
    .unwrap();
    assert!(nodes(&saved).iter().any(|n| n["name"] == "Base"));
    assert_eq!(
        nodes(&saved).iter().filter(|n| n["kind"] == "file").count(),
        2
    );
}

#[test]
fn conditional_imports_do_not_assert_selected_symbols() {
    let root = project();
    fs::write(root.path().join("lib/other.dart"), "class Base {}\n").unwrap();
    fs::write(
        root.path().join("lib/main.dart"),
        "import 'base.dart' if (dart.library.io) 'other.dart';\nclass Screen extends Base {}\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(!edges(&g).iter().any(|e| e["kind"] == "extends"));
    assert!(g["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["kind"] == "conditional_dependency"));
}
#[test]
fn generic_type_parameters_shadow_imported_types() {
    let root = project();
    fs::write(
        root.path().join("lib/main.dart"),
        "import 'base.dart';\nclass Screen<Base> implements Base {}\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(!edges(&g).iter().any(|e| e["kind"] == "implements"));
}
#[test]
fn duplicate_imports_report_ambiguity_and_repeated_show_intersects() {
    let root = project();
    fs::write(root.path().join("lib/other.dart"), "class Base {}\n").unwrap();
    fs::write(
        root.path().join("lib/main.dart"),
        "import 'base.dart';\nimport 'other.dart';\nclass Screen extends Base {}\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(!edges(&g).iter().any(|e| e["kind"] == "extends"));
    assert!(g["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["kind"] == "ambiguous_symbol" && d["target"] == "Base"));
    fs::write(
        root.path().join("lib/main.dart"),
        "import 'base.dart' show Base show Feature;\nclass Screen extends Base {}\n",
    )
    .unwrap();
    assert!(!edges(&graph(root.path()))
        .iter()
        .any(|e| e["kind"] == "extends"));
}

#[test]
fn exports_do_not_import_names_into_the_exporting_file() {
    let root = project();
    fs::write(
        root.path().join("lib/main.dart"),
        "export 'base.dart';\nclass Screen extends Base {}\n",
    )
    .unwrap();
    assert!(!edges(&graph(root.path()))
        .iter()
        .any(|e| e["kind"] == "extends"));
}
#[test]
fn parts_inherit_imports_and_duplicate_library_declarations_are_ambiguous() {
    let root = project();
    fs::write(
        root.path().join("lib/main.dart"),
        "import 'base.dart';\npart 'piece.dart';\n",
    )
    .unwrap();
    fs::write(
        root.path().join("lib/piece.dart"),
        "part of 'main.dart';\nclass Screen extends Base {}\n",
    )
    .unwrap();
    assert!(has_relation(
        &graph(root.path()),
        "extends",
        "Screen",
        "Base"
    ));
    fs::write(
        root.path().join("lib/main.dart"),
        "part 'piece.dart';\nclass Base {}\nclass Screen extends Base {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("lib/piece.dart"),
        "part of 'main.dart';\nclass Base {}\n",
    )
    .unwrap();
    let g = graph(root.path());
    assert!(!edges(&g).iter().any(|e| e["kind"] == "extends"));
    assert!(g["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["kind"] == "ambiguous_symbol"));
}
