//! Analyze quality of build_runner generated code (.g.dart, .freezed.dart, etc.)

use crate::config::Severity;
use crate::reporters::Issue;
use colored::Colorize;
use std::path::{Path, PathBuf};

/// Codegen quality report.
#[derive(Debug, Default)]
pub struct CodegenReport {
    pub total_generated_files: usize,
    pub total_generated_lines: usize,
    pub stale_files: Vec<PathBuf>,
    pub large_files: Vec<(PathBuf, usize)>,
    pub issues: Vec<Issue>,
    pub generators_found: Vec<String>,
}

/// Analyze quality of generated code in a project.
pub fn analyze_codegen(root: &Path) -> CodegenReport {
    let mut report = CodegenReport::default();
    let mut generators = std::collections::HashSet::new();

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
    {
        let path = entry.path();
        let fname = path.file_name().and_then(|f| f.to_str()).unwrap_or("");

        let is_generated = fname.ends_with(".g.dart")
            || fname.ends_with(".freezed.dart")
            || fname.ends_with(".gr.dart")
            || fname.ends_with(".mocks.dart")
            || fname.ends_with(".config.dart");

        if !is_generated {
            continue;
        }

        report.total_generated_files += 1;

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let line_count = source.lines().count();
        report.total_generated_lines += line_count;

        if fname.ends_with(".g.dart") {
            generators.insert("json_serializable / build_runner");
        }
        if fname.ends_with(".freezed.dart") {
            generators.insert("freezed");
        }
        if fname.ends_with(".gr.dart") {
            generators.insert("auto_route");
        }
        if fname.ends_with(".mocks.dart") {
            generators.insert("mockito / build_runner");
        }

        if line_count > 1000 {
            report.large_files.push((path.to_path_buf(), line_count));
            report.issues.push(Issue {
                rule: "codegen-large-file".to_string(),
                message: format!(
                    "Generated file has {} lines — consider splitting the source model",
                    line_count
                ),
                severity: Severity::Info,
                file: path.to_path_buf(),
                line: 1,
                column: 1,
            });
        }

        if !source.contains("// GENERATED CODE") && !source.contains("// coverage:ignore-file") {
            report.issues.push(Issue {
                rule: "codegen-missing-header".to_string(),
                message: "Generated file is missing the standard '// GENERATED CODE' header"
                    .to_string(),
                severity: Severity::Info,
                file: path.to_path_buf(),
                line: 1,
                column: 1,
            });
        }

        let source_file = path
            .to_string_lossy()
            .replace(".g.dart", ".dart")
            .replace(".freezed.dart", ".dart")
            .replace(".gr.dart", ".dart")
            .replace(".mocks.dart", ".dart");
        let source_path = PathBuf::from(&source_file);

        if !source_path.exists() && !fname.ends_with(".config.dart") {
            report.stale_files.push(path.to_path_buf());
            report.issues.push(Issue {
                rule: "codegen-stale-file".to_string(),
                message: format!(
                    "Generated file may be stale — source file '{}' not found",
                    source_path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("?")
                ),
                severity: Severity::Warning,
                file: path.to_path_buf(),
                line: 1,
                column: 1,
            });
        }
    }

    report.generators_found = generators.into_iter().map(|s| s.to_string()).collect();
    report.generators_found.sort();

    report
}

/// Print codegen quality report.
pub fn print_codegen_report(report: &CodegenReport) {
    println!();
    println!(
        "  {} Code Generation Quality",
        "falcon".bright_cyan().bold()
    );
    println!();

    println!("  Generated files:  {}", report.total_generated_files);
    println!("  Generated lines:  {}", report.total_generated_lines);

    if !report.generators_found.is_empty() {
        println!("  Generators:       {}", report.generators_found.join(", "));
    }

    if !report.stale_files.is_empty() {
        println!();
        println!("  {} Stale generated files (source missing):", "⚠".yellow());
        for f in &report.stale_files {
            println!("    {}", f.display());
        }
    }

    if !report.large_files.is_empty() {
        println!();
        println!("  Large generated files (>1000 lines):");
        for (f, lines) in &report.large_files {
            let rel = f.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            println!("    {} ({} lines)", rel.bright_white(), lines);
        }
    }

    if report.issues.is_empty() {
        println!();
        println!(
            "  {} No code generation quality issues found.",
            "✓".green().bold()
        );
    } else {
        println!();
        println!("  {} issue(s) found", report.issues.len());
    }

    println!();
}
