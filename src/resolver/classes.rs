use crate::parser::{dart_ast, find_descendants_by_kind, node_start_line};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedClass {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub superclass: Option<String>,
    pub mixins: Vec<String>,
    pub interfaces: Vec<String>,
    pub methods: Vec<String>,
}

impl ResolvedClass {
    fn direct_type_names(&self) -> impl Iterator<Item = &str> {
        self.superclass
            .iter()
            .map(String::as_str)
            .chain(self.mixins.iter().map(String::as_str))
            .chain(self.interfaces.iter().map(String::as_str))
    }

    pub fn has_method(&self, name: &str) -> bool {
        self.methods.iter().any(|method| method == name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolverIndex {
    classes: Vec<ResolvedClass>,
    by_name: HashMap<String, Vec<usize>>,
}

impl ResolverIndex {
    pub fn new(classes: Vec<ResolvedClass>) -> Self {
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, class) in classes.iter().enumerate() {
            by_name.entry(class.name.clone()).or_default().push(idx);
        }
        Self { classes, by_name }
    }

    pub fn classes(&self) -> &[ResolvedClass] {
        &self.classes
    }

    pub fn class(&self, name: &str) -> Option<&ResolvedClass> {
        self.by_name
            .get(name)
            .and_then(|indices| indices.first())
            .map(|idx| &self.classes[*idx])
    }

    pub fn classes_named(&self, name: &str) -> Vec<&ResolvedClass> {
        self.by_name
            .get(name)
            .into_iter()
            .flatten()
            .map(|idx| &self.classes[*idx])
            .collect()
    }

    pub fn resolver_for_file<'a>(&'a self, file: &'a Path, source: &'a str) -> Resolver<'a> {
        Resolver {
            index: self,
            file,
            source,
        }
    }

    /// Simple-name subtype check for the first resolver slice.
    ///
    /// This intentionally ignores imports and library namespaces. It is enough
    /// to power direct and transitive class-hierarchy facts while the richer
    /// EPIC 3.1 resolver is still pending.
    pub fn is_subtype_of(&self, class_name: &str, ancestor_name: &str) -> bool {
        if class_name == ancestor_name {
            return true;
        }
        self.is_subtype_of_inner(class_name, ancestor_name, &mut HashSet::new())
    }

    fn is_subtype_of_inner(
        &self,
        class_name: &str,
        ancestor_name: &str,
        seen: &mut HashSet<String>,
    ) -> bool {
        if !seen.insert(class_name.to_string()) {
            return false;
        }

        let Some(class) = self.class(class_name) else {
            return false;
        };

        for direct in class.direct_type_names() {
            if direct == ancestor_name {
                return true;
            }
            if self.is_subtype_of_inner(direct, ancestor_name, seen) {
                return true;
            }
        }

        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSymbol<'a> {
    pub name: String,
    pub kind: ResolvedSymbolKind,
    pub declaring_library: PathBuf,
    pub type_hint: Option<String>,
    pub class: Option<&'a ResolvedClass>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedSymbolKind {
    Class,
}

pub struct Resolver<'a> {
    index: &'a ResolverIndex,
    file: &'a Path,
    source: &'a str,
}

impl<'a> Resolver<'a> {
    pub fn resolve(&self, node: Node) -> Option<ResolvedSymbol<'a>> {
        let name = self.node_name(node)?;
        let class = self.resolve_class_name(&name)?;
        Some(ResolvedSymbol {
            name,
            kind: ResolvedSymbolKind::Class,
            declaring_library: class.file.clone(),
            type_hint: None,
            class: Some(class),
        })
    }

    pub fn is_ambiguous(&self, node: Node) -> bool {
        self.node_name(node)
            .is_some_and(|name| self.is_ambiguous_class_name(&name))
    }

    pub fn resolve_class_name(&self, name: &str) -> Option<&'a ResolvedClass> {
        let matches = self.index.classes_named(name);
        let same_file: Vec<&ResolvedClass> = matches
            .iter()
            .copied()
            .filter(|class| same_path(&class.file, self.file))
            .collect();

        if same_file.len() == 1 {
            return same_file.first().copied();
        }

        match matches.as_slice() {
            [class] => Some(*class),
            _ => None,
        }
    }

    pub fn is_ambiguous_class_name(&self, name: &str) -> bool {
        let matches = self.index.classes_named(name);
        if matches.len() <= 1 {
            return false;
        }

        let same_file_count = matches
            .iter()
            .filter(|class| same_path(&class.file, self.file))
            .count();
        same_file_count != 1
    }

    pub fn is_subtype_of(&self, class: &ResolvedClass, ancestor_name: &str) -> bool {
        if class.name == ancestor_name {
            return true;
        }
        self.is_subtype_of_inner(class, ancestor_name, &mut HashSet::new())
    }

    fn is_subtype_of_inner(
        &self,
        class: &ResolvedClass,
        ancestor_name: &str,
        seen: &mut HashSet<String>,
    ) -> bool {
        if !seen.insert(class.name.clone()) {
            return false;
        }

        for direct in class.direct_type_names() {
            if direct == ancestor_name {
                return true;
            }
            if self.is_ambiguous_class_name(direct) {
                return false;
            }
            if let Some(parent) = self.resolve_class_name(direct) {
                if self.is_subtype_of_inner(parent, ancestor_name, seen) {
                    return true;
                }
            }
        }

        false
    }

    fn node_name(&self, node: Node) -> Option<String> {
        if node.kind() == "class_declaration" {
            return dart_ast::get_declaration_name(node, self.source).map(ToString::to_string);
        }

        if node.kind() == "identifier" || node.kind() == "type_identifier" {
            return node
                .utf8_text(self.source.as_bytes())
                .ok()
                .map(ToString::to_string);
        }

        None
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    normalize_for_compare(left) == normalize_for_compare(right)
}

fn normalize_for_compare(path: &Path) -> PathBuf {
    path.components().collect()
}

pub fn collect_classes(root: Node, source: &str, file: &Path) -> Vec<ResolvedClass> {
    find_descendants_by_kind(root, "class_declaration")
        .into_iter()
        .filter_map(|node| {
            let name = dart_ast::get_declaration_name(node, source)?.to_string();
            Some(ResolvedClass {
                name,
                file: file.to_path_buf(),
                line: node_start_line(node),
                superclass: dart_ast::get_class_superclass(node, source),
                mixins: dart_ast::get_class_mixins(node, source),
                interfaces: dart_ast::get_class_interfaces(node, source),
                methods: dart_ast::get_method_names(node, source),
            })
        })
        .collect()
}
