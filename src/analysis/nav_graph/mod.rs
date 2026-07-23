//! Navigation graph extraction for Flutter apps.
//!
//! Produces a versioned, JSON-serializable graph of screens, edges (navigation
//! transitions), and contract violations (orphans, dangling refs, unguarded
//! sensitive routes). Renderers consume this graph, while CI checks can fail on
//! contract violations.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod compare;
mod extract;
mod render;
mod syntax;
mod violations;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Router {
    GoRouter,
    AutoRoute,
    Navigator1,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub id: String,
    pub widget: String,
    pub file: PathBuf,
    pub line: usize,
    pub route_name: Option<String>,
    pub route_path: Option<String>,
    pub require_auth: bool,
    pub has_redirect: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Push,
    Go,
    Replace,
    Pop,
    DeepLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to_route_name: Option<String>,
    pub to_screen: Option<String>,
    pub kind: EdgeKind,
    pub via: String,
    pub file: PathBuf,
    pub line: usize,
    pub dynamic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DanglingRef {
    pub route_name: String,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnguardedSensitive {
    pub screen: String,
    pub matched: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub screens: usize,
    pub edges: usize,
    pub dynamic_edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavGraph {
    pub version: u32,
    pub router: Router,
    pub initial_location: Option<String>,
    pub screens: Vec<Screen>,
    pub edges: Vec<Edge>,
    pub orphans: Vec<String>,
    pub dangling: Vec<DanglingRef>,
    pub unguarded_sensitive: Vec<UnguardedSensitive>,
    pub stats: GraphStats,
}

impl Default for NavGraph {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            router: Router::Unknown,
            initial_location: None,
            screens: Vec::new(),
            edges: Vec::new(),
            orphans: Vec::new(),
            dangling: Vec::new(),
            unguarded_sensitive: Vec::new(),
            stats: GraphStats::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedField {
    pub screen: String,
    pub field: String,
    pub from: serde_json::Value,
    pub to: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenEdge {
    pub from: String,
    pub to_route_name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NavGraphDiff {
    pub version: u32,
    pub added_screens: Vec<Screen>,
    pub removed_screens: Vec<Screen>,
    pub added_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub changed: Vec<ChangedField>,
    pub broke: Vec<BrokenEdge>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CheckOptions {
    pub strict: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckReport {
    pub orphans: Vec<String>,
    pub dangling: Vec<DanglingRef>,
    pub unguarded_sensitive: Vec<UnguardedSensitive>,
    pub broke: Vec<BrokenEdge>,
}

impl CheckReport {
    pub fn is_failure(&self, opts: CheckOptions) -> bool {
        if !self.dangling.is_empty()
            || !self.unguarded_sensitive.is_empty()
            || !self.broke.is_empty()
        {
            return true;
        }
        if opts.strict && !self.orphans.is_empty() {
            return true;
        }
        false
    }
}

/// Build a navigation graph by walking the project tree.
pub fn build_graph(project_root: &Path) -> Result<NavGraph> {
    extract::build_graph(project_root)
}

/// Diff two graphs (old → new). Edge identity is `(from, to_route_name)`.
pub fn diff(old: &NavGraph, new: &NavGraph) -> NavGraphDiff {
    compare::diff(old, new)
}

/// Compute a check report from a graph (and optionally a baseline diff for `broke`).
pub fn check(graph: &NavGraph, graph_diff: Option<&NavGraphDiff>) -> CheckReport {
    CheckReport {
        orphans: graph.orphans.clone(),
        dangling: graph.dangling.clone(),
        unguarded_sensitive: graph.unguarded_sensitive.clone(),
        broke: graph_diff
            .map(|value| value.broke.clone())
            .unwrap_or_default(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MermaidScope<'a> {
    Feature(&'a str),
    Top(usize),
}

/// Render the graph as a Mermaid flowchart.
pub fn render_mermaid(graph: &NavGraph, scope: MermaidScope) -> Result<String> {
    render::mermaid(graph, scope)
}

/// Render a diff as a markdown PR comment.
pub fn render_pr_comment(graph_diff: &NavGraphDiff) -> String {
    render::pr_comment(graph_diff)
}
