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
        RuleConfig::Simple("avoid-returning-widgets".to_string()),
        RuleConfig::Simple("prefer-extracting-callbacks".to_string()),
        RuleConfig::Simple("avoid-unnecessary-setstate".to_string()),
        RuleConfig::Simple("avoid-expanded-as-spacer".to_string()),
        RuleConfig::Simple("prefer-const-constructors".to_string()),
    ]
}
