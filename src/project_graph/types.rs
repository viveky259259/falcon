use serde::{Deserialize, Serialize};
use tree_sitter::Node;

/// Versioned, deterministic source observation. IDs are relative to the analyzed root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGraph {
    pub schema_version: u32,
    pub extractor_version: String,
    pub snapshot_digest: String,
    pub package_name: Option<String>,
    pub excluded_patterns: Vec<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub diagnostics: Vec<GraphDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Project,
    File,
    Class,
    Mixin,
    Enum,
    Extension,
    TypeAlias,
    Function,
    Method,
    Constructor,
    Variable,
    Parameter,
    TypeParameter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
    pub path: Option<String>,
    pub source_digest: Option<String>,
    pub span: Option<SourceSpan>,
}

/// One-based lines/byte columns, plus zero-based UTF-8 byte offsets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}
impl SourceSpan {
    pub(super) fn at(node: Node) -> Self {
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            line: node.start_position().row + 1,
            column: node.start_position().column + 1,
            end_line: node.end_position().row + 1,
            end_column: node.end_position().column + 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Imports,
    Exports,
    Part,
    Extends,
    MixesIn,
    Implements,
    Calls,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Observed,
    Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub kind: EdgeKind,
    pub from: String,
    pub to: String,
    pub evidence: EvidenceKind,
    pub path: String,
    pub source_digest: String,
    pub span: SourceSpan,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDiagnostic {
    pub kind: String,
    pub path: String,
    pub target: String,
    pub message: String,
    pub span: SourceSpan,
}

pub(super) struct ParsedFile {
    pub path: String,
    pub digest: String,
    pub source: String,
    pub tree: tree_sitter::Tree,
    pub declarations: Vec<Declaration>,
    pub directives: Vec<Directive>,
    pub relations: Vec<Relation>,
}
pub(super) struct Declaration {
    pub id: String,
    pub name: String,
    pub qualified: String,
    pub kind: NodeKind,
    pub span: SourceSpan,
    pub owner: String,
}
pub(super) struct Directive {
    pub kind: EdgeKind,
    pub uri: String,
    pub prefix: Option<String>,
    pub show: Option<Vec<String>>,
    pub hide: Vec<String>,
    pub conditional: bool,
    pub span: SourceSpan,
    pub target: Option<String>,
}
pub(super) struct Relation {
    pub kind: EdgeKind,
    pub from: String,
    pub target: String,
    pub span: SourceSpan,
}
