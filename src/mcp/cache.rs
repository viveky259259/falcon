use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const CACHE_DIR: &str = "falcon-mcp";
const CACHE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct McpCache {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSignature {
    mtime_secs: u64,
    mtime_nanos: u32,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    version: String,
    path: String,
    mtime_secs: u64,
    mtime_nanos: u32,
    sha256: String,
    value: Value,
}

impl McpCache {
    pub fn default_root() -> PathBuf {
        if let Ok(root) = std::env::var("FALCON_MCP_CACHE_DIR") {
            if !root.trim().is_empty() {
                return PathBuf::from(root);
            }
        }
        std::env::temp_dir().join(CACHE_DIR)
    }

    pub fn new() -> Self {
        Self {
            root: Self::default_root(),
        }
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load_lint_file(&self, path: &Path, source: &str) -> Option<Value> {
        let identity = file_identity(path);
        let signature = file_signature(path, source).ok()?;
        let cache_path = self.cache_file_path("lint-file", &identity);
        let contents = fs::read_to_string(cache_path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&contents).ok()?;

        if entry.version != CACHE_VERSION || entry.path != identity {
            return None;
        }
        if entry.mtime_secs != signature.mtime_secs
            || entry.mtime_nanos != signature.mtime_nanos
            || entry.sha256 != signature.sha256
        {
            return None;
        }

        Some(entry.value)
    }

    pub fn store_lint_file(&self, path: &Path, source: &str, value: &Value) -> io::Result<()> {
        let identity = file_identity(path);
        let signature = file_signature(path, source)?;
        let cache_path = self.cache_file_path("lint-file", &identity);
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let entry = CacheEntry {
            version: CACHE_VERSION.to_string(),
            path: identity,
            mtime_secs: signature.mtime_secs,
            mtime_nanos: signature.mtime_nanos,
            sha256: signature.sha256,
            value: value.clone(),
        };

        let tmp_path = cache_path.with_extension(format!("json.tmp.{}", std::process::id()));
        let json = serde_json::to_vec_pretty(&entry)?;
        fs::write(&tmp_path, json)?;
        fs::rename(tmp_path, cache_path)
    }

    fn cache_file_path(&self, kind: &str, identity: &str) -> PathBuf {
        self.root
            .join(CACHE_VERSION)
            .join(kind)
            .join(format!("{}.json", sha256_hex(identity.as_bytes())))
    }
}

impl Default for McpCache {
    fn default() -> Self {
        Self::new()
    }
}

fn file_identity(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn file_signature(path: &Path, source: &str) -> io::Result<FileSignature> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?;
    let duration = modified.duration_since(UNIX_EPOCH).unwrap_or_default();
    Ok(FileSignature {
        mtime_secs: duration.as_secs(),
        mtime_nanos: duration.subsec_nanos(),
        sha256: sha256_hex(source.as_bytes()),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lint_file_cache_round_trips_value() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("main.dart");
        fs::write(&file, "class Good {}\n").unwrap();
        let cache = McpCache::with_root(temp.path().join("cache"));
        let value = json!({"file": "main.dart", "issue_count": 0, "issues": []});

        assert!(cache.load_lint_file(&file, "class Good {}\n").is_none());
        cache
            .store_lint_file(&file, "class Good {}\n", &value)
            .unwrap();
        assert_eq!(cache.load_lint_file(&file, "class Good {}\n"), Some(value));
    }

    #[test]
    fn lint_file_cache_rejects_changed_content() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("main.dart");
        fs::write(&file, "class Good {}\n").unwrap();
        let cache = McpCache::with_root(temp.path().join("cache"));
        cache
            .store_lint_file(&file, "class Good {}\n", &json!({"issue_count": 0}))
            .unwrap();

        fs::write(&file, "class Bad { void run() { print('debug'); } }\n").unwrap();
        assert!(cache
            .load_lint_file(&file, "class Bad { void run() { print('debug'); } }\n")
            .is_none());
    }

    #[test]
    fn lint_file_cache_rejects_mismatched_version() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("main.dart");
        let source = "class Good {}\n";
        fs::write(&file, source).unwrap();
        let cache = McpCache::with_root(temp.path().join("cache"));
        cache
            .store_lint_file(&file, source, &json!({"issue_count": 0}))
            .unwrap();

        let identity = file_identity(&file);
        let cache_path = cache.cache_file_path("lint-file", &identity);
        let mut entry: CacheEntry =
            serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
        entry.version = "0.0.0".to_string();
        fs::write(&cache_path, serde_json::to_vec_pretty(&entry).unwrap()).unwrap();

        assert!(cache.load_lint_file(&file, source).is_none());
    }

    #[test]
    fn default_root_uses_tmp_falcon_mcp_dir() {
        let root = McpCache::default_root();
        assert!(root.ends_with(CACHE_DIR), "root: {}", root.display());
    }
}
