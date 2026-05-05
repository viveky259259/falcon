//! Generate meaningful test stubs from code analysis.

use colored::Colorize;
use std::path::Path;

/// A generated test case stub.
#[derive(Debug, Clone)]
pub struct TestStub {
    pub target_file: String,
    pub test_file: String,
    pub class_name: String,
    pub test_cases: Vec<TestCase>,
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub body: String,
    pub category: TestCategory,
}

#[derive(Debug, Clone)]
pub enum TestCategory {
    Unit,
    Widget,
    Integration,
}

impl std::fmt::Display for TestCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestCategory::Unit => write!(f, "unit"),
            TestCategory::Widget => write!(f, "widget"),
            TestCategory::Integration => write!(f, "integration"),
        }
    }
}

/// Generate test stubs for Dart files in a project.
pub fn generate_test_stubs(root: &Path) -> Vec<TestStub> {
    let mut stubs = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| {
            let p = e.path().to_string_lossy();
            !p.contains("/test/") && !p.contains(".g.dart") && !p.contains(".freezed.dart")
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        if let Some(stub) = generate_stub_for_file(&rel, &source) {
            if !stub.test_cases.is_empty() {
                stubs.push(stub);
            }
        }
    }

    stubs
}

fn generate_stub_for_file(file_path: &str, source: &str) -> Option<TestStub> {
    let mut test_cases = Vec::new();
    let mut class_name = String::new();

    let is_widget = source.contains("StatelessWidget") || source.contains("StatefulWidget");
    let _is_service = file_path.contains("service") || file_path.contains("repository");

    for line in source.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("class ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() > 1 {
                class_name = parts[1]
                    .trim_end_matches('{')
                    .trim_end_matches('<')
                    .to_string();
            }
        }

        if class_name.is_empty() {
            continue;
        }

        if (trimmed.starts_with("Future<")
            || trimmed.starts_with("void ")
            || trimmed.starts_with("String ")
            || trimmed.starts_with("int ")
            || trimmed.starts_with("bool "))
            && trimmed.contains('(')
            && !trimmed.starts_with("//")
        {
            let method_name = extract_method_name(trimmed);
            if let Some(name) = method_name {
                if name.starts_with('_')
                    || name == "build"
                    || name == "initState"
                    || name == "dispose"
                {
                    continue;
                }

                if is_widget {
                    test_cases.push(TestCase {
                        name: format!("renders {} correctly", class_name),
                        body: format!(
                            "    await tester.pumpWidget(MaterialApp(home: {}()));\n    expect(find.byType({}), findsOneWidget);",
                            class_name, class_name
                        ),
                        category: TestCategory::Widget,
                    });
                    break;
                }

                if trimmed.contains("Future<") {
                    test_cases.push(TestCase {
                        name: format!("{} completes successfully", name),
                        body: format!(
                            "    final sut = {}();\n    final result = await sut.{}();\n    expect(result, isNotNull);",
                            class_name, name
                        ),
                        category: TestCategory::Unit,
                    });
                    test_cases.push(TestCase {
                        name: format!("{} handles errors", name),
                        body: format!(
                            "    final sut = {}();\n    // TODO: inject error condition\n    expect(() => sut.{}(), throwsA(isA<Exception>()));",
                            class_name, name
                        ),
                        category: TestCategory::Unit,
                    });
                } else {
                    test_cases.push(TestCase {
                        name: format!("{} returns expected result", name),
                        body: format!(
                            "    final sut = {}();\n    final result = sut.{}();\n    expect(result, isNotNull);",
                            class_name, name
                        ),
                        category: TestCategory::Unit,
                    });
                }
            }
        }
    }

    let test_file = file_path
        .replace("lib/", "test/")
        .replace(".dart", "_test.dart");

    Some(TestStub {
        target_file: file_path.to_string(),
        test_file,
        class_name,
        test_cases,
    })
}

fn extract_method_name(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split('(').collect();
    let before_paren = parts.first()?;
    let tokens: Vec<&str> = before_paren.split_whitespace().collect();
    let name = tokens.last()?;
    Some(name.to_string())
}

/// Generate test file content from a stub.
pub fn render_test_file(stub: &TestStub) -> String {
    let mut out = String::new();
    out.push_str("import 'package:flutter_test/flutter_test.dart';\n");

    let has_widget = stub
        .test_cases
        .iter()
        .any(|t| matches!(t.category, TestCategory::Widget));
    if has_widget {
        out.push_str("import 'package:flutter/material.dart';\n");
    }

    out.push_str(&format!("// import for {}\n\n", stub.target_file));

    out.push_str("void main() {\n");
    out.push_str(&format!("  group('{}', () {{\n", stub.class_name));

    for tc in &stub.test_cases {
        let test_fn = if matches!(tc.category, TestCategory::Widget) {
            "testWidgets"
        } else {
            "test"
        };
        if matches!(tc.category, TestCategory::Widget) {
            out.push_str(&format!(
                "    {}('{}', (tester) async {{\n",
                test_fn, tc.name
            ));
        } else {
            out.push_str(&format!("    {}('{}', () async {{\n", test_fn, tc.name));
        }
        out.push_str(&format!("{}\n", tc.body));
        out.push_str("    });\n\n");
    }

    out.push_str("  });\n");
    out.push_str("}\n");
    out
}

/// Print test generation summary.
pub fn print_test_gen_summary(stubs: &[TestStub]) {
    println!();
    println!("  {} Test Generation", "falcon".bright_cyan().bold());
    println!();

    let total_tests: usize = stubs.iter().map(|s| s.test_cases.len()).sum();
    let unit: usize = stubs
        .iter()
        .flat_map(|s| &s.test_cases)
        .filter(|t| matches!(t.category, TestCategory::Unit))
        .count();
    let widget: usize = stubs
        .iter()
        .flat_map(|s| &s.test_cases)
        .filter(|t| matches!(t.category, TestCategory::Widget))
        .count();

    println!(
        "  Test stubs generated: {}",
        total_tests.to_string().bright_white()
    );
    println!("    Unit tests:    {}", unit);
    println!("    Widget tests:  {}", widget);
    println!("    Files:         {}", stubs.len());

    if !stubs.is_empty() {
        println!();
        for stub in stubs.iter().take(10) {
            println!(
                "    {} → {} ({} tests)",
                stub.target_file,
                stub.test_file.bright_white(),
                stub.test_cases.len()
            );
        }
        if stubs.len() > 10 {
            println!("    ... and {} more files", stubs.len() - 10);
        }
    }
    println!();
}
