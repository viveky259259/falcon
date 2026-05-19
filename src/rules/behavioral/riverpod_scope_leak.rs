//! `riverpod-scope-leak` — detect Riverpod providers that escape their
//! intended scope (e.g. a `ScopedProvider` accessed outside its
//! `ProviderScope`, or a provider's container leaking past widget tear-down).
//!
//! Identifying Riverpod providers reliably requires resolving symbols across
//! files (which generator output declares the provider, what scope it lives
//! in). Without the resolver, any CST-only heuristic would either miss the
//! interesting cases or fire on plain field accesses named `provider`.
//!
//! TODO(resolver): see EPIC 3.1 — currently returns empty until resolver lands.

use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct RiverpodScopeLeak;

impl Rule for RiverpodScopeLeak {
    fn name(&self) -> &'static str {
        "riverpod-scope-leak"
    }

    fn description(&self) -> &'static str {
        "Riverpod providers must not be read outside their ProviderScope. Scope leaks cause stale state and silent rebuilds."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, _root: Node, _source: &str, _file: &Path) -> Vec<Issue> {
        // TODO(resolver): see EPIC 3.1 — currently returns empty until resolver lands.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DartParser;
    use std::path::PathBuf;

    #[test]
    fn rule_metadata_is_present() {
        let rule = RiverpodScopeLeak;
        assert_eq!(rule.name(), "riverpod-scope-leak");
        assert!(!rule.description().is_empty());
        assert!(matches!(rule.default_severity(), Severity::Error));
    }

    #[test]
    fn stub_check_returns_empty_without_panic() {
        let rule = RiverpodScopeLeak;
        let source = r#"
final counterProvider = StateProvider<int>((ref) => 0);
void main() {
  ProviderScope(child: MyApp());
}
"#;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let issues = rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"));
        assert!(issues.is_empty(), "stub must not emit issues yet");
    }
}
