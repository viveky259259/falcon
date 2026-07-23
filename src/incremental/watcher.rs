use crate::config::FalconConfig;
use crate::reporters::console::ConsoleReporter;
use crate::reporters::Reporter;
use crate::Falcon;
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// Polls for file changes and re-runs analysis on modified files.
pub fn watch(root: &Path, config: FalconConfig) -> anyhow::Result<()> {
    // LCOV_EXCL_START — infinite poll loop with no exit; behavior verified manually via `falcon watch` integration
    println!(
        "\n{} Watching {} for changes... (press Ctrl+C to stop)\n",
        "falcon".bright_cyan().bold(),
        root.display()
    );

    let mut file_times: HashMap<PathBuf, SystemTime> = snapshot_times(root);
    let poll_interval = Duration::from_millis(500);

    loop {
        std::thread::sleep(poll_interval);

        let current_times = snapshot_times(root);
        let mut changed: Vec<PathBuf> = Vec::new();

        for (path, mtime) in &current_times {
            match file_times.get(path) {
                Some(prev) if prev == mtime => {}
                _ => changed.push(path.clone()),
            }
        }

        for old_path in file_times.keys() {
            if !current_times.contains_key(old_path) {
                changed.push(old_path.clone());
            }
        }

        if !changed.is_empty() {
            println!(
                "{} {} file(s) changed, re-analyzing...",
                "[watch]".bright_yellow(),
                changed.len()
            );

            match Falcon::new(config.clone()) {
                Ok(falcon) => match falcon.analyze(root) {
                    Ok(report) => {
                        let reporter = ConsoleReporter;
                        reporter.report_analysis(&report);
                    }
                    Err(e) => eprintln!("{} Analysis failed: {}", "error".red(), e),
                },
                Err(e) => eprintln!("{} Failed to init Falcon: {}", "error".red(), e),
            }

            println!(
                "\n{} Watching for changes...\n",
                "falcon".bright_cyan().bold()
            );
        }

        file_times = current_times;
    }
    // LCOV_EXCL_STOP
}

fn snapshot_times(root: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut times = HashMap::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
    {
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                times.insert(entry.path().to_path_buf(), mtime);
            }
        }
    }

    times
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn snapshot_times_empty_dir_returns_empty_map() {
        let tmp = TempDir::new().unwrap();
        let times = snapshot_times(tmp.path());
        assert!(times.is_empty(), "expected empty map, got {:?}", times);
    }

    #[test]
    fn snapshot_times_includes_dart_files() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.dart"), "void main() {}").unwrap();
        std::fs::write(tmp.path().join("b.dart"), "class B {}").unwrap();
        let times = snapshot_times(tmp.path());
        assert_eq!(times.len(), 2);
        assert!(times.contains_key(&tmp.path().join("a.dart")));
        assert!(times.contains_key(&tmp.path().join("b.dart")));
    }

    #[test]
    fn snapshot_times_excludes_non_dart_files() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.dart"), "void main() {}").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), "some notes").unwrap();
        std::fs::write(tmp.path().join("script.py"), "print('hi')").unwrap();
        let times = snapshot_times(tmp.path());
        assert_eq!(times.len(), 1);
        assert!(times.contains_key(&tmp.path().join("a.dart")));
    }

    #[test]
    fn snapshot_times_walks_subdirectories() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.dart"), "void main() {}").unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b.dart"), "class B {}").unwrap();
        let times = snapshot_times(tmp.path());
        assert_eq!(times.len(), 2);
        assert!(times.contains_key(&tmp.path().join("a.dart")));
        assert!(times.contains_key(&sub.join("b.dart")));
    }

    #[test]
    fn snapshot_times_no_extension_files_excluded() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Makefile"), "all:\n\t@echo ok").unwrap();
        let times = snapshot_times(tmp.path());
        assert!(times.is_empty(), "expected empty map, got {:?}", times);
    }

    #[test]
    fn snapshot_times_returns_modified_time_for_each_file() {
        let tmp = TempDir::new().unwrap();
        let dart_path = tmp.path().join("a.dart");
        std::fs::write(&dart_path, "void main() {}").unwrap();
        let times = snapshot_times(tmp.path());
        assert_eq!(times.len(), 1);
        let recorded_mtime = times[&dart_path];
        let actual_mtime = std::fs::metadata(&dart_path).unwrap().modified().unwrap();
        let diff = if recorded_mtime >= actual_mtime {
            recorded_mtime.duration_since(actual_mtime).unwrap()
        } else {
            actual_mtime.duration_since(recorded_mtime).unwrap()
        };
        assert!(
            diff <= Duration::from_secs(1),
            "mtime diff too large: {:?}",
            diff
        );
    }
}
