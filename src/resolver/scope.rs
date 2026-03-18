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
