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
}

fn snapshot_times(root: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut times = HashMap::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                times.insert(entry.path().to_path_buf(), mtime);
            }
        }
    }

    times
}
