use falcon::ai::confidence::{score_unused_issues, ConfidenceResult};
use falcon::ai::config::{AiConfig, AiProvider};
use falcon::ai::explain::explain_rule;
use falcon::ai::fix::generate_fixes;
use falcon::config::FalconConfig;
use falcon::config::Severity;
use falcon::reporters::Issue;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_ai_config_defaults() {
    let config = AiConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.provider, AiProvider::None);
    assert!(config.api_key.is_none());
    assert!(!config.is_available());
}

#[test]
fn test_ai_config_resolve_api_key() {
    let config = AiConfig {
        api_key: Some("sk-test123".to_string()),
        ..Default::default()
    };
    assert_eq!(config.resolve_api_key(), Some("sk-test123".to_string()));
}

#[test]
fn test_ai_config_effective_model() {
    let mut config = AiConfig {
        provider: AiProvider::OpenAi,
        ..Default::default()
    };
    assert_eq!(config.effective_model(), "gpt-4o");

    config.provider = AiProvider::Anthropic;
    assert_eq!(config.effective_model(), "claude-sonnet-4-20250514");

    config.model = Some("custom-model".to_string());
    assert_eq!(config.effective_model(), "custom-model");
}

#[test]
fn test_ai_config_availability() {
    let mut config = AiConfig {
        enabled: true,
        provider: AiProvider::OpenAi,
        ..Default::default()
    };
    assert!(!config.is_available());

    config.api_key = Some("sk-test".to_string());
    assert!(config.is_available());
}

#[test]
fn test_ai_config_in_falcon_config() {
    let config = FalconConfig::default();
    assert!(!config.ai.enabled);
    assert_eq!(config.ai.provider, AiProvider::None);
}

#[test]
fn test_ai_config_yaml_parsing() {
    let dir = TempDir::new().unwrap();
    let config_content = r#"
metrics:
  cyclomatic_complexity: 10
rules: []
ai:
  enabled: true
  provider: openai
  api_key: "sk-test"
  features:
    confidence_scoring: true
    smart_fixes: false
"#;
    std::fs::write(dir.path().join("falcon.yaml"), config_content).unwrap();

    let config = FalconConfig::load(dir.path()).unwrap();
    assert!(config.ai.enabled);
    assert_eq!(config.ai.provider, AiProvider::OpenAi);
    assert_eq!(config.ai.api_key, Some("sk-test".to_string()));
    assert!(config.ai.features.confidence_scoring);
    assert!(!config.ai.features.smart_fixes);
}

#[test]
fn test_ai_setup_generates_config() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("falcon.yaml"), "rules: []\n").unwrap();

    falcon::ai::config::generate_ai_setup(dir.path()).unwrap();

    let content = std::fs::read_to_string(dir.path().join("falcon.yaml")).unwrap();
    assert!(content.contains("ai:"));
    assert!(content.contains("provider: none"));
    assert!(content.contains("confidence_scoring: false"));
}

#[test]
fn test_ai_setup_refuses_duplicate() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("falcon.yaml"), "ai:\n  enabled: false\n").unwrap();

    let result = falcon::ai::config::generate_ai_setup(dir.path());
    assert!(result.is_err());
}

#[test]
fn test_explain_known_rule() {
    let explanation = explain_rule("no-magic-numbers");
    assert!(explanation.is_some());

    let exp = explanation.unwrap();
    assert_eq!(exp.name, "no-magic-numbers");
    assert!(!exp.summary.is_empty());
    assert!(!exp.why.is_empty());
    assert!(!exp.bad_example.is_empty());
    assert!(!exp.good_example.is_empty());
}

#[test]
fn test_explain_unknown_rule() {
    let explanation = explain_rule("nonexistent-rule-xyz");
    assert!(explanation.is_none());
}

#[test]
fn test_explain_multiple_rules() {
    let rules = [
        "avoid-dynamic",
        "avoid-global-state",
        "avoid-returning-widgets",
        "dead-code-path",
    ];
    for rule in &rules {
        let exp = explain_rule(rule);
        assert!(exp.is_some(), "Missing explanation for {}", rule);
    }
}

#[test]
fn test_confidence_scoring_high() {
    let dir = TempDir::new().unwrap();
    let issues = vec![Issue {
        rule: "unused-code".to_string(),
        message: "Declaration '_helper' appears to be unused.".to_string(),
        severity: Severity::Warning,
        file: dir.path().join("lib/helper.dart"),
        line: 5,
        column: 1,
    }];

    let results = score_unused_issues(&issues, dir.path());
    assert_eq!(results.len(), 1);
    assert!(
        results[0].confidence >= 90,
        "Private unused code should have high confidence"
    );
}

#[test]
fn test_confidence_scoring_generated_file() {
    let dir = TempDir::new().unwrap();
    let issues = vec![Issue {
        rule: "unused-code".to_string(),
        message: "Declaration 'GenModel' appears to be unused.".to_string(),
        severity: Severity::Warning,
        file: dir.path().join("lib/generated/model.g.dart"),
        line: 10,
        column: 1,
    }];

    let results = score_unused_issues(&issues, dir.path());
    assert_eq!(results.len(), 1);
    assert!(
        results[0].confidence < 80,
        "Generated file unused code should have lower confidence, got {}",
        results[0].confidence
    );
    assert!(!results[0].reducers.is_empty());
}

#[test]
fn test_confidence_scoring_dead_code() {
    let dir = TempDir::new().unwrap();
    let issues = vec![Issue {
        rule: "dead-code-path".to_string(),
        message: "Unreachable code after return statement.".to_string(),
        severity: Severity::Warning,
        file: dir.path().join("lib/utils.dart"),
        line: 20,
        column: 1,
    }];

    let results = score_unused_issues(&issues, dir.path());
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].confidence, 95);
}

#[test]
fn test_confidence_labels() {
    let result = ConfidenceResult {
        issue: Issue {
            rule: "unused-code".to_string(),
            message: "test".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("test.dart"),
            line: 1,
            column: 1,
        },
        confidence: 95,
        reason: "test".to_string(),
        reducers: vec![],
    };
    assert_eq!(result.confidence_label(), "high");

    let result_med = ConfidenceResult {
        confidence: 75,
        ..result.clone()
    };
    assert_eq!(result_med.confidence_label(), "medium");

    let result_low = ConfidenceResult {
        confidence: 55,
        ..result.clone()
    };
    assert_eq!(result_low.confidence_label(), "low");

    let result_uncertain = ConfidenceResult {
        confidence: 30,
        ..result
    };
    assert_eq!(result_uncertain.confidence_label(), "uncertain");
}

#[test]
fn test_fix_generation_for_trailing_comma() {
    let dir = TempDir::new().unwrap();
    let dart_file = dir.path().join("lib/test.dart");
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    std::fs::write(&dart_file, "void main() {\n  foo(bar)\n}\n").unwrap();

    let issues = vec![Issue {
        rule: "prefer-trailing-comma".to_string(),
        message: "Add trailing comma.".to_string(),
        severity: Severity::Info,
        file: dart_file.clone(),
        line: 2,
        column: 1,
    }];

    let fixes = generate_fixes(&issues, dir.path());
    assert_eq!(fixes.len(), 1);
    assert!(fixes[0].auto_fixable);
    assert!(fixes[0].replacement.contains(','));
}

#[test]
fn test_fix_no_match_for_unsupported_rule() {
    let dir = TempDir::new().unwrap();
    let dart_file = dir.path().join("lib/test.dart");
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    std::fs::write(&dart_file, "void main() {}\n").unwrap();

    let issues = vec![Issue {
        rule: "avoid-dynamic".to_string(),
        message: "Avoid dynamic.".to_string(),
        severity: Severity::Warning,
        file: dart_file,
        line: 1,
        column: 1,
    }];

    let fixes = generate_fixes(&issues, dir.path());
    assert!(fixes.is_empty());
}

#[test]
fn test_context_aware_magic_numbers_http() {
    use falcon::parser::DartParser;
    use falcon::rules::Rule;

    let source = r#"
void handleResponse(int statusCode) {
  if (statusCode == 200) {
    return;
  }
  if (statusCode == 404) {
    throw Exception('Not found');
  }
}
"#;

    let mut parser = DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let rule = falcon::rules::common::NoMagicNumbers::default();
    let issues = rule.check(tree.root_node(), source, std::path::Path::new("test.dart"));

    for issue in &issues {
        assert!(
            !issue.message.contains("200") && !issue.message.contains("404"),
            "HTTP status codes should be skipped in HTTP context, but found: {}",
            issue.message
        );
    }
}

#[test]
fn test_context_aware_late_keyword_skip_test() {
    use falcon::parser::DartParser;
    use falcon::rules::Rule;

    let source = r#"
late final MockApi api;

void main() {
  setUp(() {
    api = MockApi();
  });
}
"#;

    let mut parser = DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let rule = falcon::rules::common::AvoidLateKeyword::default();
    let issues = rule.check(
        tree.root_node(),
        source,
        std::path::Path::new("/project/test/widget_test.dart"),
    );

    assert!(
        issues.is_empty(),
        "avoid-late-keyword should skip test files, found {} issues",
        issues.len()
    );
}

#[test]
fn test_context_aware_late_keyword_framework() {
    use falcon::parser::DartParser;
    use falcon::rules::Rule;

    let source = r#"
class MyState extends State<MyWidget> {
  late AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(vsync: this);
  }
}
"#;

    let mut parser = DartParser::new().unwrap();
    let tree = parser.parse(source).unwrap();

    let rule = falcon::rules::common::AvoidLateKeyword::default();
    let issues = rule.check(
        tree.root_node(),
        source,
        std::path::Path::new("/project/lib/widget.dart"),
    );

    assert!(
        issues.is_empty(),
        "avoid-late-keyword should skip framework-required patterns like AnimationController, found {} issues",
        issues.len()
    );
}
