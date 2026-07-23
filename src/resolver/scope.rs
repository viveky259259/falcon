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

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
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
    //! Tests for the in-memory scope primitive used by the (in-progress)
    //! resolver layer. The richer name/import resolver from EPIC 3.1 will
    //! build on top of these primitives.
    use super::*;

    #[test]
    fn define_lookup_and_redefine_is_last_write_wins() {
        let mut s = Scope::new();
        s.define("x".to_string(), SymbolKind::Variable, 10);
        let info = s.lookup("x").expect("x should resolve");
        assert_eq!(info.kind, SymbolKind::Variable);
        assert_eq!(info.line, 10);
        assert!(!info.is_used);
        assert!(s.lookup("y").is_none(), "y was never defined");

        // Redefining overwrites kind + line (behavioural pin).
        s.define("x".to_string(), SymbolKind::Parameter, 42);
        let info = s.lookup("x").unwrap();
        assert_eq!(info.kind, SymbolKind::Parameter);
        assert_eq!(info.line, 42);
    }

    #[test]
    fn child_scope_chain_falls_back_and_mark_used_propagates() {
        let mut parent = Scope::new();
        parent.define("Widget".to_string(), SymbolKind::Class, 1);
        parent.define("foo".to_string(), SymbolKind::Function, 2);
        let mut child = Scope::child(parent);
        child.define("local".to_string(), SymbolKind::Variable, 5);

        // Local + parent both resolve through the chain.
        assert!(child.lookup("local").is_some());
        assert_eq!(child.lookup("Widget").unwrap().kind, SymbolKind::Class);
        assert!(child.lookup("Nope").is_none());

        // mark_used from the child bubbles up to the parent's symbol.
        child.mark_used("foo");
        assert!(child.lookup("foo").unwrap().is_used);
        // No panic on unknown name.
        child.mark_used("never_defined");
    }
}
