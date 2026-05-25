use crate::reporters::Issue;
use colored::Colorize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ConfidenceResult {
    pub issue: Issue,
    pub confidence: u8,
    pub reason: String,
    pub reducers: Vec<String>,
}

impl ConfidenceResult {
    pub fn confidence_label(&self) -> &'static str {
        match self.confidence {
            90..=100 => "high",
            70..=89 => "medium",
            50..=69 => "low",
            _ => "uncertain",
        }
    }

    pub fn confidence_color(&self) -> colored::Color {
        match self.confidence {
            90..=100 => colored::Color::Green,
            70..=89 => colored::Color::Yellow,
            50..=69 => colored::Color::Red,
            _ => colored::Color::BrightBlack,
        }
    }
}

/// Score unused code issues with confidence levels using local heuristics.
pub fn score_unused_issues(issues: &[Issue], project_root: &Path) -> Vec<ConfidenceResult> {
    let reflection_files = find_reflection_patterns(project_root);
    let dynamic_files = find_dynamic_usage(project_root);
    let export_files = find_barrel_exports(project_root);

    issues
        .iter()
        .filter(|i| {
            i.rule == "unused-code"
                || i.rule == "unused-file"
                || i.rule == "dead-code-path"
                || i.rule == "unused-dependency"
        })
        .map(|issue| score_single_issue(issue, &reflection_files, &dynamic_files, &export_files))
        .collect()
}

fn score_single_issue(
    issue: &Issue,
    reflection_files: &HashSet<String>,
    dynamic_files: &HashSet<String>,
    export_files: &HashSet<String>,
) -> ConfidenceResult {
    let mut confidence: i32 = 95;
    let mut reducers = Vec::new();

    let file_str = issue.file.to_string_lossy().to_string();
    let file_name = issue
        .file
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if reflection_files.contains(&file_str) {
        confidence -= 25;
        reducers.push("File contains reflection/mirrors — may be used dynamically".to_string());
    }

    if dynamic_files.contains(&file_str) {
        confidence -= 15;
        reducers
            .push("File uses dynamic types — references may not be statically visible".to_string());
    }

    if export_files.contains(file_name) {
        confidence -= 10;
        reducers
            .push("File is re-exported from a barrel file — may be used externally".to_string());
    }

    if issue.message.contains("appears to be unused") {
        let name = extract_name_from_message(&issue.message);
        if name.starts_with('_') {
            confidence += 5;
            confidence = confidence.min(100);
        } else if is_common_interface_name(&name) {
            confidence -= 20;
            reducers.push(format!(
                "'{}' follows common interface/mixin naming — may be used via subtype",
                name
            ));
        }
    }

    if file_str.contains("/generated/")
        || file_str.contains(".g.dart")
        || file_str.contains(".freezed.dart")
    {
        confidence -= 30;
        reducers.push("Generated file — code generation tools may use this".to_string());
    }

    if file_str.contains("_test.dart") || file_str.contains("/test/") {
        confidence -= 5;
        reducers.push("Test file — may be indirectly invoked by test runner".to_string());
    }

    if issue.rule == "dead-code-path" {
        if issue.message.contains("always true") {
            confidence = 85;
        } else if issue.message.contains("always false") {
            confidence = 90;
        } else if issue.message.contains("Unreachable") {
            confidence = 95;
        }
    }

    confidence = confidence.clamp(10, 100);

    let reason = if reducers.is_empty() {
        "No dynamic patterns detected — high confidence this code is unused.".to_string()
    } else {
        format!("{} factor(s) reduce confidence.", reducers.len())
    };

    ConfidenceResult {
        issue: issue.clone(),
        confidence: confidence as u8,
        reason,
        reducers,
    }
}

fn extract_name_from_message(message: &str) -> String {
    if let Some(start) = message.find('\'') {
        let rest = &message[start + 1..];
        if let Some(end) = rest.find('\'') {
            return rest[..end].to_string();
        }
    }
    String::new()
}

fn is_common_interface_name(name: &str) -> bool {
    name.ends_with("Base")
        || name.ends_with("Mixin")
        || name.ends_with("Interface")
        || name.ends_with("Protocol")
        || name.ends_with("Delegate")
        || name.ends_with("Factory")
        || name.ends_with("Strategy")
        || name.ends_with("Observer")
        || name.ends_with("Listener")
}

fn find_reflection_patterns(root: &Path) -> HashSet<String> {
    let mut files = HashSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            if source.contains("dart:mirrors")
                || source.contains("reflectable")
                || source.contains("@reflector")
                || source.contains("noSuchMethod")
            {
                files.insert(entry.path().to_string_lossy().to_string());
            }
        }
    }
    files
}

fn find_dynamic_usage(root: &Path) -> HashSet<String> {
    let mut files = HashSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            let dynamic_count = source.matches("dynamic ").count()
                + source.matches("dynamic>").count()
                + source.matches("dynamic,").count();
            if dynamic_count >= 3 {
                files.insert(entry.path().to_string_lossy().to_string());
            }
        }
    }
    files
}

fn find_barrel_exports(root: &Path) -> HashSet<String> {
    let mut exported_files = HashSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("export") {
                    if let Some(start) = trimmed.find('\'') {
                        let rest = &trimmed[start + 1..];
                        if let Some(end) = rest.find('\'') {
                            let uri = &rest[..end];
                            if let Some(fname) = std::path::Path::new(uri).file_name() {
                                exported_files.insert(fname.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    exported_files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_issue(rule: &str, message: &str, file: &str) -> Issue {
        Issue {
            rule: rule.to_string(),
            message: message.to_string(),
            severity: Severity::Warning,
            file: PathBuf::from(file),
            line: 10,
            column: 1,
        }
    }

    // ── ConfidenceResult::confidence_label ───────────────────────────────────

    #[test]
    fn label_high() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 95,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_label(), "high");
    }

    #[test]
    fn label_high_boundary() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 90,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_label(), "high");
    }

    #[test]
    fn label_medium() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 80,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_label(), "medium");
    }

    #[test]
    fn label_medium_boundary() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 70,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_label(), "medium");
    }

    #[test]
    fn label_low() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 60,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_label(), "low");
    }

    #[test]
    fn label_uncertain() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 30,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_label(), "uncertain");
    }

    #[test]
    fn label_uncertain_zero() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 0,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_label(), "uncertain");
    }

    // ── ConfidenceResult::confidence_color ───────────────────────────────────

    #[test]
    fn color_high() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 100,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_color(), colored::Color::Green);
    }

    #[test]
    fn color_medium() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 75,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_color(), colored::Color::Yellow);
    }

    #[test]
    fn color_low() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 55,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_color(), colored::Color::Red);
    }

    #[test]
    fn color_uncertain() {
        let r = ConfidenceResult {
            issue: make_issue("unused-code", "x", "a.dart"),
            confidence: 20,
            reason: String::new(),
            reducers: vec![],
        };
        assert_eq!(r.confidence_color(), colored::Color::BrightBlack);
    }

    // ── extract_name_from_message ─────────────────────────────────────────────

    #[test]
    fn extract_name_basic() {
        assert_eq!(
            extract_name_from_message("'MyClass' appears to be unused"),
            "MyClass"
        );
    }

    #[test]
    fn extract_name_no_quotes() {
        assert_eq!(extract_name_from_message("no quotes here"), "");
    }

    #[test]
    fn extract_name_unclosed_quote() {
        // only one quote — no closing found
        assert_eq!(extract_name_from_message("'unclosed"), "");
    }

    #[test]
    fn extract_name_empty_quotes() {
        assert_eq!(extract_name_from_message("'' appears unused"), "");
    }

    #[test]
    fn extract_name_with_underscore_prefix() {
        assert_eq!(
            extract_name_from_message("'_privateHelper' appears to be unused"),
            "_privateHelper"
        );
    }

    // ── is_common_interface_name ──────────────────────────────────────────────

    #[test]
    fn interface_name_base() {
        assert!(is_common_interface_name("AnimalBase"));
    }

    #[test]
    fn interface_name_mixin() {
        assert!(is_common_interface_name("LoggableMixin"));
    }

    #[test]
    fn interface_name_interface() {
        assert!(is_common_interface_name("PaymentInterface"));
    }

    #[test]
    fn interface_name_protocol() {
        assert!(is_common_interface_name("SerializationProtocol"));
    }

    #[test]
    fn interface_name_delegate() {
        assert!(is_common_interface_name("NavigationDelegate"));
    }

    #[test]
    fn interface_name_factory() {
        assert!(is_common_interface_name("WidgetFactory"));
    }

    #[test]
    fn interface_name_strategy() {
        assert!(is_common_interface_name("SortStrategy"));
    }

    #[test]
    fn interface_name_observer() {
        assert!(is_common_interface_name("EventObserver"));
    }

    #[test]
    fn interface_name_listener() {
        assert!(is_common_interface_name("ClickListener"));
    }

    #[test]
    fn interface_name_plain_does_not_match() {
        assert!(!is_common_interface_name("MyHelper"));
    }

    #[test]
    fn interface_name_empty() {
        assert!(!is_common_interface_name(""));
    }

    // ── score_single_issue (via score_unused_issues) – plain issue ───────────

    #[test]
    fn plain_unused_code_scores_95() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("widget.dart");
        std::fs::write(&file, "void foo() {}").unwrap();
        let issues = vec![make_issue("unused-code", "widget appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].confidence, 95);
        assert!(results[0].reducers.is_empty());
        assert!(results[0].reason.contains("No dynamic patterns"));
    }

    #[test]
    fn filtered_wrong_rule() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("widget.dart");
        std::fs::write(&file, "void foo() {}").unwrap();
        // "lint-error" is not an unused-* rule — should be filtered out
        let issues = vec![make_issue("lint-error", "something", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert!(results.is_empty());
    }

    #[test]
    fn unused_file_and_unused_dependency_included() {
        let dir = TempDir::new().unwrap();
        let f1 = dir.path().join("a.dart");
        let f2 = dir.path().join("b.dart");
        std::fs::write(&f1, "void a() {}").unwrap();
        std::fs::write(&f2, "void b() {}").unwrap();
        let issues = vec![
            make_issue("unused-file", "a.dart appears to be unused", f1.to_str().unwrap()),
            make_issue("unused-dependency", "dep unused", f2.to_str().unwrap()),
        ];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results.len(), 2);
    }

    // ── reflection reducer ────────────────────────────────────────────────────

    #[test]
    fn reflection_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("magic.dart");
        std::fs::write(&file, "import 'dart:mirrors';\nvoid foo() {}").unwrap();
        let issues = vec![make_issue("unused-code", "foo appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 70); // 95 - 25
        assert!(results[0].reducers.iter().any(|r| r.contains("reflection")));
    }

    #[test]
    fn reflectable_keyword_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("ref.dart");
        std::fs::write(&file, "import 'reflectable';\nvoid foo() {}").unwrap();
        let issues = vec![make_issue("unused-code", "foo appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert!(results[0].confidence <= 70);
    }

    #[test]
    fn no_such_method_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("proxy.dart");
        std::fs::write(&file, "dynamic noSuchMethod(i) => super.noSuchMethod(i);").unwrap();
        let issues = vec![make_issue("unused-code", "proxy appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert!(results[0].confidence <= 70);
    }

    // ── dynamic reducer ───────────────────────────────────────────────────────

    #[test]
    fn dynamic_usage_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("dyn.dart");
        // need >= 3 occurrences of "dynamic "
        let content = "dynamic foo;\ndynamic bar;\ndynamic baz;\n";
        std::fs::write(&file, content).unwrap();
        let issues = vec![make_issue("unused-code", "dyn appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 80); // 95 - 15
        assert!(results[0].reducers.iter().any(|r| r.contains("dynamic")));
    }

    #[test]
    fn two_dynamic_occurrences_not_enough() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("dyn2.dart");
        let content = "dynamic foo;\ndynamic bar;\n";
        std::fs::write(&file, content).unwrap();
        let issues = vec![make_issue("unused-code", "unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        // no dynamic reducer
        assert!(!results[0].reducers.iter().any(|r| r.contains("dynamic")));
    }

    // ── barrel-export reducer ─────────────────────────────────────────────────

    #[test]
    fn barrel_export_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        // barrel index file
        let barrel = dir.path().join("index.dart");
        std::fs::write(&barrel, "export 'widget.dart';\n").unwrap();
        // the exported file
        let widget = dir.path().join("widget.dart");
        std::fs::write(&widget, "class Widget {}").unwrap();

        let issues = vec![make_issue("unused-code", "Widget unused", widget.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 85); // 95 - 10
        assert!(results[0].reducers.iter().any(|r| r.contains("barrel")));
    }

    // ── generated-file reducer ────────────────────────────────────────────────

    #[test]
    fn generated_g_dart_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("model.g.dart");
        std::fs::write(&file, "void g() {}").unwrap();
        let issues = vec![make_issue("unused-code", "g appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 65); // 95 - 30
        assert!(results[0].reducers.iter().any(|r| r.contains("Generated")));
    }

    #[test]
    fn generated_freezed_dart_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("lib");
        std::fs::create_dir_all(&sub).unwrap();
        let file = sub.join("model.freezed.dart");
        std::fs::write(&file, "void f() {}").unwrap();
        let issues = vec![make_issue("unused-code", "f appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 65); // 95 - 30
    }

    #[test]
    fn generated_dir_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let gen_dir = dir.path().join("generated");
        std::fs::create_dir_all(&gen_dir).unwrap();
        let file = gen_dir.join("api.dart");
        std::fs::write(&file, "void api() {}").unwrap();
        let issues = vec![make_issue("unused-code", "api appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 65); // 95 - 30
    }

    // ── test-file reducer ─────────────────────────────────────────────────────

    #[test]
    fn test_dart_suffix_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("widget_test.dart");
        std::fs::write(&file, "void test_foo() {}").unwrap();
        let issues = vec![make_issue("unused-code", "test_foo appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 90); // 95 - 5
        assert!(results[0].reducers.iter().any(|r| r.contains("Test")));
    }

    #[test]
    fn test_directory_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&test_dir).unwrap();
        let file = test_dir.join("helper.dart");
        std::fs::write(&file, "void helper() {}").unwrap();
        let issues = vec![make_issue("unused-code", "helper appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 90); // 95 - 5
    }

    // ── underscore-name boost ─────────────────────────────────────────────────

    #[test]
    fn underscore_name_boosts_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("util.dart");
        std::fs::write(&file, "void _helper() {}").unwrap();
        let issues = vec![make_issue(
            "unused-code",
            "'_helper' appears to be unused",
            file.to_str().unwrap(),
        )];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 100); // 95 + 5 = 100, capped
    }

    // ── interface-name reducer ────────────────────────────────────────────────

    #[test]
    fn interface_name_in_message_reduces_confidence() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("base.dart");
        std::fs::write(&file, "class MyBase {}").unwrap();
        let issues = vec![make_issue(
            "unused-code",
            "'MyBase' appears to be unused",
            file.to_str().unwrap(),
        )];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 75); // 95 - 20
        assert!(results[0].reducers.iter().any(|r| r.contains("MyBase")));
    }

    // ── dead-code-path overrides ──────────────────────────────────────────────

    #[test]
    fn dead_code_path_always_true() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("cond.dart");
        std::fs::write(&file, "if (true) {}").unwrap();
        let issues = vec![make_issue(
            "dead-code-path",
            "Condition is always true",
            file.to_str().unwrap(),
        )];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 85);
    }

    #[test]
    fn dead_code_path_always_false() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("cond.dart");
        std::fs::write(&file, "if (false) {}").unwrap();
        let issues = vec![make_issue(
            "dead-code-path",
            "Condition is always false",
            file.to_str().unwrap(),
        )];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 90);
    }

    #[test]
    fn dead_code_path_unreachable() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("cond.dart");
        std::fs::write(&file, "return; doSomething();").unwrap();
        let issues = vec![make_issue(
            "dead-code-path",
            "Unreachable code after return",
            file.to_str().unwrap(),
        )];
        let results = score_unused_issues(&issues, dir.path());
        assert_eq!(results[0].confidence, 95);
    }

    // ── clamp lower bound ─────────────────────────────────────────────────────

    #[test]
    fn confidence_clamped_to_minimum_10() {
        let dir = TempDir::new().unwrap();
        // stack all reducers: reflection (-25) + dynamic (-15) + generated (-30) + barrel (-10)
        // also interface name in message (-20) = 95 - 100 = -5 → clamp to 10
        let sub = dir.path().join("generated");
        std::fs::create_dir_all(&sub).unwrap();

        // barrel barrel file
        let barrel = dir.path().join("index.dart");
        std::fs::write(&barrel, "export 'multi.dart';\n").unwrap();

        let file = sub.join("multi.dart");
        let content = "import 'dart:mirrors';\ndynamic a;\ndynamic b;\ndynamic c;\nclass MultiBase {}";
        std::fs::write(&file, content).unwrap();

        let issues = vec![make_issue(
            "unused-code",
            "'MultiBase' appears to be unused",
            file.to_str().unwrap(),
        )];
        let results = score_unused_issues(&issues, dir.path());
        assert!(results[0].confidence >= 10, "confidence {} should be >= 10", results[0].confidence);
    }

    // ── find_reflection_patterns directly ────────────────────────────────────

    #[test]
    fn find_reflection_patterns_reflector_annotation() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("ref.dart");
        std::fs::write(&file, "@reflector\nclass Foo {}").unwrap();
        let result = find_reflection_patterns(dir.path());
        assert!(result.contains(&file.to_string_lossy().to_string()));
    }

    #[test]
    fn find_reflection_patterns_ignores_non_dart() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "import 'dart:mirrors';").unwrap();
        let result = find_reflection_patterns(dir.path());
        assert!(result.is_empty());
    }

    // ── find_dynamic_usage directly ───────────────────────────────────────────

    #[test]
    fn find_dynamic_usage_exactly_three() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("dyn.dart");
        std::fs::write(&file, "dynamic x; dynamic y; dynamic z;").unwrap();
        let result = find_dynamic_usage(dir.path());
        assert!(result.contains(&file.to_string_lossy().to_string()));
    }

    #[test]
    fn find_dynamic_usage_angle_bracket_and_comma_variants() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("dyn2.dart");
        // "dynamic>" + "dynamic," + "dynamic " = 3
        std::fs::write(&file, "List<dynamic> a;\nMap<String, dynamic> b;\ndynamic c;").unwrap();
        let result = find_dynamic_usage(dir.path());
        assert!(result.contains(&file.to_string_lossy().to_string()));
    }

    // ── find_barrel_exports directly ─────────────────────────────────────────

    #[test]
    fn find_barrel_exports_returns_filenames() {
        let dir = TempDir::new().unwrap();
        let barrel = dir.path().join("exports.dart");
        std::fs::write(&barrel, "export 'src/widgets/button.dart';\nexport 'src/models/user.dart';\n").unwrap();
        let result = find_barrel_exports(dir.path());
        assert!(result.contains("button.dart"), "expected button.dart in {:?}", result);
        assert!(result.contains("user.dart"), "expected user.dart in {:?}", result);
    }

    #[test]
    fn find_barrel_exports_ignores_non_export_lines() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("lib.dart");
        std::fs::write(&file, "import 'foo.dart';\nclass Bar {}").unwrap();
        let result = find_barrel_exports(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn find_barrel_exports_ignores_non_dart() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("notes.md");
        std::fs::write(&file, "export 'something.dart';\n").unwrap();
        let result = find_barrel_exports(dir.path());
        assert!(result.is_empty());
    }

    // ── reason string ─────────────────────────────────────────────────────────

    #[test]
    fn reason_reports_factor_count() {
        let dir = TempDir::new().unwrap();
        // reflection alone = 1 reducer
        let file = dir.path().join("r.dart");
        std::fs::write(&file, "import 'dart:mirrors';").unwrap();
        let issues = vec![make_issue("unused-code", "r appears unused", file.to_str().unwrap())];
        let results = score_unused_issues(&issues, dir.path());
        assert!(results[0].reason.contains("1 factor(s) reduce confidence"));
    }
}

pub fn print_confidence_results(results: &[ConfidenceResult], min_confidence: Option<u8>) {
    let min = min_confidence.unwrap_or(0);
    let filtered: Vec<_> = results.iter().filter(|r| r.confidence >= min).collect();

    if filtered.is_empty() {
        println!(
            "  {} No issues above {}% confidence threshold.",
            "✓".green().bold(),
            min
        );
        return;
    }

    let high = filtered.iter().filter(|r| r.confidence >= 90).count();
    let medium = filtered
        .iter()
        .filter(|r| r.confidence >= 70 && r.confidence < 90)
        .count();
    let low = filtered.iter().filter(|r| r.confidence < 70).count();

    println!();
    println!(
        "  {} {} issues with confidence scoring",
        "falcon".bright_cyan().bold(),
        filtered.len()
    );
    println!(
        "    {} high (≥90%)  {} medium (70-89%)  {} low (<70%)",
        high.to_string().green(),
        medium.to_string().yellow(),
        low.to_string().red()
    );
    println!();

    for result in &filtered {
        let conf_str = format!("{}%", result.confidence);
        let rel_path = result
            .issue
            .file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("?");

        println!(
            "  {} [{}] {}:{}  {}",
            conf_str.color(result.confidence_color()),
            result.confidence_label(),
            rel_path,
            result.issue.line,
            result.issue.message
        );

        for reducer in &result.reducers {
            println!("       {} {}", "↓".dimmed(), reducer.dimmed());
        }
    }
    println!();
}
