use super::manifest::{PluginManifest, PluginRuleEntry, PluginType};
use super::wasm_runtime;
use colored::Colorize;
use std::path::Path;

/// Scaffold a new plugin project.
pub fn create_plugin(name: &str, dir: &Path, plugin_type: PluginType) -> anyhow::Result<()> {
    let plugin_dir = dir.join(name);

    if plugin_dir.exists() {
        anyhow::bail!("Directory '{}' already exists", plugin_dir.display());
    }

    std::fs::create_dir_all(&plugin_dir)?;
    std::fs::create_dir_all(plugin_dir.join("rules"))?;
    std::fs::create_dir_all(plugin_dir.join("test"))?;

    let manifest = PluginManifest {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        description: format!("Custom Falcon plugin: {}", name),
        author: String::new(),
        repository: String::new(),
        license: "MIT".to_string(),
        plugin_type: plugin_type.clone(),
        rules: vec![PluginRuleEntry {
            name: format!("{}-example-rule", name),
            description: "An example rule — replace with your own logic.".to_string(),
            default_severity: "warning".to_string(),
        }],
        min_falcon_version: "0.1.0".to_string(),
        tags: vec!["custom".to_string()],
    };

    manifest.save(&plugin_dir)?;

    let rules_content = wasm_runtime::generate_rules_template();
    std::fs::write(plugin_dir.join("rules/rules.yaml"), rules_content)?;

    let readme = format!(
        r#"# {}

A custom Falcon plugin.

## Installation

```bash
falcon x plugin install ./{}
```

## Rules

| Rule | Severity | Description |
|------|----------|-------------|
| {}-example-rule | warning | An example rule |

## Development

1. Edit `rules/rules.yaml` to define your rules
2. Test with `falcon x plugin test ./{}`
3. Publish with `falcon x plugin publish`
"#,
        name, name, name, name
    );
    std::fs::write(plugin_dir.join("README.md"), readme)?;

    let test_content = format!(
        r#"# Test cases for {name}
# Each test case has source code and expected issues

- name: "should detect pattern"
  source: |
    void main() {{
      // TODO: fix this
    }}
  expected_issues:
    - rule: "{name}-example-rule"
      line: 2

- name: "should not flag clean code"
  source: |
    void main() {{
      print('hello');
    }}
  expected_issues: []
"#,
        name = name
    );
    std::fs::write(plugin_dir.join("test/test_cases.yaml"), test_content)?;

    let gitignore = ".falcon-cache/\n*.wasm\ntarget/\n";
    std::fs::write(plugin_dir.join(".gitignore"), gitignore)?;

    println!();
    println!(
        "  {} Created plugin '{}' at {}",
        "✓".green().bold(),
        name.bright_cyan(),
        plugin_dir.display()
    );
    println!();
    println!("  Plugin type: {:?}", plugin_type);
    println!("  Files created:");
    println!("    falcon-plugin.yaml  — plugin manifest");
    println!("    rules/rules.yaml    — rule definitions");
    println!("    test/test_cases.yaml — test cases");
    println!("    README.md           — documentation");
    println!("    .gitignore");
    println!();
    println!("  Next steps:");
    println!("    1. Edit rules/rules.yaml to define your rules");
    println!("    2. Run: falcon x plugin test ./{}", name);
    println!("    3. Install: falcon x plugin install ./{}", name);
    println!();

    Ok(())
}

/// List installed plugins.
pub fn list_plugins(plugin_dir: &Path) -> anyhow::Result<Vec<PluginManifest>> {
    let mut plugins = Vec::new();

    if !plugin_dir.exists() {
        return Ok(plugins);
    }

    for entry in std::fs::read_dir(plugin_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let manifest_path = path.join("falcon-plugin.yaml");
            if manifest_path.exists() {
                match PluginManifest::load(&path) {
                    Ok(m) => plugins.push(m),
                    Err(e) => {
                        log::warn!("Skipping invalid plugin at {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plugins)
}

/// Install a plugin from a local path.
pub fn install_plugin(source: &Path, plugin_dir: &Path) -> anyhow::Result<String> {
    let manifest = PluginManifest::load(source)?;
    let target = plugin_dir.join(&manifest.name);

    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }

    copy_dir_recursive(source, &target)?;

    Ok(manifest.name)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Print plugin list in a nice format.
pub fn print_plugins(plugins: &[PluginManifest]) {
    if plugins.is_empty() {
        println!();
        println!("  No plugins installed.");
        println!("  Create one with: falcon x plugin create <name>");
        println!();
        return;
    }

    println!();
    println!(
        "  {} Installed Plugins ({})",
        "falcon".bright_cyan().bold(),
        plugins.len()
    );
    println!();

    for plugin in plugins {
        let type_badge = match plugin.plugin_type {
            PluginType::Wasm => "WASM".bright_blue(),
            PluginType::Native => "NATIVE".bright_green(),
            PluginType::Preset => "PRESET".bright_yellow(),
        };

        println!(
            "  {} {} v{} [{}]",
            "•".bright_white(),
            plugin.name.bright_white().bold(),
            plugin.version,
            type_badge
        );

        if !plugin.description.is_empty() {
            println!("    {}", plugin.description.dimmed());
        }

        if !plugin.rules.is_empty() {
            println!("    {} rule(s)", plugin.rules.len());
        }
    }
    println!();
}
