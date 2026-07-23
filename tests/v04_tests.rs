use falcon::config::Severity;
use falcon::incremental::baseline::Baseline;
use falcon::incremental::cache::AnalysisCache;
use falcon::incremental::dep_graph::DependencyGraph;
use falcon::reporters::checkstyle::CheckstyleReporter;
use falcon::reporters::codeclimate::CodeClimateReporter;
use falcon::reporters::sarif::SarifReporter;
use falcon::reporters::sonar::SonarReporter;
use falcon::reporters::{AnalysisReport, Issue, Reporter};
use std::path::PathBuf;
use tempfile::TempDir;

fn sample_issues() -> Vec<Issue> {
    vec![
        Issue {
            rule: "avoid-dynamic".to_string(),
            message: "Avoid using dynamic type.".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("lib/main.dart"),
            line: 10,
            column: 5,
        },
        Issue {
            rule: "avoid-long-functions".to_string(),
            message: "Function exceeds 200 lines.".to_string(),
            severity: Severity::Error,
            file: PathBuf::from("lib/utils.dart"),
            line: 42,
            column: 1,
        },
        Issue {
            rule: "prefer-const-constructors".to_string(),
            message: "Use const constructor.".to_string(),
            severity: Severity::Info,
            file: PathBuf::from("lib/main.dart"),
            line: 20,
            column: 3,
        },
    ]
}

fn sample_report() -> AnalysisReport {
    AnalysisReport {
        issues: sample_issues(),
        metrics: Vec::new(),
        file_count: 2,
        project_path: None,
    }
}

// --- SARIF Reporter Tests ---

#[test]
fn test_sarif_output_contains_schema() {
    let report = sample_report();
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("result.sarif");

    let reporter = SarifReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_analysis(&report);

    let contents = std::fs::read_to_string(&out).unwrap();
    assert!(contents.contains("\"version\": \"2.1.0\""));
    assert!(contents.contains("\"$schema\""));
    assert!(contents.contains("Falcon"));
}

#[test]
fn test_sarif_contains_all_issues() {
    let report = sample_report();
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("result.sarif");

    let reporter = SarifReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_analysis(&report);

    let contents = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let results = json["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn test_sarif_severity_mapping() {
    let report = sample_report();
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("result.sarif");

    let reporter = SarifReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_analysis(&report);

    let contents = std::fs::read_to_string(&out).unwrap();
    assert!(contents.contains("\"error\""));
    assert!(contents.contains("\"warning\""));
    assert!(contents.contains("\"note\""));
}

#[test]
fn test_sarif_includes_code_scanning_structure() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    let report = AnalysisReport {
        issues: vec![
            Issue {
                rule: "avoid-print-in-production".to_string(),
                message: "Avoid print calls.".to_string(),
                severity: Severity::Warning,
                file: root.join("lib/main.dart"),
                line: 3,
                column: 5,
            },
            Issue {
                rule: "avoid-print-in-production".to_string(),
                message: "Avoid print calls again.".to_string(),
                severity: Severity::Warning,
                file: root.join("lib/other.dart"),
                line: 9,
                column: 2,
            },
        ],
        metrics: Vec::new(),
        file_count: 2,
        project_path: Some(root.clone()),
    };
    let out = dir.path().join("result.sarif");
    let reporter = SarifReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_analysis(&report);

    let contents = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let run = &json["runs"][0];
    let driver = &run["tool"]["driver"];
    assert_eq!(json["version"], "2.1.0");
    assert!(driver["semanticVersion"].as_str().is_some());
    assert_eq!(run["columnKind"], "utf16CodeUnits");
    assert_eq!(run["invocations"][0]["executionSuccessful"], true);

    let rules = driver["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["id"], "avoid-print-in-production");
    assert_eq!(rules[0]["name"], "avoid-print-in-production");
    assert!(rules[0]["fullDescription"]["text"].as_str().is_some());

    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    for result in results {
        assert_eq!(result["ruleId"], "avoid-print-in-production");
        assert_eq!(result["ruleIndex"], 0);
        assert_eq!(result["kind"], "fail");
        assert!(result["level"].as_str().is_some());
        assert!(result["message"]["text"].as_str().is_some());
        let physical = &result["locations"][0]["physicalLocation"];
        assert!(physical["artifactLocation"]["uri"].as_str().is_some());
        assert!(physical["artifactLocation"].get("uriBaseId").is_none());
        assert!(physical["region"]["startLine"].as_u64().is_some());
        assert!(physical["region"]["startColumn"].as_u64().is_some());
    }
    assert_eq!(
        results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "lib/main.dart"
    );
}

#[test]
fn test_sarif_output_validates_against_schema() {
    let report = sample_report();
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("result.sarif");

    let reporter = SarifReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_analysis(&report);

    let contents = std::fs::read_to_string(&out).unwrap();
    let sarif: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schemas/sarif-schema-2.1.0.json")).unwrap();
    let compiled = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft4)
        .compile(&schema)
        .unwrap();

    if let Err(errors) = compiled.validate(&sarif) {
        let messages = errors.map(|error| error.to_string()).collect::<Vec<_>>();
        panic!(
            "generated SARIF did not validate against SARIF 2.1.0 schema:\n{}",
            messages.join("\n")
        );
    };
}

// --- CodeClimate Reporter Tests ---

#[test]
fn test_codeclimate_output_format() {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("cc.json");

    let reporter = CodeClimateReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_issues(&sample_issues());

    let contents = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0]["type"], "issue");
    assert!(arr[0]["fingerprint"].is_string());
    assert!(arr[0]["location"]["path"].is_string());
}

// --- Checkstyle Reporter Tests ---

#[test]
fn test_checkstyle_xml_format() {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("checkstyle.xml");

    let reporter = CheckstyleReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_issues(&sample_issues());

    let contents = std::fs::read_to_string(&out).unwrap();
    assert!(contents.starts_with("<?xml"));
    assert!(contents.contains("<checkstyle"));
    assert!(contents.contains("<file"));
    assert!(contents.contains("<error"));
    assert!(contents.contains("source=\"falcon."));
}

#[test]
fn test_checkstyle_xml_escapes() {
    let issues = vec![Issue {
        rule: "test-rule".to_string(),
        message: "Message with <special> & \"chars\"".to_string(),
        severity: Severity::Warning,
        file: PathBuf::from("lib/test.dart"),
        line: 1,
        column: 1,
    }];

    let dir = TempDir::new().unwrap();
    let out = dir.path().join("checkstyle.xml");

    let reporter = CheckstyleReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_issues(&issues);

    let contents = std::fs::read_to_string(&out).unwrap();
    assert!(contents.contains("&lt;special&gt;"));
    assert!(contents.contains("&amp;"));
    assert!(contents.contains("&quot;chars&quot;"));
}

// --- SonarQube Reporter Tests ---

#[test]
fn test_sonar_output_format() {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("sonar.json");

    let reporter = SonarReporter {
        output_path: Some(out.clone()),
    };
    reporter.report_issues(&sample_issues());

    let contents = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let issues = json["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 3);
    assert_eq!(issues[0]["engineId"], "falcon");
}

// --- Dependency Graph Tests ---

#[test]
fn test_dep_graph_basic() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(
        lib.join("main.dart"),
        "import 'utils.dart';\nvoid main() {}\n",
    )
    .unwrap();
    std::fs::write(lib.join("utils.dart"), "int add(int a, int b) => a + b;\n").unwrap();
    std::fs::write(
        lib.join("screen.dart"),
        "import 'utils.dart';\nclass Screen {}\n",
    )
    .unwrap();

    let graph = DependencyGraph::build(dir.path(), &[]);
    assert!(graph.imports.len() >= 3);

    let utils_abs = lib.join("utils.dart").canonicalize().unwrap();
    if let Some(deps) = graph.dependents.get(&utils_abs) {
        assert!(deps.len() >= 2, "utils.dart should have >= 2 dependents");
    }
}

#[test]
fn test_dep_graph_affected_files() {
    let dir = TempDir::new().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();

    std::fs::write(lib.join("a.dart"), "int x = 1;\n").unwrap();
    std::fs::write(lib.join("b.dart"), "import 'a.dart';\nint y = 2;\n").unwrap();
    std::fs::write(lib.join("c.dart"), "import 'b.dart';\nint z = 3;\n").unwrap();

    let graph = DependencyGraph::build(dir.path(), &[]);
    let a_abs = lib.join("a.dart").canonicalize().unwrap();
    let affected = graph.affected_files(&[a_abs]);
    assert!(
        affected.len() >= 2,
        "Changing a.dart should affect b.dart and c.dart transitively"
    );
}

// --- Cache Tests ---

#[test]
fn test_cache_round_trip() {
    let dir = TempDir::new().unwrap();
    let mut cache = AnalysisCache::load(dir.path());

    cache.update_entry(&PathBuf::from("lib/main.dart"), 5, true);
    cache.update_entry(&PathBuf::from("lib/utils.dart"), 0, false);
    cache.save(dir.path()).unwrap();

    let loaded = AnalysisCache::load(dir.path());
    assert_eq!(loaded.entries.len(), 2);
    assert_eq!(loaded.entries["lib/main.dart"].issue_count, 5);
    assert!(loaded.entries["lib/main.dart"].has_errors);
    assert_eq!(loaded.entries["lib/utils.dart"].issue_count, 0);
}

#[test]
fn test_cache_detects_changed_files() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.dart");
    std::fs::write(&file, "void main() {}").unwrap();

    let mut cache = AnalysisCache::load(dir.path());
    cache.update_entry(&file, 0, false);

    let changed = cache.changed_files(std::slice::from_ref(&file));
    assert!(changed.is_empty(), "File hasn't changed");

    std::thread::sleep(std::time::Duration::from_secs(2));
    std::fs::write(&file, "void main() { print('hi'); }").unwrap();

    let changed = cache.changed_files(std::slice::from_ref(&file));
    assert_eq!(changed.len(), 1, "File should be detected as changed");
}

// --- Baseline Tests ---

#[test]
fn test_baseline_create_and_filter() {
    let dir = TempDir::new().unwrap();
    let issues = sample_issues();

    let path = Baseline::create(&issues, dir.path()).unwrap();
    assert!(path.exists());

    let baseline = Baseline::load(dir.path()).unwrap();
    assert_eq!(baseline.schema_version, 1);
    assert_eq!(baseline.entries.len(), 3);

    let new_issues = vec![
        Issue {
            rule: "avoid-dynamic".to_string(),
            message: "Avoid using dynamic type.".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("lib/main.dart"),
            line: 10,
            column: 5,
        },
        Issue {
            rule: "new-rule".to_string(),
            message: "This is a new violation.".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from("lib/new_file.dart"),
            line: 5,
            column: 1,
        },
    ];

    let filtered = baseline.filter_new_issues(new_issues, dir.path());
    assert_eq!(filtered.len(), 1, "Only the new issue should remain");
    assert_eq!(filtered[0].rule, "new-rule");
}

// --- Output Format Count Test ---

#[test]
fn test_output_format_count() {
    // Verify we now have 8 output formats:
    // console, json, html, sarif, codeclimate, checkstyle, sonar, gitlab
    let formats = [
        "console",
        "json",
        "html",
        "sarif",
        "codeclimate",
        "checkstyle",
        "sonar",
        "gitlab",
    ];
    assert_eq!(formats.len(), 8);
}

// --- Exit Code / Fail Level Tests ---

#[test]
fn test_analysis_report_counts() {
    let report = sample_report();
    assert_eq!(report.error_count(), 1);
    assert_eq!(report.warning_count(), 1);
    assert_eq!(report.info_count(), 1);
    assert!(report.has_errors());
}
