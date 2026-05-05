// ─── Refactoring Simulation ─────────────────────────────────────────────────

#[test]
fn test_refactor_sim_setstate_to_riverpod() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("counter.dart"),
        r#"
class CounterPage extends StatefulWidget {
  @override
  State<CounterPage> createState() => _CounterPageState();
}
class _CounterPageState extends State<CounterPage> {
  int count = 0;
  void increment() { setState(() { count++; }); }
}
"#,
    )
    .unwrap();

    let impact = falcon::analysis::refactor_sim::simulate_refactor(
        tmp.path(),
        &falcon::analysis::refactor_sim::RefactorScenario::SetStateToRiverpod,
    )
    .unwrap();

    assert!(impact.files_affected >= 1);
    assert!(!impact.migration_steps.is_empty());
    assert!(impact
        .migration_steps
        .iter()
        .any(|s| s.contains("Riverpod") || s.contains("riverpod")));
    assert!(!impact
        .migration_steps
        .iter()
        .any(|s| s.contains("Remove setstate")));
}

#[test]
fn test_refactor_sim_clean_architecture() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();
    std::fs::write(lib.join("service.dart"), "class Service {}\n").unwrap();

    let impact = falcon::analysis::refactor_sim::simulate_refactor(
        tmp.path(),
        &falcon::analysis::refactor_sim::RefactorScenario::CleanArchitecture,
    )
    .unwrap();

    assert!(impact.files_affected >= 2);
    assert!(!impact.migration_steps.is_empty());
}

#[test]
fn test_refactor_sim_print_no_panic() {
    let impact = falcon::analysis::refactor_sim::RefactorImpact {
        scenario: "test".to_string(),
        files_affected: 5,
        estimated_changes: 20,
        complexity: "Medium".to_string(),
        estimated_hours: 5.0,
        affected_files: vec![],
        risks: vec!["risk1".to_string()],
        benefits: vec!["benefit1".to_string()],
        migration_steps: vec!["step1".to_string()],
    };
    falcon::analysis::refactor_sim::print_refactor_impact(&impact);
}

// ─── Vulnerability Radar ────────────────────────────────────────────────────

#[test]
fn test_vuln_scan_clean_code() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("app.dart"),
        "class App {\n  String greet() => 'hello';\n}\n",
    )
    .unwrap();

    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(tmp.path());
    assert!(findings.is_empty());
}

#[test]
fn test_vuln_scan_detects_insecure_storage() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("auth.dart"),
        "void save() {\n  SharedPreferences.setString('password', pw);\n}\n",
    )
    .unwrap();

    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(tmp.path());
    assert!(findings
        .iter()
        .any(|f| f.issue.rule == "vuln-insecure-storage"));
}

#[test]
fn test_vuln_scan_detects_cert_bypass() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("http.dart"),
        "client.badCertificateCallback = (cert, host, port) => true;\n",
    )
    .unwrap();

    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(tmp.path());
    assert!(findings
        .iter()
        .any(|f| f.issue.rule == "vuln-cert-pinning-bypass"));
}

#[test]
fn test_vuln_scan_skips_comments() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("app.dart"),
        "// SharedPreferences.setString('password', pw);\nclass App {}\n",
    )
    .unwrap();

    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(tmp.path());
    let storage = findings
        .iter()
        .filter(|f| f.issue.rule == "vuln-insecure-storage")
        .count();
    assert_eq!(storage, 0, "Should skip commented-out code");
}

// ─── Upgrade Check ──────────────────────────────────────────────────────────

#[test]
fn test_upgrade_check_clean_code() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("app.dart"),
        "class App {\n  Widget build() => Container();\n}\n",
    )
    .unwrap();

    let findings = falcon::analysis::upgrade_check::check_upgrade_compatibility(tmp.path());
    assert!(findings.is_empty());
}

#[test]
fn test_upgrade_check_detects_deprecated_text_theme() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("theme.dart"),
        "final style = textTheme.bodyText1;\nfinal sub = textTheme.subtitle1;\n",
    )
    .unwrap();

    let findings = falcon::analysis::upgrade_check::check_upgrade_compatibility(tmp.path());
    assert!(findings.len() >= 2);
    assert!(findings.iter().any(|f| f.migration.contains("bodyLarge")));
    assert!(findings.iter().any(|f| f.migration.contains("titleMedium")));
}

#[test]
fn test_upgrade_check_detects_willpopscope() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("page.dart"),
        "Widget build() {\n  return WillPopScope(\n    onWillPop: () async => true,\n  );\n}\n",
    )
    .unwrap();

    let findings = falcon::analysis::upgrade_check::check_upgrade_compatibility(tmp.path());
    assert!(findings
        .iter()
        .any(|f| f.issue.message.contains("WillPopScope") || f.migration.contains("PopScope")));
}

// ─── Platform Channels ──────────────────────────────────────────────────────

#[test]
fn test_platform_channels_no_native_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let issues = falcon::analysis::platform_channels::analyze_platform_channels(tmp.path());
    assert!(issues.is_empty());
}

#[test]
fn test_platform_channels_kotlin() {
    let tmp = tempfile::tempdir().unwrap();
    let kotlin_dir = tmp.path().join("android/app/src/main/kotlin");
    std::fs::create_dir_all(&kotlin_dir).unwrap();
    std::fs::write(
        kotlin_dir.join("MainActivity.kt"),
        "val channel = MethodChannel(\"myChannel\")\n",
    )
    .unwrap();

    let issues = falcon::analysis::platform_channels::analyze_platform_channels(tmp.path());
    assert!(issues.iter().any(|i| i.rule == "platform-channel-naming"));
}

// ─── Codegen Quality ────────────────────────────────────────────────────────

#[test]
fn test_codegen_no_generated_files() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();

    let report = falcon::analysis::codegen_quality::analyze_codegen(tmp.path());
    assert_eq!(report.total_generated_files, 0);
}

#[test]
fn test_codegen_detects_generated() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("model.g.dart"),
        "// GENERATED CODE - DO NOT MODIFY BY HAND\npart of 'model.dart';\nclass _$Model {}\n",
    )
    .unwrap();

    let report = falcon::analysis::codegen_quality::analyze_codegen(tmp.path());
    assert_eq!(report.total_generated_files, 1);
    assert!(report
        .generators_found
        .iter()
        .any(|g| g.contains("json_serializable") || g.contains("build_runner")));
}

#[test]
fn test_codegen_detects_stale() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("orphan.g.dart"),
        "// GENERATED CODE\nclass Orphan {}\n",
    )
    .unwrap();

    let report = falcon::analysis::codegen_quality::analyze_codegen(tmp.path());
    assert!(
        !report.stale_files.is_empty(),
        "Should detect stale generated file with no source"
    );
}

// ─── DevTools Bridge ────────────────────────────────────────────────────────

#[test]
fn test_devtools_clean_code() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("app.dart"),
        "class App {\n  String greet() => 'hi';\n}\n",
    )
    .unwrap();

    let report = falcon::analysis::devtools_bridge::analyze_performance(tmp.path());
    assert!(report.rebuild_issues.is_empty());
}

#[test]
fn test_devtools_detects_opacity_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("widget.dart"),
        "Widget build() {\n  return Opacity(\n    opacity: 0,\n    child: Text('hidden'),\n  );\n}\n"
    ).unwrap();

    let report = falcon::analysis::devtools_bridge::analyze_performance(tmp.path());
    assert!(report
        .render_issues
        .iter()
        .any(|i| i.rule == "perf-opacity-zero"));
}

#[test]
fn test_devtools_detects_listview_children() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("list.dart"),
        "Widget build() {\n  return ListView(\n    children: [\n      Text('1'),\n    ],\n  );\n}\n"
    ).unwrap();

    let report = falcon::analysis::devtools_bridge::analyze_performance(tmp.path());
    assert!(report
        .render_issues
        .iter()
        .any(|i| i.rule == "perf-unbounded-list"));
}

// ─── Test Generation ────────────────────────────────────────────────────────

#[test]
fn test_test_gen_generates_stubs() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("service.dart"),
        r#"
class UserService {
  Future<User> getUser(String id) async {
    return User(id: id);
  }
  void updateUser(User user) {
    // save
  }
}
"#,
    )
    .unwrap();

    let stubs = falcon::analysis::test_gen::generate_test_stubs(tmp.path());
    assert!(!stubs.is_empty());
    assert!(stubs[0].test_cases.len() >= 1);
}

#[test]
fn test_test_gen_render_file() {
    let stub = falcon::analysis::test_gen::TestStub {
        target_file: "lib/service.dart".to_string(),
        test_file: "test/service_test.dart".to_string(),
        class_name: "UserService".to_string(),
        test_cases: vec![falcon::analysis::test_gen::TestCase {
            name: "getUser works".to_string(),
            body: "    expect(true, true);".to_string(),
            category: falcon::analysis::test_gen::TestCategory::Unit,
        }],
    };

    let content = falcon::analysis::test_gen::render_test_file(&stub);
    assert!(content.contains("flutter_test"));
    assert!(content.contains("UserService"));
    assert!(content.contains("getUser works"));
}
