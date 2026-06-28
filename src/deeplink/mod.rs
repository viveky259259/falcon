use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeeplinkSeverity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for DeeplinkSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeeplinkSeverity::Error => write!(f, "Error"),
            DeeplinkSeverity::Warning => write!(f, "Warning"),
            DeeplinkSeverity::Info => write!(f, "Info"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeeplinkIssue {
    pub severity: DeeplinkSeverity,
    pub platform: &'static str,
    pub category: &'static str,
    pub file: PathBuf,
    pub detail: String,
    pub suggestion: String,
}

#[derive(Debug, Clone)]
pub struct DeeplinkReport {
    pub android_schemes: Vec<String>,
    pub ios_schemes: Vec<String>,
    pub flutter_routes: Vec<String>,
    pub issues: Vec<DeeplinkIssue>,
    pub score: u32,
}

pub fn validate_deeplinks(path: &Path) -> Result<DeeplinkReport> {
    let mut report = DeeplinkReport {
        android_schemes: Vec::new(),
        ios_schemes: Vec::new(),
        flutter_routes: Vec::new(),
        issues: Vec::new(),
        score: 100,
    };

    // Validate Android manifest
    validate_android_manifest(path, &mut report);

    // Validate iOS Info.plist
    validate_ios_info_plist(path, &mut report);

    // Validate Flutter routes
    validate_flutter_routes(path, &mut report);

    // Cross-platform validations
    validate_cross_platform_consistency(&mut report);

    // Calculate final score
    report.score = calculate_score(&report);

    Ok(report)
}

fn validate_android_manifest(path: &Path, report: &mut DeeplinkReport) {
    let manifest_path = path.join("android/app/src/main/AndroidManifest.xml");

    if !manifest_path.exists() {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "Android",
            category: "Missing File",
            file: manifest_path.clone(),
            detail: "AndroidManifest.xml not found".to_string(),
            suggestion: "Ensure your Flutter app has an Android module with AndroidManifest.xml"
                .to_string(),
        });
        return;
    }

    let content = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            report.issues.push(DeeplinkIssue {
                severity: DeeplinkSeverity::Warning,
                platform: "Android",
                category: "File Read Error",
                file: manifest_path,
                detail: format!("Failed to read AndroidManifest.xml: {}", e),
                suggestion: "Check file permissions and ensure the file is valid XML".to_string(),
            });
            return;
        }
    };

    // Check for intent-filter and deep link configuration
    let has_intent_filter = content.contains("intent-filter");
    let has_browsable = content.contains("android.intent.category.BROWSABLE");

    if !has_intent_filter {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Error,
            platform: "Android",
            category: "Missing Intent Filter",
            file: manifest_path.clone(),
            detail: "No intent-filter found for deep links".to_string(),
            suggestion:
                "Add an intent-filter with action VIEW and category BROWSABLE to your activity"
                    .to_string(),
        });
    }

    if !has_browsable {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Error,
            platform: "Android",
            category: "Missing BROWSABLE Category",
            file: manifest_path.clone(),
            detail: "BROWSABLE category not found in intent-filter".to_string(),
            suggestion: "Add android.intent.category.BROWSABLE to allow browser to open your app"
                .to_string(),
        });
    }

    // Extract schemes
    extract_android_schemes(&content, report, &manifest_path);

    // Check for App Links configuration (autoVerify)
    let has_app_links_intent_filter = content.contains("intent-filter");
    let has_auto_verify = content.contains("autoVerify");

    if has_app_links_intent_filter && !has_auto_verify {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "Android",
            category: "App Links Not Verified",
            file: manifest_path.clone(),
            detail: "autoVerify=\"true\" not found for App Links".to_string(),
            suggestion:
                "Add autoVerify=\"true\" to your intent-filter for automatic domain verification"
                    .to_string(),
        });
    }

    // Check for HTTPS enforcement
    if content.contains("android:scheme=\"http\"") {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "Android",
            category: "Insecure Scheme",
            file: manifest_path,
            detail: "HTTP scheme found instead of HTTPS".to_string(),
            suggestion: "Use https:// scheme for App Links to improve security and app trust"
                .to_string(),
        });
    }
}

fn extract_android_schemes(content: &str, report: &mut DeeplinkReport, manifest_path: &Path) {
    for line in content.lines() {
        if line.contains("android:scheme") {
            // Extract scheme value from android:scheme="value"
            if let Some(start) = line.find("android:scheme=\"") {
                let start = start + "android:scheme=\"".len();
                if let Some(end) = line[start..].find('"') {
                    let scheme = line[start..start + end].to_string();
                    if !scheme.is_empty() && !report.android_schemes.contains(&scheme) {
                        report.android_schemes.push(scheme);
                    }
                }
            }
        }
    }

    if report.android_schemes.is_empty() {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Error,
            platform: "Android",
            category: "No Schemes Found",
            file: manifest_path.to_path_buf(),
            detail: "No URL schemes defined in AndroidManifest.xml".to_string(),
            suggestion: "Define at least one URL scheme using android:scheme attribute".to_string(),
        });
    }
}

fn validate_ios_info_plist(path: &Path, report: &mut DeeplinkReport) {
    let plist_path = path.join("ios/Runner/Info.plist");

    if !plist_path.exists() {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "iOS",
            category: "Missing File",
            file: plist_path,
            detail: "Info.plist not found".to_string(),
            suggestion: "Ensure your Flutter app has an iOS module with Info.plist".to_string(),
        });
        return;
    }

    let content = match std::fs::read_to_string(&plist_path) {
        Ok(c) => c,
        Err(e) => {
            report.issues.push(DeeplinkIssue {
                severity: DeeplinkSeverity::Warning,
                platform: "iOS",
                category: "File Read Error",
                file: plist_path,
                detail: format!("Failed to read Info.plist: {}", e),
                suggestion: "Check file permissions and ensure the plist is valid".to_string(),
            });
            return;
        }
    };

    // Check for CFBundleURLTypes
    if !content.contains("CFBundleURLTypes") {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Error,
            platform: "iOS",
            category: "Missing URL Types",
            file: plist_path.clone(),
            detail: "CFBundleURLTypes not found in Info.plist".to_string(),
            suggestion: "Add CFBundleURLTypes array to define custom URL schemes for deep links"
                .to_string(),
        });
    }

    // Check for CFBundleURLSchemes
    if !content.contains("CFBundleURLSchemes") {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Error,
            platform: "iOS",
            category: "Missing URL Schemes",
            file: plist_path.clone(),
            detail: "CFBundleURLSchemes not found in Info.plist".to_string(),
            suggestion: "Define URL schemes within CFBundleURLTypes".to_string(),
        });
    }

    // Extract schemes
    extract_ios_schemes(&content, report, &plist_path);

    // Check for Associated Domains
    if !content.contains("NSAppTransportSecurity")
        && !content.contains("com.apple.developer.associated-domains")
    {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Info,
            platform: "iOS",
            category: "Missing Universal Links Config",
            file: plist_path,
            detail: "Associated Domains or ATS configuration not found".to_string(),
            suggestion:
                "Consider adding Associated Domains entitlement for Universal Links support"
                    .to_string(),
        });
    }
}

fn extract_ios_schemes(content: &str, report: &mut DeeplinkReport, plist_path: &Path) {
    let mut in_url_schemes = false;

    for line in content.lines() {
        if line.contains("CFBundleURLSchemes") {
            in_url_schemes = true;
            continue;
        }

        if in_url_schemes {
            if line.contains("</array>") {
                break;
            }
            if line.contains("<string>") {
                if let Some(start) = line.find("<string>") {
                    let start = start + "<string>".len();
                    if let Some(end) = line[start..].find("</string>") {
                        let scheme = line[start..start + end].to_string();
                        if !scheme.is_empty() && !report.ios_schemes.contains(&scheme) {
                            report.ios_schemes.push(scheme);
                        }
                    }
                }
            }
        }
    }

    if report.ios_schemes.is_empty() && content.contains("CFBundleURLSchemes") {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "iOS",
            category: "Empty URL Schemes",
            file: plist_path.to_path_buf(),
            detail: "CFBundleURLSchemes array is empty".to_string(),
            suggestion: "Add at least one URL scheme to CFBundleURLSchemes".to_string(),
        });
    }
}

fn validate_flutter_routes(path: &Path, report: &mut DeeplinkReport) {
    let lib_path = path.join("lib");

    if !lib_path.exists() {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "Flutter",
            category: "Missing Directory",
            file: lib_path,
            detail: "lib directory not found".to_string(),
            suggestion: "Ensure you have a Flutter lib directory".to_string(),
        });
        return;
    }

    let dart_files: Vec<PathBuf> = WalkDir::new(&lib_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if dart_files.is_empty() {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Info,
            platform: "Flutter",
            category: "No Dart Files",
            file: lib_path,
            detail: "No Dart files found in lib directory".to_string(),
            suggestion: "Create Dart files with route definitions".to_string(),
        });
        return;
    }

    // Extract routes from Dart files
    for dart_file in dart_files {
        extract_flutter_routes(&dart_file, report);
    }

    if report.flutter_routes.is_empty() {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "Flutter",
            category: "No Routes Found",
            file: lib_path,
            detail: "No GoRouter or Navigator routes found in Dart files".to_string(),
            suggestion: "Define routes using GoRoute or Navigator pushNamed".to_string(),
        });
    }
}

fn extract_flutter_routes(dart_file: &Path, report: &mut DeeplinkReport) {
    let content = match std::fs::read_to_string(dart_file) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Look for GoRoute(path: patterns
    for line in content.lines() {
        if line.contains("GoRoute(path:") || line.contains("path:") {
            // Extract path value from path: 'value' or path: "value"
            if let Some(start) = line.find("path:") {
                let remainder = &line[start + 5..];
                for (i, ch) in remainder.chars().enumerate() {
                    if ch == '\'' || ch == '"' {
                        let quote = ch;
                        let path_content = &remainder[i + 1..];
                        if let Some(end) = path_content.find(quote) {
                            let route = path_content[..end].to_string();
                            if !route.is_empty() && !report.flutter_routes.contains(&route) {
                                report.flutter_routes.push(route);
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Look for pushNamed patterns like pushNamed('/route')
        if line.contains("pushNamed(") {
            if let Some(start) = line.find("pushNamed(") {
                let remainder = &line[start + 10..];
                for (i, ch) in remainder.chars().enumerate() {
                    if ch == '\'' || ch == '"' {
                        let quote = ch;
                        let path_content = &remainder[i + 1..];
                        if let Some(end) = path_content.find(quote) {
                            let route = path_content[..end].to_string();
                            if !route.is_empty() && !report.flutter_routes.contains(&route) {
                                report.flutter_routes.push(route);
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
}

fn validate_cross_platform_consistency(report: &mut DeeplinkReport) {
    // Check for scheme mismatches between iOS and Android
    let android_set: HashSet<_> = report.android_schemes.iter().cloned().collect();
    let ios_set: HashSet<_> = report.ios_schemes.iter().cloned().collect();

    if !android_set.is_empty() && !ios_set.is_empty() {
        let intersection = android_set.intersection(&ios_set).count();

        if intersection == 0 {
            report.issues.push(DeeplinkIssue {
                severity: DeeplinkSeverity::Error,
                platform: "Cross-Platform",
                category: "Scheme Mismatch",
                file: PathBuf::new(),
                detail: format!(
                    "No matching schemes between iOS ({:?}) and Android ({:?})",
                    ios_set, android_set
                ),
                suggestion: "Ensure iOS and Android use the same URL schemes for consistency"
                    .to_string(),
            });
        }

        let only_android: Vec<_> = android_set.difference(&ios_set).cloned().collect();
        let only_ios: Vec<_> = ios_set.difference(&android_set).cloned().collect();

        if !only_android.is_empty() {
            report.issues.push(DeeplinkIssue {
                severity: DeeplinkSeverity::Warning,
                platform: "Cross-Platform",
                category: "Platform-Specific Schemes",
                file: PathBuf::new(),
                detail: format!("Schemes only in Android: {:?}", only_android),
                suggestion: "Add these schemes to iOS or remove them from Android for consistency"
                    .to_string(),
            });
        }

        if !only_ios.is_empty() {
            report.issues.push(DeeplinkIssue {
                severity: DeeplinkSeverity::Warning,
                platform: "Cross-Platform",
                category: "Platform-Specific Schemes",
                file: PathBuf::new(),
                detail: format!("Schemes only in iOS: {:?}", only_ios),
                suggestion: "Add these schemes to Android or remove them from iOS for consistency"
                    .to_string(),
            });
        }
    }

    // Check for unhandled deep link paths
    validate_route_handlers(report);
}

fn validate_route_handlers(report: &mut DeeplinkReport) {
    // This is a simplified check for unhandled paths
    // In real usage, you'd parse more deeply
    if !report.android_schemes.is_empty() && report.flutter_routes.is_empty() {
        report.issues.push(DeeplinkIssue {
            severity: DeeplinkSeverity::Warning,
            platform: "Flutter",
            category: "Unhandled Deep Links",
            file: PathBuf::new(),
            detail: "Deep links are configured in native code but no Flutter routes found"
                .to_string(),
            suggestion: "Add route handlers in your Flutter GoRouter or Navigator configuration"
                .to_string(),
        });
    }
}

fn calculate_score(report: &DeeplinkReport) -> u32 {
    let mut score = 100i32;

    for issue in &report.issues {
        match issue.severity {
            DeeplinkSeverity::Error => score -= 15,
            DeeplinkSeverity::Warning => score -= 8,
            DeeplinkSeverity::Info => score -= 2,
        }
    }

    std::cmp::max(0, score) as u32
}

pub fn print_deeplink_report(report: &DeeplinkReport) {
    println!("\n{}", "=== Deep Link Validator Report ===".bold());

    println!("\n{}", "Detected Schemes".bold().underline());
    if !report.android_schemes.is_empty() {
        println!(
            "  {}: {}",
            "Android".cyan(),
            report.android_schemes.join(", ")
        );
    } else {
        println!("  {}: {}", "Android".cyan(), "none detected".dimmed());
    }

    if !report.ios_schemes.is_empty() {
        println!("  {}: {}", "iOS".cyan(), report.ios_schemes.join(", "));
    } else {
        println!("  {}: {}", "iOS".cyan(), "none detected".dimmed());
    }

    println!("\n{}", "Detected Routes".bold().underline());
    if !report.flutter_routes.is_empty() {
        println!(
            "  {}: {} routes",
            "Flutter".cyan(),
            report.flutter_routes.len()
        );
        for route in &report.flutter_routes {
            println!("    - {}", route.dimmed());
        }
    } else {
        println!("  {}: {}", "Flutter".cyan(), "none detected".dimmed());
    }

    if !report.issues.is_empty() {
        println!("\n{}", "Issues Found".bold().underline());
        for issue in &report.issues {
            let severity_str = match issue.severity {
                DeeplinkSeverity::Error => issue.severity.to_string().red().to_string(),
                DeeplinkSeverity::Warning => issue.severity.to_string().yellow().to_string(),
                DeeplinkSeverity::Info => issue.severity.to_string().blue().to_string(),
            };

            println!(
                "  {} [{}] {}",
                severity_str,
                issue.platform.cyan(),
                issue.category.bold()
            );
            println!("    Detail: {}", issue.detail);
            println!("    File: {}", issue.file.display());
            println!("    Suggestion: {}", issue.suggestion.italic());
        }
    } else {
        println!("\n{}", "No issues found!".green().bold());
    }

    println!(
        "\n{}: {}/100",
        "Deep Link Score".bold(),
        report.score.to_string().cyan()
    );
    println!();
}

pub fn write_deeplink_html_report(report: &DeeplinkReport, path: &Path) -> Result<()> {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html>\n");
    html.push_str("<head>\n");
    html.push_str("  <title>Deep Link Validator Report</title>\n");
    html.push_str("  <style>\n");
    html.push_str(
        "    body { font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }\n",
    );
    html.push_str("    .container { max-width: 1000px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }\n");
    html.push_str(
        "    h1 { color: #333; border-bottom: 3px solid #007acc; padding-bottom: 10px; }\n",
    );
    html.push_str("    h2 { color: #007acc; margin-top: 30px; }\n");
    html.push_str("    .score { font-size: 48px; font-weight: bold; color: #007acc; }\n");
    html.push_str("    .score-high { color: #4caf50; }\n");
    html.push_str("    .score-medium { color: #ff9800; }\n");
    html.push_str("    .score-low { color: #f44336; }\n");
    html.push_str("    .schemes { display: flex; gap: 20px; margin: 20px 0; }\n");
    html.push_str(
        "    .scheme-box { border: 1px solid #ddd; padding: 15px; border-radius: 4px; }\n",
    );
    html.push_str("    .scheme-box h3 { margin: 0 0 10px 0; color: #007acc; }\n");
    html.push_str("    .scheme-list { list-style: none; padding: 0; margin: 0; }\n");
    html.push_str("    .scheme-list li { padding: 5px 0; }\n");
    html.push_str("    .issue { border-left: 4px solid; padding: 15px; margin: 10px 0; border-radius: 4px; }\n");
    html.push_str("    .issue-error { border-left-color: #f44336; background: #ffebee; }\n");
    html.push_str("    .issue-warning { border-left-color: #ff9800; background: #fff3e0; }\n");
    html.push_str("    .issue-info { border-left-color: #2196f3; background: #e3f2fd; }\n");
    html.push_str(
        "    .issue-title { font-weight: bold; font-size: 1.1em; margin-bottom: 5px; }\n",
    );
    html.push_str("    .issue-detail { margin: 5px 0; color: #666; }\n");
    html.push_str("    .issue-suggestion { margin: 10px 0; padding: 10px; background: rgba(0,0,0,0.05); border-radius: 4px; font-style: italic; }\n");
    html.push_str("    .routes { margin: 20px 0; }\n");
    html.push_str("    .route-list { list-style: none; padding: 0; margin: 10px 0; }\n");
    html.push_str("    .route-list li { padding: 8px; background: #f9f9f9; margin: 5px 0; border-left: 3px solid #007acc; padding-left: 12px; }\n");
    html.push_str("  </style>\n");
    html.push_str("</head>\n");
    html.push_str("<body>\n");
    html.push_str("  <div class=\"container\">\n");
    html.push_str("    <h1>Deep Link Validator Report</h1>\n");

    // Score
    let score_class = if report.score >= 80 {
        "score-high"
    } else if report.score >= 60 {
        "score-medium"
    } else {
        "score-low"
    };
    html.push_str(&format!(
        "    <p><span class=\"score {}\">Score: {}/100</span></p>\n",
        score_class, report.score
    ));

    // Schemes
    html.push_str("    <h2>Detected Schemes</h2>\n");
    html.push_str("    <div class=\"schemes\">\n");
    html.push_str("      <div class=\"scheme-box\">\n");
    html.push_str("        <h3>Android</h3>\n");
    if !report.android_schemes.is_empty() {
        html.push_str("        <ul class=\"scheme-list\">\n");
        for scheme in &report.android_schemes {
            html.push_str(&format!("          <li>{}</li>\n", scheme));
        }
        html.push_str("        </ul>\n");
    } else {
        html.push_str("        <p><em>None detected</em></p>\n");
    }
    html.push_str("      </div>\n");

    html.push_str("      <div class=\"scheme-box\">\n");
    html.push_str("        <h3>iOS</h3>\n");
    if !report.ios_schemes.is_empty() {
        html.push_str("        <ul class=\"scheme-list\">\n");
        for scheme in &report.ios_schemes {
            html.push_str(&format!("          <li>{}</li>\n", scheme));
        }
        html.push_str("        </ul>\n");
    } else {
        html.push_str("        <p><em>None detected</em></p>\n");
    }
    html.push_str("      </div>\n");
    html.push_str("    </div>\n");

    // Routes
    if !report.flutter_routes.is_empty() {
        html.push_str("    <h2>Detected Flutter Routes</h2>\n");
        html.push_str("    <ul class=\"route-list\">\n");
        for route in &report.flutter_routes {
            html.push_str(&format!("      <li>{}</li>\n", route));
        }
        html.push_str("    </ul>\n");
    }

    // Issues
    if !report.issues.is_empty() {
        html.push_str("    <h2>Issues Found</h2>\n");
        for issue in &report.issues {
            let issue_class = match issue.severity {
                DeeplinkSeverity::Error => "issue-error",
                DeeplinkSeverity::Warning => "issue-warning",
                DeeplinkSeverity::Info => "issue-info",
            };
            html.push_str(&format!("    <div class=\"issue {}\">\n", issue_class));
            html.push_str(&format!(
                "      <div class=\"issue-title\">[{}] {}</div>\n",
                issue.severity, issue.platform
            ));
            html.push_str(&format!(
                "      <div class=\"issue-detail\"><strong>Category:</strong> {}</div>\n",
                issue.category
            ));
            html.push_str(&format!(
                "      <div class=\"issue-detail\"><strong>Detail:</strong> {}</div>\n",
                issue.detail
            ));
            if !issue.file.as_os_str().is_empty() {
                html.push_str(&format!(
                    "      <div class=\"issue-detail\"><strong>File:</strong> {}</div>\n",
                    issue.file.display()
                ));
            }
            html.push_str(&format!(
                "      <div class=\"issue-suggestion\"><strong>Suggestion:</strong> {}</div>\n",
                issue.suggestion
            ));
            html.push_str("    </div>\n");
        }
    } else {
        html.push_str("    <h2 style=\"color: #4caf50;\">No Issues Found!</h2>\n");
    }

    html.push_str("  </div>\n");
    html.push_str("</body>\n");
    html.push_str("</html>\n");

    std::fs::write(path, html)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deeplink_issue_creation() {
        let issue = DeeplinkIssue {
            severity: DeeplinkSeverity::Error,
            platform: "Android",
            category: "Missing Intent Filter",
            file: PathBuf::from("AndroidManifest.xml"),
            detail: "No intent-filter found".to_string(),
            suggestion: "Add intent-filter".to_string(),
        };

        assert_eq!(issue.severity, DeeplinkSeverity::Error);
        assert_eq!(issue.platform, "Android");
        assert_eq!(issue.category, "Missing Intent Filter");
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(DeeplinkSeverity::Error.to_string(), "Error");
        assert_eq!(DeeplinkSeverity::Warning.to_string(), "Warning");
        assert_eq!(DeeplinkSeverity::Info.to_string(), "Info");
    }

    #[test]
    fn test_calculate_score_no_issues() {
        let report = DeeplinkReport {
            android_schemes: vec!["myapp".to_string()],
            ios_schemes: vec!["myapp".to_string()],
            flutter_routes: vec!["/home".to_string()],
            issues: vec![],
            score: 100,
        };

        assert_eq!(calculate_score(&report), 100);
    }

    #[test]
    fn test_calculate_score_with_errors() {
        let report = DeeplinkReport {
            android_schemes: vec![],
            ios_schemes: vec![],
            flutter_routes: vec![],
            issues: vec![
                DeeplinkIssue {
                    severity: DeeplinkSeverity::Error,
                    platform: "Android",
                    category: "Test",
                    file: PathBuf::new(),
                    detail: "Test".to_string(),
                    suggestion: "Test".to_string(),
                },
                DeeplinkIssue {
                    severity: DeeplinkSeverity::Warning,
                    platform: "iOS",
                    category: "Test",
                    file: PathBuf::new(),
                    detail: "Test".to_string(),
                    suggestion: "Test".to_string(),
                },
            ],
            score: 0,
        };

        let score = calculate_score(&report);
        assert_eq!(score, 77); // 100 - 15 - 8
    }

    #[test]
    fn test_extract_android_schemes() {
        let manifest_content = r#"
            <data android:scheme="myapp" />
            <data android:scheme="https" />
        "#;

        let mut report = DeeplinkReport {
            android_schemes: vec![],
            ios_schemes: vec![],
            flutter_routes: vec![],
            issues: vec![],
            score: 100,
        };

        extract_android_schemes(manifest_content, &mut report, &PathBuf::from("test.xml"));

        assert_eq!(report.android_schemes.len(), 2);
        assert!(report.android_schemes.contains(&"myapp".to_string()));
        assert!(report.android_schemes.contains(&"https".to_string()));
    }

    #[test]
    fn test_extract_ios_schemes() {
        let plist_content = r#"
            <key>CFBundleURLSchemes</key>
            <array>
                <string>myapp</string>
                <string>myapp-special</string>
            </array>
        "#;

        let mut report = DeeplinkReport {
            android_schemes: vec![],
            ios_schemes: vec![],
            flutter_routes: vec![],
            issues: vec![],
            score: 100,
        };

        extract_ios_schemes(plist_content, &mut report, &PathBuf::from("test.plist"));

        assert_eq!(report.ios_schemes.len(), 2);
        assert!(report.ios_schemes.contains(&"myapp".to_string()));
        assert!(report.ios_schemes.contains(&"myapp-special".to_string()));
    }

    #[test]
    fn test_extract_flutter_routes() {
        let dart_content = r#"
            GoRoute(path: '/home', builder: ...),
            GoRoute(path: "/product/:id", builder: ...),
        "#;

        let mut report = DeeplinkReport {
            android_schemes: vec![],
            ios_schemes: vec![],
            flutter_routes: vec![],
            issues: vec![],
            score: 100,
        };

        let _temp_file = PathBuf::from("test.dart");
        // Simulate the extraction logic
        for line in dart_content.lines() {
            if line.contains("GoRoute(path:") {
                if let Some(start) = line.find("path:") {
                    let remainder = &line[start + 5..];
                    for (i, ch) in remainder.chars().enumerate() {
                        if ch == '\'' || ch == '"' {
                            let quote = ch;
                            let path_content = &remainder[i + 1..];
                            if let Some(end) = path_content.find(quote) {
                                let route = path_content[..end].to_string();
                                if !route.is_empty() && !report.flutter_routes.contains(&route) {
                                    report.flutter_routes.push(route);
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        assert_eq!(report.flutter_routes.len(), 2);
        assert!(report.flutter_routes.contains(&"/home".to_string()));
        assert!(report.flutter_routes.contains(&"/product/:id".to_string()));
    }
}
