pub mod ai;
pub mod config;
pub mod docs;
pub mod incremental;
pub mod lsp;
pub mod metrics;
pub mod parser;
pub mod reporters;
pub mod resolver;
pub mod rules;
pub mod unused;
pub mod workspace;

use anyhow::Result;
use config::FalconConfig;
use metrics::MetricsResults;
use parser::DartParser;
use rayon::prelude::*;
use reporters::{AnalysisReport, Issue};
use resolver::ProjectResolver;
use rules::RuleRegistry;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Falcon {
    config: FalconConfig,
    #[allow(dead_code)]
    parser: DartParser,
    rule_registry: RuleRegistry,
}

impl Falcon {
    pub fn new(config: FalconConfig) -> Result<Self> {
        let parser = DartParser::new()?;
        let mut rule_registry = RuleRegistry::new();
        rule_registry.register_defaults(&config);
        Ok(Self {
            config,
            parser,
            rule_registry,
        })
    }

    pub fn analyze(&self, path: &Path) -> Result<AnalysisReport> {
        let files = self.collect_dart_files(path)?;
        let file_count = files.len();

        let file_results: Vec<_> = files
            .par_iter()
            .filter_map(|file| {
                let source = std::fs::read_to_string(file).ok()?;
                let mut parser = DartParser::new().ok()?;
                let tree = parser.parse(&source)?;
                let root = tree.root_node();

                let mut issues = Vec::new();

                let metrics =
                    metrics::calculate_file_metrics(root, &source, &self.config.metrics);
                for violation in metrics.violations() {
                    issues.push(violation);
                }

                let rule_issues = self.rule_registry.check(root, &source, file);
                issues.extend(rule_issues);

                Some((file.clone(), issues, metrics))
            })
            .collect();

        let mut all_issues = Vec::new();
        let mut all_metrics = Vec::new();
        for (file, issues, metrics) in file_results {
            all_issues.extend(issues);
            all_metrics.push((file, metrics));
        }

        let unused_issues = if self.config.unused.enabled {
            let resolver = ProjectResolver::new(path, &self.config)?;
            resolver.find_unused()?
        } else {
            Vec::new()
        };
        all_issues.extend(unused_issues);

        Ok(AnalysisReport {
            issues: all_issues,
            metrics: all_metrics,
            file_count,
        })
    }

    pub fn calculate_metrics(&self, path: &Path) -> Result<Vec<(PathBuf, MetricsResults)>> {
        let files = self.collect_dart_files(path)?;

        let results: Vec<_> = files
            .par_iter()
            .filter_map(|file| {
                let source = std::fs::read_to_string(file).ok()?;
                let mut parser = DartParser::new().ok()?;
                let tree = parser.parse(&source)?;
                let root = tree.root_node();
                let metrics =
                    metrics::calculate_file_metrics(root, &source, &self.config.metrics);
                Some((file.clone(), metrics))
            })
            .collect();

        Ok(results)
    }

    pub fn check_unused_code(&self, path: &Path) -> Result<Vec<Issue>> {
        let resolver = ProjectResolver::new(path, &self.config)?;
        resolver.find_unused_code()
    }

    pub fn check_unused_files(&self, path: &Path) -> Result<Vec<Issue>> {
        let resolver = ProjectResolver::new(path, &self.config)?;
        resolver.find_unused_files()
    }

    pub fn check_dependencies(&self, path: &Path) -> Result<Vec<Issue>> {
        let resolver = ProjectResolver::new(path, &self.config)?;
        resolver.find_unused_dependencies()
    }

    /// Analyze only a specific subset of files (for incremental mode).
    pub fn analyze_files(&self, _path: &Path, files: &[PathBuf]) -> Result<AnalysisReport> {
        let file_count = files.len();

        let file_results: Vec<_> = files
            .par_iter()
            .filter_map(|file| {
                let source = std::fs::read_to_string(file).ok()?;
                let mut parser = DartParser::new().ok()?;
                let tree = parser.parse(&source)?;
                let root = tree.root_node();

                let mut issues = Vec::new();

                let metrics =
                    metrics::calculate_file_metrics(root, &source, &self.config.metrics);
                for violation in metrics.violations() {
                    issues.push(violation);
                }

                let rule_issues = self.rule_registry.check(root, &source, file);
                issues.extend(rule_issues);

                Some((file.clone(), issues, metrics))
            })
            .collect();

        let mut all_issues = Vec::new();
        let mut all_metrics = Vec::new();
        for (file, issues, metrics) in file_results {
            all_issues.extend(issues);
            all_metrics.push((file, metrics));
        }

        Ok(AnalysisReport {
            issues: all_issues,
            metrics: all_metrics,
            file_count,
        })
    }

    fn collect_dart_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let exclude_patterns: Vec<glob::Pattern> = self
            .config
            .exclude
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        let files: Vec<PathBuf> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "dart")
            })
            .filter(|e| {
                let rel = e
                    .path()
                    .strip_prefix(path)
                    .unwrap_or(e.path());
                !exclude_patterns
                    .iter()
                    .any(|p| p.matches_path(rel))
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        Ok(files)
    }
}

pub fn init_config(path: &Path) -> Result<()> {
    let config_path = path.join("falcon.yaml");
    if config_path.exists() {
        anyhow::bail!("falcon.yaml already exists in {}", path.display());
    }
    let default_config = FalconConfig::default();
    let yaml = serde_yaml::to_string(&default_config)?;
    std::fs::write(&config_path, yaml)?;
    Ok(())
}
