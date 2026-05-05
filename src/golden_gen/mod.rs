use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Represents the type of widget discovered in source code
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetType {
    Stateless,
    Stateful,
    Consumer,
}

impl WidgetType {
    fn as_str(&self) -> &'static str {
        match self {
            WidgetType::Stateless => "StatelessWidget",
            WidgetType::Stateful => "StatefulWidget",
            WidgetType::Consumer => "ConsumerWidget",
        }
    }
}

/// Represents a constructor parameter of a widget
#[derive(Debug, Clone)]
pub struct WidgetParam {
    pub name: String,
    pub type_name: String,
    pub is_required: bool,
    pub default_value: String,
}

/// Represents a discovered widget class
#[derive(Debug, Clone)]
pub struct DiscoveredWidget {
    pub name: String,
    pub file: PathBuf,
    pub widget_type: WidgetType,
    pub params: Vec<WidgetParam>,
}

/// Represents a generated test file
#[derive(Debug, Clone)]
pub struct GeneratedTest {
    pub widget_name: String,
    pub source_file: PathBuf,
    pub test_file: PathBuf,
    pub test_content: String,
    pub constructor_params: Vec<WidgetParam>,
}

/// Report of golden test generation
#[derive(Debug)]
pub struct GoldenGenReport {
    pub widgets_found: usize,
    pub widgets_already_tested: usize,
    pub files_generated: usize,
    pub generated: Vec<GeneratedTest>,
    pub skipped: Vec<String>,
}

/// Discovers all testable widgets in a Flutter project
///
/// Walks through all .dart files (excluding build/, .dart_tool/, test/, and generated files)
/// and finds StatelessWidget, StatefulWidget, and ConsumerWidget classes.
pub fn discover_widgets(path: &Path) -> Result<Vec<DiscoveredWidget>> {
    let mut widgets = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| {
            let rel_path = e.path().strip_prefix(path).unwrap_or_else(|_| e.path());
            let path_str = rel_path.to_string_lossy();

            // Skip excluded directories
            !path_str.contains("build/")
                && !path_str.contains(".dart_tool/")
                && !path_str.contains("test/")
                && !path_str.contains("generated/")
                && !path_str.contains(".g.dart")
        })
    {
        let file_path = entry.path();
        match fs::read_to_string(file_path) {
            Ok(content) => {
                if let Ok(mut file_widgets) = parse_widgets_from_file(&content, file_path) {
                    widgets.append(&mut file_widgets);
                }
            }
            Err(e) => {
                log::warn!("Failed to read {}: {}", file_path.display(), e);
            }
        }
    }

    Ok(widgets)
}

/// Generates golden test files for discovered widgets
pub fn generate_golden_tests(
    path: &Path,
    output_dir: &Path,
    dry_run: bool,
) -> Result<GoldenGenReport> {
    let widgets = discover_widgets(path)?;
    let mut report = GoldenGenReport {
        widgets_found: widgets.len(),
        widgets_already_tested: 0,
        files_generated: 0,
        generated: Vec::new(),
        skipped: Vec::new(),
    };

    for widget in widgets {
        let test_file_path = get_test_file_path(&widget, output_dir);

        // Check if test already exists
        if test_file_path.exists() {
            report.widgets_already_tested += 1;
            report
                .skipped
                .push(format!("Test already exists for {}", widget.name));
            continue;
        }

        let test_content = generate_test_content(&widget, path)?;
        let generated_test = GeneratedTest {
            widget_name: widget.name.clone(),
            source_file: widget.file,
            test_file: test_file_path,
            test_content,
            constructor_params: widget.params,
        };

        if !dry_run {
            // Ensure test directory exists
            if let Some(parent) = generated_test.test_file.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&generated_test.test_file, &generated_test.test_content)?;
        }

        report.files_generated += 1;
        report.generated.push(generated_test);
    }

    Ok(report)
}

/// Prints the golden test generation report to stdout with colors
pub fn print_golden_report(report: &GoldenGenReport) {
    println!(
        "\n{}",
        "=== Golden Test Generation Report ===".bright_cyan().bold()
    );
    println!(
        "  {} widgets discovered",
        report.widgets_found.to_string().bright_green()
    );
    println!(
        "  {} widgets already tested",
        report.widgets_already_tested.to_string().bright_yellow()
    );
    println!(
        "  {} test files generated",
        report.files_generated.to_string().bright_green()
    );

    if !report.generated.is_empty() {
        println!("\n{}:", "Generated Tests".bright_cyan());
        for test in &report.generated {
            println!("  ✓ {} -> {}", test.widget_name, test.test_file.display());
        }
    }

    if !report.skipped.is_empty() {
        println!("\n{}:", "Skipped".bright_yellow());
        for skip in &report.skipped {
            println!("  ⊘ {}", skip);
        }
    }
}

/// Writes an HTML report of the golden test generation
pub fn write_golden_html_report(report: &GoldenGenReport, path: &Path) -> Result<()> {
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Golden Test Generation Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }}
        .container {{ max-width: 900px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; }}
        h1 {{ color: #333; border-bottom: 2px solid #0066cc; padding-bottom: 10px; }}
        .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; margin: 20px 0; }}
        .stat {{ background: #f0f0f0; padding: 15px; border-left: 4px solid #0066cc; border-radius: 4px; }}
        .stat-value {{ font-size: 24px; font-weight: bold; color: #0066cc; }}
        .stat-label {{ font-size: 14px; color: #666; margin-top: 5px; }}
        .generated, .skipped {{ margin: 20px 0; }}
        table {{ width: 100%; border-collapse: collapse; margin: 10px 0; }}
        th {{ background: #0066cc; color: white; padding: 10px; text-align: left; }}
        td {{ padding: 10px; border-bottom: 1px solid #eee; }}
        tr:hover {{ background: #f9f9f9; }}
        .status-generated {{ color: #28a745; font-weight: bold; }}
        .status-skipped {{ color: #ffc107; font-weight: bold; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Golden Test Generation Report</h1>

        <div class="stats">
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Widgets Discovered</div>
            </div>
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Already Tested</div>
            </div>
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Tests Generated</div>
            </div>
        </div>

        <div class="generated">
            <h2>Generated Tests</h2>
            <table>
                <thead>
                    <tr>
                        <th>Widget Name</th>
                        <th>Source File</th>
                        <th>Test File</th>
                        <th>Parameters</th>
                    </tr>
                </thead>
                <tbody>
                    {}
                </tbody>
            </table>
        </div>

        <div class="skipped">
            <h2>Skipped</h2>
            <table>
                <thead>
                    <tr>
                        <th>Reason</th>
                    </tr>
                </thead>
                <tbody>
                    {}
                </tbody>
            </table>
        </div>
    </div>
</body>
</html>"#,
        report.widgets_found,
        report.widgets_already_tested,
        report.files_generated,
        report
            .generated
            .iter()
            .map(|test| {
                let params = test
                    .constructor_params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.type_name))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    test.widget_name,
                    test.source_file.display(),
                    test.test_file.display(),
                    params
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        report
            .skipped
            .iter()
            .map(|skip| format!("<tr><td>{}</td></tr>", skip))
            .collect::<Vec<_>>()
            .join("\n")
    );

    fs::write(path, html)?;
    Ok(())
}

// Private helper functions

fn parse_widgets_from_file(content: &str, file_path: &Path) -> Result<Vec<DiscoveredWidget>> {
    let mut widgets = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        if let Some(widget_type) = detect_widget_type(line) {
            if let Some(class_name) = extract_class_name(line) {
                let params = extract_constructor_params(&lines, idx)?;
                widgets.push(DiscoveredWidget {
                    name: class_name,
                    file: file_path.to_path_buf(),
                    widget_type,
                    params,
                });
            }
        }
    }

    Ok(widgets)
}

fn detect_widget_type(line: &str) -> Option<WidgetType> {
    if line.contains("extends StatelessWidget") {
        Some(WidgetType::Stateless)
    } else if line.contains("extends StatefulWidget") {
        Some(WidgetType::Stateful)
    } else if line.contains("extends ConsumerWidget") {
        Some(WidgetType::Consumer)
    } else {
        None
    }
}

fn extract_class_name(line: &str) -> Option<String> {
    // Match: class ClassName extends WidgetType
    let line_trimmed = line.trim();
    if !line_trimmed.starts_with("class ") {
        return None;
    }

    let after_class = &line_trimmed[6..];
    if let Some(space_idx) = after_class.find(' ') {
        return Some(after_class[..space_idx].to_string());
    }

    None
}

fn extract_constructor_params(lines: &[&str], class_line_idx: usize) -> Result<Vec<WidgetParam>> {
    let params = Vec::new();

    // Find constructor: look for class_name({
    for idx in class_line_idx..lines.len() {
        let line = lines[idx];

        // Look for constructor
        if line.contains("const ") || idx == class_line_idx {
            if line.find('(').is_some() {
                // Collect full parameter list
                let mut param_content = String::new();
                let mut brace_count = 0;
                let mut in_params = false;

                for check_idx in idx..lines.len() {
                    let check_line = lines[check_idx];
                    for ch in check_line.chars() {
                        match ch {
                            '(' => {
                                in_params = true;
                                brace_count += 1;
                            }
                            ')' => {
                                brace_count -= 1;
                                if brace_count == 0 && in_params {
                                    return Ok(parse_params(&param_content));
                                }
                            }
                            _ if in_params && brace_count > 0 => {
                                param_content.push(ch);
                            }
                            _ => {}
                        }
                    }
                    if brace_count == 0 && in_params {
                        break;
                    }
                }
            }
        }
    }

    Ok(params)
}

fn parse_params(param_str: &str) -> Vec<WidgetParam> {
    let mut params = Vec::new();

    // Split by comma, but respect nested braces
    let mut current_param = String::new();
    let mut brace_depth = 0;

    for ch in param_str.chars() {
        match ch {
            '{' | '<' => {
                brace_depth += 1;
                current_param.push(ch);
            }
            '}' | '>' => {
                brace_depth -= 1;
                current_param.push(ch);
            }
            ',' if brace_depth == 0 => {
                if !current_param.trim().is_empty() {
                    if let Some(param) = parse_single_param(current_param.trim()) {
                        params.push(param);
                    }
                }
                current_param.clear();
            }
            _ => current_param.push(ch),
        }
    }

    if !current_param.trim().is_empty() {
        if let Some(param) = parse_single_param(current_param.trim()) {
            params.push(param);
        }
    }

    params
}

fn parse_single_param(param_str: &str) -> Option<WidgetParam> {
    let trimmed = param_str.trim();
    if trimmed.is_empty() {
        return None;
    }

    let is_required = trimmed.starts_with("required ");
    let param_str = if is_required {
        &trimmed[9..].trim()
    } else {
        trimmed
    };

    // Parse: type name [= default]
    let parts: Vec<&str> = param_str.split('=').collect();
    let type_and_name = parts[0].trim();

    let (type_name, name) = {
        let tokens: Vec<&str> = type_and_name.split_whitespace().collect();
        if tokens.len() >= 2 {
            (tokens[0].to_string(), tokens[tokens.len() - 1].to_string())
        } else {
            return None;
        }
    };

    let default_value = if parts.len() > 1 {
        parts[1].trim().to_string()
    } else {
        infer_default_value(&type_name)
    };

    Some(WidgetParam {
        name,
        type_name,
        is_required,
        default_value,
    })
}

fn infer_default_value(type_name: &str) -> String {
    match type_name {
        "String" => "'Test String'".to_string(),
        "int" => "0".to_string(),
        "double" => "0.0".to_string(),
        "bool" => "false".to_string(),
        "Widget" => "const SizedBox()".to_string(),
        t if t.starts_with("List<") => "const []".to_string(),
        t if t.ends_with("?") => "null".to_string(),
        t if t.contains("Callback") || t.contains("Function") => "() {}".to_string(),
        t if t.contains("Color") => "Colors.black".to_string(),
        t if t.contains("TextStyle") => "const TextStyle()".to_string(),
        t if t.contains("EdgeInsets") => "EdgeInsets.zero".to_string(),
        _ => "null".to_string(),
    }
}

fn get_test_file_path(widget: &DiscoveredWidget, output_dir: &Path) -> PathBuf {
    let test_file_name = format!(
        "{}_test.dart",
        widget.name.to_lowercase().replace("widget", "")
    );
    output_dir.join("test").join("widgets").join(test_file_name)
}

fn generate_test_content(widget: &DiscoveredWidget, project_path: &Path) -> Result<String> {
    let relative_import = widget
        .file
        .strip_prefix(project_path)
        .unwrap_or(&widget.file)
        .to_string_lossy()
        .replace('\\', "/");

    let params_str = widget
        .params
        .iter()
        .filter(|p| p.is_required || !p.default_value.is_empty())
        .map(|p| format!("{}: {}", p.name, p.default_value))
        .collect::<Vec<_>>()
        .join(",\n      ");

    let params_clause = if !params_str.is_empty() {
        format!("(\n      {},\n    )", params_str)
    } else {
        String::new()
    };

    // Infer package name from project path
    let package_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my_app");

    let widget_name = &widget.name;
    let widget_lower = widget.name.to_lowercase();
    Ok(format!(
        r#"import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:{package}/{module}';

void main() {{
  testWidgets('{name} renders correctly', (tester) async {{
    await tester.pumpWidget(
      const MaterialApp(
        home: {name}{params}
      ),
    );

    expect(find.byType({name}), findsOneWidget);

    await expectLater(
      find.byType({name}),
      matchesGoldenFile('goldens/{lower}.png'),
    );
  }});
}}
"#,
        package = package_name,
        module = relative_import,
        name = widget_name,
        params = params_clause,
        lower = widget_lower,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_widget_type_stateless() {
        let line = "class MyWidget extends StatelessWidget {";
        assert_eq!(detect_widget_type(line), Some(WidgetType::Stateless));
    }

    #[test]
    fn test_detect_widget_type_stateful() {
        let line = "class MyWidget extends StatefulWidget {";
        assert_eq!(detect_widget_type(line), Some(WidgetType::Stateful));
    }

    #[test]
    fn test_detect_widget_type_consumer() {
        let line = "class MyWidget extends ConsumerWidget {";
        assert_eq!(detect_widget_type(line), Some(WidgetType::Consumer));
    }

    #[test]
    fn test_detect_widget_type_none() {
        let line = "class MyClass {";
        assert_eq!(detect_widget_type(line), None);
    }

    #[test]
    fn test_extract_class_name() {
        let line = "class MyTestWidget extends StatelessWidget {";
        assert_eq!(extract_class_name(line), Some("MyTestWidget".to_string()));
    }

    #[test]
    fn test_infer_default_value_string() {
        assert_eq!(infer_default_value("String"), "'Test String'");
    }

    #[test]
    fn test_infer_default_value_int() {
        assert_eq!(infer_default_value("int"), "0");
    }

    #[test]
    fn test_infer_default_value_bool() {
        assert_eq!(infer_default_value("bool"), "false");
    }

    #[test]
    fn test_infer_default_value_widget() {
        assert_eq!(infer_default_value("Widget"), "const SizedBox()");
    }

    #[test]
    fn test_infer_default_value_list() {
        assert_eq!(infer_default_value("List<String>"), "const []");
    }

    #[test]
    fn test_parse_single_param_simple() {
        let param = "String name";
        let result = parse_single_param(param);
        assert!(result.is_some());
        let p = result.unwrap();
        assert_eq!(p.name, "name");
        assert_eq!(p.type_name, "String");
        assert!(!p.is_required);
    }

    #[test]
    fn test_parse_single_param_required() {
        let param = "required String name";
        let result = parse_single_param(param);
        assert!(result.is_some());
        let p = result.unwrap();
        assert_eq!(p.name, "name");
        assert_eq!(p.type_name, "String");
        assert!(p.is_required);
    }

    #[test]
    fn test_parse_params_multiple() {
        let params = "String name, int age, bool active";
        let result = parse_params(params);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "name");
        assert_eq!(result[1].name, "age");
        assert_eq!(result[2].name, "active");
    }

    #[test]
    fn test_golden_gen_report_creation() {
        let report = GoldenGenReport {
            widgets_found: 5,
            widgets_already_tested: 1,
            files_generated: 4,
            generated: vec![],
            skipped: vec!["TestWidget".to_string()],
        };

        assert_eq!(report.widgets_found, 5);
        assert_eq!(report.widgets_already_tested, 1);
        assert_eq!(report.files_generated, 4);
        assert_eq!(report.skipped.len(), 1);
    }

    #[test]
    fn test_write_golden_html_report() {
        let temp_dir = TempDir::new().unwrap();
        let report_path = temp_dir.path().join("report.html");

        let report = GoldenGenReport {
            widgets_found: 3,
            widgets_already_tested: 0,
            files_generated: 3,
            generated: vec![GeneratedTest {
                widget_name: "TestWidget".to_string(),
                source_file: PathBuf::from("lib/widgets/test_widget.dart"),
                test_file: PathBuf::from("test/widgets/test_widget_test.dart"),
                test_content: "// test".to_string(),
                constructor_params: vec![],
            }],
            skipped: vec![],
        };

        let result = write_golden_html_report(&report, &report_path);
        assert!(result.is_ok());
        assert!(report_path.exists());

        let content = fs::read_to_string(&report_path).unwrap();
        assert!(content.contains("Golden Test Generation Report"));
        assert!(content.contains("TestWidget"));
    }
}
