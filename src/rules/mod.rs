pub mod behavioral;
pub mod bloc;
pub mod common;
pub mod equatable;
pub mod flutter;
pub mod provider;
pub mod severity;

use crate::config::{FalconConfig, Severity};
use crate::reporters::Issue;
use crate::resolver::ResolverIndex;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Node;

#[derive(Default)]
pub struct RuleContext<'a> {
    pub resolver_index: Option<&'a ResolverIndex>,
}

pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue>;
    fn check_with_context(
        &self,
        root: Node,
        source: &str,
        file: &Path,
        _context: &RuleContext<'_>,
    ) -> Vec<Issue> {
        self.check(root, source, file)
    }
    fn configure(&mut self, _options: &HashMap<String, serde_yaml::Value>) {}
}

pub struct RuleRegistry {
    rules: Vec<(Box<dyn Rule>, Severity)>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register_defaults(&mut self, config: &FalconConfig) {
        let configured_rules: HashMap<String, (Severity, HashMap<String, serde_yaml::Value>)> =
            config
                .rules
                .iter()
                .map(|r| (r.name().to_string(), (r.severity(), r.options())))
                .collect();

        let all_rules: Vec<Box<dyn Rule>> = vec![
            // v0.1 Dart rules
            Box::new(common::AvoidLongFunctions::default()),
            Box::new(common::AvoidLongParameterList::default()),
            Box::new(common::AvoidNestedConditionals::default()),
            Box::new(common::AvoidDynamic),
            Box::new(common::PreferTrailingComma),
            Box::new(common::AvoidGlobalState),
            Box::new(common::AvoidLateKeyword::default()),
            Box::new(common::NoMagicNumbers::default()),
            Box::new(common::PreferMatchFileName),
            Box::new(common::AvoidDoubleNegation),
            // v0.1 Flutter rules
            Box::new(flutter::AvoidReturningWidgets),
            Box::new(flutter::PreferExtractingCallbacks),
            Box::new(flutter::AvoidUnnecessarySetState),
            Box::new(flutter::AvoidExpandedAsSpacer),
            Box::new(flutter::PreferConstConstructors),
            // v0.3 Dart rules
            Box::new(common::AvoidUnusedParameters),
            Box::new(common::PreferCorrectIdentifierLength::default()),
            Box::new(common::AvoidCascadeAfterIfNull),
            Box::new(common::AvoidCollectionMethodsUnrelatedTypes),
            Box::new(common::AvoidDuplicateExports),
            Box::new(common::AvoidMissingEnumConstantInMap),
            Box::new(common::AvoidNonAsciiSymbols),
            Box::new(common::AvoidThrowInCatch),
            Box::new(common::AvoidTopLevelMembersInTests),
            Box::new(common::AvoidUnnecessaryTypeAssertions),
            Box::new(common::AvoidUnnecessaryTypeCasts),
            Box::new(common::BinaryExpressionOperandOrder),
            Box::new(common::DoubleLiteralFormat),
            Box::new(common::NewlineBeforeReturn),
            Box::new(common::PreferFirstLast),
            // v0.3 Provider/Riverpod rules
            Box::new(provider::AvoidRefReadInsideBuild),
            Box::new(provider::AvoidWatchOutsideBuild),
            Box::new(provider::PreferAsyncValueWhen),
            Box::new(provider::AvoidPublicNotifierProperties),
            Box::new(provider::PreferRefReadForMethods),
            // v0.3 BLoC rules
            Box::new(bloc::AvoidBlocPublicMethods),
            Box::new(bloc::AvoidEmitOutsideBloc),
            Box::new(bloc::PreferMultiBlocProvider),
            Box::new(bloc::AvoidPassingBlocToWidget),
            Box::new(bloc::PreferBlocExtensions),
            // v0.3 Equatable rules
            Box::new(equatable::AlwaysOverrideEqualsHashCode),
            Box::new(equatable::AvoidMutableEquatable),
            Box::new(equatable::PreferEquatable),
            // v1.0 AI-critical rules
            Box::new(common::AvoidEmptyCatch),
            Box::new(common::AvoidPrintInProduction),
            Box::new(common::AvoidHardcodedCredentials),
            Box::new(common::PreferNamedBoolParams),
            Box::new(flutter::EnsureDisposeLifecycle),
            // v1.1 AI-critical rules
            Box::new(common::AvoidUnawaitedFutures),
            Box::new(common::PreferSpecificCatchType),
            Box::new(flutter::EnsureStreamSubscriptionCancel),
            Box::new(flutter::AvoidExcessiveWidgetNesting),
            Box::new(flutter::EnsureSemanticsLabel),
            Box::new(flutter::EnsureImageSemantics),
            Box::new(flutter::EnsureTouchTargetSize),
            // Behavioral rule pack (PR-F) — 5 implemented, 1 resolver-pending (EPIC 3.1).
            Box::new(behavioral::SetStateAfterDispose::default()),
            Box::new(behavioral::UnawaitedFutureInBuild::default()),
            Box::new(behavioral::FakeMountedCheck::default()),
            Box::new(behavioral::SilentCatch::default()),
            Box::new(behavioral::RiverpodScopeLeak::default()),
            Box::new(behavioral::DisposeNotCalled::default()),
        ];

        for mut rule in all_rules {
            if let Some((severity, options)) = configured_rules.get(rule.name()) {
                rule.configure(&options);
                self.rules.push((rule, *severity));
            }
        }
    }

    pub fn rules(&self) -> Vec<&dyn Rule> {
        self.rules.iter().map(|(r, _)| r.as_ref()).collect()
    }

    pub fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        self.check_with_context(root, source, file, &RuleContext::default())
    }

    pub fn check_with_context(
        &self,
        root: Node,
        source: &str,
        file: &Path,
        context: &RuleContext<'_>,
    ) -> Vec<Issue> {
        let mut issues = Vec::new();

        if is_suppressed_for_file(source) {
            return issues;
        }

        for (rule, severity) in &self.rules {
            let mut rule_issues = rule.check_with_context(root, source, file, context);
            for issue in &mut rule_issues {
                issue.severity = *severity;
            }
            let rule_name = rule.name();
            rule_issues.retain(|issue| !is_line_suppressed(source, issue.line, rule_name));
            issues.extend(rule_issues);
        }

        issues
    }
}

fn is_suppressed_for_file(source: &str) -> bool {
    for line in source.lines().take(10) {
        let trimmed = line.trim();
        if trimmed.contains("// ignore_for_file: falcon") {
            return true;
        }
    }
    false
}

fn is_line_suppressed(source: &str, line: usize, rule_name: &str) -> bool {
    if line == 0 {
        return false;
    }
    let lines: Vec<&str> = source.lines().collect();
    if line >= 2 {
        let prev = lines.get(line - 2).unwrap_or(&"");
        let trimmed = prev.trim();
        if trimmed.contains(&format!("// ignore: {}", rule_name))
            || trimmed.contains("// ignore: falcon")
        {
            return true;
        }
    }
    for header_line in lines.iter().take(10) {
        let trimmed = header_line.trim();
        if trimmed.contains(&format!("// ignore_for_file: {}", rule_name)) {
            return true;
        }
    }
    false
}
