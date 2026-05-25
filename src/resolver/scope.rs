use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Scope {
    pub symbols: HashMap<String, SymbolInfo>,
    pub parent: Option<Box<Scope>>,
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub is_used: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Class,
    Function,
    Method,
    Variable,
    Enum,
    Mixin,
    Extension,
    TypeAlias,
    Parameter,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            parent: None,
        }
    }

    pub fn child(parent: Scope) -> Self {
        Self {
            symbols: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    pub fn define(&mut self, name: String, kind: SymbolKind, line: usize) {
        self.symbols.insert(
            name.clone(),
            SymbolInfo {
                name,
                kind,
                line,
                is_used: false,
            },
        );
    }

    pub fn lookup(&self, name: &str) -> Option<&SymbolInfo> {
        self.symbols
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup(name)))
    }

    pub fn mark_used(&mut self, name: &str) {
        if let Some(sym) = self.symbols.get_mut(name) {
            sym.is_used = true;
        } else if let Some(ref mut parent) = self.parent {
            parent.mark_used(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scope_with_symbol(name: &str, kind: SymbolKind, line: usize) -> Scope {
        let mut s = Scope::new();
        s.define(name.to_string(), kind, line);
        s
    }

    #[test]
    fn new_creates_empty_scope_with_no_parent() {
        let s = Scope::new();
        assert!(s.symbols.is_empty());
        assert!(s.parent.is_none());
    }

    #[test]
    fn child_creates_scope_with_parent() {
        let parent = Scope::new();
        let child = Scope::child(parent);
        assert!(child.parent.is_some());
        assert!(child.symbols.is_empty());
    }

    #[test]
    fn define_inserts_symbol_with_is_used_false() {
        let mut s = Scope::new();
        s.define("x".to_string(), SymbolKind::Variable, 5);
        let sym = s.lookup("x").expect("symbol must exist");
        assert_eq!(sym.name, "x");
        assert_eq!(sym.kind, SymbolKind::Variable);
        assert_eq!(sym.line, 5);
        assert!(!sym.is_used);
    }

    #[test]
    fn define_overwrites_existing_symbol() {
        let mut s = Scope::new();
        s.define("x".to_string(), SymbolKind::Variable, 1);
        s.define("x".to_string(), SymbolKind::Variable, 99);
        let sym = s.lookup("x").expect("symbol must exist");
        assert_eq!(sym.line, 99);
    }

    #[test]
    fn lookup_returns_none_for_missing() {
        let s = Scope::new();
        assert!(s.lookup("missing").is_none());
    }

    #[test]
    fn lookup_finds_symbol_in_parent_scope() {
        let parent = make_scope_with_symbol("foo", SymbolKind::Function, 10);
        let child = Scope::child(parent);
        let sym = child.lookup("foo").expect("symbol must be found in parent");
        assert_eq!(sym.name, "foo");
        assert_eq!(sym.line, 10);
    }

    #[test]
    fn lookup_returns_local_symbol_over_parent() {
        let parent = make_scope_with_symbol("bar", SymbolKind::Variable, 1);
        let mut child = Scope::child(parent);
        child.define("bar".to_string(), SymbolKind::Variable, 42);
        let sym = child.lookup("bar").expect("symbol must exist");
        // child's definition wins; line 42 distinguishes it from the parent's line 1
        assert_eq!(sym.line, 42);
    }

    #[test]
    fn mark_used_flips_is_used_in_local_scope() {
        let mut s = Scope::new();
        s.define("v".to_string(), SymbolKind::Variable, 3);
        s.mark_used("v");
        let sym = s.lookup("v").expect("symbol must exist");
        assert!(sym.is_used);
    }

    #[test]
    fn mark_used_walks_up_to_parent() {
        let parent = make_scope_with_symbol("pvar", SymbolKind::Variable, 7);
        let mut child = Scope::child(parent);
        child.mark_used("pvar");
        // lookup walks up to the parent's SymbolInfo and it must now be marked used
        let sym = child.lookup("pvar").expect("symbol must be found via parent");
        assert!(sym.is_used);
    }

    #[test]
    fn mark_used_is_noop_for_unknown_name() {
        let mut s = Scope::new();
        // should not panic
        s.mark_used("nonexistent");
        assert!(s.lookup("nonexistent").is_none());
    }

    #[test]
    fn symbol_kind_variants_are_equatable() {
        assert_eq!(SymbolKind::Class, SymbolKind::Class);
        assert_eq!(SymbolKind::Function, SymbolKind::Function);
        assert_eq!(SymbolKind::Method, SymbolKind::Method);
        assert_ne!(SymbolKind::Class, SymbolKind::Function);
        assert_ne!(SymbolKind::Variable, SymbolKind::Enum);
        assert_ne!(SymbolKind::Mixin, SymbolKind::Extension);
    }
}
