use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// DCM rule name → Falcon rule name mapping.
pub fn rule_mapping() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    // Style rules
    m.insert("avoid-dynamic", "avoid-dynamic");
    m.insert("avoid-late-keyword", "avoid-late-keyword");
    m.insert("avoid-global-state", "avoid-global-state");
    m.insert(
        "avoid-nested-conditional-expressions",
        "avoid-nested-conditionals",
    );
    m.insert("avoid-returning-widgets", "avoid-returning-widgets");
    m.insert("avoid-unnecessary-setstate", "avoid-unnecessary-set-state");
    m.insert("avoid-expanded-as-spacer", "avoid-expanded-as-spacer");
    m.insert("prefer-trailing-comma", "prefer-trailing-comma");
    m.insert("prefer-const-border-radius", "prefer-const-constructors");
    m.insert("no-magic-number", "no-magic-numbers");
    m.insert("prefer-match-file-name", "prefer-match-file-name");
    m.insert("avoid-double-negation", "avoid-double-negation");
    m.insert("prefer-extracting-callbacks", "prefer-extracting-callbacks");
    m.insert("avoid-unused-parameters", "avoid-unused-parameters");
    m.insert(
        "prefer-correct-identifier-length",
        "prefer-correct-identifier-length",
    );
    m.insert("avoid-cascade-after-if-null", "avoid-cascade-after-if-null");
    m.insert(
        "avoid-collection-methods-with-unrelated-types",
        "avoid-collection-methods-unrelated-types",
    );
    m.insert("avoid-duplicate-exports", "avoid-duplicate-exports");
    m.insert(
        "avoid-missing-enum-constant-in-map",
        "avoid-missing-enum-constant-in-map",
    );
    m.insert("avoid-non-ascii-symbols", "avoid-non-ascii-symbols");
    m.insert("avoid-throw-in-catch-block", "avoid-throw-in-catch");
    m.insert(
        "avoid-top-level-members-in-tests",
        "avoid-top-level-members-in-tests",
    );
    m.insert(
        "avoid-unnecessary-type-assertions",
        "avoid-unnecessary-type-assertions",
    );
    m.insert(
        "avoid-unnecessary-type-casts",
        "avoid-unnecessary-type-casts",
    );
    m.insert(
        "binary-expression-operand-order",
        "binary-expression-operand-order",
    );
    m.insert("double-literal-format", "double-literal-format");
    m.insert("newline-before-return", "newline-before-return");
    m.insert("prefer-first", "prefer-first-last");
    m.insert("prefer-last", "prefer-first-last");
    // Metrics
    m.insert("cyclomatic-complexity", "cyclomatic-complexity");
    m.insert("lines-of-code", "lines-of-code");
    m.insert("number-of-parameters", "number-of-parameters");
    m.insert("maximum-nesting-level", "maximum-nesting-level");
    m.insert("maintainability-index", "maintainability-index");
    // Provider/Riverpod
    m.insert("avoid-ref-read-inside-build", "avoid-ref-read-inside-build");
    m.insert("avoid-watch-outside-build", "avoid-watch-outside-build");
    // BLoC
    m.insert("avoid-bloc-public-methods", "avoid-bloc-public-methods");
    m.insert("avoid-emit-outside-bloc", "avoid-emit-outside-bloc");
    m.insert("prefer-multi-bloc-provider", "prefer-multi-bloc-provider");
    // Equatable
    m.insert(
        "always-override-equals-and-hashcode",
        "always-override-equals-hashcode",
    );
    m
}

/// Rules that exist in DCM but not in Falcon.
pub fn unmapped_dcm_rules() -> Vec<&'static str> {
    vec![
        "avoid-banned-imports",
        "avoid-border-all",
        "avoid-collapsible-if",
        "avoid-declaring-call-method",
        "avoid-explicit-type-declaration",
        "avoid-function-type-in-records",
        "avoid-ignoring-return-values",
        "avoid-importing-entrypoint-exports",
        "avoid-inverted-boolean-checks",
        "avoid-long-records",
        "avoid-map-keys-ordering",
        "avoid-nested-records",
        "avoid-nested-switch-expressions",
        "avoid-one-field-records",
        "avoid-passing-async-when-sync-expected",
        "avoid-positional-fields-in-records",
        "avoid-redundant-async",
        "avoid-redundant-else",
        "avoid-redundant-pragma-inline",
        "avoid-referencing-discarded-variables",
        "avoid-returning-void",
        "avoid-shadowed-extension-methods",
        "avoid-shrink-wrap-in-lists",
        "avoid-similar-names",
        "avoid-substring",
        "avoid-unassigned-late-fields-keyword",
        "avoid-unnecessary-conditionals",
        "avoid-unnecessary-futures",
        "avoid-unnecessary-getter",
        "avoid-unnecessary-if",
        "avoid-unnecessary-local-late",
        "avoid-unnecessary-negations",
        "avoid-unnecessary-reassignment",
        "avoid-unnecessary-return",
        "avoid-unnecessary-stateful-widgets",
        "avoid-unsafe-collection-methods",
        "avoid-weak-cryptographic-algorithms",
        "avoid-wrapping-in-padding",
        "enum-constants-ordering",
        "format-comment",
        "match-class-name-pattern",
        "match-getter-setter-field-names",
        "match-lib-folder-structure",
        "match-positional-field-names-on-assignment",
        "missing-test-assertion",
        "move-records-to-typedefs",
        "no-boolean-literal-compare",
        "no-empty-block",
        "no-equal-then-else",
        "no-object-declaration",
        "prefer-abstract-final-static-class",
        "prefer-commenting-analyzer-ignores",
        "prefer-conditional-expressions",
        "prefer-correct-callback-field-name",
        "prefer-correct-for-loop-increment",
        "prefer-correct-handler-name",
        "prefer-correct-json-casts",
        "prefer-correct-switch-length",
        "prefer-correct-test-file-name",
        "prefer-correct-type-name",
        "prefer-declaring-const-constructors",
        "prefer-define-hero-tag",
        "prefer-early-return",
        "prefer-enums-by-name",
        "prefer-explicit-function-type",
        "prefer-explicit-parameter-names",
        "prefer-getter-over-method",
        "prefer-immediate-return",
        "prefer-iterable-of",
        "prefer-moving-to-variable",
        "prefer-named-boolean-parameters",
        "prefer-overriding-parent-equality",
        "prefer-provide-intl-description",
        "prefer-public-exception-types",
        "prefer-return-await",
        "prefer-simpler-patterns-null-check",
        "prefer-single-widget-per-file",
        "prefer-specific-cases-first",
        "prefer-static-class",
        "prefer-text-rich",
        "prefer-type-over-var",
        "prefer-typedefs-for-callbacks",
        "prefer-unwrapping-future-or",
        "prefer-visible-for-testing",
        "prefer-void-callback",
        "prefer-widget-private-members",
        "proper-super-calls",
        "tag-name",
        "unnecessary-trailing-comma",
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcmConfig {
    #[serde(default, rename = "dart_code_metrics")]
    pub dart_code_metrics: Option<DcmSection>,
    #[serde(default)]
    pub analyzer: Option<AnalyzerSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcmSection {
    #[serde(default)]
    pub rules: Option<DcmRules>,
    #[serde(default)]
    pub metrics: Option<HashMap<String, serde_yaml::Value>>,
    #[serde(default, rename = "anti-patterns")]
    pub anti_patterns: Option<HashMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DcmRules {
    List(Vec<serde_yaml::Value>),
    Map(HashMap<String, serde_yaml::Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerSection {
    #[serde(default)]
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct MigrationResult {
    pub mapped_rules: Vec<(String, String)>,
    pub unmapped_rules: Vec<String>,
    pub mapped_metrics: Vec<String>,
    pub excludes: Vec<String>,
    pub falcon_yaml_content: String,
}

/// Migrate a DCM analysis_options.yaml to falcon.yaml.
pub fn migrate_from_dcm(dcm_config_path: &Path) -> anyhow::Result<MigrationResult> {
    let content = std::fs::read_to_string(dcm_config_path)?;
    let dcm: DcmConfig = serde_yaml::from_str(&content)?;

    let mapping = rule_mapping();
    let mut mapped_rules = Vec::new();
    let mut unmapped_rules = Vec::new();
    let mut mapped_metrics = Vec::new();
    let mut excludes = Vec::new();

    if let Some(ref section) = dcm.dart_code_metrics {
        if let Some(ref rules) = section.rules {
            let rule_names: Vec<String> = match rules {
                DcmRules::List(list) => list
                    .iter()
                    .filter_map(|v| match v {
                        serde_yaml::Value::String(s) => Some(s.clone()),
                        serde_yaml::Value::Mapping(m) => m
                            .keys()
                            .next()
                            .and_then(|k| k.as_str().map(|s| s.to_string())),
                        _ => None,
                    })
                    .collect(),
                DcmRules::Map(map) => map.keys().cloned().collect(),
            };

            for rule_name in rule_names {
                if let Some(falcon_name) = mapping.get(rule_name.as_str()) {
                    mapped_rules.push((rule_name, falcon_name.to_string()));
                } else {
                    unmapped_rules.push(rule_name);
                }
            }
        }

        if let Some(ref metrics) = section.metrics {
            for (metric_name, _) in metrics {
                mapped_metrics.push(metric_name.clone());
            }
        }
    }

    if let Some(ref analyzer) = dcm.analyzer {
        if let Some(ref exc) = analyzer.exclude {
            excludes = exc.clone();
        }
    }

    let mut falcon_config = crate::config::FalconConfig::default();
    falcon_config.rules = mapped_rules
        .iter()
        .map(|(_, falcon_name)| crate::config::RuleConfig::Simple(falcon_name.clone()))
        .collect();
    falcon_config.exclude = if excludes.is_empty() {
        crate::config::default_excludes()
    } else {
        excludes.clone()
    };

    let yaml = serde_yaml::to_string(&falcon_config)?;

    Ok(MigrationResult {
        mapped_rules,
        unmapped_rules,
        mapped_metrics,
        excludes,
        falcon_yaml_content: yaml,
    })
}

/// Generate a feature gap report.
pub fn feature_gap_report() -> String {
    let unmapped = unmapped_dcm_rules();
    let mapping = rule_mapping();

    let mut report = String::new();
    report.push_str(&format!("Falcon vs DCM Feature Gap Report\n"));
    report.push_str(&format!("==================================\n\n"));
    report.push_str(&format!(
        "Mapped rules:   {} (direct equivalents in Falcon)\n",
        mapping.len()
    ));
    report.push_str(&format!(
        "Unmapped rules: {} (exist in DCM, not yet in Falcon)\n\n",
        unmapped.len()
    ));
    report.push_str("Unmapped DCM rules:\n");
    for rule in &unmapped {
        report.push_str(&format!("  - {}\n", rule));
    }
    report
}

pub fn print_migration_result(result: &MigrationResult) {
    println!();
    println!("  {} DCM → Falcon Migration", "falcon".bright_cyan().bold());
    println!();

    println!(
        "  {} {} rules mapped successfully",
        "✓".green().bold(),
        result.mapped_rules.len()
    );
    for (dcm, falcon) in &result.mapped_rules {
        if dcm != falcon {
            println!("    {} → {}", dcm.dimmed(), falcon.bright_white());
        } else {
            println!("    {} (identical)", dcm.bright_white());
        }
    }

    if !result.unmapped_rules.is_empty() {
        println!();
        println!(
            "  {} {} rules have no Falcon equivalent yet:",
            "⚠".yellow(),
            result.unmapped_rules.len()
        );
        for rule in &result.unmapped_rules {
            println!("    {} {}", "✗".red(), rule);
        }
    }

    if !result.mapped_metrics.is_empty() {
        println!();
        println!(
            "  {} {} metrics carried over",
            "✓".green().bold(),
            result.mapped_metrics.len()
        );
    }

    println!();
    println!(
        "  Migration coverage: {:.0}%",
        if result.mapped_rules.len() + result.unmapped_rules.len() > 0 {
            result.mapped_rules.len() as f64
                / (result.mapped_rules.len() + result.unmapped_rules.len()) as f64
                * 100.0
        } else {
            100.0
        }
    );
    println!();
}
