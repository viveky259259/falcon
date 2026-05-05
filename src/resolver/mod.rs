pub mod cyclic;
pub mod dead_code;
pub mod references;
pub mod scope;
pub mod unused_l10n;
pub mod unused_params;

use crate::config::{FalconConfig, Severity};
use crate::parser::DartParser;
use crate::reporters::Issue;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ProjectResolver {
    root: PathBuf,
    dart_files: Vec<PathBuf>,
}

impl ProjectResolver {
    pub fn new(root: &Path, config: &FalconConfig) -> Result<Self> {
        let exclude_patterns: Vec<glob::Pattern> = config
            .exclude
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        let unused_exclude_patterns: Vec<glob::Pattern> = config
            .unused
            .exclude
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        let dart_files: Vec<PathBuf> = WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
            .filter(|e| {
                let rel = e.path().strip_prefix(root).unwrap_or(e.path());
                !exclude_patterns.iter().any(|p| p.matches_path(rel))
                    && !unused_exclude_patterns.iter().any(|p| p.matches_path(rel))
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        Ok(Self {
            root: root.to_path_buf(),
            dart_files,
        })
    }

    pub fn find_unused(&self) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        issues.extend(self.find_unused_files()?);
        issues.extend(self.find_unused_code()?);
        issues.extend(self.find_unused_dependencies()?);
        Ok(issues)
    }

    pub fn find_unused_files(&self) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let imported_files = self.collect_all_imports()?;

        for file in &self.dart_files {
            let rel = file.strip_prefix(&self.root).unwrap_or(file);
            let rel_str = rel.to_string_lossy().to_string();

            if rel_str.contains("main.dart") || rel_str.contains("test") {
                continue;
            }

            let file_name = file.file_name().and_then(|f| f.to_str()).unwrap_or("");

            if !imported_files.contains(file_name) && !imported_files.contains(&rel_str) {
                issues.push(Issue {
                    rule: "unused-file".to_string(),
                    message: format!("File '{}' is not imported by any other file.", rel_str),
                    severity: Severity::Warning,
                    file: file.clone(),
                    line: 1,
                    column: 1,
                });
            }
        }

        Ok(issues)
    }

    pub fn find_unused_code(&self) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let mut all_declarations: Vec<(String, PathBuf, usize)> = Vec::new();
        let mut all_references: HashMap<String, HashSet<PathBuf>> = HashMap::new();

        for file in &self.dart_files {
            let source = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut parser = DartParser::new()?;
            let tree = match parser.parse(&source) {
                Some(t) => t,
                None => continue,
            };
            let root = tree.root_node();

            let decls = references::collect_declarations(root, &source);
            for (name, line) in decls {
                if !name.starts_with('_') {
                    all_declarations.push((name, file.clone(), line));
                }
            }

            let refs = references::collect_references_by_file(root, &source, file);
            for (name, files) in refs {
                all_references.entry(name).or_default().extend(files);
            }
        }

        let skip_names: HashSet<&str> = [
            "main",
            "build",
            "createState",
            "initState",
            "dispose",
            "didChangeDependencies",
        ]
        .into_iter()
        .collect();

        for (name, decl_file, line) in &all_declarations {
            if skip_names.contains(name.as_str()) {
                continue;
            }

            if !all_references.contains_key(name) {
                issues.push(Issue {
                    rule: "unused-code".to_string(),
                    message: format!("Declaration '{}' appears to be unused.", name),
                    severity: Severity::Warning,
                    file: decl_file.clone(),
                    line: *line,
                    column: 1,
                });
            }
        }

        Ok(issues)
    }

    pub fn find_unused_dependencies(&self) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let pubspec_path = self.root.join("pubspec.yaml");

        if !pubspec_path.exists() {
            return Ok(issues);
        }

        let pubspec_content = std::fs::read_to_string(&pubspec_path)?;
        let dependencies = parse_pubspec_dependencies(&pubspec_content);

        let mut used_packages: HashSet<String> = HashSet::new();
        for file in &self.dart_files {
            let source = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("import") && trimmed.contains("package:") {
                    if let Some(pkg) = extract_package_name(trimmed) {
                        used_packages.insert(pkg);
                    }
                }
            }
        }

        for dep in &dependencies {
            if !used_packages.contains(dep) {
                issues.push(Issue {
                    rule: "unused-dependency".to_string(),
                    message: format!(
                        "Dependency '{}' is declared in pubspec.yaml but not imported.",
                        dep
                    ),
                    severity: Severity::Info,
                    file: pubspec_path.clone(),
                    line: 1,
                    column: 1,
                });
            }
        }

        Ok(issues)
    }

    fn collect_all_imports(&self) -> Result<HashSet<String>> {
        let mut imports = HashSet::new();

        for file in &self.dart_files {
            let source = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("import")
                    || trimmed.starts_with("part")
                    || trimmed.starts_with("export")
                {
                    if let Some(uri) = extract_import_uri(trimmed) {
                        imports.insert(uri.clone());
                        if let Some(file_name) = Path::new(&uri).file_name() {
                            imports.insert(file_name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        Ok(imports)
    }
}

fn extract_import_uri(line: &str) -> Option<String> {
    let start = line.find('\'')?;
    let rest = &line[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

fn extract_package_name(import_line: &str) -> Option<String> {
    let start = import_line.find("package:")?;
    let rest = &import_line[start + 8..];
    let end = rest.find('/')?;
    Some(rest[..end].to_string())
}

fn parse_pubspec_dependencies(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_dependencies = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "dependencies:" {
            in_dependencies = true;
            continue;
        }

        if in_dependencies {
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                break;
            }

            if let Some(name) = trimmed.strip_suffix(':') {
                if !name.contains(' ') {
                    deps.push(name.to_string());
                }
            } else if trimmed.contains(':') {
                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if !parts[0].contains(' ') {
                    deps.push(parts[0].to_string());
                }
            }
        }
    }

    deps.retain(|d| d != "flutter" && d != "flutter_test" && d != "flutter_localizations");
    deps
}
