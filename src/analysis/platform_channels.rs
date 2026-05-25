//! Multi-language analysis for platform channel code (Kotlin/Swift).
//! Uses text-based heuristics since tree-sitter grammars aren't bundled.

use crate::config::Severity;
use crate::reporters::Issue;
use std::path::Path;

/// Analyze platform channel code in Kotlin and Swift files.
pub fn analyze_platform_channels(root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    let android_dir = root.join("android");
    let ios_dir = root.join("ios");

    if android_dir.exists() {
        issues.extend(analyze_kotlin_files(&android_dir, root));
    }
    if ios_dir.exists() {
        issues.extend(analyze_swift_files(&ios_dir, root));
    }

    issues
}

fn analyze_kotlin_files(dir: &Path, _root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "kt" || ext == "java")
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        check_kotlin_channel_issues(entry.path(), &source, &mut issues);
    }

    issues
}

fn analyze_swift_files(dir: &Path, _root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "swift"))
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        check_swift_channel_issues(entry.path(), &source, &mut issues);
    }

    issues
}

fn check_kotlin_channel_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("MethodChannel") && trimmed.contains("\"") {
            if let Some(channel_name) = extract_string_literal(trimmed) {
                if !channel_name.contains('/') && !channel_name.contains('.') {
                    issues.push(Issue {
                        rule: "platform-channel-naming".to_string(),
                        message: format!(
                            "Platform channel '{}' should use reverse-domain naming (e.g. 'com.example/channel')",
                            channel_name
                        ),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    });
                }
            }
        }

        if trimmed.contains("catch")
            && trimmed.contains("Exception")
            && !trimmed.contains("FlutterError")
        {
            let next_lines: String = source.lines().skip(i).take(3).collect::<Vec<_>>().join(" ");
            if next_lines.contains("{}") || next_lines.contains("{ }") {
                issues.push(Issue {
                    rule: "platform-empty-catch".to_string(),
                    message: "Empty catch block in platform channel code — errors should be forwarded to Flutter via result.error()".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }

        if trimmed.contains("runOnUiThread")
            && source
                .lines()
                .skip(i)
                .take(5)
                .any(|l| l.contains("result.success") || l.contains("result.error"))
        {
            issues.push(Issue {
                rule: "platform-thread-safety".to_string(),
                message: "Ensure MethodChannel result is called on the main thread".to_string(),
                severity: Severity::Info,
                file: file.to_path_buf(),
                line: i + 1,
                column: 1,
            });
        }
    }
}

fn check_swift_channel_issues(file: &Path, source: &str, issues: &mut Vec<Issue>) {
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("FlutterMethodChannel") && trimmed.contains("\"") {
            if let Some(channel_name) = extract_string_literal(trimmed) {
                if !channel_name.contains('/') && !channel_name.contains('.') {
                    issues.push(Issue {
                        rule: "platform-channel-naming".to_string(),
                        message: format!(
                            "Platform channel '{}' should use reverse-domain naming (e.g. 'com.example/channel')",
                            channel_name
                        ),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    });
                }
            }
        }

        if trimmed.contains("catch") && !trimmed.contains("FlutterError") {
            let next_lines: String = source.lines().skip(i).take(3).collect::<Vec<_>>().join(" ");
            if next_lines.contains("{}")
                || next_lines.contains("{ }")
                || next_lines.contains("catch { }")
            {
                issues.push(Issue {
                    rule: "platform-empty-catch".to_string(),
                    message: "Empty catch block in platform channel code — errors should be forwarded to Flutter via result()".to_string(),
                    severity: Severity::Warning,
                    file: file.to_path_buf(),
                    line: i + 1,
                    column: 1,
                });
            }
        }

        if trimmed.contains("DispatchQueue.global")
            && source
                .lines()
                .skip(i)
                .take(5)
                .any(|l| l.contains("result(") || l.contains("FlutterResult"))
        {
            issues.push(Issue {
                rule: "platform-thread-safety".to_string(),
                message: "Ensure FlutterResult is called on the main thread (DispatchQueue.main)"
                    .to_string(),
                severity: Severity::Info,
                file: file.to_path_buf(),
                line: i + 1,
                column: 1,
            });
        }
    }
}

fn extract_string_literal(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Write `content` to `<dir>/<name>` and return the path.
    fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&path).unwrap();
        write!(f, "{}", content).unwrap();
        path
    }

    /// Run `check_kotlin_channel_issues` on in-memory source and collect issues.
    fn run_kotlin(source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let fake_path = std::path::Path::new("test.kt");
        check_kotlin_channel_issues(fake_path, source, &mut issues);
        issues
    }

    /// Run `check_swift_channel_issues` on in-memory source and collect issues.
    fn run_swift(source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let fake_path = std::path::Path::new("test.swift");
        check_swift_channel_issues(fake_path, source, &mut issues);
        issues
    }

    // ── extract_string_literal ────────────────────────────────────────────────

    #[test]
    fn extract_string_literal_basic() {
        assert_eq!(
            extract_string_literal(r#"MethodChannel("mychannel")"#),
            Some("mychannel".to_string())
        );
    }

    #[test]
    fn extract_string_literal_returns_first_quoted_segment() {
        assert_eq!(
            extract_string_literal(r#"val ch = MethodChannel("com.example/channel", codec)"#),
            Some("com.example/channel".to_string())
        );
    }

    #[test]
    fn extract_string_literal_empty_string() {
        assert_eq!(extract_string_literal(r#"foo("")"#), Some("".to_string()));
    }

    #[test]
    fn extract_string_literal_no_quotes_returns_none() {
        assert_eq!(extract_string_literal("no quotes here"), None);
    }

    #[test]
    fn extract_string_literal_single_quote_returns_none() {
        // Only one `"` — start found but end not found
        assert_eq!(extract_string_literal(r#"prefix "only_open"#), None);
    }

    // ── check_kotlin_channel_issues — channel naming ──────────────────────────

    #[test]
    fn kotlin_channel_naming_no_slash_or_dot_triggers() {
        let src = r#"val channel = MethodChannel(flutterEngine.dartExecutor, "mychannel")"#;
        let issues = run_kotlin(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-channel-naming");
        assert!(issues[0].message.contains("mychannel"));
        assert_eq!(issues[0].line, 1);
    }

    #[test]
    fn kotlin_channel_naming_with_slash_is_clean() {
        let src = r#"val channel = MethodChannel(executor, "com.example/channel")"#;
        let issues = run_kotlin(src);
        assert!(issues.iter().all(|i| i.rule != "platform-channel-naming"));
    }

    #[test]
    fn kotlin_channel_naming_with_dot_is_clean() {
        let src = r#"val channel = MethodChannel(executor, "com.example.channel")"#;
        let issues = run_kotlin(src);
        assert!(issues.iter().all(|i| i.rule != "platform-channel-naming"));
    }

    #[test]
    fn kotlin_channel_naming_no_quote_no_trigger() {
        // MethodChannel present but no string literal
        let src = "val channel = MethodChannel(executor, name)";
        let issues = run_kotlin(src);
        assert!(issues.iter().all(|i| i.rule != "platform-channel-naming"));
    }

    // ── check_kotlin_channel_issues — empty catch ─────────────────────────────

    #[test]
    fn kotlin_empty_catch_curly_triggers() {
        let src = "} catch (e: Exception) {}";
        let issues = run_kotlin(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-empty-catch");
    }

    #[test]
    fn kotlin_empty_catch_spaced_curly_triggers() {
        let src = "} catch (e: Exception) { }";
        let issues = run_kotlin(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-empty-catch");
    }

    #[test]
    fn kotlin_empty_catch_in_next_lines_triggers() {
        // The check looks at next 3 lines joined; put `{}` on the following line.
        let src = "} catch (e: Exception) {\n  {}\n}";
        let issues = run_kotlin(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-empty-catch");
    }

    #[test]
    fn kotlin_catch_with_flutter_error_no_trigger() {
        let src = "} catch (e: FlutterError) {}";
        let issues = run_kotlin(src);
        assert!(issues.iter().all(|i| i.rule != "platform-empty-catch"));
    }

    #[test]
    fn kotlin_catch_exception_with_body_no_trigger() {
        let src = "} catch (e: Exception) {\n    result.error(\"ERR\", e.message, null)\n}";
        let issues = run_kotlin(src);
        assert!(issues.iter().all(|i| i.rule != "platform-empty-catch"));
    }

    // ── check_kotlin_channel_issues — thread safety ───────────────────────────

    #[test]
    fn kotlin_thread_safety_result_success_triggers() {
        let src = "runOnUiThread {\n    result.success(data)\n}";
        let issues = run_kotlin(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-thread-safety");
        assert_eq!(issues[0].line, 1);
    }

    #[test]
    fn kotlin_thread_safety_result_error_triggers() {
        let src = "runOnUiThread {\n    result.error(\"E\", \"msg\", null)\n}";
        let issues = run_kotlin(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-thread-safety");
    }

    #[test]
    fn kotlin_run_on_ui_thread_without_result_call_no_trigger() {
        let src = "runOnUiThread {\n    doSomethingElse()\n}";
        let issues = run_kotlin(src);
        assert!(issues.iter().all(|i| i.rule != "platform-thread-safety"));
    }

    // ── check_swift_channel_issues — channel naming ───────────────────────────

    #[test]
    fn swift_channel_naming_no_slash_or_dot_triggers() {
        let src = r#"let channel = FlutterMethodChannel(name: "mychannel", binaryMessenger: messenger)"#;
        let issues = run_swift(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-channel-naming");
        assert!(issues[0].message.contains("mychannel"));
    }

    #[test]
    fn swift_channel_naming_with_slash_is_clean() {
        let src = r#"let channel = FlutterMethodChannel(name: "com.example/channel", binaryMessenger: m)"#;
        let issues = run_swift(src);
        assert!(issues.iter().all(|i| i.rule != "platform-channel-naming"));
    }

    #[test]
    fn swift_channel_naming_with_dot_is_clean() {
        let src = r#"let channel = FlutterMethodChannel(name: "com.example.channel", binaryMessenger: m)"#;
        let issues = run_swift(src);
        assert!(issues.iter().all(|i| i.rule != "platform-channel-naming"));
    }

    #[test]
    fn swift_flutter_method_channel_no_quote_no_trigger() {
        let src = "let channel = FlutterMethodChannel(name: channelName, binaryMessenger: m)";
        let issues = run_swift(src);
        assert!(issues.iter().all(|i| i.rule != "platform-channel-naming"));
    }

    // ── check_swift_channel_issues — empty catch ──────────────────────────────

    #[test]
    fn swift_empty_catch_curly_triggers() {
        let src = "} catch {}";
        let issues = run_swift(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-empty-catch");
    }

    #[test]
    fn swift_empty_catch_spaced_curly_triggers() {
        let src = "} catch { }";
        let issues = run_swift(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-empty-catch");
    }

    #[test]
    fn swift_catch_with_flutter_error_no_trigger() {
        // Contains both "catch" and "FlutterError" — should not fire
        let src = "} catch let error as FlutterError { }";
        let issues = run_swift(src);
        assert!(issues.iter().all(|i| i.rule != "platform-empty-catch"));
    }

    #[test]
    fn swift_catch_with_body_no_trigger() {
        let src = "} catch {\n    result(FlutterError(code: \"ERR\", message: e.localizedDescription, details: nil))\n}";
        let issues = run_swift(src);
        assert!(issues.iter().all(|i| i.rule != "platform-empty-catch"));
    }

    // ── check_swift_channel_issues — thread safety ────────────────────────────

    #[test]
    fn swift_thread_safety_result_call_triggers() {
        let src = "DispatchQueue.global(qos: .background).async {\n    result(value)\n}";
        let issues = run_swift(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-thread-safety");
        assert_eq!(issues[0].line, 1);
    }

    #[test]
    fn swift_thread_safety_flutter_result_triggers() {
        let src = "DispatchQueue.global().async {\n    let r: FlutterResult = result\n}";
        let issues = run_swift(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "platform-thread-safety");
    }

    #[test]
    fn swift_dispatch_global_without_result_no_trigger() {
        let src = "DispatchQueue.global().async {\n    processData()\n}";
        let issues = run_swift(src);
        assert!(issues.iter().all(|i| i.rule != "platform-thread-safety"));
    }

    // ── analyze_platform_channels (orchestrator) ──────────────────────────────

    #[test]
    fn orchestrator_empty_dir_returns_no_issues() {
        let root = TempDir::new().unwrap();
        let issues = analyze_platform_channels(root.path());
        assert!(issues.is_empty());
    }

    #[test]
    fn orchestrator_no_android_or_ios_dir_returns_no_issues() {
        let root = TempDir::new().unwrap();
        // Create some unrelated dir
        fs::create_dir_all(root.path().join("lib")).unwrap();
        let issues = analyze_platform_channels(root.path());
        assert!(issues.is_empty());
    }

    #[test]
    fn orchestrator_android_dir_with_bad_naming_kotlin_file() {
        let root = TempDir::new().unwrap();
        let android_dir = root.path().join("android");
        write_file(
            &android_dir,
            "MainActivity.kt",
            r#"val ch = MethodChannel(executor, "badname")"#,
        );
        let issues = analyze_platform_channels(root.path());
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == "platform-channel-naming"));
    }

    #[test]
    fn orchestrator_android_dir_with_java_file() {
        let root = TempDir::new().unwrap();
        let android_dir = root.path().join("android");
        write_file(
            &android_dir,
            "MainActivity.java",
            r#"new MethodChannel(executor, "badchannel");"#,
        );
        let issues = analyze_platform_channels(root.path());
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == "platform-channel-naming"));
    }

    #[test]
    fn orchestrator_ios_dir_with_bad_naming_swift_file() {
        let root = TempDir::new().unwrap();
        let ios_dir = root.path().join("ios");
        write_file(
            &ios_dir,
            "AppDelegate.swift",
            r#"let ch = FlutterMethodChannel(name: "badname", binaryMessenger: m)"#,
        );
        let issues = analyze_platform_channels(root.path());
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == "platform-channel-naming"));
    }

    #[test]
    fn orchestrator_ios_dir_ignores_non_swift_files() {
        let root = TempDir::new().unwrap();
        let ios_dir = root.path().join("ios");
        // Write a .m (Objective-C) file — should be ignored
        write_file(
            &ios_dir,
            "AppDelegate.m",
            r#"FlutterMethodChannel* ch = [FlutterMethodChannel channelWithName:@"badname"];"#,
        );
        let issues = analyze_platform_channels(root.path());
        assert!(issues.is_empty());
    }

    #[test]
    fn orchestrator_android_clean_channel_naming_no_issues() {
        let root = TempDir::new().unwrap();
        let android_dir = root.path().join("android");
        write_file(
            &android_dir,
            "MainActivity.kt",
            r#"val ch = MethodChannel(executor, "com.example/channel")"#,
        );
        let issues = analyze_platform_channels(root.path());
        assert!(issues.iter().all(|i| i.rule != "platform-channel-naming"));
    }

    #[test]
    fn orchestrator_both_android_and_ios_collect_from_both() {
        let root = TempDir::new().unwrap();
        let android_dir = root.path().join("android");
        let ios_dir = root.path().join("ios");

        write_file(
            &android_dir,
            "Main.kt",
            r#"val ch = MethodChannel(executor, "badkotlin")"#,
        );
        write_file(
            &ios_dir,
            "App.swift",
            r#"let ch = FlutterMethodChannel(name: "badswift", binaryMessenger: m)"#,
        );
        let issues = analyze_platform_channels(root.path());
        // Should find naming issues from both files
        let naming_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.rule == "platform-channel-naming")
            .collect();
        assert_eq!(naming_issues.len(), 2);
    }

    #[test]
    fn kotlin_multiple_issues_in_one_file() {
        let src = concat!(
            "val ch = MethodChannel(executor, \"badname\")\n",
            "} catch (e: Exception) {}\n",
        );
        let issues = run_kotlin(src);
        assert!(issues.iter().any(|i| i.rule == "platform-channel-naming"));
        assert!(issues.iter().any(|i| i.rule == "platform-empty-catch"));
    }

    #[test]
    fn swift_multiple_issues_in_one_file() {
        let src = concat!(
            "let ch = FlutterMethodChannel(name: \"noformat\", binaryMessenger: m)\n",
            "} catch {}\n",
        );
        let issues = run_swift(src);
        assert!(issues.iter().any(|i| i.rule == "platform-channel-naming"));
        assert!(issues.iter().any(|i| i.rule == "platform-empty-catch"));
    }

    #[test]
    fn kotlin_line_numbers_reported_correctly() {
        let src = "// first line\nval ch = MethodChannel(executor, \"badname\")\n";
        let issues = run_kotlin(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 2);
    }

    #[test]
    fn swift_line_numbers_reported_correctly() {
        let src = "// first line\nlet ch = FlutterMethodChannel(name: \"badname\", binaryMessenger: m)\n";
        let issues = run_swift(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 2);
    }
}

/// Summary of platform channel analysis.
pub fn print_platform_summary(issues: &[Issue]) {
    use colored::Colorize;

    let kt_count = issues
        .iter()
        .filter(|i| {
            i.file
                .extension()
                .map_or(false, |e| e == "kt" || e == "java")
        })
        .count();
    let swift_count = issues
        .iter()
        .filter(|i| i.file.extension().map_or(false, |e| e == "swift"))
        .count();

    println!();
    println!(
        "  {} Platform Channel Analysis",
        "falcon".bright_cyan().bold()
    );
    println!();
    println!("  Kotlin/Java issues: {}", kt_count);
    println!("  Swift issues:       {}", swift_count);
    println!("  Total:              {}", issues.len());

    if issues.is_empty() {
        println!();
        println!("  {} No platform channel issues found.", "✓".green().bold());
    }
    println!();
}
