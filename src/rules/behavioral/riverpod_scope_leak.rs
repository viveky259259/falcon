//! `riverpod-scope-leak` — detect Riverpod providers that escape their
//! intended scope (e.g. a `ScopedProvider` accessed outside its
//! `ProviderScope`, or a provider's container leaking past widget tear-down).
//!
//! Full provider-scope checking needs generated-provider and import resolution.
//! This first conservative slice catches provider factory bodies that allocate
//! leak-prone resources but neither use an auto-dispose provider nor register a
//! `ref.onDispose` cleanup hook.

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

    fn check(&self, _root: Node, source: &str, file: &Path) -> Vec<Issue> {
        provider_blocks(source)
            .into_iter()
            .filter(|block| provider_factory_can_leak(&block.text))
            .map(|block| Issue {
                rule: self.name().to_string(),
                message: "Riverpod provider creates a subscription/controller/timer without autoDispose or ref.onDispose cleanup.".to_string(),
                severity: self.default_severity(),
                file: file.to_path_buf(),
                line: block.line,
                column: 1,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderBlock {
    line: usize,
    text: String,
}

fn provider_blocks(source: &str) -> Vec<ProviderBlock> {
    let lines: Vec<&str> = source.lines().collect();
    let mut blocks = Vec::new();
    let mut idx = 0;

    while idx < lines.len() {
        if !looks_like_provider_declaration(lines[idx]) {
            idx += 1;
            continue;
        }

        let start = idx;
        let mut text = String::new();
        let mut brace_depth = 0isize;
        let mut saw_body = false;

        while idx < lines.len() {
            let line = lines[idx];
            text.push_str(line);
            text.push('\n');

            brace_depth += line.matches('{').count() as isize;
            brace_depth -= line.matches('}').count() as isize;
            saw_body |= line.contains('{');

            if (saw_body && brace_depth <= 0) || (!saw_body && line.trim_end().ends_with(';')) {
                break;
            }

            idx += 1;
        }

        blocks.push(ProviderBlock {
            line: start + 1,
            text,
        });
        idx += 1;
    }

    blocks
}

fn looks_like_provider_declaration(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.contains("ProviderScope") || !trimmed.contains('=') {
        return false;
    }

    trimmed.contains("Provider(")
        || trimmed.contains("Provider<")
        || trimmed.contains("Provider.autoDispose")
        || trimmed.contains("AutoDisposeProvider")
}

fn provider_factory_can_leak(block: &str) -> bool {
    creates_leak_prone_resource(block)
        && !has_auto_dispose(block)
        && !block.contains("ref.onDispose")
}

fn creates_leak_prone_resource(block: &str) -> bool {
    [
        ".listen(",
        "StreamSubscription",
        "StreamController(",
        "Timer(",
        "Timer.periodic(",
        "AnimationController(",
        "TextEditingController(",
    ]
    .iter()
    .any(|pattern| block.contains(pattern))
}

fn has_auto_dispose(block: &str) -> bool {
    block.contains("AutoDisposeProvider") || block.contains("Provider.autoDispose")
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
    fn provider_returning_subscription_without_cleanup_is_flagged() {
        let rule = RiverpodScopeLeak;
        let source = r#"
final subscriptionProvider = Provider((ref) {
  return stream.listen((event) {});
});
"#;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let issues = rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"));

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "riverpod-scope-leak");
        assert_eq!(issues[0].line, 2);
    }

    #[test]
    fn provider_with_ref_on_dispose_is_allowed() {
        let issues = run(r#"
final subscriptionProvider = Provider((ref) {
  final sub = stream.listen((event) {});
  ref.onDispose(sub.cancel);
  return sub;
});
"#);

        assert!(issues.is_empty());
    }

    #[test]
    fn auto_dispose_provider_with_subscription_is_allowed() {
        let issues = run(r#"
final subscriptionProvider = AutoDisposeProvider((ref) {
  return stream.listen((event) {});
});

final timerProvider = Provider.autoDispose((ref) {
  return Timer.periodic(Duration(seconds: 1), (_) {});
});
"#);

        assert!(issues.is_empty());
    }

    #[test]
    fn provider_scope_and_plain_provider_values_are_ignored() {
        let issues = run(r#"
void main() {
  ProviderScope(child: MyApp());
}

final counterProvider = StateProvider<int>((ref) => 0);
"#);

        assert!(issues.is_empty());
    }

    fn run(source: &str) -> Vec<Issue> {
        let rule = RiverpodScopeLeak;
        let mut parser = DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        rule.check(tree.root_node(), source, &PathBuf::from("lib/foo.dart"))
    }
}
