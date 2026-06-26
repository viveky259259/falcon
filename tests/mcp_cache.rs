use falcon::mcp::cache::McpCache;
use serde_json::json;

#[test]
fn lint_file_mcp_call_stores_disk_cache_entry() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("main.dart");
    let source = "class Good {}\n";
    std::fs::write(&file, source).unwrap();

    let result = falcon::mcp::tools::execute_tool(
        "lint_file",
        &json!({ "file_path": file.to_string_lossy() }),
    )
    .expect("lint_file should analyze disk-backed source");

    let cached = McpCache::new()
        .load_lint_file(&file, source)
        .expect("lint_file should store cache entry");
    assert_eq!(cached, result);
}

#[test]
fn lint_file_mcp_cache_invalidates_changed_content() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("main.dart");
    let clean_source = "class Good {}\n";
    std::fs::write(&file, clean_source).unwrap();

    let clean = falcon::mcp::tools::execute_tool(
        "lint_file",
        &json!({ "file_path": file.to_string_lossy() }),
    )
    .expect("initial lint_file should analyze");

    let noisy_source = "class badName { void run() { print('debug'); } }\n";
    std::fs::write(&file, noisy_source).unwrap();
    let noisy = falcon::mcp::tools::execute_tool(
        "lint_file",
        &json!({ "file_path": file.to_string_lossy() }),
    )
    .expect("changed lint_file should analyze");

    assert_ne!(clean, noisy, "changed source must not reuse stale cache");
    assert!(
        noisy["issue_count"].as_u64().unwrap_or_default()
            > clean["issue_count"].as_u64().unwrap_or_default(),
        "changed source should produce additional findings: clean={clean:?} noisy={noisy:?}"
    );

    let cached = McpCache::new()
        .load_lint_file(&file, noisy_source)
        .expect("changed source should replace cache entry");
    assert_eq!(cached, noisy);
}
