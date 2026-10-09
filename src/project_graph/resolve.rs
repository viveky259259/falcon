use super::{identity, types::*};
use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path};

pub(super) fn link(files: &mut [ParsedFile], package: Option<&str>, graph: &mut ProjectGraph) {
    let inventory: BTreeSet<_> = files.iter().map(|file| file.path.clone()).collect();
    for file in files.iter_mut() {
        for directive in &mut file.directives {
            let target = resolve_uri(&file.path, &directive.uri, package);
            directive.target = target.filter(|path| inventory.contains(path));
            if directive.target.is_none() {
                let external = directive.uri.contains(':');
                graph.diagnostics.push(diagnostic(
                    if external {
                        "external_dependency"
                    } else {
                        "unresolved_dependency"
                    },
                    &file.path,
                    &directive.uri,
                    directive.span.clone(),
                    "Dependency is outside the selected project inventory",
                ));
            }
            if directive.conditional {
                graph.diagnostics.push(diagnostic(
                    "conditional_dependency",
                    &file.path,
                    &directive.uri,
                    directive.span.clone(),
                    "Conditional URI observed; runtime selection is not inferred",
                ));
            }
        }
    }
    for file in files.iter() {
        for directive in &file.directives {
            if let Some(target) = &directive.target {
                graph.edges.push(edge(
                    directive.kind,
                    &identity(&["file", &file.path]),
                    &identity(&["file", target]),
                    file,
                    directive.span.clone(),
                    EvidenceKind::Inferred,
                ));
            }
        }
        for relation in &file.relations {
            let owner = file.declarations.iter().find(|d| d.id == relation.from);
            let shadowed = file.declarations.iter().any(|d| {
                let shadows = if relation.kind == EdgeKind::Calls {
                    matches!(d.kind, NodeKind::Variable | NodeKind::Parameter)
                } else {
                    d.kind == NodeKind::TypeParameter
                };
                shadows
                    && d.name == relation.target
                    && (d.owner == relation.from
                        || owner.is_some_and(|owner| d.owner == owner.owner))
            });
            let candidates = if shadowed || relation.target.starts_with("<receiver>") {
                Vec::new()
            } else {
                candidates(files, file, relation, owner)
            };
            match candidates.as_slice() {
                [target] => graph.edges.push(edge(
                    relation.kind,
                    &relation.from,
                    &target.id,
                    file,
                    relation.span.clone(),
                    EvidenceKind::Inferred,
                )),
                _ => graph.diagnostics.push(diagnostic(
                    if candidates.is_empty() {
                        "unresolved_symbol"
                    } else {
                        "ambiguous_symbol"
                    },
                    &file.path,
                    &relation.target,
                    relation.span.clone(),
                    "No unique visible declaration; no relationship asserted",
                )),
            }
        }
    }
}
fn candidates<'a>(
    files: &'a [ParsedFile],
    file: &'a ParsedFile,
    relation: &Relation,
    owner: Option<&Declaration>,
) -> Vec<&'a Declaration> {
    let (prefix, name) = relation
        .target
        .split_once('.')
        .map_or((None, relation.target.as_str()), |(prefix, name)| {
            (Some(prefix), name)
        });
    let callable = relation.kind == EdgeKind::Calls;
    let valid_kind = |d: &&Declaration| {
        if callable {
            matches!(d.kind, NodeKind::Function | NodeKind::Method)
        } else {
            matches!(
                d.kind,
                NodeKind::Class | NodeKind::Mixin | NodeKind::Enum | NodeKind::TypeAlias
            )
        }
    };
    if prefix.is_none() {
        let local: Vec<_> = file
            .declarations
            .iter()
            .filter(|d| d.name == name)
            .filter(valid_kind)
            .filter(|d| {
                callable
                    && d.owner != identity(&["file", &file.path])
                    && (d.owner == relation.from
                        || owner.is_some_and(|owner| d.owner == owner.owner))
            })
            .collect();
        if !local.is_empty() {
            return local;
        }
    }
    let mut visible = Vec::new();
    if prefix.is_none() {
        collect_library(
            files,
            &file.path,
            name,
            &mut HashSet::new(),
            &mut visible,
            false,
        );
        visible.retain(valid_kind);
        if !visible.is_empty() {
            return visible;
        }
    }
    for directive in library_files(files, &file.path)
        .iter()
        .flat_map(|scope| scope.directives.iter())
    {
        if directive.kind != EdgeKind::Imports
            || directive.prefix.as_deref() != prefix
            || !admits(directive, name)
        {
            continue;
        }
        if let Some(target) = &directive.target {
            collect_library(files, target, name, &mut HashSet::new(), &mut visible, true);
        }
    }
    visible.retain(valid_kind);
    visible.sort_by(|a, b| a.id.cmp(&b.id));
    visible.dedup_by(|a, b| a.id == b.id);
    visible
}
fn library_files<'a>(files: &'a [ParsedFile], start: &str) -> Vec<&'a ParsedFile> {
    let mut visited = HashSet::new();
    let mut queue = vec![start.to_owned()];
    let mut result = Vec::new();
    while let Some(path) = queue.pop() {
        if !visited.insert(path.clone()) {
            continue;
        }
        let Some(file) = files.iter().find(|file| file.path == path) else {
            continue;
        };
        result.push(file);
        queue.extend(
            file.directives
                .iter()
                .filter(|d| d.kind == EdgeKind::Part)
                .filter_map(|d| d.target.clone()),
        );
        queue.extend(
            files
                .iter()
                .filter(|parent| {
                    parent
                        .directives
                        .iter()
                        .any(|d| d.kind == EdgeKind::Part && d.target.as_deref() == Some(&path))
                })
                .map(|parent| parent.path.clone()),
        );
    }
    result
}
fn collect_library<'a>(
    files: &'a [ParsedFile],
    path: &str,
    name: &str,
    visited: &mut HashSet<String>,
    result: &mut Vec<&'a Declaration>,
    imported: bool,
) {
    if !visited.insert(path.to_owned()) || (imported && name.starts_with('_')) {
        return;
    }
    let Some(file) = files.iter().find(|file| file.path == path) else {
        return;
    };
    let file_id = identity(&["file", path]);
    result.extend(
        file.declarations
            .iter()
            .filter(|d| d.name == name && d.owner == file_id),
    );
    // Parts belong to the declaring library, including its private declarations.
    for parent in files.iter().filter(|parent| {
        parent
            .directives
            .iter()
            .any(|d| d.kind == EdgeKind::Part && d.target.as_deref() == Some(path))
    }) {
        collect_library(files, &parent.path, name, visited, result, imported);
    }
    for directive in &file.directives {
        if !(directive.kind == EdgeKind::Part || (imported && directive.kind == EdgeKind::Exports))
            || !admits(directive, name)
        {
            continue;
        }
        if let Some(target) = &directive.target {
            collect_library(files, target, name, visited, result, imported);
        }
    }
}
fn admits(directive: &Directive, name: &str) -> bool {
    !directive.conditional
        && directive
            .show
            .as_ref()
            .is_none_or(|values| values.iter().any(|value| value == name))
        && !directive.hide.iter().any(|value| value == name)
}
fn resolve_uri(file: &str, uri: &str, package: Option<&str>) -> Option<String> {
    let path = if let Some(uri) = uri.strip_prefix("package:") {
        let (name, path) = uri.split_once('/')?;
        if Some(name) != package {
            return None;
        }
        Path::new("lib").join(path)
    } else {
        if uri.contains(':') || uri.contains('\\') || uri.contains('$') || uri.starts_with('/') {
            return None;
        }
        Path::new(file).parent()?.join(uri)
    };
    let mut parts = Vec::new();
    for part in path.components() {
        match part {
            Component::Normal(value) => parts.push(value.to_str()?),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            _ => return None,
        }
    }
    Some(parts.join("/"))
}
pub(super) fn edge(
    kind: EdgeKind,
    from: &str,
    to: &str,
    file: &ParsedFile,
    span: SourceSpan,
    evidence: EvidenceKind,
) -> GraphEdge {
    let id = identity(&[
        "edge",
        &format!("{kind:?}"),
        from,
        to,
        &span.start_byte.to_string(),
    ]);
    GraphEdge {
        id,
        kind,
        from: from.into(),
        to: to.into(),
        evidence,
        path: file.path.clone(),
        source_digest: file.digest.clone(),
        span,
    }
}
pub(super) fn diagnostic(
    kind: &str,
    path: &str,
    target: &str,
    span: SourceSpan,
    message: &str,
) -> GraphDiagnostic {
    GraphDiagnostic {
        kind: kind.into(),
        path: path.into(),
        target: target.into(),
        span,
        message: message.into(),
    }
}
