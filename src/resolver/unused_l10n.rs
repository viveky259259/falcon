use crate::config::Severity;
use crate::reporters::Issue;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Detect unused l10n/localization keys.
/// Scans ARB files for keys and checks if they're referenced in Dart code.
pub fn detect_unused_l10n(root: &Path, exclude: &[glob::Pattern]) -> Vec<Issue> {
    let mut issues = Vec::new();

    let arb_files = find_arb_files(root);
    if arb_files.is_empty() {
        return issues;
    }

    let all_l10n_keys = collect_l10n_keys(&arb_files);
    if all_l10n_keys.is_empty() {
        return issues;
    }

    let used_identifiers = collect_dart_identifiers(root, exclude);

    for (key, arb_file) in &all_l10n_keys {
        if key.starts_with('@') || key.starts_with("@@") {
            continue;
        }
        if !used_identifiers.contains(key.as_str()) {
            issues.push(Issue {
                rule: "unused-l10n-key".to_string(),
                message: format!(
                    "Localization key '{}' is defined but not used in any Dart file.",
                    key
                ),
                severity: Severity::Info,
                file: arb_file.clone(),
                line: 1,
                column: 1,
            });
        }
    }

    issues
}

fn find_arb_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "arb"))
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn collect_l10n_keys(arb_files: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut keys = Vec::new();
    let mut seen = HashSet::new();

    for file in arb_files {
        let contents = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let json: serde_json::Value = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(map) = json.as_object() {
            for key in map.keys() {
                if !key.starts_with('@') && !key.starts_with("@@") && seen.insert(key.clone()) {
                    keys.push((key.clone(), file.clone()));
                }
            }
        }
    }

    keys
}

fn collect_dart_identifiers(root: &Path, exclude: &[glob::Pattern]) -> HashSet<String> {
    let mut identifiers = HashSet::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            for word in source.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if !word.is_empty() {
                    identifiers.insert(word.to_string());
                }
            }
        }
    }

    identifiers
}
