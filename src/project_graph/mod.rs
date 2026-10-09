//! Deterministic Dart source relationships, with explicit provenance and uncertainty.
mod extract;
mod resolve;
mod types;
pub use types::{
    EdgeKind, EvidenceKind, GraphDiagnostic, GraphEdge, GraphNode, NodeKind, ProjectGraph,
    SourceSpan,
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use types::ParsedFile;

const EXTRACTOR_VERSION: &str = "falcon.dart-graph/1";
static WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn build(root: &Path, files: Vec<PathBuf>, excludes: &[String]) -> Result<ProjectGraph> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Reading graph root {}", root.display()))?;
    let base = if root.is_file() {
        root.parent().unwrap_or(&root)
    } else {
        &root
    };
    let package_name = package_name(base)?;
    let mut parsed = Vec::new();
    for path in files {
        let canonical = path
            .canonicalize()
            .with_context(|| format!("Reading graph file {}", path.display()))?;
        let relative = canonical
            .strip_prefix(base)
            .with_context(|| format!("Graph file {} escapes project root", path.display()))?;
        let relative = relative
            .to_str()
            .context("Graph paths must be UTF-8")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let source = fs::read_to_string(&canonical)
            .with_context(|| format!("Reading graph source {}", path.display()))?;
        let tree = crate::parser::DartParser::new()?
            .parse(&source)
            .with_context(|| format!("Parsing graph source {}", path.display()))?;
        parsed.push(ParsedFile {
            path: relative,
            digest: digest(source.as_bytes()),
            source,
            tree,
            declarations: Vec::new(),
            directives: Vec::new(),
            relations: Vec::new(),
        });
    }
    parsed.sort_by(|a, b| a.path.cmp(&b.path));
    let mut excluded_patterns = excludes.to_vec();
    excluded_patterns.sort();
    excluded_patterns.dedup();
    let snapshot = serde_json::to_vec(&(
        EXTRACTOR_VERSION,
        &package_name,
        &excluded_patterns,
        parsed
            .iter()
            .map(|f| (&f.path, &f.digest))
            .collect::<Vec<_>>(),
    ))?;
    let project_id = identity(&["project"]);
    let mut graph = ProjectGraph {
        schema_version: 1,
        extractor_version: EXTRACTOR_VERSION.into(),
        snapshot_digest: digest(&snapshot),
        package_name: package_name.clone(),
        excluded_patterns,
        nodes: vec![GraphNode {
            id: project_id.clone(),
            kind: NodeKind::Project,
            name: ".".into(),
            qualified_name: ".".into(),
            path: None,
            source_digest: None,
            span: None,
        }],
        edges: Vec::new(),
        diagnostics: Vec::new(),
    };
    for file in &mut parsed {
        extract::extract(file);
        append_file(file, &project_id, &mut graph);
    }
    resolve::link(&mut parsed, package_name.as_deref(), &mut graph);
    graph.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    graph.edges.sort_by(|a, b| a.id.cmp(&b.id));
    graph.edges.dedup_by(|a, b| a.id == b.id);
    graph.diagnostics.sort_by(|a, b| {
        (&a.path, a.span.start_byte, &a.kind, &a.target).cmp(&(
            &b.path,
            b.span.start_byte,
            &b.kind,
            &b.target,
        ))
    });
    Ok(graph)
}
fn append_file(file: &ParsedFile, project_id: &str, graph: &mut ProjectGraph) {
    let file_id = identity(&["file", &file.path]);
    let span = SourceSpan::at(file.tree.root_node());
    graph.nodes.push(GraphNode {
        id: file_id.clone(),
        kind: NodeKind::File,
        name: file.path.clone(),
        qualified_name: file.path.clone(),
        path: Some(file.path.clone()),
        source_digest: Some(file.digest.clone()),
        span: Some(span.clone()),
    });
    graph.edges.push(resolve::edge(
        EdgeKind::Contains,
        project_id,
        &file_id,
        file,
        span,
        EvidenceKind::Observed,
    ));
    for declaration in &file.declarations {
        graph.nodes.push(GraphNode {
            id: declaration.id.clone(),
            kind: declaration.kind,
            name: declaration.name.clone(),
            qualified_name: declaration.qualified.clone(),
            path: Some(file.path.clone()),
            source_digest: Some(file.digest.clone()),
            span: Some(declaration.span.clone()),
        });
        graph.edges.push(resolve::edge(
            EdgeKind::Contains,
            &declaration.owner,
            &declaration.id,
            file,
            declaration.span.clone(),
            EvidenceKind::Observed,
        ));
    }
    crate::parser::walk_tree(file.tree.root_node(), &mut |node| {
        if node.is_error() || node.is_missing() {
            graph.diagnostics.push(resolve::diagnostic(
                "syntax_error",
                &file.path,
                node.kind(),
                SourceSpan::at(node),
                "Parser recovery: graph coverage for this file may be incomplete",
            ));
        }
    });
}
fn package_name(root: &Path) -> Result<Option<String>> {
    let path = root.join("pubspec.yaml");
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?;
    let document: serde_yaml::Value =
        serde_yaml::from_str(&text).with_context(|| format!("Parsing {}", path.display()))?;
    Ok(document
        .get("name")
        .and_then(|name| name.as_str())
        .map(ToOwned::to_owned))
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn identity(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
impl ProjectGraph {
    /// Publish complete JSON with a same-directory atomic rename. Parent must exist.
    pub fn write_to(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = path
            .file_name()
            .context("Graph output requires a filename")?;
        let sequence = WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp = parent.join(format!(
            ".{}.{}.{}.tmp",
            name.to_string_lossy(),
            std::process::id(),
            sequence
        ));
        // Cleanup is safe only after this invocation has created the temporary file.
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .with_context(|| format!("Creating graph temp file for {}", path.display()))?;
        let result = (|| -> Result<()> {
            serde_json::to_writer_pretty(&mut output, self)?;
            output.write_all(b"\n")?;
            output.sync_all()?;
            fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() && temp.exists() {
            if let Err(error) = fs::remove_file(&temp) {
                log::warn!("Removing graph temp file {}: {}", temp.display(), error);
            }
        }
        result.with_context(|| format!("Writing project graph {}", path.display()))
    }
    /// Store the conventional project artifact, creating only its output directory.
    pub fn save_project(&self, root: &Path) -> Result<PathBuf> {
        let directory = root.join(".falcon-data");
        fs::create_dir_all(&directory)
            .with_context(|| format!("Creating graph directory {}", directory.display()))?;
        let path = directory.join("project-graph.json");
        self.write_to(&path)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_temp_creation_preserves_the_existing_file() {
        let root = tempfile::tempdir().unwrap();
        let graph = build(root.path(), Vec::new(), &[]).unwrap();
        let sequence = WRITE_SEQUENCE.load(Ordering::Relaxed);
        let temp = root
            .path()
            .join(format!(".out.json.{}.{sequence}.tmp", std::process::id()));
        fs::write(&temp, "existing unrelated file").unwrap();
        assert!(graph.write_to(&root.path().join("out.json")).is_err());
        assert_eq!(fs::read_to_string(temp).unwrap(), "existing unrelated file");
    }
}
