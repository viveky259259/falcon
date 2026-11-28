use std::path::Path;

/// `path` as text with `/` separators on every OS.
///
/// Use it wherever path text is matched against patterns such as `"/test/"`,
/// `"/build/"` or `"lib/src/"`: on Windows `to_string_lossy` gives `\`
/// separators, which those patterns never match.
pub fn slash(path: &Path) -> String {
    let text = path.to_string_lossy();
    if std::path::MAIN_SEPARATOR == '\\' {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

pub fn has_package_config(root: &Path) -> bool {
    root.join(".dart_tool")
        .join("package_config.json")
        .is_file()
}

#[cfg(test)]
mod tests {
    use super::{has_package_config, slash};
    use std::path::Path;

    #[test]
    fn slash_uses_forward_slashes() {
        let path = Path::new("lib").join("src").join("a.dart");
        assert_eq!(slash(&path), "lib/src/a.dart");
    }
    use std::fs;

    #[test]
    fn detects_resolved_dart_project() {
        let root = tempfile::tempdir().unwrap();
        let dart_tool = root.path().join(".dart_tool");
        fs::create_dir_all(&dart_tool).unwrap();
        fs::write(dart_tool.join("package_config.json"), "{}").unwrap();

        assert!(has_package_config(root.path()));
    }

    #[test]
    fn missing_package_config_is_false() {
        let root = tempfile::tempdir().unwrap();

        assert!(!has_package_config(root.path()));
    }
}
