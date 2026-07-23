use crate::config::Severity;
use crate::reporters::Issue;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRestriction {
    pub from: String,
    pub deny: Vec<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Enforce import restriction rules defined in config.
pub fn enforce_import_restrictions(
    root: &Path,
    restrictions: &[ImportRestriction],
    exclude: &[glob::Pattern],
) -> Vec<Issue> {
    let mut issues = Vec::new();

    if restrictions.is_empty() {
        return issues;
    }

    let lib_path = root.join("lib");
    let base = if lib_path.exists() { &lib_path } else { root };

    let dart_files: Vec<PathBuf> = WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    for file in &dart_files {
        let rel = file.strip_prefix(base).unwrap_or(file);
        let rel_str = rel.to_string_lossy();

        let applicable: Vec<&ImportRestriction> = restrictions
            .iter()
            .filter(|r| matches_glob_pattern(&rel_str, &r.from))
            .collect();

        if applicable.is_empty() {
            continue;
        }

        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("import") && !trimmed.starts_with("export") {
                continue;
            }

            for restriction in &applicable {
                for denied in &restriction.deny {
                    if trimmed.contains(denied.as_str()) {
                        let reason = restriction
                            .reason
                            .as_deref()
                            .unwrap_or("Import restriction violated.");
                        issues.push(Issue {
                            rule: "import-restriction".to_string(),
                            message: format!(
                                "Files in '{}' cannot import '{}'. {}",
                                restriction.from, denied, reason
                            ),
                            severity: Severity::Error,
                            file: file.clone(),
                            line: line_num + 1,
                            column: 1,
                        });
                    }
                }
            }
        }
    }

    issues
}

fn matches_glob_pattern(path: &str, pattern: &str) -> bool {
    if let Ok(p) = glob::Pattern::new(pattern) {
        p.matches(path)
    } else {
        path.starts_with(pattern) || path.contains(&format!("/{}/", pattern))
    }
}

/// Detect cross-package boundary violations in monorepo.
pub fn check_package_boundaries(root: &Path, exclude: &[glob::Pattern]) -> Vec<Issue> {
    let mut issues = Vec::new();

    let lib_path = root.join("lib");
    if !lib_path.exists() {
        return issues;
    }

    let src_path = lib_path.join("src");
    if !src_path.exists() {
        return issues;
    }

    let dart_files: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            let rel_str = rel.to_string_lossy();
            !rel_str.starts_with("lib/src/")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let pkg_name = read_package_name(root);

    for file in &dart_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("import") {
                continue;
            }

            if let Some(ref name) = pkg_name {
                let src_import = format!("package:{}/src/", name);
                if trimmed.contains(&src_import) {
                    let rel = file.strip_prefix(root).unwrap_or(file);
                    let file_in_lib_src = rel.to_string_lossy().starts_with("lib/src/");
                    if !file_in_lib_src {
                        issues.push(Issue {
                            rule: "package-boundary".to_string(),
                            message: "Importing from 'lib/src/' violates package boundary. Use the public API in 'lib/' instead.".to_string(),
                            severity: Severity::Warning,
                            file: file.clone(),
                            line: line_num + 1,
                            column: 1,
                        });
                    }
                }
            }
        }
    }

    issues
}

fn read_package_name(root: &Path) -> Option<String> {
    let pubspec = root.join("pubspec.yaml");
    let content = std::fs::read_to_string(pubspec).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(stripped) = trimmed.strip_prefix("name:") {
            return Some(stripped.trim().to_string());
        }
    }
    None
}
