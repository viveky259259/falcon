use std::path::Path;

pub fn has_package_config(root: &Path) -> bool {
    root.join(".dart_tool")
        .join("package_config.json")
        .is_file()
}

#[cfg(test)]
mod tests {
    use super::has_package_config;
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
