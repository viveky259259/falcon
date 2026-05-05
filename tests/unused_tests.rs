use falcon::config::FalconConfig;
use falcon::resolver::ProjectResolver;
use std::fs;
use tempfile::TempDir;

fn setup_project(files: Vec<(&str, &str)>) -> TempDir {
    let dir = TempDir::new().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    dir
}

#[test]
fn test_unused_files_detection() {
    let dir = setup_project(vec![
        (
            "lib/main.dart",
            r#"
import 'used.dart';

void main() {
  greet();
}
"#,
        ),
        (
            "lib/used.dart",
            r#"
void greet() {
  print('hello');
}
"#,
        ),
        (
            "lib/unused_helper.dart",
            r#"
void neverCalled() {
  print('unused');
}
"#,
        ),
    ]);

    let config = FalconConfig::default();
    let resolver = ProjectResolver::new(dir.path(), &config).unwrap();
    let issues = resolver.find_unused_files().unwrap();

    let unused_files: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();

    assert!(
        unused_files
            .iter()
            .any(|m| m.contains("unused_helper.dart")),
        "Should detect unused_helper.dart as unused. Found: {:?}",
        unused_files
    );
}

#[test]
fn test_unused_dependencies_detection() {
    let dir = setup_project(vec![
        (
            "pubspec.yaml",
            r#"
name: test_app
dependencies:
  http: ^1.0.0
  provider: ^6.0.0
  unused_package: ^1.0.0
"#,
        ),
        (
            "lib/main.dart",
            r#"
import 'package:http/http.dart';
import 'package:provider/provider.dart';

void main() {}
"#,
        ),
    ]);

    let config = FalconConfig::default();
    let resolver = ProjectResolver::new(dir.path(), &config).unwrap();
    let issues = resolver.find_unused_dependencies().unwrap();

    let unused_deps: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();

    assert!(
        unused_deps.iter().any(|m| m.contains("unused_package")),
        "Should detect unused_package as unused dependency. Found: {:?}",
        unused_deps
    );

    assert!(
        !unused_deps.iter().any(|m| m.contains("http")),
        "Should NOT flag 'http' as unused"
    );
}

#[test]
fn test_unused_code_detection() {
    let dir = setup_project(vec![(
        "lib/main.dart",
        r#"
class UsedClass {
  void method() {}
}

class UnusedClass {
  void method() {}
}

void main() {
  var x = UsedClass();
  x.method();
}
"#,
    )]);

    let config = FalconConfig::default();
    let resolver = ProjectResolver::new(dir.path(), &config).unwrap();
    let issues = resolver.find_unused_code().unwrap();

    let messages: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();
    let has_main_unused = messages.iter().any(|m| m.contains("main"));
    assert!(!has_main_unused, "Should not flag 'main' as unused");
}

#[test]
fn test_no_unused_with_empty_project() {
    let dir = TempDir::new().unwrap();
    let config = FalconConfig::default();
    let resolver = ProjectResolver::new(dir.path(), &config).unwrap();

    let file_issues = resolver.find_unused_files().unwrap();
    let code_issues = resolver.find_unused_code().unwrap();
    let dep_issues = resolver.find_unused_dependencies().unwrap();

    assert!(file_issues.is_empty());
    assert!(code_issues.is_empty());
    assert!(dep_issues.is_empty());
}
