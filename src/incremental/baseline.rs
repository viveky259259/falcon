use crate::reporters::Issue;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const BASELINE_FILE: &str = "falcon-baseline.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub rule: String,
    pub file: String,
    pub line: usize,
    pub message_hash: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Baseline {
    pub version: String,
    pub created_at: String,
    pub entries: Vec<BaselineEntry>,
}

impl Baseline {
    pub fn create(issues: &[Issue], root: &Path) -> anyhow::Result<PathBuf> {
        let entries: Vec<BaselineEntry> = issues
            .iter()
            .map(|issue| {
                let rel = issue
                    .file
                    .strip_prefix(root)
                    .unwrap_or(&issue.file)
                    .to_string_lossy()
                    .to_string();
                BaselineEntry {
                    rule: issue.rule.clone(),
                    file: rel,
                    line: issue.line,
                    message_hash: fnv_hash(&issue.message),
                }
            })
            .collect();

        let baseline = Baseline {
            version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono_now(),
            entries,
        };

        let path = root.join(BASELINE_FILE);
        let json = serde_json::to_string_pretty(&baseline)?;
        std::fs::write(&path, json)?;

        Ok(path)
    }

    pub fn load(root: &Path) -> anyhow::Result<Self> {
        let path = root.join(BASELINE_FILE);
        if !path.exists() {
            anyhow::bail!("No baseline file found. Run `falcon baseline create` first.");
        }
        let contents = std::fs::read_to_string(&path)?;
        let baseline: Baseline = serde_json::from_str(&contents)?;
        Ok(baseline)
    }

    pub fn filter_new_issues(&self, issues: Vec<Issue>, root: &Path) -> Vec<Issue> {
        let baseline_keys: HashSet<String> = self
            .entries
            .iter()
            .map(|e| format!("{}:{}:{}", e.rule, e.file, e.line))
            .collect();

        issues
            .into_iter()
            .filter(|issue| {
                let rel = issue
                    .file
                    .strip_prefix(root)
                    .unwrap_or(&issue.file)
                    .to_string_lossy()
                    .to_string();
                let key = format!("{}:{}:{}", issue.rule, rel, issue.line);
                !baseline_keys.contains(&key)
            })
            .collect()
    }
}

fn fnv_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn chrono_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}
