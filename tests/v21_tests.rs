// ─── PR Comment Bot ─────────────────────────────────────────────────────────

#[test]
fn test_pr_comment_format_clean() {
    let report = falcon::reporters::AnalysisReport {
        issues: vec![],
        metrics: vec![],
        file_count: 10,
        project_path: None,
    };

    let comment =
        falcon::ci::pr_comment::format_pr_comment(&report, std::path::Path::new("/project"));

    assert!(comment.contains("✅ Falcon Analysis"));
    assert!(comment.contains("No issues found"));
    assert!(comment.contains("Files analyzed | 10"));
}

#[test]
fn test_pr_comment_format_with_errors() {
    let issues = vec![
        falcon::reporters::Issue {
            rule: "avoid-dynamic".to_string(),
            message: "Avoid using dynamic type".to_string(),
            severity: falcon::config::Severity::Error,
            file: std::path::PathBuf::from("/project/lib/main.dart"),
            line: 10,
            column: 5,
        },
        falcon::reporters::Issue {
            rule: "avoid-dynamic".to_string(),
            message: "Avoid using dynamic type".to_string(),
            severity: falcon::config::Severity::Error,
            file: std::path::PathBuf::from("/project/lib/app.dart"),
            line: 20,
            column: 1,
        },
        falcon::reporters::Issue {
            rule: "prefer-trailing-comma".to_string(),
            message: "Missing trailing comma".to_string(),
            severity: falcon::config::Severity::Warning,
            file: std::path::PathBuf::from("/project/lib/main.dart"),
            line: 15,
            column: 1,
        },
    ];

    let report = falcon::reporters::AnalysisReport {
        issues,
        metrics: vec![],
        file_count: 5,
        project_path: None,
    };

    let comment =
        falcon::ci::pr_comment::format_pr_comment(&report, std::path::Path::new("/project"));

    assert!(comment.contains("❌ Falcon Analysis"));
    assert!(comment.contains("Errors | 2"));
    assert!(comment.contains("Warnings | 1"));
    assert!(comment.contains("`avoid-dynamic`"));
    assert!(comment.contains("Errors (must fix)"));
}

#[test]
fn test_pr_comment_format_warnings_only() {
    let issues = vec![falcon::reporters::Issue {
        rule: "prefer-trailing-comma".to_string(),
        message: "Add trailing comma".to_string(),
        severity: falcon::config::Severity::Warning,
        file: std::path::PathBuf::from("/project/lib/a.dart"),
        line: 5,
        column: 1,
    }];

    let report = falcon::reporters::AnalysisReport {
        issues,
        metrics: vec![],
        file_count: 3,
        project_path: None,
    };

    let comment =
        falcon::ci::pr_comment::format_pr_comment(&report, std::path::Path::new("/project"));

    assert!(comment.contains("⚠️ Falcon Analysis"));
    assert!(!comment.contains("Errors (must fix)"));
}

#[test]
fn test_ci_summary_format() {
    let report = falcon::reporters::AnalysisReport {
        issues: vec![falcon::reporters::Issue {
            rule: "test".to_string(),
            message: "msg".to_string(),
            severity: falcon::config::Severity::Error,
            file: std::path::PathBuf::from("a.dart"),
            line: 1,
            column: 1,
        }],
        metrics: vec![],
        file_count: 5,
        project_path: None,
    };

    let summary = falcon::ci::pr_comment::format_ci_summary(&report);
    assert!(summary.contains("FAIL"));
    assert!(summary.contains("1 errors"));
}

#[test]
fn test_ci_summary_pass() {
    let report = falcon::reporters::AnalysisReport {
        issues: vec![],
        metrics: vec![],
        file_count: 10,
        project_path: None,
    };

    let summary = falcon::ci::pr_comment::format_ci_summary(&report);
    assert!(summary.contains("PASS"));
}

// ─── Falcon SDK ─────────────────────────────────────────────────────────────

#[test]
fn test_sdk_analyze_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "class App {}\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let sdk = falcon::sdk::FalconSdk::new();
    let result = sdk
        .analyze_project(&tmp.path().to_string_lossy(), None)
        .unwrap();

    assert!(result.file_count >= 1);
    assert!(result.file_count >= 1);
    assert!(result.passed);
}

#[test]
fn test_sdk_analyze_with_score() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("main.dart"), "class Hello {}\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let sdk = falcon::sdk::FalconSdk::new();
    let opts = falcon::sdk::AnalysisOptions {
        include_score: true,
        ..Default::default()
    };
    let result = sdk
        .analyze_project(&tmp.path().to_string_lossy(), Some(opts))
        .unwrap();

    assert!(result.score.is_some());
    let score = result.score.unwrap();
    assert!(score.overall <= 100);
    assert!(!score.grade.is_empty());
}

#[test]
fn test_sdk_analyze_source() {
    let sdk = falcon::sdk::FalconSdk::new();
    let issues = sdk
        .analyze_source("class MyApp {\n  String greet() => 'hi';\n}\n", "test.dart")
        .unwrap();

    assert!(issues.iter().all(|i| !i.rule.is_empty()));
}

#[test]
fn test_sdk_analyze_source_with_issues() {
    let sdk = falcon::sdk::FalconSdk::new();
    let source = r#"
class BadService {
  void doStuff() {
    var x = 42;
    print(x);
  }
}
"#;
    let issues = sdk.analyze_source(source, "bad.dart").unwrap();
    assert!(
        !issues.is_empty(),
        "Should find issues in code with print and magic numbers"
    );
}

#[test]
fn test_sdk_analyze_to_json() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("a.dart"), "class A {}\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let sdk = falcon::sdk::FalconSdk::new();
    let json = sdk
        .analyze_to_json(&tmp.path().to_string_lossy(), None)
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("file_count").is_some());
    assert!(parsed.get("issues").is_some());
}

#[test]
fn test_sdk_detect_conventions() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("widget.dart"), "class MyWidget {}\n").unwrap();

    let sdk = falcon::sdk::FalconSdk::new();
    let result = sdk
        .detect_conventions(&tmp.path().to_string_lossy())
        .unwrap();

    assert!(result.get("naming").is_some());
    assert!(result.get("architecture").is_some());
}

#[test]
fn test_sdk_score_project() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("app.dart"), "class App {}\n").unwrap();
    std::fs::write(
        tmp.path().join("falcon.yaml"),
        "metrics:\n  cyclomatic_complexity: 20\n",
    )
    .unwrap();

    let sdk = falcon::sdk::FalconSdk::new();
    let score = sdk.score_project(&tmp.path().to_string_lossy()).unwrap();

    assert!(score.overall <= 100);
}

// ─── HTTP API (unit tests for request parsing) ──────────────────────────────

#[test]
fn test_api_health_endpoint() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::thread;

    let port = 18091u16;
    let handle = thread::spawn(move || {
        let _ = falcon::api::server::start_api_server("127.0.0.1", port);
    });

    thread::sleep(std::time::Duration::from_millis(200));

    if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .ok();
        let request = "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        stream.write_all(request.as_bytes()).unwrap();

        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);

        assert!(response.contains("200 OK"), "Should return 200 OK");
        assert!(response.contains("falcon"), "Should contain falcon");
    }

    drop(handle);
}
