use crate::reporters::Issue;
use colored::Colorize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ConfidenceResult {
    pub issue: Issue,
    pub confidence: u8,
    pub reason: String,
    pub reducers: Vec<String>,
}

impl ConfidenceResult {
    pub fn confidence_label(&self) -> &'static str {
        match self.confidence {
            90..=100 => "high",
            70..=89 => "medium",
            50..=69 => "low",
            _ => "uncertain",
        }
    }

    pub fn confidence_color(&self) -> colored::Color {
        match self.confidence {
            90..=100 => colored::Color::Green,
            70..=89 => colored::Color::Yellow,
            50..=69 => colored::Color::Red,
            _ => colored::Color::BrightBlack,
        }
    }
}

/// Score unused code issues with confidence levels using local heuristics.
pub fn score_unused_issues(issues: &[Issue], project_root: &Path) -> Vec<ConfidenceResult> {
    let reflection_files = find_reflection_patterns(project_root);
    let dynamic_files = find_dynamic_usage(project_root);
    let export_files = find_barrel_exports(project_root);

    issues
        .iter()
        .filter(|i| {
            i.rule == "unused-code"
                || i.rule == "unused-file"
                || i.rule == "dead-code-path"
                || i.rule == "unused-dependency"
        })
        .map(|issue| score_single_issue(issue, &reflection_files, &dynamic_files, &export_files))
        .collect()
}

fn score_single_issue(
    issue: &Issue,
    reflection_files: &HashSet<String>,
    dynamic_files: &HashSet<String>,
    export_files: &HashSet<String>,
) -> ConfidenceResult {
    let mut confidence: i32 = 95;
    let mut reducers = Vec::new();

    let file_str = issue.file.to_string_lossy().to_string();
    let file_name = issue
        .file
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if reflection_files.contains(&file_str) {
        confidence -= 25;
        reducers.push("File contains reflection/mirrors — may be used dynamically".to_string());
    }

    if dynamic_files.contains(&file_str) {
        confidence -= 15;
        reducers
            .push("File uses dynamic types — references may not be statically visible".to_string());
    }

    if export_files.contains(file_name) {
        confidence -= 10;
        reducers
            .push("File is re-exported from a barrel file — may be used externally".to_string());
    }

    if issue.message.contains("appears to be unused") {
        let name = extract_name_from_message(&issue.message);
        if name.starts_with('_') {
            confidence += 5;
            confidence = confidence.min(100);
        } else if is_common_interface_name(&name) {
            confidence -= 20;
            reducers.push(format!(
                "'{}' follows common interface/mixin naming — may be used via subtype",
                name
            ));
        }
    }

    if file_str.contains("/generated/")
        || file_str.contains(".g.dart")
        || file_str.contains(".freezed.dart")
    {
        confidence -= 30;
        reducers.push("Generated file — code generation tools may use this".to_string());
    }

    if file_str.contains("_test.dart") || file_str.contains("/test/") {
        confidence -= 5;
        reducers.push("Test file — may be indirectly invoked by test runner".to_string());
    }

    if issue.rule == "dead-code-path" {
        if issue.message.contains("always true") {
            confidence = 85;
        } else if issue.message.contains("always false") {
            confidence = 90;
        } else if issue.message.contains("Unreachable") {
            confidence = 95;
        }
    }

    confidence = confidence.clamp(10, 100);

    let reason = if reducers.is_empty() {
        "No dynamic patterns detected — high confidence this code is unused.".to_string()
    } else {
        format!("{} factor(s) reduce confidence.", reducers.len())
    };

    ConfidenceResult {
        issue: issue.clone(),
        confidence: confidence as u8,
        reason,
        reducers,
    }
}

fn extract_name_from_message(message: &str) -> String {
    if let Some(start) = message.find('\'') {
        let rest = &message[start + 1..];
        if let Some(end) = rest.find('\'') {
            return rest[..end].to_string();
        }
    }
    String::new()
}

fn is_common_interface_name(name: &str) -> bool {
    name.ends_with("Base")
        || name.ends_with("Mixin")
        || name.ends_with("Interface")
        || name.ends_with("Protocol")
        || name.ends_with("Delegate")
        || name.ends_with("Factory")
        || name.ends_with("Strategy")
        || name.ends_with("Observer")
        || name.ends_with("Listener")
}

fn find_reflection_patterns(root: &Path) -> HashSet<String> {
    let mut files = HashSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            if source.contains("dart:mirrors")
                || source.contains("reflectable")
                || source.contains("@reflector")
                || source.contains("noSuchMethod")
            {
                files.insert(entry.path().to_string_lossy().to_string());
            }
        }
    }
    files
}

fn find_dynamic_usage(root: &Path) -> HashSet<String> {
    let mut files = HashSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            let dynamic_count = source.matches("dynamic ").count()
                + source.matches("dynamic>").count()
                + source.matches("dynamic,").count();
            if dynamic_count >= 3 {
                files.insert(entry.path().to_string_lossy().to_string());
            }
        }
    }
    files
}

fn find_barrel_exports(root: &Path) -> HashSet<String> {
    let mut exported_files = HashSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
    {
        if let Ok(source) = std::fs::read_to_string(entry.path()) {
            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("export") {
                    if let Some(start) = trimmed.find('\'') {
                        let rest = &trimmed[start + 1..];
                        if let Some(end) = rest.find('\'') {
                            let uri = &rest[..end];
                            if let Some(fname) = std::path::Path::new(uri).file_name() {
                                exported_files.insert(fname.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    exported_files
}

pub fn print_confidence_results(results: &[ConfidenceResult], min_confidence: Option<u8>) {
    let min = min_confidence.unwrap_or(0);
    let filtered: Vec<_> = results.iter().filter(|r| r.confidence >= min).collect();

    if filtered.is_empty() {
        println!(
            "  {} No issues above {}% confidence threshold.",
            "✓".green().bold(),
            min
        );
        return;
    }

    let high = filtered.iter().filter(|r| r.confidence >= 90).count();
    let medium = filtered
        .iter()
        .filter(|r| r.confidence >= 70 && r.confidence < 90)
        .count();
    let low = filtered.iter().filter(|r| r.confidence < 70).count();

    println!();
    println!(
        "  {} {} issues with confidence scoring",
        "falcon".bright_cyan().bold(),
        filtered.len()
    );
    println!(
        "    {} high (≥90%)  {} medium (70-89%)  {} low (<70%)",
        high.to_string().green(),
        medium.to_string().yellow(),
        low.to_string().red()
    );
    println!();

    for result in &filtered {
        let conf_str = format!("{}%", result.confidence);
        let rel_path = result
            .issue
            .file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("?");

        println!(
            "  {} [{}] {}:{}  {}",
            conf_str.color(result.confidence_color()),
            result.confidence_label(),
            rel_path,
            result.issue.line,
            result.issue.message
        );

        for reducer in &result.reducers {
            println!("       {} {}", "↓".dimmed(), reducer.dimmed());
        }
    }
    println!();
}
