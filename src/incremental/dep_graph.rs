use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Tracks which Dart files import which other files, enabling
/// "changed files + dependents" incremental analysis.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// file -> set of files it imports (forward edges)
    pub imports: HashMap<PathBuf, HashSet<PathBuf>>,
    /// file -> set of files that import it (reverse edges)
    pub dependents: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl DependencyGraph {
    pub fn build(root: &Path, exclude: &[glob::Pattern]) -> Self {
        let mut imports: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
        let mut dependents: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();

        let dart_files: Vec<PathBuf> = WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
            .filter(|e| {
                let rel = e.path().strip_prefix(root).unwrap_or(e.path());
                !exclude.iter().any(|p| p.matches_path(rel))
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        let all_files_index = build_file_index(&dart_files, root);

        for file in &dart_files {
            let file_imports = extract_imports(file, root, &all_files_index);
            for imp in &file_imports {
                dependents.entry(imp.clone()).or_default().insert(file.clone());
            }
            imports.insert(file.clone(), file_imports);
        }

        DependencyGraph { imports, dependents }
    }

    /// Given a set of changed files, return all files that need re-analysis:
    /// the changed files themselves plus all transitive dependents.
    pub fn affected_files(&self, changed: &[PathBuf]) -> HashSet<PathBuf> {
        let mut affected = HashSet::new();
        let mut queue: Vec<PathBuf> = changed.to_vec();

        while let Some(file) = queue.pop() {
            if affected.insert(file.clone()) {
                if let Some(deps) = self.dependents.get(&file) {
                    for dep in deps {
                        if !affected.contains(dep) {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }

        affected
    }
}

fn build_file_index(files: &[PathBuf], root: &Path) -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    for file in files {
        let rel = file.strip_prefix(root).unwrap_or(file);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        index.insert(rel_str, file.clone());

        if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
            index.entry(name.to_string()).or_insert_with(|| file.clone());
        }
    }
    index
}

fn extract_imports(file: &Path, root: &Path, file_index: &HashMap<String, PathBuf>) -> HashSet<PathBuf> {
    let mut result = HashSet::new();

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(_) => return result,
    };

    let file_dir = file.parent().unwrap_or(Path::new("."));

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("import") && !trimmed.starts_with("part") && !trimmed.starts_with("export") {
            continue;
        }

        if let Some(uri) = extract_uri(trimmed) {
            if uri.starts_with("dart:") {
                continue;
            }

            if uri.starts_with("package:") {
                if let Some(rel_path) = uri.strip_prefix("package:") {
                    if let Some(slash) = rel_path.find('/') {
                        let path_part = &rel_path[slash + 1..];
                        let lib_path = format!("lib/{}", path_part);
                        if let Some(abs) = file_index.get(&lib_path) {
                            result.insert(abs.clone());
                        }
                    }
                }
            } else {
                let resolved = file_dir.join(&uri);
                if let Ok(canonical) = resolved.canonicalize() {
                    result.insert(canonical);
                } else {
                    let rel = resolved
                        .strip_prefix(root)
                        .unwrap_or(&resolved)
                        .to_string_lossy()
                        .replace('\\', "/");
                    if let Some(abs) = file_index.get(&rel) {
                        result.insert(abs.clone());
                    }
                }
            }
        }
    }

    result
}

fn extract_uri(line: &str) -> Option<String> {
    let start = line.find('\'')?;
    let rest = &line[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}
