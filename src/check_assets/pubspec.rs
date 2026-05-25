//! Parse `flutter.assets:` from `pubspec.yaml`, capturing the original line
//! number of each declaration so errors can point users at the exact spot.

use anyhow::{Context, Result};

/// One declared asset entry from `pubspec.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDecl {
    /// The path string exactly as the user wrote it.
    pub path: String,
    /// 1-based line number in `pubspec.yaml`.
    pub line: usize,
}

/// Parse the `flutter: { assets: [...] }` list from a `pubspec.yaml` string.
///
/// Returns an empty vec if there is no `flutter.assets:` section. Returns an
/// error only if the YAML itself is unparseable.
pub fn parse_assets(yaml: &str) -> Result<Vec<AssetDecl>> {
    // Use serde_yaml for structural parse; track line numbers via a second pass
    // over the raw text. serde_yaml does not expose line numbers, but
    // pubspec.yaml's `flutter.assets:` block is always a list of strings, so
    // matching each value back to its line is unambiguous.
    let doc: serde_yaml::Value =
        serde_yaml::from_str(yaml).context("Failed to parse pubspec.yaml as YAML")?;
    let assets = doc
        .get("flutter")
        .and_then(|f| f.get("assets"))
        .and_then(|a| a.as_sequence());
    let Some(seq) = assets else {
        return Ok(Vec::new());
    };

    // Collect declared paths in order.
    let paths: Vec<String> = seq
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    // Second pass: scan the raw text for the line-number annotation. We look for
    // any line under the `assets:` key that has the form `  - <value>` and the
    // value matches one of `paths` (in order). This handles both quoted and
    // unquoted strings.
    let mut lines: Vec<usize> = Vec::with_capacity(paths.len());
    let mut in_assets_block = false;
    let mut idx = 0;
    for (lineno, line) in yaml.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("assets:") {
            in_assets_block = true;
            continue;
        }
        if in_assets_block {
            // Any line that's not indented as a list item under assets ends the block.
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                break;
            }
            if let Some(item) = trimmed.strip_prefix("- ") {
                let item = item.trim().trim_matches(|c| c == '"' || c == '\'');
                if idx < paths.len() && paths[idx] == item {
                    lines.push(lineno + 1);
                    idx += 1;
                }
            }
        }
    }

    // If we somehow under-counted, fall back to line 0 (unknown). This is rare.
    while lines.len() < paths.len() {
        lines.push(0);
    }

    Ok(paths
        .into_iter()
        .zip(lines)
        .map(|(path, line)| AssetDecl { path, line })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assets_empty_when_no_flutter_section() {
        let yaml = "name: foo\nversion: 1.0.0\n";
        assert!(parse_assets(yaml).unwrap().is_empty());
    }

    #[test]
    fn parse_assets_empty_when_flutter_section_has_no_assets() {
        let yaml = "name: foo\nflutter:\n  uses-material-design: true\n";
        assert!(parse_assets(yaml).unwrap().is_empty());
    }

    #[test]
    fn parse_assets_single_entry_with_line_number() {
        let yaml = "name: foo\nflutter:\n  assets:\n    - assets/logo.png\n";
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "assets/logo.png");
        assert_eq!(result[0].line, 4);
    }

    #[test]
    fn parse_assets_multiple_entries_track_distinct_lines() {
        let yaml = r#"name: foo
flutter:
  assets:
    - assets/a.png
    - assets/b.png
    - assets/c.png
"#;
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], AssetDecl { path: "assets/a.png".into(), line: 4 });
        assert_eq!(result[1], AssetDecl { path: "assets/b.png".into(), line: 5 });
        assert_eq!(result[2], AssetDecl { path: "assets/c.png".into(), line: 6 });
    }

    #[test]
    fn parse_assets_handles_quoted_strings() {
        let yaml = "flutter:\n  assets:\n    - \".env.development\"\n    - '.env.production'\n";
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path, ".env.development");
        assert_eq!(result[1].path, ".env.production");
    }

    #[test]
    fn parse_assets_dir_entry_kept_with_trailing_slash() {
        let yaml = "flutter:\n  assets:\n    - assets/icons/\n";
        let result = parse_assets(yaml).unwrap();
        assert_eq!(result[0].path, "assets/icons/");
    }

    #[test]
    fn parse_assets_invalid_yaml_returns_err() {
        let yaml = "flutter:\n  assets:\n  - [unclosed\n";
        assert!(parse_assets(yaml).is_err());
    }

    #[test]
    fn parse_assets_top_level_assets_ignored() {
        // Flutter only recognises `flutter: { assets: ... }`, never top-level
        // `assets:`. We mirror that.
        let yaml = "assets:\n  - top/level.png\nname: foo\n";
        assert!(parse_assets(yaml).unwrap().is_empty());
    }
}
