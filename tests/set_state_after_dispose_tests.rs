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
fn full_project_analyze_uses_resolver_for_state_async_gap() {
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
  Future<void> load() async {
    await api.fetch();
    setState(() {});
  }
}
"#,
    );

    let mut config = FalconConfig::default();
    config.rules = vec![RuleConfig::Simple("set-state-after-dispose".to_string())];
    config.unused.enabled = false;
    let falcon = Falcon::new(config).unwrap();
    let report = falcon.analyze(root).unwrap();
    let set_state_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|issue| issue.rule == "set-state-after-dispose")
        .collect();

    assert_eq!(set_state_issues.len(), 1, "{:#?}", report.issues);
    assert!(set_state_issues[0]
        .file
        .to_string_lossy()
        .ends_with("lib/screen.dart"));
    assert!(set_state_issues[0]
        .message
        .contains("must re-check `mounted`"));
}

#[test]
fn full_project_analyze_bails_on_ambiguous_state_parent_for_async_gap() {
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
  Future<void> load() async {
    await api.fetch();
    setState(() {});
  }
}
"#,
    );

    let mut config = FalconConfig::default();
    config.rules = vec![RuleConfig::Simple("set-state-after-dispose".to_string())];
    config.unused.enabled = false;
    let falcon = Falcon::new(config).unwrap();
    let report = falcon.analyze(root).unwrap();

    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.rule != "set-state-after-dispose"),
        "ambiguous BaseState should make set-state rule bail: {:#?}",
        report.issues
    );
}
