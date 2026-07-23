//! Focused coverage for the four `falcon::manage` subsystems.
//!
//! Lives alongside `tests/manage_tests.rs` (which we deliberately do NOT touch
//! to avoid merge friction). Each subsystem gets tests pinning down behaviour
//! that the existing suite leaves uncovered.

use std::fs;

// ── Helpers ────────────────────────────────────────────────────────────────

/// Minimal falcon.yaml so health/maintenance config loading succeeds quickly.
fn write_min_falcon_yaml(root: &std::path::Path) {
    fs::write(
        root.join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();
}

// ── health::generate_health_report ─────────────────────────────────────────

#[test]
fn health_grade_letter_is_one_of_a_through_f() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("main.dart"), "class App {}\n").unwrap();
    // Supply a matching test dir so test_coverage doesn't dominate.
    let tests = tmp.path().join("test");
    fs::create_dir_all(&tests).unwrap();
    fs::write(tests.join("main_test.dart"), "void main() {}\n").unwrap();
    write_min_falcon_yaml(tmp.path());

    let report = falcon::manage::health::generate_health_report(tmp.path()).unwrap();

    // Grade letter must be one of A..F.
    assert!(
        matches!(report.grade.as_str(), "A" | "B" | "C" | "D" | "F"),
        "unexpected grade letter: {}",
        report.grade
    );
    // overall_health is bounded.
    assert!(report.overall_health <= 100);
    // Project name derives from the directory name and is non-empty.
    assert!(!report.project_name.is_empty());
}

#[test]
fn health_report_is_deterministic_for_same_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("main.dart"), "class App { String x = '1'; }\n").unwrap();
    write_min_falcon_yaml(tmp.path());

    let a = falcon::manage::health::generate_health_report(tmp.path()).unwrap();
    let b = falcon::manage::health::generate_health_report(tmp.path()).unwrap();

    assert_eq!(a.overall_health, b.overall_health);
    assert_eq!(a.grade, b.grade);
    assert_eq!(a.dimensions.code_quality, b.dimensions.code_quality);
    assert_eq!(a.stats.dart_files, b.stats.dart_files);
}

#[test]
fn health_priority_ranks_are_sequential() {
    // Build a project that triggers low test coverage (no /test dir, many lib files).
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    for i in 0..10 {
        fs::write(
            lib.join(format!("f{}.dart", i)),
            "class F{} {}\n".replace("{}", &i.to_string()),
        )
        .unwrap();
    }
    write_min_falcon_yaml(tmp.path());

    let report = falcon::manage::health::generate_health_report(tmp.path()).unwrap();
    // If we got any priorities, their ranks must be 1, 2, 3... in order.
    for (idx, p) in report.top_priorities.iter().enumerate() {
        assert_eq!(p.rank, idx + 1, "priorities should be ranked 1..N in order");
    }
}

// ── deps::analyze_dependencies ─────────────────────────────────────────────

#[test]
fn deps_detects_path_dependency() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("pubspec.yaml"),
        r#"
name: test_app
dependencies:
  my_local:
    path: ../my_local
"#,
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("lib")).unwrap();
    fs::write(tmp.path().join("lib/main.dart"), "\n").unwrap();

    let report = falcon::manage::deps::analyze_dependencies(tmp.path()).unwrap();
    assert!(report.issues.iter().any(|i| matches!(
        i.issue_type,
        falcon::manage::deps::DepIssueType::PathDependency
    )));
}

#[test]
fn deps_flags_dependency_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("pubspec.yaml"),
        r#"
name: test_app
dependencies:
  http: ^1.0.0
dependency_overrides:
  http: 1.0.0
"#,
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("lib")).unwrap();
    fs::write(
        tmp.path().join("lib/main.dart"),
        "import 'package:http/http.dart';\n",
    )
    .unwrap();

    let report = falcon::manage::deps::analyze_dependencies(tmp.path()).unwrap();
    assert!(report.issues.iter().any(|i| matches!(
        i.issue_type,
        falcon::manage::deps::DepIssueType::OverrideDep
    )));
}

#[test]
fn deps_collects_dev_dependencies_skipping_built_ins() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("pubspec.yaml"),
        r#"
name: test_app
dependencies:
  http: ^1.0.0
dev_dependencies:
  flutter_test:
    sdk: flutter
  flutter_lints: ^4.0.0
  build_runner: ^2.0.0
"#,
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("lib")).unwrap();
    fs::write(
        tmp.path().join("lib/main.dart"),
        "import 'package:http/http.dart';\n",
    )
    .unwrap();

    let report = falcon::manage::deps::analyze_dependencies(tmp.path()).unwrap();
    let dev_names: Vec<&str> = report.dev_deps.iter().map(|d| d.name.as_str()).collect();
    // flutter_test and flutter_lints are filtered out; build_runner is kept.
    assert!(dev_names.contains(&"build_runner"));
    assert!(!dev_names.contains(&"flutter_test"));
    assert!(!dev_names.contains(&"flutter_lints"));
}

#[test]
fn deps_total_count_matches_direct_plus_dev() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("pubspec.yaml"),
        r#"
name: test_app
dependencies:
  http: ^1.0.0
  provider: ^6.0.0
dev_dependencies:
  build_runner: ^2.0.0
"#,
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("lib")).unwrap();
    fs::write(tmp.path().join("lib/main.dart"), "\n").unwrap();

    let report = falcon::manage::deps::analyze_dependencies(tmp.path()).unwrap();
    assert_eq!(
        report.total_deps,
        report.direct_deps.len() + report.dev_deps.len()
    );
}

// ── build_opt::analyze_build ────────────────────────────────────────────────

#[test]
fn build_large_assets_are_sorted_by_size_desc() {
    let tmp = tempfile::tempdir().unwrap();
    let assets = tmp.path().join("assets/images");
    fs::create_dir_all(&assets).unwrap();
    fs::write(assets.join("small.png"), vec![0u8; 250 * 1024]).unwrap();
    fs::write(assets.join("medium.png"), vec![0u8; 300 * 1024]).unwrap();
    fs::write(assets.join("large.png"), vec![0u8; 450 * 1024]).unwrap();

    let report = falcon::manage::build_opt::analyze_build(tmp.path()).unwrap();
    let sizes: Vec<u64> = report
        .asset_analysis
        .large_assets
        .iter()
        .map(|(_, size)| *size)
        .collect();
    let mut sorted = sizes.clone();
    sorted.sort_by(|a, b| b.cmp(a));

    assert_eq!(sizes, sorted, "large assets must be sorted by size desc");
}

// ── architect::analyze_architecture ────────────────────────────────────────

#[test]
fn arch_detects_clean_architecture_layer_violation() {
    // Domain layer importing from /data/ is a textbook clean-architecture violation.
    let tmp = tempfile::tempdir().unwrap();
    for d in &[
        "lib/domain/entities",
        "lib/data/repos",
        "lib/presentation/pages",
    ] {
        fs::create_dir_all(tmp.path().join(d)).unwrap();
    }
    fs::write(
        tmp.path().join("lib/domain/entities/user.dart"),
        // Domain importing from data — violation.
        "import 'package:test_app/data/repos/user_repo.dart';\nclass User {}\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("lib/data/repos/user_repo.dart"),
        "class UserRepo {}\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("lib/presentation/pages/home.dart"),
        "class HomePage {}\n",
    )
    .unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    assert_eq!(report.detected_pattern, "Clean Architecture");
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.violation_type == "layer-violation"),
        "expected layer-violation, got: {:?}",
        report.violations
    );
    // Compliance must drop below 100% when violations are present.
    assert!(report.layer_compliance < 100.0);
}

#[test]
fn arch_layer_compliance_is_100_on_clean_layout() {
    let tmp = tempfile::tempdir().unwrap();
    for d in &[
        "lib/domain/entities",
        "lib/data/repos",
        "lib/presentation/pages",
    ] {
        fs::create_dir_all(tmp.path().join(d)).unwrap();
    }
    fs::write(
        tmp.path().join("lib/domain/entities/user.dart"),
        "class User {}\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("lib/data/repos/user_repo.dart"),
        "class UserRepo {}\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("lib/presentation/pages/home.dart"),
        "class HomePage {}\n",
    )
    .unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    assert_eq!(report.detected_pattern, "Clean Architecture");
    assert!(
        report.violations.is_empty(),
        "violations: {:?}",
        report.violations
    );
    assert!((report.layer_compliance - 100.0).abs() < f64::EPSILON);
}

#[test]
fn arch_module_map_is_sorted_by_file_count_desc() {
    let tmp = tempfile::tempdir().unwrap();
    let big = tmp.path().join("lib/big");
    let small = tmp.path().join("lib/small");
    fs::create_dir_all(&big).unwrap();
    fs::create_dir_all(&small).unwrap();
    for i in 0..5 {
        fs::write(big.join(format!("f{}.dart", i)), "class X {}\n").unwrap();
    }
    fs::write(small.join("only.dart"), "class Y {}\n").unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    let counts: Vec<usize> = report.module_map.iter().map(|m| m.file_count).collect();
    let mut sorted = counts.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(
        counts, sorted,
        "module_map must be sorted by file_count desc"
    );
}

#[test]
fn arch_complexity_hotspots_are_sorted_by_lines_desc() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(
        lib.join("medium.dart"),
        (0..600)
            .map(|i| format!("// medium {}\n", i))
            .collect::<String>(),
    )
    .unwrap();
    fs::write(
        lib.join("large.dart"),
        (0..750)
            .map(|i| format!("// large {}\n", i))
            .collect::<String>(),
    )
    .unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    let lines: Vec<usize> = report.complexity_hotspots.iter().map(|h| h.lines).collect();
    let mut sorted = lines.clone();
    sorted.sort_by(|a, b| b.cmp(a));

    assert_eq!(
        lines, sorted,
        "complexity hotspots must be sorted by lines desc"
    );
}

#[test]
fn arch_feature_first_cross_feature_violation_is_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let feed = tmp.path().join("lib/features/feed");
    let profile = tmp.path().join("lib/features/profile");
    fs::create_dir_all(&feed).unwrap();
    fs::create_dir_all(&profile).unwrap();
    fs::write(
        feed.join("feed.dart"),
        "import 'package:test_app/features/profile/profile.dart';\nclass Feed {}\n",
    )
    .unwrap();
    fs::write(profile.join("profile.dart"), "class Profile {}\n").unwrap();

    let report = falcon::manage::architect::analyze_architecture(tmp.path()).unwrap();
    assert_eq!(report.detected_pattern, "Feature-First");
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.violation_type == "cross-feature"),
        "expected cross-feature violation, got: {:?}",
        report.violations
    );
}

// ── maintenance::analyze_maintenance ───────────────────────────────────────

#[test]
fn maintenance_tech_debt_is_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(
        lib.join("app.dart"),
        "class App {\n  String greet() => 'hi';\n}\n",
    )
    .unwrap();
    write_min_falcon_yaml(tmp.path());

    let a = falcon::manage::maintenance::analyze_maintenance(tmp.path()).unwrap();
    let b = falcon::manage::maintenance::analyze_maintenance(tmp.path()).unwrap();
    assert_eq!(a.tech_debt_score, b.tech_debt_score);
    assert_eq!(a.cleanup_tasks.len(), b.cleanup_tasks.len());
    assert_eq!(a.auto_fixable, b.auto_fixable);
    assert_eq!(a.manual_review, b.manual_review);
}

#[test]
fn maintenance_tech_debt_score_is_bounded_for_trivial_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    // Single, trivial file.
    fs::write(lib.join("a.dart"), "class A {}\n").unwrap();
    write_min_falcon_yaml(tmp.path());

    let report = falcon::manage::maintenance::analyze_maintenance(tmp.path()).unwrap();
    // tech_debt_score is bounded to [0, 100].
    assert!(report.tech_debt_score <= 100);
    // estimated_savings_lines is a non-negative usize by type; the dead-code
    // and unused-files savings calculations should not overflow this small case.
    assert!(report.estimated_savings_lines < 10_000);
}

#[test]
fn maintenance_auto_fixable_plus_manual_equals_total_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("a.dart"), "class A {}\n").unwrap();
    write_min_falcon_yaml(tmp.path());

    let report = falcon::manage::maintenance::analyze_maintenance(tmp.path()).unwrap();
    // Invariant: every task is classified as exactly auto_fixable or manual.
    assert_eq!(
        report.cleanup_tasks.len(),
        report.auto_fixable + report.manual_review
    );
}
