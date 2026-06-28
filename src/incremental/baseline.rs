use crate::reporters::Issue;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const BASELINE_FILE: &str = "falcon-baseline.json";

fn schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub rule: String,
    pub file: String,
    pub line: usize,
    pub message_hash: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Baseline {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    pub version: String,
    pub created_at: String,
    pub entries: Vec<BaselineEntry>,
}

impl Baseline {
    pub fn create(issues: &[Issue], root: &Path) -> anyhow::Result<PathBuf> {
        Self::create_at_path(issues, root, root.join(BASELINE_FILE))
    }

    pub fn create_at_path(
        issues: &[Issue],
        root: &Path,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<PathBuf> {
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
            schema_version: schema_version(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono_now(),
            entries,
        };

        let path = path.as_ref().to_path_buf();
        let json = serde_json::to_string_pretty(&baseline)?;
        std::fs::write(&path, json)?;

        Ok(path)
    }

    pub fn load(root: &Path) -> anyhow::Result<Self> {
        Self::load_path(root.join(BASELINE_FILE))
    }

    pub fn load_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            anyhow::bail!(
                "No baseline file found at {}. Run `falcon baseline create` or `falcon review --update-baseline` first.",
                path.display()
            );
        }
        let contents = std::fs::read_to_string(path)?;
        let baseline: Baseline = serde_json::from_str(&contents)?;
        Ok(baseline)
    }

    pub fn filter_new_issues(&self, issues: Vec<Issue>, root: &Path) -> Vec<Issue> {
        self.filter_new_issues_with_file_aliases(issues, root, &HashMap::new())
    }

    pub fn filter_new_issues_with_file_aliases(
        &self,
        issues: Vec<Issue>,
        root: &Path,
        file_aliases: &HashMap<String, Vec<String>>,
    ) -> Vec<Issue> {
        let baseline_keys: HashSet<String> = self
            .entries
            .iter()
            .map(|e| format!("{}:{}:{}:{}", e.rule, e.file, e.line, e.message_hash))
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
                let message_hash = fnv_hash(&issue.message);
                let current_key = format!("{}:{}:{}:{}", issue.rule, rel, issue.line, message_hash);
                let alias_matches = file_aliases.get(&rel).is_some_and(|aliases| {
                    aliases.iter().any(|alias| {
                        baseline_keys.contains(&format!(
                            "{}:{}:{}:{}",
                            issue.rule, alias, issue.line, message_hash
                        ))
                    })
                });

                !baseline_keys.contains(&current_key) && !alias_matches
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
