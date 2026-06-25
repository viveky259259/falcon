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
    by_name: HashMap<String, usize>,
}

impl ResolverIndex {
    pub fn new(classes: Vec<ResolvedClass>) -> Self {
        let by_name = classes
            .iter()
            .enumerate()
            .map(|(idx, class)| (class.name.clone(), idx))
            .collect();
        Self { classes, by_name }
    }

    pub fn classes(&self) -> &[ResolvedClass] {
        &self.classes
    }

    pub fn class(&self, name: &str) -> Option<&ResolvedClass> {
        self.by_name.get(name).map(|idx| &self.classes[*idx])
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
