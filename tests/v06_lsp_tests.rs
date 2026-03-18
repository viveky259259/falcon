use falcon::config::FalconConfig;
use falcon::lsp::diagnostics::{analyze_source, issues_to_diagnostics};
use falcon::rules::RuleRegistry;
use std::path::Path;

#[test]
fn test_lsp_diagnostics_from_source() {
    let source = r#"
var globalState = 42;

void main() {
  dynamic x = "hello";
}
"#;

    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = analyze_source(source, Path::new("test.dart"), &config, &registry);
    assert!(!issues.is_empty(), "Should detect issues in source");

    let diagnostics = issues_to_diagnostics(&issues);
    assert_eq!(issues.len(), diagnostics.len());

    for diag in &diagnostics {
        assert_eq!(diag.source.as_deref(), Some("falcon"));
        assert!(diag.code.is_some());
    }
}

#[test]
fn test_lsp_diagnostics_clean_code() {
    let source = r#"
void main() {
  final message = 'Hello';
  logger.info(message);
}
"#;

    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = analyze_source(source, Path::new("clean.dart"), &config, &registry);
    let rule_issues: Vec<_> = issues
        .iter()
        .filter(|i| !i.rule.starts_with("metric-"))
        .collect();

    assert!(
        rule_issues.is_empty(),
        "Clean code should have no rule violations, found: {:?}",
        rule_issues.iter().map(|i| &i.rule).collect::<Vec<_>>()
    );
}

#[test]
fn test_lsp_diagnostics_severity_mapping() {
    use tower_lsp::lsp_types::DiagnosticSeverity;

    let source = r#"
dynamic bad = "oops";
"#;

    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = analyze_source(source, Path::new("test.dart"), &config, &registry);
    let diagnostics = issues_to_diagnostics(&issues);

    for diag in &diagnostics {
        assert!(diag.severity.is_some());
        let sev = diag.severity.unwrap();
        assert!(
            sev == DiagnosticSeverity::ERROR
                || sev == DiagnosticSeverity::WARNING
                || sev == DiagnosticSeverity::INFORMATION,
            "Unknown severity: {:?}",
            sev
        );
    }
}

#[test]
fn test_lsp_diagnostics_contain_data() {
    let source = r#"
dynamic x = 1;
"#;

    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = analyze_source(source, Path::new("test.dart"), &config, &registry);
    let diagnostics = issues_to_diagnostics(&issues);

    for diag in &diagnostics {
        assert!(diag.data.is_some(), "Diagnostic should contain data for code actions");
        let data = diag.data.as_ref().unwrap();
        assert!(data.get("rule").is_some());
        assert!(data.get("line").is_some());
    }
}

#[test]
fn test_lsp_code_actions_generated() {
    use tower_lsp::lsp_types::Url;

    let source = "dynamic x = 1;\n";
    let uri = Url::parse("file:///test.dart").unwrap();

    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = analyze_source(source, Path::new("test.dart"), &config, &registry);
    assert!(!issues.is_empty(), "Should have issues for code action test");

    let diagnostics = issues_to_diagnostics(&issues);
    let actions = falcon::lsp::actions::generate_code_actions(&uri, &diagnostics, source);

    assert!(
        !actions.is_empty(),
        "Should generate code actions (at least suppress actions) for diagnostics"
    );
}

#[test]
fn test_lsp_suppress_action_format() {
    use tower_lsp::lsp_types::{CodeActionOrCommand, Url};

    let source = "dynamic x = 1;\n";
    let uri = Url::parse("file:///test.dart").unwrap();

    let config = FalconConfig::default();
    let mut registry = RuleRegistry::new();
    registry.register_defaults(&config);

    let issues = analyze_source(source, Path::new("test.dart"), &config, &registry);
    let diagnostics = issues_to_diagnostics(&issues);

    let actions = falcon::lsp::actions::generate_code_actions(&uri, &diagnostics, source);

    let suppress_actions: Vec<_> = actions
        .iter()
        .filter_map(|a| match a {
            CodeActionOrCommand::CodeAction(ca) => {
                if ca.title.starts_with("Suppress") {
                    Some(ca)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    assert!(
        !suppress_actions.is_empty(),
        "Should generate suppress actions"
    );

    for action in &suppress_actions {
        assert!(action.edit.is_some());
        let edit = action.edit.as_ref().unwrap();
        assert!(edit.changes.is_some());
    }
}

#[test]
fn test_lsp_binary_exists() {
    let output = std::process::Command::new("cargo")
        .args(["build", "--bin", "falcon-lsp"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to build falcon-lsp");

    assert!(
        output.status.success(),
        "falcon-lsp binary should build successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
