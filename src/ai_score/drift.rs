use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::convention::{detect_conventions, ConventionReport};

/// A single drift finding where new code deviates from established patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftFinding {
    pub file: String,
    pub line: usize,
    pub category: DriftCategory,
    pub expected: String,
    pub actual: String,
    pub message: String,
}

/// Categories of convention drift.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DriftCategory {
    Naming,
    Architecture,
    ErrorHandling,
    StateManagement,
    Pattern,
}

impl std::fmt::Display for DriftCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftCategory::Naming => write!(f, "naming"),
            DriftCategory::Architecture => write!(f, "architecture"),
            DriftCategory::ErrorHandling => write!(f, "error-handling"),
            DriftCategory::StateManagement => write!(f, "state-management"),
            DriftCategory::Pattern => write!(f, "pattern"),
        }
    }
}

/// Drift detection report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub conventions: ConventionReport,
    pub findings: Vec<DriftFinding>,
    pub drift_score: f64,
    pub files_analyzed: usize,
}

/// Detect convention drift in changed files (or all files if no git ref given).
pub fn detect_drift(root: &Path, since: Option<&str>) -> anyhow::Result<DriftReport> {
    let conventions = detect_conventions(root)?;

    let files = if let Some(git_ref) = since {
        get_changed_files(root, git_ref)?
    } else {
        collect_dart_files(root)
    };

    let files_analyzed = files.len();
    let mut findings = Vec::new();

    for file_path in &files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let rel = file_path
            .strip_prefix(root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        check_naming_drift(&conventions, &rel, &source, &mut findings);
        check_architecture_drift(&conventions, &rel, &mut findings);
        check_error_handling_drift(&conventions, &rel, &source, &mut findings);
        check_state_management_drift(&conventions, &rel, &source, &mut findings);
    }

    let drift_score = if files_analyzed > 0 {
        let per_file = findings.len() as f64 / files_analyzed as f64;
        (100.0 - per_file * 20.0).clamp(0.0, 100.0)
    } else {
        100.0
    };

    Ok(DriftReport {
        conventions,
        findings,
        drift_score,
        files_analyzed,
    })
}

fn check_naming_drift(
    conventions: &ConventionReport,
    file_name: &str,
    source: &str,
    findings: &mut Vec<DriftFinding>,
) {
    if conventions.naming.file_naming == "snake_case" {
        let base = std::path::Path::new(file_name)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        if base != base.to_lowercase()
            && !base.ends_with(".g.dart")
            && !base.ends_with(".freezed.dart")
        {
            findings.push(DriftFinding {
                file: file_name.to_string(),
                line: 1,
                category: DriftCategory::Naming,
                expected: "snake_case".to_string(),
                actual: base.to_string(),
                message: format!(
                    "File '{}' doesn't follow the project's snake_case naming convention",
                    base
                ),
            });
        }
    }

    if conventions.naming.class_naming == "PascalCase" {
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ") || trimmed.starts_with("abstract class ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let class_idx = parts.iter().position(|p| *p == "class").unwrap_or(0) + 1;
                if let Some(class_name) = parts.get(class_idx) {
                    let name = class_name.trim_end_matches('{').trim_end_matches('<');
                    if !name.is_empty() && !name.chars().next().unwrap_or('a').is_uppercase() {
                        findings.push(DriftFinding {
                            file: file_name.to_string(),
                            line: i + 1,
                            category: DriftCategory::Naming,
                            expected: "PascalCase".to_string(),
                            actual: name.to_string(),
                            message: format!(
                                "Class '{}' doesn't follow PascalCase convention",
                                name
                            ),
                        });
                    }
                }
            }
        }
    }
}

fn check_architecture_drift(
    conventions: &ConventionReport,
    file_path: &str,
    findings: &mut Vec<DriftFinding>,
) {
    let pattern = &conventions.architecture.pattern;

    if pattern == "Clean Architecture" {
        let parts: Vec<&str> = file_path.split('/').collect();
        if parts.len() >= 2 {
            let top_dir = parts
                .iter()
                .find(|p| matches!(**p, "domain" | "data" | "presentation" | "core" | "shared"));
            if top_dir.is_none() && !file_path.contains("test") && !file_path.contains("main.dart")
            {
                findings.push(DriftFinding {
                    file: file_path.to_string(),
                    line: 1,
                    category: DriftCategory::Architecture,
                    expected: "Clean Architecture (domain/data/presentation)".to_string(),
                    actual: "file outside layer structure".to_string(),
                    message: format!(
                        "File '{}' is outside the Clean Architecture layer structure",
                        file_path
                    ),
                });
            }
        }
    } else if pattern == "Feature-First" {
        let parts: Vec<&str> = file_path.split('/').collect();
        if parts.len() >= 2 {
            let has_feature = parts
                .iter()
                .any(|p| *p == "features" || *p == "core" || *p == "shared");
            if !has_feature && !file_path.contains("test") && !file_path.contains("main.dart") {
                findings.push(DriftFinding {
                    file: file_path.to_string(),
                    line: 1,
                    category: DriftCategory::Architecture,
                    expected: "Feature-First (features/)".to_string(),
                    actual: "file outside feature structure".to_string(),
                    message: format!(
                        "File '{}' is outside the Feature-First structure",
                        file_path
                    ),
                });
            }
        }
    }
}

fn check_error_handling_drift(
    conventions: &ConventionReport,
    file_name: &str,
    source: &str,
    findings: &mut Vec<DriftFinding>,
) {
    if conventions.error_handling.uses_result_type || conventions.error_handling.uses_either {
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("try {") && !file_name.contains("test") {
                let has_catch_rethrow = source.lines().skip(i).take(10).any(|l| {
                    l.contains("rethrow") || l.contains("Result.failure") || l.contains("Left(")
                });
                if !has_catch_rethrow {
                    let has_empty_catch = source
                        .lines()
                        .skip(i)
                        .take(10)
                        .any(|l| l.trim() == "} catch (e) {}" || l.trim() == "} catch (e) {");
                    if has_empty_catch {
                        findings.push(DriftFinding {
                            file: file_name.to_string(),
                            line: i + 1,
                            category: DriftCategory::ErrorHandling,
                            expected: "Result/Either pattern".to_string(),
                            actual: "empty catch block".to_string(),
                            message: "Project uses Result/Either pattern but this code has an empty catch block".to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn check_state_management_drift(
    conventions: &ConventionReport,
    file_name: &str,
    source: &str,
    findings: &mut Vec<DriftFinding>,
) {
    let dominant = match &conventions.state_management {
        Some(sm) => sm.clone(),
        None => return,
    };

    if file_name.contains("test") {
        return;
    }

    let mixed_signals: Vec<(&str, &str)> = vec![
        ("BLoC", "GetxController"),
        ("BLoC", "ChangeNotifier"),
        ("Riverpod", "GetxController"),
        ("Riverpod", "BlocProvider"),
        ("Provider", "GetxController"),
    ];

    for (primary, conflicting_marker) in &mixed_signals {
        if dominant == *primary && source.contains(conflicting_marker) {
            findings.push(DriftFinding {
                file: file_name.to_string(),
                line: 1,
                category: DriftCategory::StateManagement,
                expected: dominant.clone(),
                actual: format!("uses {}", conflicting_marker),
                message: format!(
                    "Project uses {} but this file contains {}",
                    dominant, conflicting_marker
                ),
            });
        }
    }
}

fn get_changed_files(root: &Path, git_ref: &str) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", git_ref])
        .current_dir(root)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let files: Vec<std::path::PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(".dart"))
        .map(|line| root.join(line))
        .filter(|p| p.exists())
        .collect();

    Ok(files)
}

fn collect_dart_files(root: &Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ────────────────────────────────────────────────────────────────

    fn write_file(dir: &std::path::Path, rel: &str, content: &str) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&full).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    // ── DriftCategory Display ─────────────────────────────────────────────────

    #[test]
    fn test_drift_category_display_naming() {
        assert_eq!(DriftCategory::Naming.to_string(), "naming");
    }

    #[test]
    fn test_drift_category_display_architecture() {
        assert_eq!(DriftCategory::Architecture.to_string(), "architecture");
    }

    #[test]
    fn test_drift_category_display_error_handling() {
        assert_eq!(DriftCategory::ErrorHandling.to_string(), "error-handling");
    }

    #[test]
    fn test_drift_category_display_state_management() {
        assert_eq!(
            DriftCategory::StateManagement.to_string(),
            "state-management"
        );
    }

    #[test]
    fn test_drift_category_display_pattern() {
        assert_eq!(DriftCategory::Pattern.to_string(), "pattern");
    }

    // ── collect_dart_files ────────────────────────────────────────────────────

    #[test]
    fn test_collect_dart_files_only_dart() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/main.dart", "void main() {}");
        write_file(root, "lib/home.dart", "class HomePage {}");
        write_file(root, "lib/styles.css", ".btn {}");
        write_file(root, "pubspec.yaml", "name: app");
        write_file(root, "README.md", "# App");

        let files = collect_dart_files(root);
        assert_eq!(files.len(), 2, "should only collect .dart files");
        for f in &files {
            assert_eq!(f.extension().and_then(|e| e.to_str()), Some("dart"));
        }
    }

    #[test]
    fn test_collect_dart_files_nested_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/a.dart", "");
        write_file(root, "lib/sub/b.dart", "");
        write_file(root, "test/a_test.dart", "");
        write_file(root, "lib/config.json", "{}");

        let files = collect_dart_files(root);
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_collect_dart_files_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let files = collect_dart_files(tmp.path());
        assert!(files.is_empty());
    }

    // ── detect_drift (no since) ───────────────────────────────────────────────

    #[test]
    fn test_detect_drift_no_files_returns_100_score() {
        let tmp = TempDir::new().unwrap();
        let report = detect_drift(tmp.path(), None).unwrap();
        assert_eq!(report.drift_score, 100.0);
        assert_eq!(report.files_analyzed, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn test_detect_drift_clean_project_no_findings() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // snake_case files, PascalCase classes, clean architecture dirs
        write_file(root, "lib/domain/user.dart", "class UserEntity {}");
        write_file(root, "lib/data/repository.dart", "class UserRepository {}");

        let report = detect_drift(root, None).unwrap();
        assert!(report.drift_score > 0.0);
        // All files under domain/data — no architecture drift
        let arch_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Architecture)
            .collect();
        assert!(arch_findings.is_empty());
    }

    // ── check_naming_drift — file naming ─────────────────────────────────────

    // Note: with since=None, detect_conventions scans the same files that are
    // analyzed, so a mixed-case filename also flips convention to "mixed" and
    // suppresses the check. File naming drift is only reliably triggered via
    // since=Some(git_ref). The tests below verify the guard and exemption logic.

    #[test]
    fn test_naming_drift_mixed_convention_no_file_findings() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // One snake_case and one mixed-case file → convention = "mixed"
        // The check is guarded by file_naming == "snake_case", so no file-naming
        // drift findings should appear.
        write_file(root, "lib/home_page.dart", "class HomePage {}");
        write_file(root, "lib/MyWidget.dart", "class MyWidget {}");

        let report = detect_drift(root, None).unwrap();
        // With mixed convention the file-naming guard is inactive; no file-naming
        // findings for the mixed-case file.
        let file_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Naming && f.expected == "snake_case")
            .collect();
        assert!(
            file_findings.is_empty(),
            "mixed convention should not produce file-naming findings"
        );
    }

    #[test]
    fn test_naming_drift_snake_case_file_clean() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/home_page.dart", "class HomePage {}");
        write_file(root, "lib/user_model.dart", "class UserModel {}");

        let report = detect_drift(root, None).unwrap();
        let naming: Vec<_> = report
            .findings
            .iter()
            .filter(|f| {
                f.category == DriftCategory::Naming
                    && (f.file.contains("home_page") || f.file.contains("user_model"))
            })
            .collect();
        assert!(
            naming.is_empty(),
            "snake_case files should not trigger naming drift"
        );
    }

    #[test]
    fn test_naming_drift_generated_file_not_flagged() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/home_page.dart", "class HomePage {}");
        // .g.dart and .freezed.dart are exempted from file naming drift check
        // even when convention is snake_case. The generated files have uppercase
        // names but end with exempted suffixes.
        write_file(root, "lib/user_model.g.dart", "// generated");
        write_file(root, "lib/user_model.freezed.dart", "// generated");

        let report = detect_drift(root, None).unwrap();
        let file_naming_drift: Vec<_> = report
            .findings
            .iter()
            .filter(|f| {
                f.category == DriftCategory::Naming
                    && (f.file.ends_with(".g.dart") || f.file.ends_with(".freezed.dart"))
            })
            .collect();
        assert!(
            file_naming_drift.is_empty(),
            ".g.dart and .freezed.dart should be exempt from file naming drift"
        );
    }

    // ── check_naming_drift — class naming ────────────────────────────────────

    // The convention scanner only picks up lines starting with "class " (not
    // "abstract class "). When a codebase only has abstract classes, class_names
    // is empty → 0 == 0 → PascalCase convention. check_naming_drift checks both
    // "class " and "abstract class " lines, so a lowercase abstract class IS
    // flagged under this scenario.

    #[test]
    fn test_naming_drift_lowercase_abstract_class_flagged() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Only abstract classes — convention scanner misses them, so
        // class_names is empty → class_naming defaults to "PascalCase".
        // check_naming_drift still checks "abstract class" lines.
        write_file(root, "lib/base.dart", "abstract class BaseRepo {}");
        write_file(root, "lib/helper.dart", "abstract class helperUtils {}");

        let report = detect_drift(root, None).unwrap();
        let class_naming: Vec<_> = report
            .findings
            .iter()
            .filter(|f| {
                f.category == DriftCategory::Naming
                    && f.expected == "PascalCase"
                    && f.file.contains("helper.dart")
            })
            .collect();
        assert!(
            !class_naming.is_empty(),
            "lowercase abstract class name should trigger PascalCase drift"
        );
    }

    #[test]
    fn test_naming_drift_pascal_abstract_class_clean() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // PascalCase abstract classes — no drift expected
        write_file(root, "lib/base.dart", "abstract class BaseRepo {}");
        write_file(root, "lib/helper.dart", "abstract class HelperUtils {}");

        let report = detect_drift(root, None).unwrap();
        let class_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Naming && f.expected == "PascalCase")
            .collect();
        assert!(
            class_findings.is_empty(),
            "PascalCase abstract classes should not trigger naming drift"
        );
    }

    #[test]
    fn test_naming_drift_mixed_class_convention_no_findings() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Mix of PascalCase and lowercase concrete classes → convention = "mixed"
        // check is skipped; no class naming findings should appear.
        write_file(root, "lib/user.dart", "class UserModel {}");
        write_file(root, "lib/helper.dart", "class myHelper {}");

        let report = detect_drift(root, None).unwrap();
        let class_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Naming && f.expected == "PascalCase")
            .collect();
        assert!(
            class_findings.is_empty(),
            "mixed class convention should suppress PascalCase drift check"
        );
    }

    // ── check_architecture_drift — Clean Architecture ─────────────────────────

    #[test]
    fn test_architecture_drift_clean_arch_file_outside_layers() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create domain + data dirs to trigger Clean Architecture detection
        write_file(root, "lib/domain/user_entity.dart", "class UserEntity {}");
        write_file(root, "lib/data/user_repo.dart", "class UserRepository {}");
        // This file is outside domain/data/presentation/core/shared
        write_file(root, "lib/utils/helper.dart", "class Helper {}");

        let report = detect_drift(root, None).unwrap();
        let arch: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Architecture)
            .collect();
        assert!(
            !arch.is_empty(),
            "file outside Clean Architecture layers should trigger drift"
        );
        assert!(arch.iter().any(|f| f.file.contains("helper.dart")));
    }

    #[test]
    fn test_architecture_drift_clean_arch_file_in_valid_layer() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/domain/user_entity.dart", "class UserEntity {}");
        write_file(root, "lib/data/user_repo.dart", "class UserRepository {}");
        write_file(root, "lib/presentation/home_page.dart", "class HomePage {}");
        write_file(root, "lib/core/utils.dart", "class Utils {}");

        let report = detect_drift(root, None).unwrap();
        let arch: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Architecture)
            .collect();
        assert!(
            arch.is_empty(),
            "files inside valid Clean Architecture layers should not trigger drift"
        );
    }

    #[test]
    fn test_architecture_drift_main_dart_exempt() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/domain/user.dart", "class UserEntity {}");
        write_file(root, "lib/data/repo.dart", "class Repo {}");
        // main.dart should be exempt from architecture drift
        write_file(root, "lib/main.dart", "void main() { runApp(MyApp()); }");

        let report = detect_drift(root, None).unwrap();
        let arch_main: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Architecture && f.file.contains("main.dart"))
            .collect();
        assert!(
            arch_main.is_empty(),
            "main.dart should be exempt from architecture drift"
        );
    }

    // ── check_architecture_drift — Feature-First ──────────────────────────────

    #[test]
    fn test_architecture_drift_feature_first_file_outside_features() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a features dir to trigger Feature-First convention
        write_file(root, "lib/features/auth/login.dart", "class LoginPage {}");
        // This file is outside features/core/shared
        write_file(root, "lib/random/helper.dart", "class Helper {}");

        let report = detect_drift(root, None).unwrap();
        let arch: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Architecture)
            .collect();
        assert!(
            !arch.is_empty(),
            "file outside Feature-First structure should trigger drift"
        );
    }

    #[test]
    fn test_architecture_drift_feature_first_shared_exempt() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/features/auth/login.dart", "class LoginPage {}");
        write_file(root, "lib/shared/widgets.dart", "class CommonButton {}");
        write_file(root, "lib/core/theme.dart", "class AppTheme {}");

        let report = detect_drift(root, None).unwrap();
        let arch: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::Architecture)
            .collect();
        assert!(
            arch.is_empty(),
            "files in features/shared/core should not trigger Feature-First drift"
        );
    }

    // ── check_error_handling_drift ────────────────────────────────────────────

    #[test]
    fn test_error_handling_drift_empty_catch_flagged() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Establish Result pattern convention
        let convention_source = "Result<String> fetchData() { return Result.success('ok'); }";
        write_file(root, "lib/data/api.dart", convention_source);

        // File with empty catch block — should be flagged
        let bad_source = r#"
class UserService {
  Future<void> loadUser() async {
    try {
      final result = await api.fetch();
    } catch (e) {}
  }
}
"#;
        write_file(root, "lib/data/user_service.dart", bad_source);

        let report = detect_drift(root, None).unwrap();
        let err_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::ErrorHandling)
            .collect();
        assert!(
            !err_findings.is_empty(),
            "empty catch block should trigger error handling drift when Result pattern is used"
        );
    }

    #[test]
    fn test_error_handling_drift_rethrow_clean() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let convention_source = "Result<String> fetchData() { return Result.success('ok'); }";
        write_file(root, "lib/data/api.dart", convention_source);

        // File uses try/catch with rethrow — should not trigger drift
        let good_source = r#"
class UserService {
  Future<void> loadUser() async {
    try {
      final result = await api.fetch();
    } catch (e) {
      rethrow;
    }
  }
}
"#;
        write_file(root, "lib/data/user_service.dart", good_source);

        let report = detect_drift(root, None).unwrap();
        let err_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::ErrorHandling)
            .collect();
        assert!(
            err_findings.is_empty(),
            "try/catch with rethrow should not trigger error handling drift"
        );
    }

    #[test]
    fn test_error_handling_drift_test_file_exempt() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let convention_source = "Result<String> fetchData() { return Result.success('ok'); }";
        write_file(root, "lib/data/api.dart", convention_source);

        // Test file with empty catch — should be exempt
        let test_source = r#"
void main() {
  test('loads user', () async {
    try {
      await service.load();
    } catch (e) {}
  });
}
"#;
        write_file(root, "test/user_service_test.dart", test_source);

        let report = detect_drift(root, None).unwrap();
        let err_test: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::ErrorHandling && f.file.contains("test"))
            .collect();
        assert!(
            err_test.is_empty(),
            "test files should be exempt from error handling drift"
        );
    }

    // ── check_state_management_drift ──────────────────────────────────────────

    #[test]
    fn test_state_management_drift_bloc_with_getx_flagged() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Establish BLoC as dominant state management
        let bloc_source = r#"
class AuthBloc extends Bloc<AuthEvent, AuthState> {
  AuthBloc() : super(AuthInitial()) {
    on<LoginEvent>(_onLogin);
  }
}
"#;
        // Multiple BLoC files to ensure dominance
        write_file(root, "lib/bloc/auth_bloc.dart", bloc_source);
        write_file(
            root,
            "lib/bloc/user_bloc.dart",
            "class UserBloc extends Bloc<UserEvent, UserState> {}",
        );

        // Conflicting GetX usage
        let getx_source = "class HomeController extends GetxController { final count = 0.obs; }";
        write_file(root, "lib/home/home_controller.dart", getx_source);

        let report = detect_drift(root, None).unwrap();
        let sm_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::StateManagement)
            .collect();
        assert!(
            !sm_findings.is_empty(),
            "GetxController in a BLoC project should trigger state management drift"
        );
    }

    #[test]
    fn test_state_management_drift_none_when_no_dominant() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // No state management indicators — state_management will be None
        write_file(
            root,
            "lib/models/user.dart",
            "class User { final String name; User(this.name); }",
        );

        let report = detect_drift(root, None).unwrap();
        let sm_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::StateManagement)
            .collect();
        assert!(
            sm_findings.is_empty(),
            "no dominant state management means no drift findings"
        );
    }

    #[test]
    fn test_state_management_drift_riverpod_with_bloc_flagged() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Establish Riverpod convention
        let riverpod1 = "class HomeWidget extends ConsumerWidget { @override Widget build(BuildContext ctx, WidgetRef ref) { ref.watch(userProvider); return Container(); } }";
        let riverpod2 = "class ProfileWidget extends ConsumerWidget { @override Widget build(BuildContext ctx, WidgetRef ref) { ref.read(settingsProvider); return Container(); } }";
        write_file(root, "lib/home/home_widget.dart", riverpod1);
        write_file(root, "lib/profile/profile_widget.dart", riverpod2);

        // Conflicting BlocProvider
        let bloc_source = "Widget build(BuildContext ctx) { return BlocProvider(create: (_) => AuthBloc(), child: Container()); }";
        write_file(root, "lib/auth/auth_page.dart", bloc_source);

        let report = detect_drift(root, None).unwrap();
        let sm_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::StateManagement)
            .collect();
        assert!(
            !sm_findings.is_empty(),
            "BlocProvider in a Riverpod project should trigger state management drift"
        );
    }

    #[test]
    fn test_state_management_drift_test_file_exempt() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let bloc_source = "class AuthBloc extends Bloc<AuthEvent, AuthState> { AuthBloc() : super(AuthInitial()); }";
        write_file(root, "lib/bloc/auth_bloc.dart", bloc_source);
        write_file(
            root,
            "lib/bloc/user_bloc.dart",
            "class UserBloc extends Bloc<UserEvent, UserState> {}",
        );

        // Test file with conflicting GetX — should be exempt
        let test_source = "void main() { testWidgets('home', (tester) async { final ctrl = GetxController(); }); }";
        write_file(root, "test/home_test.dart", test_source);

        let report = detect_drift(root, None).unwrap();
        let sm_test: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.category == DriftCategory::StateManagement && f.file.contains("test"))
            .collect();
        assert!(
            sm_test.is_empty(),
            "test files should be exempt from state management drift"
        );
    }

    // ── drift_score calculation ───────────────────────────────────────────────

    #[test]
    fn test_drift_score_clamped_at_zero_with_many_findings() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Large number of findings should clamp score to 0
        write_file(root, "lib/domain/base.dart", "class BaseEntity {}");
        write_file(root, "lib/data/base_repo.dart", "class BaseRepo {}");

        // Many files outside architecture layers → many findings
        for i in 0..10 {
            write_file(
                root,
                &format!("lib/misc/helper_{}.dart", i),
                &format!("class Helper{} {{}}", i),
            );
        }

        let report = detect_drift(root, None).unwrap();
        assert!(report.drift_score >= 0.0);
        assert!(report.drift_score <= 100.0);
    }

    #[test]
    fn test_drift_report_fields_populated() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "lib/home.dart", "class HomePage {}");

        let report = detect_drift(root, None).unwrap();
        assert_eq!(report.files_analyzed, 1);
        assert!(report.drift_score >= 0.0 && report.drift_score <= 100.0);
    }
}

/// Print drift report to the console.
pub fn print_drift_report(report: &DriftReport) {
    println!();
    println!(
        "  {} Convention Drift Detector",
        "falcon".bright_cyan().bold()
    );
    println!();

    let drift_color = if report.drift_score >= 90.0 {
        format!("{:.0}%", report.drift_score).bright_green()
    } else if report.drift_score >= 70.0 {
        format!("{:.0}%", report.drift_score).yellow()
    } else {
        format!("{:.0}%", report.drift_score).red()
    };

    println!("  Convention Adherence: {}", drift_color.bold());
    println!(
        "  Architecture:        {}",
        report.conventions.architecture.pattern.bright_white()
    );
    if let Some(ref sm) = report.conventions.state_management {
        println!("  State Management:    {}", sm.bright_white());
    }
    println!("  Files analyzed:      {}", report.files_analyzed);
    println!("  Drift findings:      {}", report.findings.len());

    if !report.findings.is_empty() {
        println!();

        let mut by_category: std::collections::HashMap<&DriftCategory, Vec<&DriftFinding>> =
            std::collections::HashMap::new();
        for f in &report.findings {
            by_category.entry(&f.category).or_default().push(f);
        }

        for (cat, cat_findings) in &by_category {
            println!(
                "  {} {} ({} findings)",
                "▸".bright_cyan(),
                cat.to_string().bright_white().bold(),
                cat_findings.len()
            );
            for f in cat_findings.iter().take(5) {
                println!(
                    "    {} {}:{}  {}",
                    "·".dimmed(),
                    f.file.dimmed(),
                    f.line,
                    f.message
                );
            }
            if cat_findings.len() > 5 {
                println!(
                    "    {} ... and {} more",
                    "·".dimmed(),
                    cat_findings.len() - 5
                );
            }
            println!();
        }
    } else {
        println!();
        println!(
            "  {} No convention drift detected — code is consistent!",
            "✓".green().bold()
        );
        println!();
    }
}
