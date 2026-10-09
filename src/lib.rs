#![allow(dead_code)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::too_many_arguments)]

pub mod agents;
pub mod ai;
pub mod ai_score;
pub mod analysis;
pub mod analyzer_bridge;
pub mod animation_audit;
pub mod api;
pub mod asset_audit;
pub mod benchmark;
pub mod benchmark_compare;
pub mod check_a11y;
pub mod check_assets;
pub mod check_platform_deps;
pub mod check_pods;
pub mod ci;
pub mod cli;
pub mod community;
pub mod config;
pub mod dashboard;
pub mod deeplink;
pub mod docs;
pub mod doctor;
pub mod flutter_run;
pub mod golden_gen;
pub mod incremental;
pub mod l10n_coverage;
pub mod leaderboard;
pub mod lsp;
pub mod manage;
pub mod mcp;
pub mod metrics;
pub mod migration;
pub mod parser;
pub mod paths;
pub mod platform;
pub mod plugins;
pub mod preflight;
pub mod project_graph;
pub mod reporters;
pub mod resolver;
pub mod review;
pub mod rules;
pub mod runtime;
pub mod sdk;
pub mod self_update;
pub mod showcase;
pub mod smells;
pub mod stability;
pub mod theme_audit;
pub mod unused;
pub mod version_util;
pub mod workspace;

use anyhow::{Context, Result};
use config::FalconConfig;
use metrics::MetricsResults;
use parser::DartParser;
use rayon::prelude::*;
use reporters::{AnalysisReport, Issue};
use resolver::ProjectResolver;
use rules::{RuleContext, RuleRegistry};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Falcon {
    config: FalconConfig,
    rule_registry: RuleRegistry,
}

impl Falcon {
    pub fn new(config: FalconConfig) -> Result<Self> {
        let mut rule_registry = RuleRegistry::new();
        rule_registry.register_defaults(&config);
        Ok(Self {
            config,
            rule_registry,
        })
    }

    /// Generate a read-only, content-pinned graph of the configured Dart inventory.
    pub fn generate_project_graph(&self, path: &Path) -> Result<project_graph::ProjectGraph> {
        project_graph::build(path, self.collect_dart_files(path)?, &self.config.exclude)
    }

    pub fn analyze(&self, path: &Path) -> Result<AnalysisReport> {
        let files = self.collect_dart_files(path)?;
        let file_count = files.len();
        let resolver = ProjectResolver::new(path, &self.config)?;
        let resolver_index = resolver.build_index()?;

        let file_results: Vec<_> = files
            .par_iter()
            .map(|file| -> Result<_> {
                let source = std::fs::read_to_string(file)
                    .with_context(|| format!("Reading {}", file.display()))?;
                let mut parser = DartParser::new()?;
                let tree = parser
                    .parse(&source)
                    .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file.display()))?;
                let root = tree.root_node();

                let mut issues = Vec::new();

                let metrics = metrics::calculate_file_metrics(root, &source, &self.config.metrics);
                for violation in metrics.violations() {
                    issues.push(violation);
                }

                let resolver = resolver_index.resolver_for_file(file, &source);
                let context = RuleContext {
                    resolver_index: Some(&resolver_index),
                    resolver: Some(&resolver),
                };
                let rule_issues = self
                    .rule_registry
                    .check_with_context(root, &source, file, &context);
                issues.extend(rule_issues);

                Ok((file.clone(), issues, metrics))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut all_issues = Vec::new();
        let mut all_metrics = Vec::new();
        for (file, issues, metrics) in file_results {
            all_issues.extend(issues);
            all_metrics.push((file, metrics));
        }

        let unused_issues = if self.config.unused.enabled {
            resolver.find_unused()?
        } else {
            Vec::new()
        };
        all_issues.extend(unused_issues);

        Ok(AnalysisReport {
            issues: all_issues,
            metrics: all_metrics,
            file_count,
            project_path: Some(path.to_path_buf()),
        })
    }

    pub fn calculate_metrics(&self, path: &Path) -> Result<Vec<(PathBuf, MetricsResults)>> {
        let files = self.collect_dart_files(path)?;

        let results: Vec<_> = files
            .par_iter()
            .map(|file| -> Result<_> {
                let source = std::fs::read_to_string(file)
                    .with_context(|| format!("Reading {}", file.display()))?;
                let mut parser = DartParser::new()?;
                let tree = parser
                    .parse(&source)
                    .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file.display()))?;
                let root = tree.root_node();
                let metrics = metrics::calculate_file_metrics(root, &source, &self.config.metrics);
                Ok((file.clone(), metrics))
            })
            .collect::<Result<Vec<_>>>()?;

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
    pub fn analyze_files(&self, files: &[PathBuf]) -> Result<AnalysisReport> {
        self.analyze_files_with_rule_context(files, RuleContext::default(), None)
    }

    /// Analyze a subset of files with project-level resolver context.
    pub fn analyze_files_with_project_context(
        &self,
        project_root: &Path,
        files: &[PathBuf],
    ) -> Result<AnalysisReport> {
        let resolver = ProjectResolver::new(project_root, &self.config)?;
        let resolver_index = resolver.build_index()?;
        let context = RuleContext {
            resolver_index: Some(&resolver_index),
            resolver: None,
        };
        self.analyze_files_with_rule_context(files, context, Some(project_root.to_path_buf()))
    }

    fn analyze_files_with_rule_context(
        &self,
        files: &[PathBuf],
        context: RuleContext<'_>,
        project_path: Option<PathBuf>,
    ) -> Result<AnalysisReport> {
        let file_count = files.len();

        let file_results: Vec<_> = files
            .par_iter()
            .map(|file| -> Result<_> {
                let source = std::fs::read_to_string(file)
                    .with_context(|| format!("Reading {}", file.display()))?;
                let mut parser = DartParser::new()?;
                let tree = parser
                    .parse(&source)
                    .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file.display()))?;
                let root = tree.root_node();

                let mut issues = Vec::new();

                let metrics = metrics::calculate_file_metrics(root, &source, &self.config.metrics);
                for violation in metrics.violations() {
                    issues.push(violation);
                }

                let file_resolver = context
                    .resolver_index
                    .map(|index| index.resolver_for_file(file, &source));
                let file_context = RuleContext {
                    resolver_index: context.resolver_index,
                    resolver: file_resolver.as_ref().or(context.resolver),
                };
                let rule_issues =
                    self.rule_registry
                        .check_with_context(root, &source, file, &file_context);
                issues.extend(rule_issues);

                Ok((file.clone(), issues, metrics))
            })
            .collect::<Result<Vec<_>>>()?;

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
            project_path,
        })
    }

    fn collect_dart_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let exclude_patterns: Vec<glob::Pattern> = self
            .config
            .exclude
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        std::fs::metadata(path)
            .with_context(|| format!("Reading analysis root {}", path.display()))?;
        let files: Vec<PathBuf> = WalkDir::new(path)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
            .filter(|e| {
                let rel = e.path().strip_prefix(path).unwrap_or(e.path());
                !exclude_patterns.iter().any(|p| p.matches_path(rel))
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
