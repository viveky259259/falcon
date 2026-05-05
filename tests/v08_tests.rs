use std::path::PathBuf;

// ============================================================
// Plugin Manifest Tests
// ============================================================

#[test]
fn test_plugin_manifest_create_and_load() {
    let dir = tempfile::tempdir().unwrap();

    let manifest = falcon::plugins::manifest::PluginManifest {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        description: "A test plugin".to_string(),
        plugin_type: falcon::plugins::manifest::PluginType::Wasm,
        rules: vec![falcon::plugins::manifest::PluginRuleEntry {
            name: "my-rule".to_string(),
            description: "Test rule".to_string(),
            default_severity: "warning".to_string(),
        }],
        ..Default::default()
    };

    manifest.save(dir.path()).unwrap();

    let loaded = falcon::plugins::manifest::PluginManifest::load(dir.path()).unwrap();
    assert_eq!(loaded.name, "test-plugin");
    assert_eq!(loaded.version, "1.0.0");
    assert_eq!(loaded.rules.len(), 1);
    assert_eq!(
        loaded.plugin_type,
        falcon::plugins::manifest::PluginType::Wasm
    );
}

#[test]
fn test_plugin_manifest_validation_empty_name() {
    let manifest = falcon::plugins::manifest::PluginManifest {
        name: "".to_string(),
        ..Default::default()
    };
    assert!(manifest.validate().is_err());
}

#[test]
fn test_plugin_manifest_validation_invalid_chars() {
    let manifest = falcon::plugins::manifest::PluginManifest {
        name: "my plugin!".to_string(),
        ..Default::default()
    };
    assert!(manifest.validate().is_err());
}

#[test]
fn test_plugin_manifest_validation_valid() {
    let manifest = falcon::plugins::manifest::PluginManifest {
        name: "my-valid-plugin_v2".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    };
    assert!(manifest.validate().is_ok());
}

// ============================================================
// Plugin Scaffold Tests
// ============================================================

#[test]
fn test_create_plugin_scaffold() {
    let dir = tempfile::tempdir().unwrap();

    falcon::plugins::scaffold::create_plugin(
        "my-custom-lint",
        dir.path(),
        falcon::plugins::manifest::PluginType::Wasm,
    )
    .unwrap();

    let plugin_dir = dir.path().join("my-custom-lint");
    assert!(plugin_dir.exists());
    assert!(plugin_dir.join("falcon-plugin.yaml").exists());
    assert!(plugin_dir.join("rules/rules.yaml").exists());
    assert!(plugin_dir.join("test/test_cases.yaml").exists());
    assert!(plugin_dir.join("README.md").exists());
    assert!(plugin_dir.join(".gitignore").exists());
}

#[test]
fn test_create_plugin_already_exists() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("existing")).unwrap();

    let result = falcon::plugins::scaffold::create_plugin(
        "existing",
        dir.path(),
        falcon::plugins::manifest::PluginType::Wasm,
    );
    assert!(result.is_err());
}

#[test]
fn test_install_and_list_plugins() {
    let source_dir = tempfile::tempdir().unwrap();
    falcon::plugins::scaffold::create_plugin(
        "test-plugin",
        source_dir.path(),
        falcon::plugins::manifest::PluginType::Wasm,
    )
    .unwrap();

    let install_dir = tempfile::tempdir().unwrap();
    let plugin_source = source_dir.path().join("test-plugin");

    let name =
        falcon::plugins::scaffold::install_plugin(&plugin_source, install_dir.path()).unwrap();
    assert_eq!(name, "test-plugin");

    let plugins = falcon::plugins::scaffold::list_plugins(install_dir.path()).unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "test-plugin");
}

// ============================================================
// Plugin Loader Tests
// ============================================================

#[test]
fn test_plugin_loader_discover() {
    let dir = tempfile::tempdir().unwrap();

    falcon::plugins::scaffold::create_plugin(
        "plugin-a",
        dir.path(),
        falcon::plugins::manifest::PluginType::Wasm,
    )
    .unwrap();

    let install_dir = tempfile::tempdir().unwrap();
    falcon::plugins::scaffold::install_plugin(&dir.path().join("plugin-a"), install_dir.path())
        .unwrap();

    let mut loader = falcon::plugins::loader::PluginLoader::new();
    loader.add_plugin_dir(install_dir.path().to_path_buf());
    let count = loader.discover().unwrap();
    assert_eq!(count, 1);
    assert_eq!(loader.loaded_plugins().len(), 1);
    assert_eq!(loader.loaded_plugins()[0].name(), "plugin-a");
}

#[test]
fn test_plugin_loader_check() {
    let loader = falcon::plugins::loader::PluginLoader::new();
    let issues = loader.check_all("void main() {}", &PathBuf::from("test.dart"));
    assert!(issues.is_empty());
}

// ============================================================
// WASM Runtime Tests
// ============================================================

#[test]
fn test_wasm_rule_compilation() {
    let def = falcon::plugins::wasm_runtime::WasmRuleDefinition {
        name: "no-print".to_string(),
        description: "Avoid using print() in production code".to_string(),
        severity: "error".to_string(),
        node_kinds: vec![],
        patterns: vec!["print(".to_string()],
        anti_patterns: vec!["// ignore".to_string()],
        message_template: Some("Found '{match}' — use a logger instead.".to_string()),
    };

    let rule = falcon::plugins::wasm_runtime::CompiledWasmRule::compile(def);
    let source = "void main() {\n  print('hello');\n}\n";
    let issues = rule.check(source, &PathBuf::from("test.dart"));

    assert_eq!(issues.len(), 1);
    assert!(issues[0].message.contains("print("));
    assert_eq!(issues[0].line, 2);
}

#[test]
fn test_wasm_rule_anti_pattern() {
    let def = falcon::plugins::wasm_runtime::WasmRuleDefinition {
        name: "no-print".to_string(),
        description: "Avoid print".to_string(),
        severity: "warning".to_string(),
        node_kinds: vec![],
        patterns: vec!["print(".to_string()],
        anti_patterns: vec!["// ignore".to_string()],
        message_template: None,
    };

    let rule = falcon::plugins::wasm_runtime::CompiledWasmRule::compile(def);
    let source = "  print('test'); // ignore\n";
    let issues = rule.check(source, &PathBuf::from("test.dart"));

    assert!(issues.is_empty(), "Anti-pattern should suppress match");
}

#[test]
fn test_wasm_sandbox_limits() {
    let sandbox = falcon::plugins::wasm_runtime::WasmSandbox {
        max_execution_ms: 5000,
        max_memory_mb: 64,
        max_issues_per_file: 2,
    };

    let def = falcon::plugins::wasm_runtime::WasmRuleDefinition {
        name: "no-var".to_string(),
        description: "Avoid var".to_string(),
        severity: "warning".to_string(),
        node_kinds: vec![],
        patterns: vec!["var ".to_string()],
        anti_patterns: vec![],
        message_template: None,
    };

    let rule = falcon::plugins::wasm_runtime::CompiledWasmRule::compile(def);
    let source = "var a = 1;\nvar b = 2;\nvar c = 3;\nvar d = 4;\n";
    let issues = sandbox.check_with_limits(&[rule], source, &PathBuf::from("test.dart"));

    assert_eq!(
        issues.len(),
        2,
        "Should be limited to max_issues_per_file=2"
    );
}

#[test]
fn test_load_wasm_rules_from_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let rules_yaml = r#"
- name: no-todo
  description: "Remove TODO comments before merging"
  severity: warning
  patterns:
    - "TODO"
    - "FIXME"
  anti_patterns:
    - "// ignore"
"#;
    let rules_path = dir.path().join("rules.yaml");
    std::fs::write(&rules_path, rules_yaml).unwrap();

    let rules = falcon::plugins::wasm_runtime::load_wasm_rules(&rules_path).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].definition.name, "no-todo");
    assert_eq!(rules[0].definition.patterns.len(), 2);
}

#[test]
fn test_generate_rules_template() {
    let template = falcon::plugins::wasm_runtime::generate_rules_template();
    assert!(template.contains("my-custom-rule"));
    assert!(template.contains("patterns"));
}

// ============================================================
// Presets Tests
// ============================================================

#[test]
fn test_get_preset_recommended() {
    let preset = falcon::plugins::presets::get_preset("recommended");
    assert!(preset.is_some());
    let preset = preset.unwrap();
    assert_eq!(preset.name, "recommended");
    assert!(!preset.rules.is_empty());
}

#[test]
fn test_get_preset_strict() {
    let preset = falcon::plugins::presets::get_preset("strict").unwrap();
    assert_eq!(preset.name, "strict");
    assert!(preset.rules.len() > 20, "Strict should have many rules");
}

#[test]
fn test_get_preset_flutter() {
    let preset = falcon::plugins::presets::get_preset("flutter").unwrap();
    assert_eq!(preset.name, "flutter");
    let rule_names: Vec<&str> = preset.rules.iter().map(|r| r.name()).collect();
    assert!(rule_names.contains(&"avoid-returning-widgets"));
}

#[test]
fn test_get_preset_riverpod() {
    let preset = falcon::plugins::presets::get_preset("riverpod").unwrap();
    let rule_names: Vec<&str> = preset.rules.iter().map(|r| r.name()).collect();
    assert!(rule_names.contains(&"avoid-ref-read-inside-build"));
}

#[test]
fn test_get_preset_bloc() {
    let preset = falcon::plugins::presets::get_preset("bloc").unwrap();
    let rule_names: Vec<&str> = preset.rules.iter().map(|r| r.name()).collect();
    assert!(rule_names.contains(&"avoid-bloc-public-methods"));
}

#[test]
fn test_get_preset_performance() {
    let preset = falcon::plugins::presets::get_preset("performance").unwrap();
    let rule_names: Vec<&str> = preset.rules.iter().map(|r| r.name()).collect();
    assert!(rule_names.contains(&"prefer-const-constructors"));
}

#[test]
fn test_get_preset_invalid() {
    assert!(falcon::plugins::presets::get_preset("nonexistent").is_none());
}

#[test]
fn test_list_presets() {
    let presets = falcon::plugins::presets::list_presets();
    assert!(
        presets.len() >= 6,
        "Should have at least 6 presets, got {}",
        presets.len()
    );
    let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"recommended"));
    assert!(names.contains(&"strict"));
    assert!(names.contains(&"flutter"));
    assert!(names.contains(&"riverpod"));
    assert!(names.contains(&"bloc"));
    assert!(names.contains(&"performance"));
}

#[test]
fn test_apply_preset() {
    let dir = tempfile::tempdir().unwrap();
    let preset = falcon::plugins::presets::get_preset("recommended").unwrap();
    falcon::plugins::presets::apply_preset(&preset, dir.path()).unwrap();

    assert!(dir.path().join("falcon.yaml").exists());

    let config = falcon::config::FalconConfig::load(dir.path()).unwrap();
    assert!(!config.rules.is_empty());
}

// ============================================================
// Registry Tests
// ============================================================

#[test]
fn test_search_registry_all() {
    let results = falcon::plugins::registry::search_registry("*");
    assert!(!results.is_empty());
    assert!(results.len() >= 5);
}

#[test]
fn test_search_registry_flutter() {
    let results = falcon::plugins::registry::search_registry("flutter");
    assert!(!results.is_empty());
}

#[test]
fn test_search_registry_no_results() {
    let results = falcon::plugins::registry::search_registry("zzz_nonexistent_xyz");
    assert!(results.is_empty());
}

// ============================================================
// RuleConfig::simple Tests
// ============================================================

#[test]
fn test_rule_config_simple_warning() {
    let rc = falcon::config::RuleConfig::simple(
        "test-rule".to_string(),
        falcon::config::Severity::Warning,
    );
    assert_eq!(rc.name(), "test-rule");
    assert_eq!(rc.severity(), falcon::config::Severity::Warning);
}

#[test]
fn test_rule_config_simple_error() {
    let rc = falcon::config::RuleConfig::simple(
        "test-rule".to_string(),
        falcon::config::Severity::Error,
    );
    assert_eq!(rc.name(), "test-rule");
    assert_eq!(rc.severity(), falcon::config::Severity::Error);
}
