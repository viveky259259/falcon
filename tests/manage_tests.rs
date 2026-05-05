// ─── Manage: Health Dashboard ────────────────────────────────────────────────

#[test]
fn test_health_report_clean_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "class App {}\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let report = falcon::manage::health::generate_health_report(tmp.path()).unwrap();
    assert!(report.overall_health <= 100);
    assert!(!report.grade.is_empty());
    assert!(report.stats.dart_files >= 1);
}

#[test]
fn test_health_report_dimensions_bounded() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("a.dart"), "void main() { print('hi'); }\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let report = falcon::manage::health::generate_health_report(tmp.path()).unwrap();
    assert!(report.dimensions.code_quality <= 100);
    assert!(report.dimensions.security <= 100);
    assert!(report.dimensions.performance <= 100);
    assert!(report.dimensions.maintainability <= 100);
    assert!(report.dimensions.test_coverage <= 100);
    assert!(report.dimensions.dependency_health <= 100);
}

#[test]
fn test_health_print_no_panic() {
    let report = falcon::manage::health::HealthReport {
        project_name: "test".to_string(),
        overall_health: 75,
        grade: "C".to_string(),
        dimensions: falcon::manage::health::HealthDimensions {
            code_quality: 70,
            security: 100,
            performance: 80,
            maintainability: 60,
            test_coverage: 50,
            dependency_health: 90,
        },
        top_priorities: vec![],
        stats: falcon::manage::health::ProjectStats {
            dart_files: 10,
            total_lines: 1000,
            total_issues: 50,
            error_count: 2,
            warning_count: 48,
            unused_files: 1,
            generated_files: 0,
        },
    };
    falcon::manage::health::print_health_report(&report);
}

// ─── Manage: Dependencies ───────────────────────────────────────────────────

#[test]
fn test_dep_analysis_no_pubspec() {
    let tmp = tempfile::tempdir().unwrap();
    let result = falcon::manage::deps::analyze_dependencies(tmp.path());
    assert!(result.is_err());
}

#[test]
fn test_dep_analysis_simple_pubspec() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("pubspec.yaml"),
        r#"
name: test_app
dependencies:
  http: ^1.0.0
  flutter:
    sdk: flutter
dev_dependencies:
  flutter_test:
    sdk: flutter
"#,
    )
    .unwrap();

    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("main.dart"),
        "import 'package:http/http.dart';\nvoid main() {}\n",
    )
    .unwrap();

    let report = falcon::manage::deps::analyze_dependencies(tmp.path()).unwrap();
    assert!(report.direct_deps.iter().any(|d| d.name == "http"));
    assert!(
        report
            .direct_deps
            .iter()
            .find(|d| d.name == "http")
            .unwrap()
            .is_used
    );
}

#[test]
fn test_dep_analysis_detects_unused() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("pubspec.yaml"),
        r#"
name: test_app
dependencies:
  http: ^1.0.0
  unused_pkg: ^2.0.0
"#,
    )
    .unwrap();

    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "import 'package:http/http.dart';\n").unwrap();

    let report = falcon::manage::deps::analyze_dependencies(tmp.path()).unwrap();
    assert!(report.unused.contains(&"unused_pkg".to_string()));
}

#[test]
fn test_dep_analysis_detects_git_dep() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("pubspec.yaml"),
        r#"
name: test_app
dependencies:
  my_pkg:
    git:
      url: https://github.com/example/my_pkg
"#,
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("lib")).unwrap();
    std::fs::write(tmp.path().join("lib/main.dart"), "\n").unwrap();

    let report = falcon::manage::deps::analyze_dependencies(tmp.path()).unwrap();
    assert!(report.issues.iter().any(|i| matches!(
        i.issue_type,
        falcon::manage::deps::DepIssueType::GitDependency
    )));
}

// ─── Manage: Architecture ───────────────────────────────────────────────────

#[test]
fn test_arch_flat_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    assert_eq!(report.detected_pattern, "Flat/Custom");
    assert!(report
        .suggestions
        .iter()
        .any(|s| s.contains("Clean Architecture") || s.contains("Feature-First")));
}

#[test]
fn test_arch_clean_architecture_detected() {
    let tmp = tempfile::tempdir().unwrap();
    for dir in &[
        "lib/domain/entities",
        "lib/data/repos",
        "lib/presentation/pages",
    ] {
        std::fs::create_dir_all(tmp.path().join(dir)).unwrap();
    }
    std::fs::write(
        tmp.path().join("lib/domain/entities/user.dart"),
        "class User {}\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("lib/data/repos/user_repo.dart"),
        "class UserRepo {}\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("lib/presentation/pages/home.dart"),
        "class HomePage {}\n",
    )
    .unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    assert_eq!(report.detected_pattern, "Clean Architecture");
}

#[test]
fn test_arch_hotspot_detection() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    let long_file: String = (0..600).map(|i| format!("// line {}\n", i)).collect();
    std::fs::write(lib.join("big_file.dart"), &long_file).unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    assert!(
        !report.complexity_hotspots.is_empty(),
        "Should detect files >500 lines"
    );
}

// ─── Manage: Maintenance ────────────────────────────────────────────────────

#[test]
fn test_maintenance_clean_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("app.dart"),
        "class App {\n  String greet() => 'hi';\n}\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let report = falcon::manage::maintenance::analyze_maintenance(tmp.path()).unwrap();
    assert!(report.tech_debt_score <= 100);
}

#[test]
fn test_maintenance_print_no_panic() {
    let report = falcon::manage::maintenance::MaintenanceReport {
        cleanup_tasks: vec![],
        auto_fixable: 0,
        manual_review: 0,
        estimated_savings_lines: 0,
        tech_debt_score: 85,
    };
    falcon::manage::maintenance::print_maintenance_report(&report);
}

// ─── Manage: Build Optimizer ────────────────────────────────────────────────

#[test]
fn test_build_optimizer_no_assets() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("pubspec.yaml"),
        "name: test\nflutter:\n  uses-material-design: true\n",
    )
    .unwrap();

    let report = falcon::manage::build_opt::analyze_build(tmp.path()).unwrap();
    assert_eq!(report.asset_analysis.total_assets, 0);
    assert!(report.pubspec_analysis.has_tree_shake_icons);
}

#[test]
fn test_build_optimizer_detects_large_assets() {
    let tmp = tempfile::tempdir().unwrap();
    let assets = tmp.path().join("assets/images");
    std::fs::create_dir_all(&assets).unwrap();

    let large_data = vec![0u8; 300 * 1024];
    std::fs::write(assets.join("hero.png"), &large_data).unwrap();

    std::fs::write(tmp.path().join("pubspec.yaml"), "name: test\n").unwrap();

    let report = falcon::manage::build_opt::analyze_build(tmp.path()).unwrap();
    assert!(!report.asset_analysis.large_assets.is_empty());
    assert_eq!(report.asset_analysis.image_count, 1);
}

#[test]
fn test_build_optimizer_platform_count() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("android")).unwrap();
    std::fs::create_dir_all(tmp.path().join("ios")).unwrap();
    std::fs::create_dir_all(tmp.path().join("web")).unwrap();
    std::fs::write(tmp.path().join("pubspec.yaml"), "name: test\n").unwrap();

    let report = falcon::manage::build_opt::analyze_build(tmp.path()).unwrap();
    assert_eq!(report.pubspec_analysis.platform_count, 3);
}
