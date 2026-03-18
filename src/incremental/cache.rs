use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CACHE_DIR: &str = ".falcon-cache";
const CACHE_FILE: &str = "analysis-cache.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub modified: u64,
    pub hash: u64,
    pub issue_count: usize,
    pub has_errors: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisCache {
    pub version: String,
    pub entries: HashMap<String, CacheEntry>,
}

impl AnalysisCache {
    pub fn load(root: &Path) -> Self {
        let cache_path = root.join(CACHE_DIR).join(CACHE_FILE);
        if let Ok(contents) = std::fs::read_to_string(&cache_path) {
            if let Ok(cache) = serde_json::from_str::<AnalysisCache>(&contents) {
                if cache.version == env!("CARGO_PKG_VERSION") {
                    return cache;
                }
            }
        }
        AnalysisCache {
            version: env!("CARGO_PKG_VERSION").to_string(),
            entries: HashMap::new(),
        }
    }

    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        let cache_dir = root.join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir)?;
        let cache_path = cache_dir.join(CACHE_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&cache_path, json)?;
        Ok(())
    }

    /// Returns files that have changed since the last cached analysis.
    pub fn changed_files(&self, files: &[PathBuf]) -> Vec<PathBuf> {
        files
            .iter()
            .filter(|file| {
                let key = file.to_string_lossy().to_string();
                match self.entries.get(&key) {
                    Some(entry) => {
                        let current_modified = file_modified_secs(file);
                        current_modified != entry.modified
                    }
                    None => true,
                }
            })
            .cloned()
            .collect()
    }

    pub fn update_entry(&mut self, file: &Path, issue_count: usize, has_errors: bool) {
        let key = file.to_string_lossy().to_string();
        let modified = file_modified_secs(file);
        let hash = simple_hash(&key);
        self.entries.insert(
            key,
            CacheEntry {
                modified,
                hash,
                issue_count,
                has_errors,
            },
        );
    }
}

fn file_modified_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
