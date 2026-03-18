use super::RuleConfig;

pub fn cyclomatic_complexity() -> u32 {
    20
}

pub fn lines_of_code() -> u32 {
    100
}

pub fn number_of_parameters() -> u32 {
    4
}

pub fn maximum_nesting_level() -> u32 {
    5
}

pub fn number_of_methods() -> u32 {
    10
}

pub fn maintainability_index() -> u32 {
    50
}

pub fn source_lines_of_code() -> u32 {
    50
}

pub fn default_rules() -> Vec<RuleConfig> {
    vec![
        // v0.1 Dart rules
        RuleConfig::Simple("avoid-long-functions".to_string()),
        RuleConfig::Simple("avoid-long-parameter-list".to_string()),
        RuleConfig::Simple("avoid-nested-conditionals".to_string()),
        RuleConfig::Simple("avoid-dynamic".to_string()),
        RuleConfig::Simple("prefer-trailing-comma".to_string()),
        RuleConfig::Simple("avoid-global-state".to_string()),
        RuleConfig::Simple("avoid-late-keyword".to_string()),
        RuleConfig::Simple("no-magic-numbers".to_string()),
        RuleConfig::Simple("prefer-match-file-name".to_string()),
        RuleConfig::Simple("avoid-double-negation".to_string()),
        // v0.1 Flutter rules
        RuleConfig::Simple("avoid-returning-widgets".to_string()),
        RuleConfig::Simple("prefer-extracting-callbacks".to_string()),
        RuleConfig::Simple("avoid-unnecessary-setstate".to_string()),
        RuleConfig::Simple("avoid-expanded-as-spacer".to_string()),
        RuleConfig::Simple("prefer-const-constructors".to_string()),
        // v0.3 Dart rules
        RuleConfig::Simple("avoid-unused-parameters".to_string()),
        RuleConfig::Simple("prefer-correct-identifier-length".to_string()),
        RuleConfig::Simple("avoid-cascade-after-if-null".to_string()),
        RuleConfig::Simple("avoid-collection-methods-with-unrelated-types".to_string()),
        RuleConfig::Simple("avoid-duplicate-exports".to_string()),
        RuleConfig::Simple("avoid-missing-enum-constant-in-map".to_string()),
        RuleConfig::Simple("avoid-non-ascii-symbols".to_string()),
        RuleConfig::Simple("avoid-throw-in-catch-block".to_string()),
        RuleConfig::Simple("avoid-top-level-members-in-tests".to_string()),
        RuleConfig::Simple("avoid-unnecessary-type-assertions".to_string()),
        RuleConfig::Simple("avoid-unnecessary-type-casts".to_string()),
        RuleConfig::Simple("binary-expression-operand-order".to_string()),
        RuleConfig::Simple("double-literal-format".to_string()),
        RuleConfig::Simple("newline-before-return".to_string()),
        RuleConfig::Simple("prefer-first-last".to_string()),
        // v0.3 Provider/Riverpod rules
        RuleConfig::Simple("avoid-ref-read-inside-build".to_string()),
        RuleConfig::Simple("avoid-watch-outside-build".to_string()),
        RuleConfig::Simple("prefer-async-value-when".to_string()),
        RuleConfig::Simple("avoid-public-notifier-properties".to_string()),
        RuleConfig::Simple("prefer-ref-read-for-methods".to_string()),
        // v0.3 BLoC rules
        RuleConfig::Simple("avoid-bloc-public-methods".to_string()),
        RuleConfig::Simple("avoid-emit-outside-bloc".to_string()),
        RuleConfig::Simple("prefer-multi-bloc-provider".to_string()),
        RuleConfig::Simple("avoid-passing-bloc-to-widget".to_string()),
        RuleConfig::Simple("prefer-bloc-extensions".to_string()),
        // v0.3 Equatable rules
        RuleConfig::Simple("always-override-equals-and-hashcode".to_string()),
        RuleConfig::Simple("avoid-mutable-equatable".to_string()),
        RuleConfig::Simple("prefer-equatable".to_string()),
        // v1.0 AI-critical rules
        RuleConfig::Simple("avoid-empty-catch".to_string()),
        RuleConfig::Simple("avoid-print-in-production".to_string()),
        RuleConfig::Simple("avoid-hardcoded-credentials".to_string()),
        RuleConfig::Simple("prefer-named-boolean-parameters".to_string()),
        RuleConfig::Simple("ensure-dispose-lifecycle".to_string()),
        // v1.1 AI-critical rules
        RuleConfig::Simple("avoid-unawaited-futures".to_string()),
        RuleConfig::Simple("prefer-specific-catch-type".to_string()),
        RuleConfig::Simple("ensure-stream-subscription-cancel".to_string()),
        RuleConfig::Simple("avoid-excessive-widget-nesting".to_string()),
    ]
}
