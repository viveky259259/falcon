use crate::incremental::dep_graph::DependencyGraph;
use crate::reporters::Issue;
use crate::config::Severity;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Detect cyclic import dependencies and produce issues + ASCII visualization.
pub fn detect_cycles(graph: &DependencyGraph, root: &Path) -> (Vec<Issue>, Vec<Vec<PathBuf>>) {
    let mut visited = HashSet::new();
    let mut on_stack = HashSet::new();
    let mut cycles: Vec<Vec<PathBuf>> = Vec::new();
    let mut path_stack: Vec<PathBuf> = Vec::new();

    for file in graph.imports.keys() {
        if !visited.contains(file) {
            dfs(
                file,
                graph,
                &mut visited,
                &mut on_stack,
                &mut path_stack,
                &mut cycles,
            );
        }
    }

    cycles.sort_by_key(|c| c.len());
    cycles.dedup();

    let issues: Vec<Issue> = cycles
        .iter()
        .flat_map(|cycle| {
            let cycle_str: Vec<String> = cycle
                .iter()
                .map(|p| {
                    p.strip_prefix(root)
                        .unwrap_or(p)
                        .to_string_lossy()
                        .to_string()
                })
                .collect();
            let display = cycle_str.join(" → ");

            cycle.iter().map(move |file| Issue {
                rule: "cyclic-dependency".to_string(),
                message: format!("File is part of a cyclic dependency: {}", display),
                severity: Severity::Warning,
                file: file.clone(),
                line: 1,
                column: 1,
            }).collect::<Vec<_>>()
        })
        .collect();

    (issues, cycles)
}

fn dfs(
    node: &PathBuf,
    graph: &DependencyGraph,
    visited: &mut HashSet<PathBuf>,
    on_stack: &mut HashSet<PathBuf>,
    path_stack: &mut Vec<PathBuf>,
    cycles: &mut Vec<Vec<PathBuf>>,
) {
    visited.insert(node.clone());
    on_stack.insert(node.clone());
    path_stack.push(node.clone());

    if let Some(imports) = graph.imports.get(node) {
        for imported in imports {
            if !visited.contains(imported) {
                dfs(imported, graph, visited, on_stack, path_stack, cycles);
            } else if on_stack.contains(imported) {
                if let Some(start_idx) = path_stack.iter().position(|p| p == imported) {
                    let mut cycle: Vec<PathBuf> = path_stack[start_idx..].to_vec();
                    cycle.push(imported.clone());
                    cycles.push(cycle);
                }
            }
        }
    }

    path_stack.pop();
    on_stack.remove(node);
}

/// Format cycles as ASCII art for CLI display.
pub fn format_cycles(cycles: &[Vec<PathBuf>], root: &Path) -> String {
    if cycles.is_empty() {
        return "No cyclic dependencies found.".to_string();
    }

    let mut output = format!("Found {} cyclic dependency chain(s):\n\n", cycles.len());

    for (i, cycle) in cycles.iter().enumerate() {
        output.push_str(&format!("  Cycle #{} ({} files):\n", i + 1, cycle.len() - 1));
        for (j, file) in cycle.iter().enumerate() {
            let rel = file.strip_prefix(root).unwrap_or(file);
            if j == cycle.len() - 1 {
                output.push_str(&format!("    └──→ {} (back to start)\n", rel.display()));
            } else {
                output.push_str(&format!("    ├── {}\n", rel.display()));
            }
        }
        output.push('\n');
    }

    output
}

/// Detect over-promoted dependencies (deps that should be dev_dependencies).
pub fn detect_promoted_deps(root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();
    let pubspec_path = root.join("pubspec.yaml");

    if !pubspec_path.exists() {
        return issues;
    }

    let contents = match std::fs::read_to_string(&pubspec_path) {
        Ok(c) => c,
        Err(_) => return issues,
    };

    let yaml: serde_yaml::Value = match serde_yaml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return issues,
    };

    let deps = extract_dep_names(&yaml, "dependencies");
    let dev_deps = extract_dep_names(&yaml, "dev_dependencies");

    let lib_imports = collect_imports_in_dir(&root.join("lib"));
    let test_imports = collect_imports_in_dir(&root.join("test"));

    for dep in &deps {
        if dep == "flutter" || dep == "flutter_localizations" {
            continue;
        }
        let used_in_lib = lib_imports.contains(dep);
        let used_in_test = test_imports.contains(dep);

        if !used_in_lib && used_in_test {
            issues.push(Issue {
                rule: "over-promoted-dependency".to_string(),
                message: format!(
                    "Dependency '{}' is only used in tests — move to dev_dependencies.",
                    dep
                ),
                severity: Severity::Info,
                file: pubspec_path.clone(),
                line: 1,
                column: 1,
            });
        }
    }

    for dep in &dev_deps {
        let used_in_lib = lib_imports.contains(dep);
        if used_in_lib {
            issues.push(Issue {
                rule: "under-promoted-dependency".to_string(),
                message: format!(
                    "Dev dependency '{}' is imported in lib/ — promote to dependencies.",
                    dep
                ),
                severity: Severity::Warning,
                file: pubspec_path.clone(),
                line: 1,
                column: 1,
            });
        }
    }

    issues
}

fn extract_dep_names(yaml: &serde_yaml::Value, section: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Some(deps) = yaml.get(section).and_then(|d| d.as_mapping()) {
        for key in deps.keys() {
            if let Some(name) = key.as_str() {
                names.insert(name.to_string());
            }
        }
    }
    names
}

fn collect_imports_in_dir(dir: &Path) -> HashSet<String> {
    let mut packages = HashSet::new();
    if !dir.exists() {
        return packages;
    }

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("import") && trimmed.contains("package:") {
                    if let Some(pkg) = extract_package(trimmed) {
                        packages.insert(pkg);
                    }
                }
            }
        }
    }

    packages
}

fn extract_package(line: &str) -> Option<String> {
    let start = line.find("package:")?;
    let rest = &line[start + 8..];
    let end = rest.find('/')?;
    Some(rest[..end].to_string())
}
