use falcon::config::{FalconConfig, RuleConfig};
use falcon::Falcon;
use std::fs;
use std::path::Path;

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir -p");
    }
    fs::write(path, body).expect("write");
}

#[test]
fn full_project_analyze_uses_resolver_for_transitive_state_dispose() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join("lib/base.dart"),
        r#"
class BaseState extends State<W> {}
"#,
    );
    write(
        &root.join("lib/screen.dart"),
        r#"
class _ScreenState extends BaseState {
  final TextEditingController controller = TextEditingController();
}
"#,
    );

    let mut config = FalconConfig::default();
    config.rules = vec![RuleConfig::Simple("dispose-not-called".to_string())];
    config.unused.enabled = false;
    let falcon = Falcon::new(config).unwrap();
    let report = falcon.analyze(root).unwrap();
    let dispose_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|issue| issue.rule == "dispose-not-called")
        .collect();

    assert_eq!(dispose_issues.len(), 1, "{:#?}", report.issues);
    assert!(dispose_issues[0]
        .file
        .to_string_lossy()
        .ends_with("lib/screen.dart"));
    assert!(dispose_issues[0]
        .message
        .contains("requires a dispose() override"));
}

#[test]
fn full_project_analyze_bails_on_ambiguous_state_parent_for_dispose() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join("lib/base_a.dart"),
        r#"
class BaseState extends State<A> {}
"#,
    );
    write(
        &root.join("lib/base_b.dart"),
        r#"
class BaseState extends State<B> {}
"#,
    );
    write(
        &root.join("lib/screen.dart"),
        r#"
class _ScreenState extends BaseState {
  final TextEditingController controller = TextEditingController();
}
"#,
    );

    let mut config = FalconConfig::default();
    config.rules = vec![RuleConfig::Simple("dispose-not-called".to_string())];
    config.unused.enabled = false;
    let falcon = Falcon::new(config).unwrap();
    let report = falcon.analyze(root).unwrap();

    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.rule != "dispose-not-called"),
        "ambiguous BaseState should make dispose rule bail: {:#?}",
        report.issues
    );
}
