//! Architecture map — a static, visual overview of how a Flutter/Dart codebase
//! is organized.
//!
//! Builds on the existing `DependencyGraph` (file-level imports) and collapses it
//! to a **module** graph: files are grouped into modules (feature/folder), each
//! module is classified into an architectural layer (presentation / domain /
//! data / core / other), and cross-module imports become weighted edges.
//!
//! Output: a console summary, a Mermaid diagram (layers as subgraphs), and a
//! self-contained HTML page embedding that diagram via mermaid.js.

use crate::incremental::dep_graph::DependencyGraph;
use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Architectural layer a module belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Layer {
    Presentation,
    Domain,
    Data,
    Core,
    Other,
}

impl Layer {
    pub fn label(self) -> &'static str {
        match self {
            Layer::Presentation => "presentation",
            Layer::Domain => "domain",
            Layer::Data => "data",
            Layer::Core => "core",
            Layer::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleNode {
    pub name: String,
    pub layer: Layer,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleEdge {
    pub from: String,
    pub to: String,
    pub weight: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchMapReport {
    pub root: String,
    pub modules: Vec<ModuleNode>,
    pub edges: Vec<ModuleEdge>,
    /// Edges that point "upstream" against clean-architecture direction
    /// (e.g. domain → data, or data → presentation).
    pub layer_violations: Vec<ModuleEdge>,
    pub mermaid: String,
}

/// Derive a module name from a Dart file path relative to `root`.
///
/// Recognises the common Flutter layouts: `lib/features/<mod>/…`,
/// `lib/src/<mod>/…`, `lib/<mod>/…`. Files directly in `lib/` map to `root`.
pub fn module_of(path: &Path, root: &Path) -> String {
    let segments: Vec<String> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
        .collect();

    // Anchor on the `lib/` directory wherever it appears, since dep-graph paths
    // are normalized to absolute. A Dart project's modules all live under `lib/`;
    // anything else is an out-of-tree / package import.
    let _ = root;
    let mut idx = match segments.iter().rposition(|s| s == "lib") {
        Some(i) => i + 1,
        None => return "external".to_string(),
    };

    // Skip common grouping folders to reach the real module name.
    const GROUPING: [&str; 6] = ["features", "feature", "modules", "module", "src", "pages"];
    if let Some(seg) = segments.get(idx) {
        if GROUPING.contains(&seg.as_str()) {
            idx += 1;
        }
    }

    match segments.get(idx) {
        // A file (has an extension) at this position means we're at lib root.
        Some(seg) if seg.ends_with(".dart") => "root".to_string(),
        Some(seg) => seg.clone(),
        None => "root".to_string(),
    }
}

/// Classify a path into an architectural layer by folder keywords.
pub fn classify_layer(path: &Path) -> Layer {
    let lower = path.to_string_lossy().to_lowercase();
    let has = |needle: &str| lower.contains(needle);

    if has("/presentation")
        || has("/ui")
        || has("/widgets")
        || has("/screens")
        || has("/pages")
        || has("/views")
        || has("/view/")
    {
        Layer::Presentation
    } else if has("/domain")
        || has("/usecases")
        || has("/use_cases")
        || has("/entities")
        || (has("/repositories/") && !has("_impl"))
    {
        Layer::Domain
    } else if has("/data")
        || has("/datasources")
        || has("/data_sources")
        || has("/api")
        || has("/network")
        || has("/repository_impl")
        || has("/models")
    {
        Layer::Data
    } else if has("/core") || has("/shared") || has("/common") || has("/utils") || has("/config") {
        Layer::Core
    } else {
        Layer::Other
    }
}

/// Build the module-level architecture map from a project at `root`.
pub fn build_arch_map(root: &Path, exclude: &[glob::Pattern]) -> ArchMapReport {
    let graph = DependencyGraph::build(root, exclude);
    let file_imports: Vec<(PathBuf, Vec<PathBuf>)> = graph
        .imports
        .iter()
        .map(|(file, imps)| (file.clone(), imps.iter().cloned().collect()))
        .collect();
    aggregate(root, &file_imports)
}

/// Pure aggregation step: collapse file-level imports into a module graph.
pub fn aggregate(root: &Path, file_imports: &[(PathBuf, Vec<PathBuf>)]) -> ArchMapReport {
    let mut file_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut layers: BTreeMap<String, Layer> = BTreeMap::new();
    let mut edge_weights: BTreeMap<(String, String), usize> = BTreeMap::new();

    for (file, imports) in file_imports {
        let module = module_of(file, root);
        *file_counts.entry(module.clone()).or_insert(0) += 1;
        // A module's layer is the strongest (lowest-ordinal) classification seen.
        let layer = classify_layer(file);
        layers
            .entry(module.clone())
            .and_modify(|existing| {
                if layer < *existing {
                    *existing = layer;
                }
            })
            .or_insert(layer);

        for imp in imports {
            let target = module_of(imp, root);
            // Classify the import target by its own path so import-only modules
            // (no file of their own in the graph) still get a layer for
            // violation detection.
            let target_layer = classify_layer(imp);
            layers
                .entry(target.clone())
                .and_modify(|existing| {
                    if target_layer < *existing {
                        *existing = target_layer;
                    }
                })
                .or_insert(target_layer);

            if target != module {
                *edge_weights.entry((module.clone(), target)).or_insert(0) += 1;
            }
        }
    }

    let modules: Vec<ModuleNode> = file_counts
        .iter()
        .map(|(name, &file_count)| ModuleNode {
            name: name.clone(),
            layer: *layers.get(name).unwrap_or(&Layer::Other),
            file_count,
        })
        .collect();

    let edges: Vec<ModuleEdge> = edge_weights
        .iter()
        .map(|((from, to), &weight)| ModuleEdge {
            from: from.clone(),
            to: to.clone(),
            weight,
        })
        .collect();

    let layer_violations = edges
        .iter()
        .filter(|e| {
            let from = layers.get(&e.from).copied().unwrap_or(Layer::Other);
            let to = layers.get(&e.to).copied().unwrap_or(Layer::Other);
            is_layer_violation(from, to)
        })
        .cloned()
        .collect();

    let mermaid = render_mermaid(&modules, &edges);

    ArchMapReport {
        root: root.display().to_string(),
        modules,
        edges,
        layer_violations,
        mermaid,
    }
}

/// A dependency violates clean-architecture direction when it points toward a
/// more outer layer. Order (inner→outer): Domain < Data < Presentation.
/// Core is foundational (anyone may import it); Other is unclassified.
fn is_layer_violation(from: Layer, to: Layer) -> bool {
    fn rank(l: Layer) -> Option<u8> {
        match l {
            Layer::Domain => Some(0),
            Layer::Data => Some(1),
            Layer::Presentation => Some(2),
            Layer::Core | Layer::Other => None,
        }
    }
    match (rank(from), rank(to)) {
        (Some(f), Some(t)) => t > f, // importing a more-outer layer is a violation
        _ => false,
    }
}

/// Sanitize a module name into a Mermaid-safe node id.
pub fn mermaid_id(name: &str) -> String {
    let id: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("m_{id}")
}

fn render_mermaid(modules: &[ModuleNode], edges: &[ModuleEdge]) -> String {
    let mut out = String::from("graph TD\n");

    // Group modules into subgraphs by layer.
    for layer in [
        Layer::Presentation,
        Layer::Domain,
        Layer::Data,
        Layer::Core,
        Layer::Other,
    ] {
        let in_layer: Vec<&ModuleNode> = modules.iter().filter(|m| m.layer == layer).collect();
        if in_layer.is_empty() {
            continue;
        }
        out.push_str(&format!("  subgraph {}\n", layer.label()));
        for m in in_layer {
            out.push_str(&format!(
                "    {}[\"{} ({})\"]\n",
                mermaid_id(&m.name),
                m.name,
                m.file_count
            ));
        }
        out.push_str("  end\n");
    }

    for e in edges {
        out.push_str(&format!(
            "  {} --> {}\n",
            mermaid_id(&e.from),
            mermaid_id(&e.to)
        ));
    }
    out
}

/// Write a self-contained HTML page rendering the Mermaid diagram.
pub fn write_html(report: &ArchMapReport, path: &Path) -> Result<()> {
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Falcon Architecture Map</title>\
<script src=\"https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js\"></script>\
<style>body{{font-family:-apple-system,Segoe UI,Roboto,sans-serif;margin:0;background:#0d1117;color:#e6edf3}}\
header{{padding:24px 32px;background:#161b22;border-bottom:1px solid #30363d}}h1{{margin:0;font-size:20px}}\
.summary{{padding:16px 32px;color:#8b949e}}.diagram{{padding:24px 32px;background:#fff}}</style></head>\
<body><header><h1>Falcon — Architecture Map</h1></header>\
<div class=\"summary\"><p>{modules} modules · {edges} dependencies · {viol} layer violations</p></div>\
<div class=\"diagram\"><pre class=\"mermaid\">{mermaid}</pre></div>\
<script>mermaid.initialize({{startOnLoad:true}});</script></body></html>",
        modules = report.modules.len(),
        edges = report.edges.len(),
        viol = report.layer_violations.len(),
        mermaid = html_escape(&report.mermaid),
    );
    std::fs::write(path, html)
        .map_err(|e| anyhow::anyhow!("Failed to write arch-map HTML to {}: {e}", path.display()))?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn print_report(report: &ArchMapReport) {
    println!("{} {}", "Project root:".bright_cyan(), report.root);
    println!("{} {}", "Modules:".bright_cyan(), report.modules.len());
    println!("{} {}", "Dependencies:".bright_cyan(), report.edges.len());

    for layer in [
        Layer::Presentation,
        Layer::Domain,
        Layer::Data,
        Layer::Core,
        Layer::Other,
    ] {
        let in_layer: Vec<&ModuleNode> =
            report.modules.iter().filter(|m| m.layer == layer).collect();
        if in_layer.is_empty() {
            continue;
        }
        println!();
        println!("{}", layer.label().bright_white().bold());
        for m in in_layer {
            println!("  - {} ({} files)", m.name, m.file_count);
        }
    }

    if !report.layer_violations.is_empty() {
        println!();
        println!("{}", "Layer violations (outward dependencies)".red().bold());
        for v in &report.layer_violations {
            println!("  - {} → {} (×{})", v.from, v.to, v.weight);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from("/app")
    }

    #[test]
    fn module_of_handles_feature_first_layout() {
        assert_eq!(
            module_of(&PathBuf::from("/app/lib/features/auth/login.dart"), &root()),
            "auth"
        );
        assert_eq!(
            module_of(&PathBuf::from("/app/lib/src/data/repo.dart"), &root()),
            "data"
        );
        assert_eq!(
            module_of(
                &PathBuf::from("/app/lib/profile/profile_page.dart"),
                &root()
            ),
            "profile"
        );
    }

    #[test]
    fn module_of_maps_lib_root_files_to_root() {
        assert_eq!(
            module_of(&PathBuf::from("/app/lib/main.dart"), &root()),
            "root"
        );
    }

    #[test]
    fn classify_layer_recognises_common_folders() {
        assert_eq!(
            classify_layer(&PathBuf::from(
                "/app/lib/features/auth/presentation/login.dart"
            )),
            Layer::Presentation
        );
        assert_eq!(
            classify_layer(&PathBuf::from(
                "/app/lib/features/auth/domain/usecases/x.dart"
            )),
            Layer::Domain
        );
        assert_eq!(
            classify_layer(&PathBuf::from("/app/lib/features/auth/data/api/x.dart")),
            Layer::Data
        );
        assert_eq!(
            classify_layer(&PathBuf::from("/app/lib/core/utils/x.dart")),
            Layer::Core
        );
        assert_eq!(
            classify_layer(&PathBuf::from("/app/lib/random/x.dart")),
            Layer::Other
        );
    }

    #[test]
    fn mermaid_id_sanitizes_non_alphanumeric() {
        assert_eq!(mermaid_id("auth-feature.v2"), "m_auth_feature_v2");
    }

    #[test]
    fn aggregate_collapses_files_to_modules_and_skips_self_edges() {
        let files = vec![
            (
                PathBuf::from("/app/lib/features/auth/presentation/login.dart"),
                vec![
                    PathBuf::from("/app/lib/features/auth/domain/login_uc.dart"),
                    PathBuf::from("/app/lib/features/auth/presentation/widget.dart"),
                ],
            ),
            (
                PathBuf::from("/app/lib/features/auth/domain/login_uc.dart"),
                vec![PathBuf::from("/app/lib/core/result.dart")],
            ),
        ];
        let report = aggregate(&root(), &files);
        // Both auth files collapse to one module "auth"; core is its own module.
        let auth = report.modules.iter().find(|m| m.name == "auth").unwrap();
        assert_eq!(auth.file_count, 2);
        // Self-edge (auth → auth) is skipped; auth → core remains.
        assert!(report
            .edges
            .iter()
            .any(|e| e.from == "auth" && e.to == "core"));
        assert!(!report.edges.iter().any(|e| e.from == e.to));
    }

    #[test]
    fn aggregate_flags_outward_layer_violation() {
        // domain importing data is an outward (illegal) dependency.
        let files = vec![(
            PathBuf::from("/app/lib/features/x/domain/svc.dart"),
            vec![PathBuf::from("/app/lib/features/y/data/repo.dart")],
        )];
        let report = aggregate(&root(), &files);
        assert_eq!(report.layer_violations.len(), 1);
        assert_eq!(report.layer_violations[0].from, "x");
        assert_eq!(report.layer_violations[0].to, "y");
    }
}
