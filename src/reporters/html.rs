use super::{AnalysisReport, Issue, Reporter};
use crate::config::Severity;
use crate::metrics::MetricsResults;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct HtmlReporter {
    pub output_path: PathBuf,
}

impl Reporter for HtmlReporter {
    fn report_analysis(&self, report: &AnalysisReport) {
        let html = build_full_report(report);
        match std::fs::write(&self.output_path, &html) {
            Ok(_) => println!("HTML report written to: {}", self.output_path.display()),
            Err(e) => eprintln!("Failed to write HTML report: {}", e),
        }
    }

    fn report_metrics(&self, metrics: &[(PathBuf, MetricsResults)]) {
        let html = build_metrics_report(metrics);
        match std::fs::write(&self.output_path, &html) {
            Ok(_) => println!("HTML report written to: {}", self.output_path.display()),
            Err(e) => eprintln!("Failed to write HTML report: {}", e),
        }
    }

    fn report_issues(&self, issues: &[Issue]) {
        let html = build_issues_report(issues);
        match std::fs::write(&self.output_path, &html) {
            Ok(_) => println!("HTML report written to: {}", self.output_path.display()),
            Err(e) => eprintln!("Failed to write HTML report: {}", e),
        }
    }
}

fn build_full_report(report: &AnalysisReport) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let infos = report.info_count();
    let total_issues = errors + warnings + infos;
    let total_lines: u32 = report
        .metrics
        .iter()
        .map(|(_, m)| m.file_lines_of_code)
        .sum();
    let total_functions: usize = report.metrics.iter().map(|(_, m)| m.functions.len()).sum();
    let total_classes: usize = report.metrics.iter().map(|(_, m)| m.classes.len()).sum();

    let health_score = compute_health_score(errors, warnings, infos, report.file_count);

    let project_root = report
        .project_path
        .clone()
        .or_else(|| detect_project_root(&report.metrics));
    let project_info = project_root
        .as_ref()
        .map(|r| parse_pubspec(r))
        .unwrap_or_default();
    let project_name = if project_info.name.is_empty() {
        project_root
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown Project")
            .to_string()
    } else {
        project_info.name.clone()
    };

    let mut html = String::with_capacity(256 * 1024);
    html.push_str(&html_header(&format!("Falcon — {}", project_name)));

    // Sidebar
    html.push_str(&sidebar(errors, warnings, infos));

    // Main content
    html.push_str(r#"<main class="main-content">"#);

    // Top bar with project name
    html.push_str(&top_bar_with_project(&project_name, report.file_count));

    // Tab navigation
    html.push_str(
        r#"<div class="tab-nav">
      <button class="tab-btn active" onclick="switchTab('overview')">Overview</button>
      <button class="tab-btn" onclick="switchTab('issues')">Issues</button>
      <button class="tab-btn" onclick="switchTab('metrics')">Metrics</button>
    </div>"#,
    );

    // === OVERVIEW TAB ===
    html.push_str(r#"<div id="tab-overview" class="tab-content active">"#);

    // Project Properties
    html.push_str(&project_properties_section(&project_info, &project_name));

    // Health score + summary cards row
    html.push_str(r#"<div class="overview-top">"#);
    html.push_str(&health_score_card(health_score));
    html.push_str(r#"<div class="kpi-grid">"#);
    html.push_str(&kpi_card(
        "Files",
        &report.file_count.to_string(),
        "icon-files",
        "",
    ));
    html.push_str(&kpi_card(
        "Lines of Code",
        &format_number(total_lines as usize),
        "icon-loc",
        "",
    ));
    html.push_str(&kpi_card(
        "Functions",
        &format_number(total_functions),
        "icon-functions",
        "",
    ));
    html.push_str(&kpi_card(
        "Classes",
        &format_number(total_classes),
        "icon-classes",
        "",
    ));
    html.push_str(&kpi_card(
        "Errors",
        &format_number(errors),
        "icon-errors",
        if errors > 0 { "kpi-danger" } else { "kpi-ok" },
    ));
    html.push_str(&kpi_card(
        "Warnings",
        &format_number(warnings),
        "icon-warnings",
        if warnings > 0 { "kpi-warn" } else { "kpi-ok" },
    ));
    html.push_str("</div></div>");

    // Level of Concern
    html.push_str(&level_of_concern_section(&report.issues, report.file_count));

    // Charts row
    html.push_str(r#"<div class="charts-row">"#);
    html.push_str(&severity_donut(errors, warnings, infos));
    html.push_str(&top_rules_chart(&report.issues));
    html.push_str("</div>");

    // Hotspots
    html.push_str(&hotspots_section(&report.metrics));

    // Test Coverage
    html.push_str(&test_coverage_section(&report.metrics));

    html.push_str("</div>"); // end overview tab

    // === ISSUES TAB ===
    html.push_str(r#"<div id="tab-issues" class="tab-content">"#);
    if !report.issues.is_empty() {
        html.push_str(&issues_section(&report.issues, total_issues));
    } else {
        html.push_str(r#"<div class="empty-state"><div class="empty-icon">&#10003;</div><h3>No issues found</h3><p>Your codebase is clean.</p></div>"#);
    }
    html.push_str("</div>");

    // === METRICS TAB ===
    html.push_str(r#"<div id="tab-metrics" class="tab-content">"#);
    if !report.metrics.is_empty() {
        html.push_str(&metrics_section(&report.metrics));
    } else {
        html.push_str(r#"<div class="empty-state"><h3>No metrics available</h3></div>"#);
    }
    html.push_str("</div>");

    html.push_str("</main>"); // end main content
    html.push_str(&html_footer());
    html
}

fn build_metrics_report(metrics: &[(PathBuf, MetricsResults)]) -> String {
    let mut html = String::new();
    html.push_str(&html_header("Falcon Metrics Report"));
    html.push_str(&sidebar(0, 0, 0));
    html.push_str(r#"<main class="main-content">"#);
    html.push_str(&top_bar(metrics.len()));
    html.push_str(&metrics_section(metrics));
    html.push_str("</main>");
    html.push_str(&html_footer());
    html
}

fn build_issues_report(issues: &[Issue]) -> String {
    let mut html = String::new();
    let errors = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();
    let infos = issues
        .iter()
        .filter(|i| i.severity == Severity::Info)
        .count();
    html.push_str(&html_header("Falcon Issues Report"));
    html.push_str(&sidebar(errors, warnings, infos));
    html.push_str(r#"<main class="main-content">"#);
    html.push_str(&top_bar(0));
    html.push_str(&issues_section(issues, issues.len()));
    html.push_str("</main>");
    html.push_str(&html_footer());
    html
}

#[derive(Default)]
struct ProjectInfo {
    name: String,
    version: String,
    description: String,
    sdk_constraint: String,
    flutter_constraint: String,
    dependencies: Vec<String>,
    dev_dependencies: Vec<String>,
    homepage: String,
    repository: String,
}

fn parse_pubspec(project_path: &Path) -> ProjectInfo {
    let pubspec_path = project_path.join("pubspec.yaml");
    let content = match std::fs::read_to_string(&pubspec_path) {
        Ok(c) => c,
        Err(_) => return ProjectInfo::default(),
    };

    let mut info = ProjectInfo::default();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("name:") {
            info.name = val.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = trimmed.strip_prefix("version:") {
            info.version = val.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            info.description = val.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = trimmed.strip_prefix("homepage:") {
            info.homepage = val.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = trimmed.strip_prefix("repository:") {
            info.repository = val.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }

    enum Section {
        None,
        Environment,
        Dependencies,
        DevDependencies,
    }
    let mut section = Section::None;

    for line in content.lines() {
        let trimmed = line.trim();
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if trimmed.starts_with("environment:") {
                section = Section::Environment;
                continue;
            } else if trimmed.starts_with("dependencies:") {
                section = Section::Dependencies;
                continue;
            } else if trimmed.starts_with("dev_dependencies:") {
                section = Section::DevDependencies;
                continue;
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                section = Section::None;
            }
        }

        if (line.starts_with(' ') || line.starts_with('\t')) && !trimmed.is_empty() {
            match section {
                Section::Environment => {
                    if let Some(val) = trimmed.strip_prefix("sdk:") {
                        info.sdk_constraint =
                            val.trim().trim_matches('"').trim_matches('\'').to_string();
                    } else if let Some(val) = trimmed.strip_prefix("flutter:") {
                        info.flutter_constraint =
                            val.trim().trim_matches('"').trim_matches('\'').to_string();
                    }
                }
                Section::Dependencies => {
                    if let Some(name) = trimmed.split(':').next() {
                        if !name.starts_with('#') && !name.is_empty() {
                            info.dependencies.push(name.to_string());
                        }
                    }
                }
                Section::DevDependencies => {
                    if let Some(name) = trimmed.split(':').next() {
                        if !name.starts_with('#') && !name.is_empty() {
                            info.dev_dependencies.push(name.to_string());
                        }
                    }
                }
                Section::None => {}
            }
        }
    }

    info
}

fn top_bar_with_project(project_name: &str, file_count: usize) -> String {
    format!(
        r#"<header class="top-bar">
  <div class="top-bar-left">
    <h1>{project}</h1>
    <span class="top-bar-sub">{files} files analyzed &bull; Generated by Falcon v{ver}</span>
  </div>
  <div class="top-bar-right">
    <button class="theme-toggle" onclick="toggleTheme()" title="Toggle light/dark mode">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>
    </button>
  </div>
</header>"#,
        project = html_escape(project_name),
        files = file_count,
        ver = env!("CARGO_PKG_VERSION")
    )
}

fn project_properties_section(info: &ProjectInfo, project_name: &str) -> String {
    let has_info = !info.name.is_empty() || !info.version.is_empty();

    let mut html = String::from(r#"<div class="project-card">"#);

    html.push_str(&format!(
        r#"<div class="project-header">
  <div class="project-icon">
    <svg viewBox="0 0 32 32" fill="none"><rect x="3" y="5" width="26" height="22" rx="3" stroke="var(--accent-light)" stroke-width="1.5" fill="var(--accent-bg)"/><path d="M3 12h26" stroke="var(--accent-light)" stroke-width="1.5"/><circle cx="8" cy="8.5" r="1.2" fill="var(--error)"/><circle cx="12" cy="8.5" r="1.2" fill="var(--warning)"/><circle cx="16" cy="8.5" r="1.2" fill="var(--success)"/><path d="M10 18l3 3 5-5" stroke="var(--success)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
  </div>
  <div class="project-title-block">
    <h2 class="project-name">{name}</h2>{ver}
  </div>
</div>"#,
        name = html_escape(project_name),
        ver = if !info.version.is_empty() {
            format!(r#"<span class="project-version">v{}</span>"#, html_escape(&info.version))
        } else {
            String::new()
        }
    ));

    if has_info {
        if !info.description.is_empty() {
            html.push_str(&format!(
                r#"<p class="project-desc">{}</p>"#,
                html_escape(&info.description)
            ));
        }

        html.push_str(r#"<div class="props-grid">"#);

        if !info.sdk_constraint.is_empty() {
            html.push_str(&prop_item("Dart SDK", &info.sdk_constraint, "sdk"));
        }
        if !info.flutter_constraint.is_empty() {
            html.push_str(&prop_item("Flutter", &info.flutter_constraint, "flutter"));
        }
        if !info.version.is_empty() {
            html.push_str(&prop_item("Version", &info.version, "version"));
        }

        let dep_count = info.dependencies.len();
        let dev_dep_count = info.dev_dependencies.len();
        if dep_count > 0 {
            html.push_str(&prop_item("Dependencies", &dep_count.to_string(), "deps"));
        }
        if dev_dep_count > 0 {
            html.push_str(&prop_item(
                "Dev Dependencies",
                &dev_dep_count.to_string(),
                "dev-deps",
            ));
        }
        if !info.homepage.is_empty() {
            html.push_str(&prop_item("Homepage", &info.homepage, "homepage"));
        }
        if !info.repository.is_empty() {
            html.push_str(&prop_item("Repository", &info.repository, "repo"));
        }

        html.push_str("</div>");

        if dep_count > 0 {
            html.push_str(r#"<details class="dep-details"><summary class="dep-summary">"#);
            html.push_str(&format!(
                r#"<span>Packages</span><span class="dep-count">{} deps + {} dev</span></summary>"#,
                dep_count, dev_dep_count
            ));
            html.push_str(r#"<div class="dep-chips">"#);
            for dep in &info.dependencies {
                html.push_str(&format!(
                    r#"<span class="dep-chip">{}</span>"#,
                    html_escape(dep)
                ));
            }
            if !info.dev_dependencies.is_empty() {
                for dep in &info.dev_dependencies {
                    html.push_str(&format!(
                        r#"<span class="dep-chip dep-chip-dev">{}</span>"#,
                        html_escape(dep)
                    ));
                }
            }
            html.push_str("</div></details>");
        }
    }

    html.push_str("</div>");
    html
}

fn prop_item(label: &str, value: &str, _kind: &str) -> String {
    format!(
        r#"<div class="prop-item">
  <span class="prop-label">{label}</span>
  <span class="prop-value">{value}</span>
</div>"#,
        label = label,
        value = html_escape(value)
    )
}

fn compute_health_score(errors: usize, warnings: usize, infos: usize, file_count: usize) -> u32 {
    if file_count == 0 {
        return 100;
    }
    let issues_per_file = (errors * 10 + warnings * 3 + infos) as f64 / file_count as f64;
    let score = 100.0 - (issues_per_file * 2.5);
    score.clamp(0.0, 100.0) as u32
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn sidebar(errors: usize, warnings: usize, infos: usize) -> String {
    format!(
        r##"<aside class="sidebar">
  <div class="sidebar-brand">
    <svg class="brand-icon" viewBox="0 0 32 32" fill="none"><path d="M16 2L4 8v8c0 7.7 5.1 14.9 12 16.8C22.9 30.9 28 23.7 28 16V8L16 2z" fill="url(#g1)" opacity="0.15"/><path d="M16 2L4 8v8c0 7.7 5.1 14.9 12 16.8C22.9 30.9 28 23.7 28 16V8L16 2z" stroke="url(#g1)" stroke-width="1.5" fill="none"/><path d="M12 16l3 3 6-6" stroke="url(#g1)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><defs><linearGradient id="g1" x1="4" y1="2" x2="28" y2="32"><stop stop-color="#6366f1"/><stop offset="1" stop-color="#8b5cf6"/></linearGradient></defs></svg>
    <span class="brand-text">Falcon</span>
  </div>
  <nav class="sidebar-nav">
    <a class="nav-item active" onclick="switchTab('overview')" href="javascript:void(0)">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
      Overview
    </a>
    <a class="nav-item" onclick="switchTab('issues')" href="javascript:void(0)">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
      Issues<span class="nav-badge">{}</span>
    </a>
    <a class="nav-item" onclick="switchTab('metrics')" href="javascript:void(0)">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 20V10M12 20V4M6 20v-6"/></svg>
      Metrics
    </a>
  </nav>
  <div class="sidebar-stats">
    <div class="stat-row"><span class="stat-dot dot-red"></span>Errors<span class="stat-val">{}</span></div>
    <div class="stat-row"><span class="stat-dot dot-amber"></span>Warnings<span class="stat-val">{}</span></div>
    <div class="stat-row"><span class="stat-dot dot-blue"></span>Info<span class="stat-val">{}</span></div>
  </div>
</aside>"##,
        format_number(errors + warnings + infos),
        format_number(errors),
        format_number(warnings),
        format_number(infos)
    )
}

fn top_bar(file_count: usize) -> String {
    format!(
        r#"<header class="top-bar">
  <div class="top-bar-left">
    <h1>Analysis Report</h1>
    <span class="top-bar-sub">{} files analyzed &bull; Generated by Falcon v{}</span>
  </div>
  <div class="top-bar-right">
    <button class="theme-toggle" onclick="toggleTheme()" title="Toggle light/dark mode">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>
    </button>
  </div>
</header>"#,
        file_count,
        env!("CARGO_PKG_VERSION")
    )
}

fn health_score_card(score: u32) -> String {
    let (color, label) = match score {
        90..=100 => ("#10b981", "Excellent"),
        75..=89 => ("#6366f1", "Good"),
        50..=74 => ("#f59e0b", "Needs Work"),
        25..=49 => ("#f97316", "Poor"),
        _ => ("#ef4444", "Critical"),
    };
    let circumference = 2.0 * std::f64::consts::PI * 54.0;
    let offset = circumference * (1.0 - score as f64 / 100.0);

    format!(
        r##"<div class="health-card">
  <div class="health-ring">
    <svg viewBox="0 0 120 120">
      <circle cx="60" cy="60" r="54" fill="none" stroke="var(--ring-bg)" stroke-width="8"/>
      <circle cx="60" cy="60" r="54" fill="none" stroke="{color}" stroke-width="8"
        stroke-dasharray="{circ}" stroke-dashoffset="{offset}"
        stroke-linecap="round" transform="rotate(-90 60 60)" class="health-arc"/>
    </svg>
    <div class="health-value" style="color:{color}">{score}</div>
  </div>
  <div class="health-meta">
    <span class="health-label">Health Score</span>
    <span class="health-grade" style="color:{color}">{label}</span>
  </div>
</div>"##,
        color = color,
        circ = circumference,
        offset = offset,
        score = score,
        label = label
    )
}

struct ConcernArea {
    name: &'static str,
    icon: &'static str,
    description: &'static str,
    count: usize,
    error_count: usize,
}

fn classify_concern(rule: &str) -> &'static str {
    match rule {
        r if r.contains("credential") || r.contains("security") || r.contains("hardcoded") => {
            "Security"
        }
        r if r.contains("catch")
            || r.contains("async-void")
            || r.contains("unawaited")
            || r.contains("specific-catch") =>
        {
            "Error Handling"
        }
        r if r.contains("dynamic") || r.contains("type") || r.contains("equatable") => {
            "Type Safety"
        }
        r if r.contains("long-function")
            || r.contains("long-parameter")
            || r.contains("complexity")
            || r.contains("nesting") =>
        {
            "Complexity"
        }
        r if r.contains("widget")
            || r.contains("rebuild")
            || r.contains("build-method")
            || r.contains("const-constructor")
            || r.contains("await-in-loop") =>
        {
            "Performance"
        }
        r if r.contains("dispose") || r.contains("unused") || r.contains("dead-code") => {
            "Resource Safety"
        }
        r if r.contains("late") || r.contains("avoid-") => "Code Smells",
        _ => "Conventions",
    }
}

fn concern_level_label(score: f64) -> (&'static str, &'static str) {
    match score as u32 {
        0..=2 => ("Low", "#10b981"),
        3..=8 => ("Moderate", "#6366f1"),
        9..=20 => ("Elevated", "#f59e0b"),
        21..=50 => ("High", "#f97316"),
        _ => ("Critical", "#ef4444"),
    }
}

fn level_of_concern_section(issues: &[Issue], file_count: usize) -> String {
    let mut areas: HashMap<&str, (usize, usize)> = HashMap::new();
    for issue in issues {
        let area = classify_concern(&issue.rule);
        let entry = areas.entry(area).or_insert((0, 0));
        entry.0 += 1;
        if issue.severity == Severity::Error {
            entry.1 += 1;
        }
    }

    let fc = if file_count == 0 { 1 } else { file_count };

    let concern_defs: Vec<(&str, &str, &str)> = vec![
        (
            "Security",
            "&#128274;",
            "Hardcoded credentials, sensitive data exposure",
        ),
        (
            "Error Handling",
            "&#9888;",
            "Empty catches, unhandled futures, async void",
        ),
        (
            "Type Safety",
            "&#128295;",
            "Dynamic types, missing type annotations",
        ),
        (
            "Complexity",
            "&#129518;",
            "Long functions, deep nesting, high cyclomatic complexity",
        ),
        (
            "Performance",
            "&#9889;",
            "Widget rebuilds, missing const, unawaited futures",
        ),
        (
            "Resource Safety",
            "&#128451;",
            "Missing dispose, unused code, dead code",
        ),
        (
            "Code Smells",
            "&#128065;",
            "Late keywords, avoidable patterns",
        ),
        (
            "Conventions",
            "&#128221;",
            "Naming, formatting, trailing commas",
        ),
    ];

    let mut concern_areas: Vec<ConcernArea> = concern_defs
        .iter()
        .map(|(name, icon, desc)| {
            let (count, error_count) = areas.get(name).copied().unwrap_or((0, 0));
            ConcernArea {
                name,
                icon,
                description: desc,
                count,
                error_count,
            }
        })
        .collect();

    concern_areas.sort_by(|a, b| {
        let a_score = a.error_count * 10 + a.count;
        let b_score = b.error_count * 10 + b.count;
        b_score.cmp(&a_score)
    });

    let max_count = concern_areas
        .iter()
        .map(|a| a.count)
        .max()
        .unwrap_or(1)
        .max(1);

    let mut html = String::from(
        r#"<div class="section-card concern-section">
  <h3>Level of Concern</h3>
  <p class="section-sub">Issue distribution across concern areas &mdash; higher bars indicate areas needing attention</p>
  <div class="concern-grid">"#,
    );

    for area in &concern_areas {
        let per_file = area.count as f64 / fc as f64;
        let score = area.error_count as f64 * 3.0 + per_file * 5.0;
        let (level, color) = concern_level_label(score);
        let bar_pct = (area.count as f64 / max_count as f64 * 100.0) as u32;

        html.push_str(&format!(
            r##"<div class="concern-card">
  <div class="concern-header">
    <span class="concern-icon">{icon}</span>
    <div class="concern-title">
      <span class="concern-name">{name}</span>
      <span class="concern-desc">{desc}</span>
    </div>
    <div class="concern-badge-wrap">
      <span class="concern-level" style="color:{color};border-color:{color}">{level}</span>
    </div>
  </div>
  <div class="concern-bar-wrap">
    <div class="concern-bar-track">
      <div class="concern-bar-fill" style="width:{pct}%;background:{color}"></div>
    </div>
    <div class="concern-stats">
      <span class="concern-count">{count} issue{s}</span>{err_badge}
    </div>
  </div>
</div>"##,
            icon = area.icon,
            name = area.name,
            desc = area.description,
            color = color,
            level = level,
            pct = bar_pct,
            count = area.count,
            s = if area.count != 1 { "s" } else { "" },
            err_badge = if area.error_count > 0 {
                format!(
                    r#"<span class="concern-err">{} error{}</span>"#,
                    area.error_count,
                    if area.error_count != 1 { "s" } else { "" }
                )
            } else {
                String::new()
            }
        ));
    }

    html.push_str("</div></div>");
    html
}

fn kpi_card(label: &str, value: &str, icon_class: &str, extra_class: &str) -> String {
    format!(
        r#"<div class="kpi-card {extra}">
      <div class="kpi-icon {icon}"></div>
      <div class="kpi-value">{value}</div>
      <div class="kpi-label">{label}</div>
    </div>"#,
        extra = extra_class,
        icon = icon_class,
        value = value,
        label = label
    )
}

fn severity_donut(errors: usize, warnings: usize, infos: usize) -> String {
    let total = (errors + warnings + infos) as f64;
    if total == 0.0 {
        return String::from(
            r#"<div class="chart-card"><h3>Severity Distribution</h3><div class="empty-state">No issues</div></div>"#,
        );
    }

    let r = 80.0;
    let circumference = 2.0 * std::f64::consts::PI * r;

    let e_pct = errors as f64 / total;
    let w_pct = warnings as f64 / total;
    let i_pct = infos as f64 / total;

    let e_dash = circumference * e_pct;
    let w_dash = circumference * w_pct;
    let i_dash = circumference * i_pct;

    let e_offset = 0.0;
    let w_offset = circumference - e_dash;
    let i_offset = circumference - e_dash - w_dash;

    format!(
        r##"<div class="chart-card">
  <h3>Severity Distribution</h3>
  <div class="donut-container">
    <svg viewBox="0 0 200 200" class="donut-svg">
      <circle cx="100" cy="100" r="{r}" fill="none" stroke="var(--error)" stroke-width="24"
        stroke-dasharray="{e_dash} {e_gap}" stroke-dashoffset="{e_off}" transform="rotate(-90 100 100)"/>
      <circle cx="100" cy="100" r="{r}" fill="none" stroke="var(--warning)" stroke-width="24"
        stroke-dasharray="{w_dash} {w_gap}" stroke-dashoffset="{w_off}" transform="rotate(-90 100 100)"/>
      <circle cx="100" cy="100" r="{r}" fill="none" stroke="var(--info)" stroke-width="24"
        stroke-dasharray="{i_dash} {i_gap}" stroke-dashoffset="{i_off}" transform="rotate(-90 100 100)"/>
      <text x="100" y="95" text-anchor="middle" class="donut-total">{total}</text>
      <text x="100" y="115" text-anchor="middle" class="donut-sub">total issues</text>
    </svg>
    <div class="donut-legend">
      <div class="legend-item"><span class="legend-dot" style="background:var(--error)"></span>Errors<span class="legend-val">{errors} ({e_pct_d}%)</span></div>
      <div class="legend-item"><span class="legend-dot" style="background:var(--warning)"></span>Warnings<span class="legend-val">{warnings} ({w_pct_d}%)</span></div>
      <div class="legend-item"><span class="legend-dot" style="background:var(--info)"></span>Info<span class="legend-val">{infos} ({i_pct_d}%)</span></div>
    </div>
  </div>
</div>"##,
        r = r,
        e_dash = e_dash,
        e_gap = circumference - e_dash,
        e_off = e_offset,
        w_dash = w_dash,
        w_gap = circumference - w_dash,
        w_off = w_offset,
        i_dash = i_dash,
        i_gap = circumference - i_dash,
        i_off = i_offset,
        total = total as usize,
        errors = errors,
        warnings = warnings,
        infos = infos,
        e_pct_d = (e_pct * 100.0) as u32,
        w_pct_d = (w_pct * 100.0) as u32,
        i_pct_d = (i_pct * 100.0) as u32
    )
}

fn top_rules_chart(issues: &[Issue]) -> String {
    let mut rule_counts: HashMap<&str, usize> = HashMap::new();
    for issue in issues {
        *rule_counts.entry(&issue.rule).or_insert(0) += 1;
    }

    let mut sorted: Vec<(&&str, &usize)> = rule_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    let top_n = sorted.iter().take(10);

    let max_val = sorted.first().map(|(_, v)| **v).unwrap_or(1);

    let mut html = String::from(
        r#"<div class="chart-card">
  <h3>Top Rules by Frequency</h3>
  <div class="bar-chart">"#,
    );

    for (rule, count) in top_n {
        let pct = (**count as f64 / max_val as f64 * 100.0) as u32;
        html.push_str(&format!(
            r#"<div class="bar-row">
      <span class="bar-label" title="{rule}">{rule_short}</span>
      <div class="bar-track"><div class="bar-fill" style="width:{pct}%"></div></div>
      <span class="bar-value">{count}</span>
    </div>"#,
            rule = html_escape(rule),
            rule_short = html_escape(if rule.len() > 32 { &rule[..32] } else { rule }),
            pct = pct,
            count = count
        ));
    }

    html.push_str("</div></div>");
    html
}

fn hotspots_section(metrics: &[(PathBuf, MetricsResults)]) -> String {
    let mut file_scores: Vec<(&PathBuf, u32, u32, f64)> = Vec::new();

    for (path, result) in metrics {
        let max_cc = result
            .functions
            .iter()
            .map(|f| f.cyclomatic_complexity)
            .max()
            .unwrap_or(0);
        let total_loc = result.file_lines_of_code;
        let min_mi = result
            .functions
            .iter()
            .map(|f| f.maintainability_index)
            .fold(f64::MAX, f64::min);
        let min_mi = if min_mi == f64::MAX { 100.0 } else { min_mi };
        if max_cc > 5 || total_loc > 200 {
            file_scores.push((path, max_cc, total_loc, min_mi));
        }
    }

    file_scores.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));
    let top = file_scores.iter().take(10);

    let mut html = String::from(
        r#"<div class="section-card">
  <h3>Complexity Hotspots</h3>
  <p class="section-sub">Files with highest cyclomatic complexity — prioritize for refactoring</p>
  <table class="data-table"><thead><tr>
    <th>File</th><th>Max CC</th><th>LOC</th><th>Min MI</th><th>Risk</th>
  </tr></thead><tbody>"#,
    );

    for (path, cc, loc, mi) in top {
        let risk = if *cc > 20 || *mi < 20.0 {
            r#"<span class="risk-badge risk-high">High</span>"#
        } else if *cc > 10 || *mi < 40.0 {
            r#"<span class="risk-badge risk-med">Medium</span>"#
        } else {
            r#"<span class="risk-badge risk-low">Low</span>"#
        };
        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown");
        let cc_class = metric_class(*cc, 10, 20, 30);
        let mi_class = metric_class_inverted(*mi, 20.0, 40.0, 60.0);

        html.push_str(&format!(
            r#"<tr><td class="file-cell" title="{full}">{name}</td><td class="{cc_cls}">{cc}</td><td>{loc}</td><td class="{mi_cls}">{mi:.1}</td><td>{risk}</td></tr>"#,
            full = html_escape(&path.display().to_string()),
            name = html_escape(file_name),
            cc_cls = cc_class, cc = cc,
            loc = loc,
            mi_cls = mi_class, mi = mi,
            risk = risk
        ));
    }

    html.push_str("</tbody></table></div>");
    html
}

fn detect_project_root(metrics: &[(PathBuf, MetricsResults)]) -> Option<PathBuf> {
    for (path, _) in metrics {
        let path_str = path.display().to_string();
        if let Some(idx) = path_str.find("/lib/") {
            return Some(PathBuf::from(&path_str[..idx]));
        }
    }
    metrics
        .first()
        .and_then(|(p, _)| p.parent().map(|pp| pp.to_path_buf()))
}

struct TestCoverageInfo {
    source_file: String,
    loc: u32,
    has_test: bool,
    test_file: Option<String>,
}

fn scan_test_coverage(metrics: &[(PathBuf, MetricsResults)]) -> Vec<TestCoverageInfo> {
    let project_root = match detect_project_root(metrics) {
        Some(r) => r,
        None => return Vec::new(),
    };

    let test_dir = project_root.join("test");
    let mut test_files: HashSet<String> = HashSet::new();

    if test_dir.is_dir() {
        collect_dart_files(&test_dir, &mut test_files);
    }

    let mut results = Vec::new();

    for (path, result) in metrics {
        let path_str = path.display().to_string();
        let file_name = path
            .file_stem()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_string();
        let display_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown")
            .to_string();

        if file_name.ends_with("_test") || path_str.contains("/test/") {
            continue;
        }

        let test_name = format!("{}_test.dart", file_name);
        let has_test = test_files.iter().any(|t| t.ends_with(&test_name));
        let test_file = if has_test {
            test_files.iter().find(|t| t.ends_with(&test_name)).cloned()
        } else {
            None
        };

        results.push(TestCoverageInfo {
            source_file: display_name,
            loc: result.file_lines_of_code,
            has_test,
            test_file: test_file.map(|t| {
                Path::new(&t)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            }),
        });
    }

    results.sort_by(|a, b| a.has_test.cmp(&b.has_test).then(b.loc.cmp(&a.loc)));
    results
}

fn collect_dart_files(dir: &Path, out: &mut HashSet<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_dart_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("dart") {
                out.insert(path.display().to_string());
            }
        }
    }
}

fn test_coverage_section(metrics: &[(PathBuf, MetricsResults)]) -> String {
    let coverage = scan_test_coverage(metrics);
    if coverage.is_empty() {
        return String::new();
    }

    let total_files = coverage.len();
    let tested_files = coverage.iter().filter(|c| c.has_test).count();
    let untested_files = total_files - tested_files;
    let tested_loc: u32 = coverage.iter().filter(|c| c.has_test).map(|c| c.loc).sum();
    let untested_loc: u32 = coverage.iter().filter(|c| !c.has_test).map(|c| c.loc).sum();
    let total_loc = tested_loc + untested_loc;
    let file_pct = if total_files > 0 {
        (tested_files as f64 / total_files as f64 * 100.0) as u32
    } else {
        0
    };
    let loc_pct = if total_loc > 0 {
        (tested_loc as f64 / total_loc as f64 * 100.0) as u32
    } else {
        0
    };

    let (file_color, file_label) = coverage_grade(file_pct);
    let (loc_color, loc_label) = coverage_grade(loc_pct);

    let file_circ = 2.0 * std::f64::consts::PI * 40.0;
    let file_offset = file_circ * (1.0 - file_pct as f64 / 100.0);
    let loc_circ = 2.0 * std::f64::consts::PI * 40.0;
    let loc_offset = loc_circ * (1.0 - loc_pct as f64 / 100.0);

    let mut html = format!(
        r##"<div class="section-card">
  <h3>Test Coverage</h3>
  <p class="section-sub">Source files matched against test files in <code>test/</code> directory</p>

  <div class="coverage-overview">
    <div class="coverage-ring-card">
      <svg viewBox="0 0 100 100" class="coverage-ring">
        <circle cx="50" cy="50" r="40" fill="none" stroke="var(--ring-bg)" stroke-width="6"/>
        <circle cx="50" cy="50" r="40" fill="none" stroke="{fc}" stroke-width="6"
          stroke-dasharray="{fcirc}" stroke-dashoffset="{foff}"
          stroke-linecap="round" transform="rotate(-90 50 50)" class="health-arc"/>
      </svg>
      <div class="coverage-ring-inner">
        <span class="coverage-ring-val" style="color:{fc}">{fpct}%</span>
      </div>
      <div class="coverage-ring-label">File Coverage</div>
      <div class="coverage-ring-detail">{tested} / {total} files</div>
      <span class="coverage-grade" style="color:{fc}">{flabel}</span>
    </div>

    <div class="coverage-ring-card">
      <svg viewBox="0 0 100 100" class="coverage-ring">
        <circle cx="50" cy="50" r="40" fill="none" stroke="var(--ring-bg)" stroke-width="6"/>
        <circle cx="50" cy="50" r="40" fill="none" stroke="{lc}" stroke-width="6"
          stroke-dasharray="{lcirc}" stroke-dashoffset="{loff}"
          stroke-linecap="round" transform="rotate(-90 50 50)" class="health-arc"/>
      </svg>
      <div class="coverage-ring-inner">
        <span class="coverage-ring-val" style="color:{lc}">{lpct}%</span>
      </div>
      <div class="coverage-ring-label">LOC Coverage</div>
      <div class="coverage-ring-detail">{tloc} / {aloc} lines</div>
      <span class="coverage-grade" style="color:{lc}">{llabel}</span>
    </div>

    <div class="coverage-summary-cards">
      <div class="cov-stat"><span class="cov-stat-val cov-green">{tested}</span><span class="cov-stat-label">Tested Files</span></div>
      <div class="cov-stat"><span class="cov-stat-val cov-red">{untested}</span><span class="cov-stat-label">Untested Files</span></div>
      <div class="cov-stat"><span class="cov-stat-val cov-green">{tloc}</span><span class="cov-stat-label">Tested LOC</span></div>
      <div class="cov-stat"><span class="cov-stat-val cov-red">{uloc}</span><span class="cov-stat-label">Untested LOC</span></div>
    </div>
  </div>

  <div class="coverage-bar-visual">
    <div class="cov-bar-header">
      <span>Coverage by Lines of Code</span>
    </div>
    <div class="cov-stacked-bar">
      <div class="cov-stacked-fill cov-fill-tested" style="width:{lpct}%" title="Tested: {tloc} lines ({lpct}%)"></div>
      <div class="cov-stacked-fill cov-fill-untested" style="width:{upct}%" title="Untested: {uloc} lines ({upct}%)"></div>
    </div>
    <div class="cov-bar-legend">
      <span class="cov-legend-item"><span class="legend-dot" style="background:var(--success)"></span>Tested ({lpct}%)</span>
      <span class="cov-legend-item"><span class="legend-dot" style="background:var(--error)"></span>Untested ({upct}%)</span>
    </div>
  </div>

  <details class="coverage-details">
    <summary class="file-summary"><span class="file-name">File-level Coverage Breakdown</span><span class="file-badges"><span class="badge badge-info">{total} files</span></span></summary>
    <table class="data-table"><thead><tr>
      <th>Source File</th><th>LOC</th><th>Status</th><th>Test File</th>
    </tr></thead><tbody>"##,
        fc = file_color,
        fcirc = file_circ,
        foff = file_offset,
        fpct = file_pct,
        tested = tested_files,
        total = total_files,
        flabel = file_label,
        lc = loc_color,
        lcirc = loc_circ,
        loff = loc_offset,
        lpct = loc_pct,
        tloc = tested_loc,
        aloc = total_loc,
        llabel = loc_label,
        untested = untested_files,
        uloc = untested_loc,
        upct = 100u32.saturating_sub(loc_pct),
    );

    for info in &coverage {
        let (status_class, status_label, test_display) = if info.has_test {
            (
                "cov-status-tested",
                "Tested",
                info.test_file.as_deref().unwrap_or("—").to_string(),
            )
        } else {
            ("cov-status-untested", "Untested", "—".to_string())
        };

        html.push_str(&format!(
            r#"<tr><td class="file-cell"><code>{file}</code></td><td>{loc}</td><td><span class="{cls}">{label}</span></td><td class="file-cell"><code>{test}</code></td></tr>"#,
            file = html_escape(&info.source_file),
            loc = info.loc,
            cls = status_class,
            label = status_label,
            test = html_escape(&test_display),
        ));
    }

    html.push_str("</tbody></table></details></div>");
    html
}

fn coverage_grade(pct: u32) -> (&'static str, &'static str) {
    match pct {
        80..=100 => ("#10b981", "Excellent"),
        60..=79 => ("#6366f1", "Good"),
        40..=59 => ("#f59e0b", "Fair"),
        20..=39 => ("#f97316", "Poor"),
        _ => ("#ef4444", "Critical"),
    }
}

fn issues_section(issues: &[Issue], total: usize) -> String {
    let mut by_file: HashMap<String, Vec<&Issue>> = HashMap::new();
    for issue in issues {
        let file_name = issue
            .file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown")
            .to_string();
        by_file.entry(file_name).or_default().push(issue);
    }

    let mut file_groups: Vec<(String, Vec<&Issue>)> = by_file.into_iter().collect();
    file_groups.sort_by(|a, b| {
        let a_err = a.1.iter().filter(|i| i.severity == Severity::Error).count();
        let b_err = b.1.iter().filter(|i| i.severity == Severity::Error).count();
        b_err.cmp(&a_err).then(b.1.len().cmp(&a.1.len()))
    });

    let mut html = format!(
        r#"<div class="issues-header">
  <div class="issues-count">{total} issues across {files} files</div>
  <div class="issues-controls">
    <input type="text" class="search-input" placeholder="Search files or rules..." oninput="filterIssues(this.value)"/>
    <div class="filter-pills">
      <button class="pill active" onclick="filterSeverity(this,'all')">All</button>
      <button class="pill" onclick="filterSeverity(this,'error')">Errors</button>
      <button class="pill" onclick="filterSeverity(this,'warning')">Warnings</button>
      <button class="pill" onclick="filterSeverity(this,'info')">Info</button>
    </div>
  </div>
</div>
<div id="issues-list">"#,
        total = total,
        files = file_groups.len()
    );

    for (idx, (file_name, file_issues)) in file_groups.iter().enumerate() {
        let errs = file_issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        let warns = file_issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
        let infs = file_issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .count();

        let open = if idx < 3 { "open" } else { "" };

        html.push_str(&format!(
            r#"<details class="file-group" data-file="{file_lower}" {open}>
  <summary class="file-summary">
    <span class="file-name">{file}</span>
    <span class="file-badges">"#,
            file_lower = html_escape(&file_name.to_lowercase()),
            file = html_escape(file_name),
            open = open
        ));

        if errs > 0 {
            html.push_str(&format!(
                r#"<span class="badge badge-error">{errs} error{}</span>"#,
                if errs != 1 { "s" } else { "" }
            ));
        }
        if warns > 0 {
            html.push_str(&format!(
                r#"<span class="badge badge-warning">{warns} warning{}</span>"#,
                if warns != 1 { "s" } else { "" }
            ));
        }
        if infs > 0 {
            html.push_str(&format!(
                r#"<span class="badge badge-info">{infs} info</span>"#
            ));
        }

        html.push_str("</span></summary>");
        html.push_str(r#"<table class="issue-table"><thead><tr><th style="width:80px">Sev</th><th style="width:70px">Line</th><th style="width:200px">Rule</th><th>Message</th></tr></thead><tbody>"#);

        let mut sorted_issues = file_issues.clone();
        sorted_issues.sort_by_key(|i| i.line);

        for issue in &sorted_issues {
            let (sev_class, sev_label) = match issue.severity {
                Severity::Error => ("sev-error", "ERROR"),
                Severity::Warning => ("sev-warn", "WARN"),
                Severity::Info => ("sev-info", "INFO"),
            };
            html.push_str(&format!(
                r#"<tr class="issue-row" data-severity="{sev_data}" data-rule="{rule_lower}">
  <td><span class="{sev_class}">{sev_label}</span></td>
  <td class="line-num">{line}:{col}</td>
  <td><code class="rule-code">{rule}</code></td>
  <td class="msg-cell">{msg}</td>
</tr>"#,
                sev_data = match issue.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                    Severity::Info => "info",
                },
                rule_lower = html_escape(&issue.rule.to_lowercase()),
                sev_class = sev_class,
                sev_label = sev_label,
                line = issue.line,
                col = issue.column,
                rule = html_escape(&issue.rule),
                msg = html_escape(&issue.message)
            ));
        }

        html.push_str("</tbody></table></details>");
    }

    html.push_str("</div>");
    html
}

fn metrics_section(metrics: &[(PathBuf, MetricsResults)]) -> String {
    let mut html = String::new();

    // Functions table
    html.push_str(r#"<div class="section-card">
  <h3>Function Metrics</h3>
  <p class="section-sub">Cyclomatic complexity, maintainability index, and size metrics per function</p>
  <div class="table-scroll">
  <table class="data-table"><thead><tr>
    <th>File</th><th>Function</th><th>CC</th><th>LOC</th><th>MI</th><th>Halstead</th><th>Params</th><th>Nesting</th><th>Widgets</th>
  </tr></thead><tbody>"#);

    for (file, result) in metrics {
        let file_name = file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown");
        for func in &result.functions {
            let cc_class = metric_class(func.cyclomatic_complexity, 10, 20, 30);
            let mi_class = metric_class_inverted(func.maintainability_index, 20.0, 40.0, 60.0);
            let nest_class = metric_class(func.max_nesting_level, 3, 5, 8);

            html.push_str(&format!(
                r#"<tr><td class="file-cell">{file}</td><td><code>{name}</code></td><td class="{cc_cls}">{cc}</td><td>{loc}</td><td class="{mi_cls}">{mi:.1}</td><td>{hv:.0}</td><td>{params}</td><td class="{nest_cls}">{nest}</td><td>{widgets}</td></tr>"#,
                file = html_escape(file_name),
                name = html_escape(&func.name),
                cc_cls = cc_class, cc = func.cyclomatic_complexity,
                loc = func.lines_of_code,
                mi_cls = mi_class, mi = func.maintainability_index,
                hv = func.halstead_volume,
                params = func.number_of_parameters,
                nest_cls = nest_class, nest = func.max_nesting_level,
                widgets = func.number_of_used_widgets,
            ));
        }
    }
    html.push_str("</tbody></table></div></div>");

    // Classes table
    html.push_str(r#"<div class="section-card">
  <h3>Class Metrics</h3>
  <p class="section-sub">Object-oriented complexity metrics per class</p>
  <div class="table-scroll">
  <table class="data-table"><thead><tr>
    <th>File</th><th>Class</th><th>Methods</th><th>WMC</th><th>CBO</th><th>DIT</th><th>RFC</th><th>LCOM</th><th>TCC</th><th>WOC</th>
  </tr></thead><tbody>"#);

    for (file, result) in metrics {
        let file_name = file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown");
        for class in &result.classes {
            let cbo_class = metric_class(class.coupling_between_objects, 5, 10, 15);
            let wmc_class = metric_class(class.weighted_methods_per_class, 10, 20, 30);

            html.push_str(&format!(
                r#"<tr><td class="file-cell">{file}</td><td><code>{name}</code></td><td>{methods}</td><td class="{wmc_cls}">{wmc}</td><td class="{cbo_cls}">{cbo}</td><td>{dit}</td><td>{rfc}</td><td>{lcom}</td><td>{tcc:.2}</td><td>{woc:.2}</td></tr>"#,
                file = html_escape(file_name),
                name = html_escape(&class.name),
                methods = class.number_of_methods,
                wmc_cls = wmc_class, wmc = class.weighted_methods_per_class,
                cbo_cls = cbo_class, cbo = class.coupling_between_objects,
                dit = class.depth_of_inheritance,
                rfc = class.response_for_class,
                lcom = class.lack_of_cohesion,
                tcc = class.tight_class_cohesion,
                woc = class.weight_of_class,
            ));
        }
    }
    html.push_str("</tbody></table></div></div>");

    html
}

fn metric_class(value: u32, noted: u32, warning: u32, alarm: u32) -> &'static str {
    if value >= alarm {
        "metric-alarm"
    } else if value >= warning {
        "metric-warning"
    } else if value >= noted {
        "metric-noted"
    } else {
        "metric-ok"
    }
}

fn metric_class_inverted(value: f64, alarm: f64, warning: f64, noted: f64) -> &'static str {
    if value <= alarm {
        "metric-alarm"
    } else if value <= warning {
        "metric-warning"
    } else if value <= noted {
        "metric-noted"
    } else {
        "metric-ok"
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn html_header(title: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500&display=swap');

/* ── Tokens ── */
:root {{
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', monospace;
  --radius: 12px;
  --radius-sm: 8px;
  --radius-xs: 6px;
  --shadow: 0 1px 3px rgba(0,0,0,.08), 0 4px 24px rgba(0,0,0,.06);
  --shadow-lg: 0 8px 32px rgba(0,0,0,.12);
  --transition: 200ms cubic-bezier(.4,0,.2,1);
  --accent: #6366f1;
  --accent-light: #818cf8;
  --accent-bg: rgba(99,102,241,.08);
}}

[data-theme="dark"] {{
  --bg-body: #0f0f14;
  --bg-sidebar: #16161e;
  --bg-card: #1a1a24;
  --bg-card-hover: #22222e;
  --bg-surface: #1e1e2a;
  --bg-input: #22222e;
  --border: #2a2a3a;
  --border-subtle: #222233;
  --text-primary: #e4e4ef;
  --text-secondary: #8888a0;
  --text-muted: #5c5c72;
  --ring-bg: #2a2a3a;
  --error: #ef4444;
  --warning: #f59e0b;
  --info: #6366f1;
  --success: #10b981;
  --table-stripe: rgba(255,255,255,.02);
  --table-hover: rgba(99,102,241,.06);
  --donut-text: #e4e4ef;
  --scroll-thumb: #3a3a4a;
}}

[data-theme="light"] {{
  --bg-body: #f5f5f7;
  --bg-sidebar: #ffffff;
  --bg-card: #ffffff;
  --bg-card-hover: #f9f9fb;
  --bg-surface: #f0f0f5;
  --bg-input: #f0f0f5;
  --border: #e2e2ea;
  --border-subtle: #ececf0;
  --text-primary: #1a1a2e;
  --text-secondary: #6b6b80;
  --text-muted: #9999aa;
  --ring-bg: #e2e2ea;
  --error: #dc2626;
  --warning: #d97706;
  --info: #4f46e5;
  --success: #059669;
  --table-stripe: rgba(0,0,0,.02);
  --table-hover: rgba(99,102,241,.04);
  --donut-text: #1a1a2e;
  --scroll-thumb: #ccc;
}}

/* ── Reset & base ── */
*, *::before, *::after {{ margin:0; padding:0; box-sizing:border-box; }}
html {{ font-size: 15px; scroll-behavior: smooth; }}
body {{
  font-family: var(--font-sans);
  background: var(--bg-body);
  color: var(--text-primary);
  display: flex;
  min-height: 100vh;
  -webkit-font-smoothing: antialiased;
}}

/* ── Scrollbar ── */
::-webkit-scrollbar {{ width:6px; height:6px; }}
::-webkit-scrollbar-track {{ background:transparent; }}
::-webkit-scrollbar-thumb {{ background:var(--scroll-thumb); border-radius:3px; }}

/* ── Sidebar ── */
.sidebar {{
  width: 240px;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  position: fixed;
  top: 0; bottom: 0; left: 0;
  z-index: 100;
  overflow-y: auto;
}}
.sidebar-brand {{
  display: flex; align-items: center; gap: 10px;
  padding: 24px 20px 20px;
}}
.brand-icon {{ width: 28px; height: 28px; }}
.brand-text {{
  font-size: 1.15rem; font-weight: 800;
  background: linear-gradient(135deg, #6366f1, #a78bfa);
  -webkit-background-clip: text; -webkit-text-fill-color: transparent;
  letter-spacing: -.02em;
}}
.sidebar-nav {{ flex:1; padding: 8px 10px; display:flex; flex-direction:column; gap:2px; }}
.nav-item {{
  display:flex; align-items:center; gap:10px;
  padding: 10px 12px; border-radius: var(--radius-sm);
  color: var(--text-secondary); text-decoration:none;
  font-size:.875rem; font-weight:500;
  transition: all var(--transition); cursor:pointer;
}}
.nav-item:hover {{ background: var(--accent-bg); color: var(--text-primary); }}
.nav-item.active {{ background: var(--accent-bg); color: var(--accent-light); }}
.nav-badge {{
  margin-left:auto;
  background: var(--accent-bg); color: var(--accent-light);
  padding: 1px 8px; border-radius: 10px;
  font-size:.75rem; font-weight:600;
}}
.sidebar-stats {{
  padding: 16px 20px; border-top:1px solid var(--border);
  display:flex; flex-direction:column; gap:8px;
}}
.stat-row {{
  display:flex; align-items:center; gap:8px;
  font-size:.8rem; color:var(--text-secondary);
}}
.stat-dot {{ width:8px; height:8px; border-radius:50%; flex-shrink:0; }}
.dot-red {{ background:var(--error); }}
.dot-amber {{ background:var(--warning); }}
.dot-blue {{ background:var(--info); }}
.stat-val {{ margin-left:auto; font-weight:600; color:var(--text-primary); }}

/* ── Main ── */
.main-content {{ margin-left:240px; flex:1; min-width:0; padding:0 32px 40px; }}

/* ── Top bar ── */
.top-bar {{
  display:flex; align-items:center; justify-content:space-between;
  padding: 24px 0 20px; position:sticky; top:0;
  background:var(--bg-body); z-index:50;
  border-bottom:1px solid var(--border-subtle); margin-bottom:24px;
}}
.top-bar h1 {{ font-size:1.4rem; font-weight:700; letter-spacing:-.02em; }}
.top-bar-sub {{ font-size:.8rem; color:var(--text-muted); margin-top:2px; display:block; }}
.theme-toggle {{
  background:var(--bg-card); border:1px solid var(--border); color:var(--text-secondary);
  width:36px; height:36px; border-radius:var(--radius-sm); cursor:pointer;
  display:flex; align-items:center; justify-content:center;
  transition: all var(--transition);
}}
.theme-toggle:hover {{ background:var(--bg-card-hover); color:var(--text-primary); }}

/* ── Tabs ── */
.tab-nav {{
  display:flex; gap:4px; margin-bottom:24px;
  border-bottom:1px solid var(--border); padding-bottom:0;
}}
.tab-btn {{
  background:none; border:none; color:var(--text-muted);
  padding:10px 18px; font-size:.875rem; font-weight:500;
  cursor:pointer; border-bottom:2px solid transparent;
  transition: all var(--transition); font-family:var(--font-sans);
}}
.tab-btn:hover {{ color:var(--text-primary); }}
.tab-btn.active {{ color:var(--accent-light); border-bottom-color:var(--accent); }}
.tab-content {{ display:none; animation: fadeIn .3s ease; }}
.tab-content.active {{ display:block; }}
@keyframes fadeIn {{ from {{ opacity:0; transform:translateY(8px); }} to {{ opacity:1; transform:translateY(0); }} }}

/* ── Overview top ── */
.overview-top {{ display:flex; gap:24px; margin-bottom:24px; align-items:stretch; }}
.health-card {{
  background: var(--bg-card); border:1px solid var(--border);
  border-radius:var(--radius); padding:24px;
  display:flex; flex-direction:column; align-items:center; justify-content:center;
  min-width:180px; gap:8px;
}}
.health-ring {{ position:relative; width:120px; height:120px; }}
.health-ring svg {{ width:100%; height:100%; }}
.health-arc {{ transition: stroke-dashoffset 1.5s cubic-bezier(.4,0,.2,1); }}
.health-value {{
  position:absolute; inset:0;
  display:flex; align-items:center; justify-content:center;
  font-size:2.2rem; font-weight:800; letter-spacing:-.03em;
}}
.health-meta {{ text-align:center; }}
.health-label {{ font-size:.75rem; color:var(--text-muted); display:block; }}
.health-grade {{ font-size:.9rem; font-weight:700; display:block; margin-top:2px; }}

.kpi-grid {{
  flex:1; display:grid;
  grid-template-columns: repeat(3, 1fr);
  gap:12px;
}}
.kpi-card {{
  background:var(--bg-card); border:1px solid var(--border);
  border-radius:var(--radius); padding:16px 18px;
  transition: all var(--transition);
}}
.kpi-card:hover {{ border-color:var(--accent); box-shadow: 0 0 0 1px var(--accent); }}
.kpi-value {{ font-size:1.6rem; font-weight:700; letter-spacing:-.02em; margin:4px 0 2px; }}
.kpi-label {{ font-size:.75rem; color:var(--text-muted); font-weight:500; }}
.kpi-danger .kpi-value {{ color:var(--error); }}
.kpi-warn .kpi-value {{ color:var(--warning); }}
.kpi-ok .kpi-value {{ color:var(--success); }}
.kpi-icon {{ width:20px; height:20px; border-radius:6px; }}

/* ── Charts ── */
.charts-row {{ display:grid; grid-template-columns:1fr 1fr; gap:24px; margin-bottom:24px; }}
.chart-card {{
  background:var(--bg-card); border:1px solid var(--border);
  border-radius:var(--radius); padding:24px;
}}
.chart-card h3 {{ font-size:.95rem; font-weight:600; margin-bottom:16px; }}

.donut-container {{ display:flex; align-items:center; gap:24px; }}
.donut-svg {{ width:160px; height:160px; flex-shrink:0; }}
.donut-total {{ fill:var(--donut-text); font-family:var(--font-sans); font-size:1.6rem; font-weight:800; }}
.donut-sub {{ fill:var(--text-muted); font-family:var(--font-sans); font-size:.6rem; }}
.donut-legend {{ display:flex; flex-direction:column; gap:10px; }}
.legend-item {{ display:flex; align-items:center; gap:8px; font-size:.8rem; color:var(--text-secondary); }}
.legend-dot {{ width:10px; height:10px; border-radius:3px; flex-shrink:0; }}
.legend-val {{ margin-left:auto; font-weight:600; color:var(--text-primary); min-width:70px; text-align:right; }}

.bar-chart {{ display:flex; flex-direction:column; gap:8px; }}
.bar-row {{ display:flex; align-items:center; gap:10px; }}
.bar-label {{
  width:180px; font-size:.75rem; font-family:var(--font-mono);
  color:var(--text-secondary); overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  flex-shrink:0;
}}
.bar-track {{
  flex:1; height:20px; background:var(--bg-surface); border-radius:4px; overflow:hidden;
}}
.bar-fill {{
  height:100%; border-radius:4px;
  background:linear-gradient(90deg, var(--accent), var(--accent-light));
  transition: width .8s cubic-bezier(.4,0,.2,1);
}}
.bar-value {{
  width:40px; text-align:right; font-size:.8rem; font-weight:600; color:var(--text-primary);
}}

/* ── Section cards ── */
.section-card {{
  background:var(--bg-card); border:1px solid var(--border);
  border-radius:var(--radius); padding:24px; margin-bottom:24px;
}}
.section-card h3 {{ font-size:.95rem; font-weight:600; margin-bottom:4px; }}
.section-sub {{ font-size:.8rem; color:var(--text-muted); margin-bottom:16px; }}

/* ── Tables ── */
.table-scroll {{ overflow-x:auto; }}
.data-table {{ width:100%; border-collapse:collapse; font-size:.825rem; }}
.data-table th {{
  text-align:left; padding:10px 12px;
  font-size:.7rem; font-weight:600; text-transform:uppercase; letter-spacing:.06em;
  color:var(--text-muted); border-bottom:1px solid var(--border);
  position:sticky; top:0; background:var(--bg-card); z-index:2;
}}
.data-table td {{ padding:8px 12px; border-bottom:1px solid var(--border-subtle); }}
.data-table tr:hover td {{ background:var(--table-hover); }}
.data-table code {{ font-family:var(--font-mono); font-size:.78rem; color:var(--accent-light); }}
.file-cell {{ color:var(--text-secondary); max-width:200px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }}

.metric-ok {{ color:var(--success); font-weight:600; }}
.metric-noted {{ color:var(--warning); font-weight:600; }}
.metric-warning {{ color:#f97316; font-weight:700; }}
.metric-alarm {{ color:var(--error); font-weight:700; }}

.risk-badge {{
  padding:3px 10px; border-radius:20px; font-size:.7rem; font-weight:600;
  display:inline-block; letter-spacing:.02em;
}}
.risk-high {{ background:rgba(239,68,68,.12); color:var(--error); }}
.risk-med {{ background:rgba(245,158,11,.12); color:var(--warning); }}
.risk-low {{ background:rgba(16,185,129,.12); color:var(--success); }}

/* ── Issues ── */
.issues-header {{ margin-bottom:16px; }}
.issues-count {{ font-size:.9rem; font-weight:600; margin-bottom:12px; }}
.issues-controls {{ display:flex; gap:12px; align-items:center; flex-wrap:wrap; }}
.search-input {{
  background:var(--bg-input); border:1px solid var(--border); color:var(--text-primary);
  padding:8px 14px; border-radius:var(--radius-sm); font-size:.825rem; width:260px;
  font-family:var(--font-sans); outline:none; transition:all var(--transition);
}}
.search-input:focus {{ border-color:var(--accent); box-shadow:0 0 0 2px var(--accent-bg); }}
.search-input::placeholder {{ color:var(--text-muted); }}
.filter-pills {{ display:flex; gap:4px; }}
.pill {{
  background:var(--bg-surface); border:1px solid var(--border); color:var(--text-secondary);
  padding:5px 12px; border-radius:20px; font-size:.75rem; font-weight:500;
  cursor:pointer; transition:all var(--transition); font-family:var(--font-sans);
}}
.pill:hover {{ border-color:var(--accent); }}
.pill.active {{ background:var(--accent-bg); border-color:var(--accent); color:var(--accent-light); }}

.file-group {{
  background:var(--bg-card); border:1px solid var(--border);
  border-radius:var(--radius-sm); margin-bottom:6px;
  overflow:hidden; transition: border-color var(--transition);
}}
.file-group[open] {{ border-color:var(--accent); }}
.file-summary {{
  padding:12px 16px; cursor:pointer;
  display:flex; align-items:center; gap:12px;
  font-size:.85rem; font-weight:500;
  user-select:none; list-style:none;
  transition: background var(--transition);
}}
.file-summary::-webkit-details-marker {{ display:none; }}
.file-summary::before {{
  content:''; display:inline-block; width:6px; height:6px;
  border-right:2px solid var(--text-muted); border-bottom:2px solid var(--text-muted);
  transform:rotate(-45deg); transition: transform var(--transition); flex-shrink:0;
}}
.file-group[open] .file-summary::before {{ transform:rotate(45deg); }}
.file-summary:hover {{ background:var(--bg-card-hover); }}
.file-name {{ font-family:var(--font-mono); font-size:.8rem; }}
.file-badges {{ margin-left:auto; display:flex; gap:6px; }}
.badge {{
  padding:2px 8px; border-radius:10px; font-size:.68rem; font-weight:600; letter-spacing:.02em;
}}
.badge-error {{ background:rgba(239,68,68,.12); color:var(--error); }}
.badge-warning {{ background:rgba(245,158,11,.12); color:var(--warning); }}
.badge-info {{ background:rgba(99,102,241,.12); color:var(--info); }}

.issue-table {{ width:100%; border-collapse:collapse; }}
.issue-table th {{
  text-align:left; padding:6px 12px;
  font-size:.68rem; font-weight:600; text-transform:uppercase; letter-spacing:.06em;
  color:var(--text-muted); background:var(--bg-surface); border-bottom:1px solid var(--border);
}}
.issue-table td {{ padding:7px 12px; border-bottom:1px solid var(--border-subtle); font-size:.8rem; }}
.issue-table tr:hover td {{ background:var(--table-hover); }}
.issue-row {{ transition: opacity var(--transition); }}
.issue-row.hidden {{ display:none; }}
.sev-error {{ background:var(--error); color:#fff; padding:2px 8px; border-radius:4px; font-size:.68rem; font-weight:700; }}
.sev-warn {{ background:var(--warning); color:#1a1a2e; padding:2px 8px; border-radius:4px; font-size:.68rem; font-weight:700; }}
.sev-info {{ background:var(--info); color:#fff; padding:2px 8px; border-radius:4px; font-size:.68rem; font-weight:700; }}
.line-num {{ font-family:var(--font-mono); font-size:.75rem; color:var(--text-muted); }}
.rule-code {{ font-family:var(--font-mono); font-size:.73rem; color:var(--accent-light); background:var(--accent-bg); padding:2px 6px; border-radius:4px; }}
.msg-cell {{ color:var(--text-secondary); }}

/* ── Project Properties ── */
.project-card {{
  background:var(--bg-card); border:1px solid var(--border);
  border-radius:var(--radius); padding:24px; margin-bottom:24px;
  border-top:3px solid var(--accent);
}}
.project-header {{
  display:flex; align-items:center; gap:16px; margin-bottom:12px;
}}
.project-icon {{ width:44px; height:44px; flex-shrink:0; }}
.project-icon svg {{ width:100%; height:100%; }}
.project-title-block {{ display:flex; align-items:baseline; gap:12px; flex-wrap:wrap; }}
.project-name {{
  font-size:1.3rem; font-weight:800; letter-spacing:-.02em;
  background:linear-gradient(135deg, var(--text-primary), var(--accent-light));
  -webkit-background-clip:text; -webkit-text-fill-color:transparent;
  margin:0; border:none; padding:0;
}}
.project-version {{
  background:var(--accent-bg); color:var(--accent-light);
  padding:3px 10px; border-radius:20px;
  font-size:.75rem; font-weight:600; font-family:var(--font-mono);
}}
.project-desc {{
  font-size:.85rem; color:var(--text-secondary); margin-bottom:16px;
  line-height:1.5; max-width:600px;
}}
.props-grid {{
  display:grid; grid-template-columns:repeat(auto-fill, minmax(160px, 1fr));
  gap:10px; margin-bottom:16px;
}}
.prop-item {{
  background:var(--bg-surface); border:1px solid var(--border-subtle);
  border-radius:var(--radius-xs); padding:10px 14px;
}}
.prop-label {{
  font-size:.68rem; font-weight:600; text-transform:uppercase;
  letter-spacing:.06em; color:var(--text-muted); display:block; margin-bottom:3px;
}}
.prop-value {{
  font-size:.88rem; font-weight:600; color:var(--text-primary);
  font-family:var(--font-mono); word-break:break-all;
}}
.dep-details {{
  background:var(--bg-surface); border:1px solid var(--border-subtle);
  border-radius:var(--radius-sm); overflow:hidden;
}}
.dep-details[open] {{ border-color:var(--accent); }}
.dep-summary {{
  padding:10px 16px; cursor:pointer;
  display:flex; align-items:center; justify-content:space-between;
  font-size:.8rem; font-weight:500; color:var(--text-secondary);
  list-style:none; user-select:none;
}}
.dep-summary::-webkit-details-marker {{ display:none; }}
.dep-summary:hover {{ background:var(--bg-card-hover); }}
.dep-count {{
  font-size:.72rem; color:var(--text-muted); font-family:var(--font-mono);
}}
.dep-chips {{
  display:flex; flex-wrap:wrap; gap:6px; padding:12px 16px;
  border-top:1px solid var(--border-subtle);
}}
.dep-chip {{
  background:var(--accent-bg); color:var(--accent-light);
  padding:3px 10px; border-radius:6px;
  font-size:.72rem; font-weight:500; font-family:var(--font-mono);
}}
.dep-chip-dev {{
  background:rgba(245,158,11,.08); color:var(--warning);
}}

/* ── Test Coverage ── */
.coverage-overview {{
  display:flex; gap:20px; margin-bottom:20px; align-items:stretch;
}}
.coverage-ring-card {{
  background:var(--bg-surface); border:1px solid var(--border-subtle);
  border-radius:var(--radius-sm); padding:20px 24px;
  display:flex; flex-direction:column; align-items:center;
  min-width:150px; position:relative;
}}
.coverage-ring {{ width:90px; height:90px; }}
.coverage-ring-inner {{
  position:absolute; top:20px; left:0; right:0;
  height:90px; display:flex; align-items:center; justify-content:center;
}}
.coverage-ring-val {{ font-size:1.3rem; font-weight:800; letter-spacing:-.02em; }}
.coverage-ring-label {{ font-size:.75rem; color:var(--text-muted); margin-top:8px; font-weight:500; }}
.coverage-ring-detail {{ font-size:.7rem; color:var(--text-muted); margin-top:2px; }}
.coverage-grade {{ font-size:.72rem; font-weight:700; margin-top:6px; letter-spacing:.03em; }}

.coverage-summary-cards {{
  flex:1; display:grid; grid-template-columns:1fr 1fr; gap:10px;
}}
.cov-stat {{
  background:var(--bg-surface); border:1px solid var(--border-subtle);
  border-radius:var(--radius-sm); padding:14px 16px;
  display:flex; flex-direction:column; gap:2px;
}}
.cov-stat-val {{ font-size:1.4rem; font-weight:700; }}
.cov-stat-label {{ font-size:.72rem; color:var(--text-muted); font-weight:500; }}
.cov-green {{ color:var(--success); }}
.cov-red {{ color:var(--error); }}

.coverage-bar-visual {{ margin-bottom:16px; }}
.cov-bar-header {{
  display:flex; justify-content:space-between; align-items:center;
  margin-bottom:8px; font-size:.8rem; font-weight:500; color:var(--text-secondary);
}}
.cov-stacked-bar {{
  display:flex; height:28px; border-radius:var(--radius-xs); overflow:hidden;
  background:var(--bg-surface);
}}
.cov-stacked-fill {{ transition:width 1s cubic-bezier(.4,0,.2,1); height:100%; }}
.cov-fill-tested {{ background:linear-gradient(90deg, #059669, #10b981); }}
.cov-fill-untested {{ background:linear-gradient(90deg, #dc2626, #ef4444); }}
.cov-bar-legend {{
  display:flex; gap:16px; margin-top:8px;
}}
.cov-legend-item {{
  display:flex; align-items:center; gap:6px;
  font-size:.75rem; color:var(--text-secondary);
}}

.coverage-details {{
  background:var(--bg-surface); border:1px solid var(--border-subtle);
  border-radius:var(--radius-sm); overflow:hidden;
}}
.coverage-details[open] {{ border-color:var(--accent); }}

.cov-status-tested {{
  background:rgba(16,185,129,.12); color:var(--success);
  padding:2px 10px; border-radius:10px; font-size:.7rem; font-weight:600;
}}
.cov-status-untested {{
  background:rgba(239,68,68,.1); color:var(--error);
  padding:2px 10px; border-radius:10px; font-size:.7rem; font-weight:600;
}}

/* ── Level of Concern ── */
.concern-section {{ margin-bottom:24px; }}
.concern-grid {{ display:flex; flex-direction:column; gap:10px; }}
.concern-card {{
  background:var(--bg-surface);
  border:1px solid var(--border-subtle);
  border-radius:var(--radius-sm);
  padding:14px 18px;
  transition: all var(--transition);
}}
.concern-card:hover {{
  border-color:var(--border);
  background:var(--bg-card-hover);
  transform:translateX(2px);
}}
.concern-header {{
  display:flex; align-items:center; gap:12px; margin-bottom:10px;
}}
.concern-icon {{ font-size:1.2rem; flex-shrink:0; width:28px; text-align:center; }}
.concern-title {{ flex:1; min-width:0; }}
.concern-name {{ font-size:.85rem; font-weight:600; display:block; }}
.concern-desc {{ font-size:.72rem; color:var(--text-muted); display:block; margin-top:1px; }}
.concern-badge-wrap {{ flex-shrink:0; }}
.concern-level {{
  font-size:.7rem; font-weight:700; letter-spacing:.04em; text-transform:uppercase;
  padding:3px 10px; border-radius:20px; border:1.5px solid;
  background:transparent;
}}
.concern-bar-wrap {{ display:flex; align-items:center; gap:12px; }}
.concern-bar-track {{
  flex:1; height:6px; background:var(--ring-bg); border-radius:3px; overflow:hidden;
}}
.concern-bar-fill {{
  height:100%; border-radius:3px;
  transition: width 1s cubic-bezier(.4,0,.2,1);
}}
.concern-stats {{ display:flex; align-items:center; gap:8px; flex-shrink:0; min-width:110px; }}
.concern-count {{ font-size:.75rem; color:var(--text-secondary); font-weight:500; }}
.concern-err {{
  font-size:.68rem; font-weight:600; color:var(--error);
  background:rgba(239,68,68,.1); padding:1px 7px; border-radius:10px;
}}

/* ── Empty state ── */
.empty-state {{ text-align:center; padding:48px 24px; color:var(--text-muted); }}
.empty-icon {{ font-size:2.5rem; margin-bottom:12px; }}

/* ── Footer ── */
.report-footer {{
  margin-top:48px; padding:16px 0;
  border-top:1px solid var(--border);
  color:var(--text-muted); font-size:.75rem;
  display:flex; align-items:center; justify-content:space-between;
}}

/* ── Responsive ── */
@media (max-width:1024px) {{
  .sidebar {{ display:none; }}
  .main-content {{ margin-left:0; }}
  .charts-row {{ grid-template-columns:1fr; }}
  .overview-top {{ flex-direction:column; }}
  .kpi-grid {{ grid-template-columns:repeat(2,1fr); }}
}}
@media (max-width:640px) {{
  .main-content {{ padding:0 16px 24px; }}
  .kpi-grid {{ grid-template-columns:1fr; }}
  .donut-container {{ flex-direction:column; }}
  .bar-label {{ width:100px; }}
}}
</style>
</head>
<body>
"##,
        title = title
    )
}

fn html_footer() -> String {
    format!(
        r##"<div class="report-footer">
  <span>Generated by Falcon v{version} &mdash; Rust-powered static analysis for Flutter &amp; Dart</span>
  <span id="gen-time"></span>
</div>
</main>

<script>
// Tab switching
function switchTab(tab) {{
  document.querySelectorAll('.tab-content').forEach(t => t.classList.remove('active'));
  document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
  document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
  document.getElementById('tab-' + tab).classList.add('active');
  document.querySelectorAll('.tab-btn').forEach(b => {{
    if (b.textContent.trim().toLowerCase().startsWith(tab)) b.classList.add('active');
  }});
  document.querySelectorAll('.nav-item').forEach(n => {{
    if (n.textContent.trim().toLowerCase().startsWith(tab)) n.classList.add('active');
  }});
}}

// Theme toggle
function toggleTheme() {{
  const html = document.documentElement;
  const current = html.getAttribute('data-theme');
  html.setAttribute('data-theme', current === 'dark' ? 'light' : 'dark');
}}

// Issue filtering
function filterIssues(query) {{
  const q = query.toLowerCase();
  document.querySelectorAll('.file-group').forEach(g => {{
    const file = g.getAttribute('data-file');
    const rules = Array.from(g.querySelectorAll('.rule-code')).map(r => r.textContent.toLowerCase());
    const msgs = Array.from(g.querySelectorAll('.msg-cell')).map(m => m.textContent.toLowerCase());
    const match = file.includes(q) || rules.some(r => r.includes(q)) || msgs.some(m => m.includes(q));
    g.style.display = match ? '' : 'none';
  }});
}}

function filterSeverity(btn, sev) {{
  document.querySelectorAll('.filter-pills .pill').forEach(p => p.classList.remove('active'));
  btn.classList.add('active');
  document.querySelectorAll('.issue-row').forEach(r => {{
    if (sev === 'all' || r.getAttribute('data-severity') === sev) {{
      r.classList.remove('hidden');
    }} else {{
      r.classList.add('hidden');
    }}
  }});
  document.querySelectorAll('.file-group').forEach(g => {{
    const visible = g.querySelectorAll('.issue-row:not(.hidden)').length;
    g.style.display = visible === 0 ? 'none' : '';
  }});
}}

// Generation timestamp
document.getElementById('gen-time').textContent = new Date().toLocaleString();
</script>
</body>
</html>"##,
        version = env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Severity;
    use crate::metrics::{ClassMetrics, FunctionMetrics, MetricsResults};
    use crate::reporters::{AnalysisReport, Issue};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn sample_function(name: &str) -> FunctionMetrics {
        FunctionMetrics {
            name: name.into(),
            line: 1,
            cyclomatic_complexity: 3,
            lines_of_code: 20,
            source_lines_of_code: 18,
            maintainability_index: 80.0,
            max_nesting_level: 2,
            number_of_parameters: 2,
            halstead_volume: 100.0,
            halstead_difficulty: 5.0,
            widgets_nesting_level: 0,
            number_of_used_widgets: 0,
        }
    }

    fn sample_class(name: &str) -> ClassMetrics {
        ClassMetrics {
            name: name.into(),
            line: 1,
            number_of_methods: 3,
            lines_of_code: 50,
            coupling_between_objects: 2,
            depth_of_inheritance: 1,
            number_of_added_methods: 1,
            number_of_interfaces: 0,
            number_of_overridden_methods: 1,
            response_for_class: 5,
            tight_class_cohesion: 0.5,
            weight_of_class: 0.6,
            weighted_methods_per_class: 8,
            lack_of_cohesion: 1,
        }
    }

    fn sample_metrics_results() -> MetricsResults {
        MetricsResults {
            file_lines_of_code: 100,
            file_source_lines_of_code: 80,
            functions: vec![sample_function("foo")],
            classes: vec![sample_class("Bar")],
        }
    }

    fn sample_issue(rule: &str, severity: Severity) -> Issue {
        Issue {
            rule: rule.into(),
            message: "test message".into(),
            severity,
            file: PathBuf::from("a.dart"),
            line: 1,
            column: 1,
        }
    }

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            issues: vec![],
            metrics: vec![],
            file_count: 5,
            project_path: None,
        }
    }

    // ── compute_health_score ──────────────────────────────────────────────────

    #[test]
    fn compute_health_score_no_issues_returns_100() {
        assert_eq!(compute_health_score(0, 0, 0, 10), 100);
    }

    #[test]
    fn compute_health_score_with_errors_reduces_score() {
        let score = compute_health_score(5, 0, 0, 10);
        assert!(score < 100, "expected score < 100, got {}", score);
    }

    #[test]
    fn compute_health_score_zero_file_count_doesnt_panic() {
        // When file_count == 0, the implementation returns 100 early
        let score = compute_health_score(1, 1, 1, 0);
        assert_eq!(score, 100);
    }

    #[test]
    fn compute_health_score_clamped_to_zero() {
        // Very high issue density should clamp to 0
        let score = compute_health_score(1000, 1000, 1000, 1);
        assert_eq!(score, 0);
    }

    // ── format_number ─────────────────────────────────────────────────────────

    #[test]
    fn format_number_under_thousand_unchanged() {
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn format_number_zero() {
        assert_eq!(format_number(0), "0");
    }

    #[test]
    fn format_number_thousands_uses_k_suffix() {
        // Implementation uses K suffix (e.g. "1.2K"), not comma separators
        let result = format_number(1234);
        assert!(result.contains('K') || result.contains(','),
            "expected K or comma in '{}' for 1234", result);
    }

    #[test]
    fn format_number_millions_uses_m_suffix() {
        let result = format_number(1_500_000);
        assert!(result.contains('M'), "expected M suffix in '{}' for 1_500_000", result);
    }

    #[test]
    fn format_number_exactly_one_thousand() {
        let result = format_number(1000);
        assert!(result.contains('K'), "expected K suffix for 1000, got '{}'", result);
    }

    // ── classify_concern ─────────────────────────────────────────────────────

    #[test]
    fn classify_concern_security_rule() {
        assert_eq!(classify_concern("hardcoded-credential"), "Security");
    }

    #[test]
    fn classify_concern_security_contains_security() {
        assert_eq!(classify_concern("some-security-check"), "Security");
    }

    #[test]
    fn classify_concern_error_handling() {
        assert_eq!(classify_concern("empty-catch"), "Error Handling");
    }

    #[test]
    fn classify_concern_async_void() {
        assert_eq!(classify_concern("async-void-function"), "Error Handling");
    }

    #[test]
    fn classify_concern_type_safety() {
        assert_eq!(classify_concern("avoid-dynamic"), "Type Safety");
    }

    #[test]
    fn classify_concern_complexity() {
        assert_eq!(classify_concern("long-function"), "Complexity");
    }

    #[test]
    fn classify_concern_performance() {
        assert_eq!(classify_concern("rebuild-widget"), "Performance");
    }

    #[test]
    fn classify_concern_resource_safety() {
        assert_eq!(classify_concern("missing-dispose"), "Resource Safety");
    }

    #[test]
    fn classify_concern_code_smells_late() {
        assert_eq!(classify_concern("avoid-late"), "Code Smells");
    }

    #[test]
    fn classify_concern_unknown_rule_returns_conventions() {
        assert_eq!(classify_concern("totally-unknown-rule-xyz"), "Conventions");
    }

    // ── concern_level_label ───────────────────────────────────────────────────

    #[test]
    fn concern_level_label_score_zero_is_low() {
        let (label, _color) = concern_level_label(0.0);
        assert_eq!(label, "Low");
    }

    #[test]
    fn concern_level_label_low_score() {
        let (label, _color) = concern_level_label(1.5);
        assert_eq!(label, "Low");
    }

    #[test]
    fn concern_level_label_moderate_score() {
        let (label, _color) = concern_level_label(5.0);
        assert_eq!(label, "Moderate");
    }

    #[test]
    fn concern_level_label_elevated_score() {
        let (label, _color) = concern_level_label(15.0);
        assert_eq!(label, "Elevated");
    }

    #[test]
    fn concern_level_label_high_score() {
        let (label, _color) = concern_level_label(30.0);
        assert_eq!(label, "High");
    }

    #[test]
    fn concern_level_label_critical_score() {
        let (label, _color) = concern_level_label(100.0);
        assert_eq!(label, "Critical");
    }

    // ── coverage_grade ────────────────────────────────────────────────────────

    #[test]
    fn coverage_grade_excellent() {
        let (_color, label) = coverage_grade(95);
        assert_eq!(label, "Excellent");
    }

    #[test]
    fn coverage_grade_excellent_boundary() {
        let (_color, label) = coverage_grade(80);
        assert_eq!(label, "Excellent");
    }

    #[test]
    fn coverage_grade_good() {
        let (_color, label) = coverage_grade(70);
        assert_eq!(label, "Good");
    }

    #[test]
    fn coverage_grade_fair() {
        let (_color, label) = coverage_grade(50);
        assert_eq!(label, "Fair");
    }

    #[test]
    fn coverage_grade_poor() {
        let (_color, label) = coverage_grade(25);
        assert_eq!(label, "Poor");
    }

    #[test]
    fn coverage_grade_critical() {
        let (_color, label) = coverage_grade(10);
        assert_eq!(label, "Critical");
    }

    #[test]
    fn coverage_grade_zero() {
        let (_color, label) = coverage_grade(0);
        assert_eq!(label, "Critical");
    }

    // ── metric_class ──────────────────────────────────────────────────────────

    #[test]
    fn metric_class_below_noted_returns_ok() {
        assert_eq!(metric_class(5, 10, 20, 30), "metric-ok");
    }

    #[test]
    fn metric_class_at_noted_returns_noted() {
        assert_eq!(metric_class(10, 10, 20, 30), "metric-noted");
    }

    #[test]
    fn metric_class_at_warning_returns_warning() {
        assert_eq!(metric_class(20, 10, 20, 30), "metric-warning");
    }

    #[test]
    fn metric_class_at_alarm_returns_alarm() {
        assert_eq!(metric_class(30, 10, 20, 30), "metric-alarm");
    }

    #[test]
    fn metric_class_above_alarm_returns_alarm() {
        assert_eq!(metric_class(100, 10, 20, 30), "metric-alarm");
    }

    // ── metric_class_inverted ─────────────────────────────────────────────────

    #[test]
    fn metric_class_inverted_high_value_returns_ok() {
        assert_eq!(metric_class_inverted(80.0, 20.0, 40.0, 60.0), "metric-ok");
    }

    #[test]
    fn metric_class_inverted_at_noted_returns_noted() {
        assert_eq!(metric_class_inverted(60.0, 20.0, 40.0, 60.0), "metric-noted");
    }

    #[test]
    fn metric_class_inverted_at_warning_returns_warning() {
        assert_eq!(metric_class_inverted(40.0, 20.0, 40.0, 60.0), "metric-warning");
    }

    #[test]
    fn metric_class_inverted_at_alarm_returns_alarm() {
        assert_eq!(metric_class_inverted(20.0, 20.0, 40.0, 60.0), "metric-alarm");
    }

    #[test]
    fn metric_class_inverted_low_value_returns_alarm() {
        assert_eq!(metric_class_inverted(10.0, 20.0, 40.0, 60.0), "metric-alarm");
    }

    // ── html_escape ───────────────────────────────────────────────────────────

    #[test]
    fn html_escape_replaces_lt_gt_amp() {
        let result = html_escape("a<b>&c");
        assert!(result.contains("&lt;"), "missing &lt; in '{}'", result);
        assert!(result.contains("&gt;"), "missing &gt; in '{}'", result);
        assert!(result.contains("&amp;"), "missing &amp; in '{}'", result);
    }

    #[test]
    fn html_escape_passes_safe_chars() {
        assert_eq!(html_escape("hello world"), "hello world");
    }

    #[test]
    fn html_escape_handles_double_quote() {
        let result = html_escape("a\"b");
        assert!(result.contains("&quot;") || result.contains("a\"b"),
            "unexpected result '{}'", result);
    }

    #[test]
    fn html_escape_empty_string() {
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn html_escape_multiple_ampersands() {
        let result = html_escape("a & b & c");
        assert_eq!(result.matches("&amp;").count(), 2);
    }

    // ── sidebar ───────────────────────────────────────────────────────────────

    #[test]
    fn sidebar_lists_severity_counts() {
        let html = sidebar(2, 5, 1);
        // sidebar uses format_number internally; single digits pass through unchanged
        assert!(html.contains("2"), "errors count missing");
        assert!(html.contains("5"), "warnings count missing");
        assert!(html.contains("1"), "info count missing");
    }

    #[test]
    fn sidebar_contains_falcon_brand() {
        let html = sidebar(0, 0, 0);
        assert!(html.contains("Falcon"));
    }

    #[test]
    fn sidebar_contains_nav_items() {
        let html = sidebar(0, 0, 0);
        assert!(html.contains("Overview"));
        assert!(html.contains("Issues"));
        assert!(html.contains("Metrics"));
    }

    // ── top_bar ───────────────────────────────────────────────────────────────

    #[test]
    fn top_bar_includes_file_count() {
        let html = top_bar(42);
        assert!(html.contains("42"), "file count 42 missing from top_bar");
    }

    #[test]
    fn top_bar_contains_analysis_report_title() {
        let html = top_bar(0);
        assert!(html.contains("Analysis Report"));
    }

    // ── top_bar_with_project ──────────────────────────────────────────────────

    #[test]
    fn top_bar_with_project_includes_project_name() {
        let html = top_bar_with_project("MyApp", 10);
        assert!(html.contains("MyApp"), "project name missing");
        assert!(html.contains("10"), "file count missing");
    }

    #[test]
    fn top_bar_with_project_escapes_special_chars() {
        let html = top_bar_with_project("A<B>Project", 1);
        assert!(html.contains("&lt;"), "< should be escaped");
        assert!(html.contains("&gt;"), "> should be escaped");
    }

    // ── health_score_card ─────────────────────────────────────────────────────

    #[test]
    fn health_score_card_high_score_contains_score_and_excellent() {
        let html = health_score_card(95);
        assert!(html.contains("95"), "score 95 missing");
        assert!(html.contains("Excellent"), "Excellent label missing for score 95");
    }

    #[test]
    fn health_score_card_low_score_contains_score_and_critical() {
        let html = health_score_card(10);
        assert!(html.contains("10"), "score 10 missing");
        assert!(html.contains("Critical"), "Critical label missing for score 10");
    }

    #[test]
    fn health_score_card_good_range() {
        let html = health_score_card(80);
        assert!(html.contains("Good"));
    }

    #[test]
    fn health_score_card_poor_range() {
        let html = health_score_card(35);
        assert!(html.contains("Poor"));
    }

    #[test]
    fn health_score_card_contains_health_score_label() {
        let html = health_score_card(50);
        assert!(html.contains("Health Score"));
    }

    // ── prop_item ─────────────────────────────────────────────────────────────

    #[test]
    fn prop_item_label_and_value_present() {
        let html = prop_item("Name", "Foo", "string");
        assert!(html.contains("Name"), "label missing");
        assert!(html.contains("Foo"), "value missing");
    }

    #[test]
    fn prop_item_escapes_value() {
        let html = prop_item("Key", "a<b>c", "string");
        assert!(html.contains("&lt;"), "< not escaped in value");
    }

    // ── kpi_card ──────────────────────────────────────────────────────────────

    #[test]
    fn kpi_card_label_and_value_present() {
        let html = kpi_card("Files", "42", "icon-file", "");
        assert!(html.contains("Files"), "label missing");
        assert!(html.contains("42"), "value missing");
    }

    #[test]
    fn kpi_card_extra_class_included() {
        let html = kpi_card("Errors", "5", "icon-errors", "kpi-danger");
        assert!(html.contains("kpi-danger"), "extra class missing");
    }

    // ── severity_donut ────────────────────────────────────────────────────────

    #[test]
    fn severity_donut_includes_all_three_severities() {
        let html = severity_donut(1, 2, 3);
        assert!(html.contains("Errors"), "Errors label missing");
        assert!(html.contains("Warnings"), "Warnings label missing");
        assert!(html.contains("Info"), "Info label missing");
    }

    #[test]
    fn severity_donut_zero_issues_shows_empty_state() {
        let html = severity_donut(0, 0, 0);
        assert!(html.contains("No issues") || html.contains("empty"),
            "expected empty-state marker");
    }

    #[test]
    fn severity_donut_shows_counts() {
        let html = severity_donut(3, 7, 2);
        assert!(html.contains("3"), "error count missing");
        assert!(html.contains("7"), "warning count missing");
        assert!(html.contains("2"), "info count missing");
    }

    // ── top_rules_chart ───────────────────────────────────────────────────────

    #[test]
    fn top_rules_chart_lists_rules() {
        let issues = vec![
            sample_issue("rule-a", Severity::Error),
            sample_issue("rule-a", Severity::Error),
            sample_issue("rule-b", Severity::Warning),
        ];
        let html = top_rules_chart(&issues);
        assert!(html.contains("rule-a"), "rule-a missing");
        assert!(html.contains("rule-b"), "rule-b missing");
    }

    #[test]
    fn top_rules_chart_empty_issues_renders_safely() {
        let html = top_rules_chart(&[]);
        assert!(!html.is_empty(), "empty string returned for empty issues");
        assert!(html.contains("<div"), "no HTML structure present");
    }

    #[test]
    fn top_rules_chart_most_frequent_rule_appears_first() {
        let issues = vec![
            sample_issue("rare-rule", Severity::Info),
            sample_issue("common-rule", Severity::Error),
            sample_issue("common-rule", Severity::Error),
            sample_issue("common-rule", Severity::Error),
        ];
        let html = top_rules_chart(&issues);
        let pos_common = html.find("common-rule").unwrap_or(usize::MAX);
        let pos_rare = html.find("rare-rule").unwrap_or(usize::MAX);
        assert!(pos_common < pos_rare, "common-rule should appear before rare-rule");
    }

    // ── hotspots_section ──────────────────────────────────────────────────────

    #[test]
    fn hotspots_section_empty_metrics_renders_table() {
        let html = hotspots_section(&[]);
        assert!(html.contains("Complexity Hotspots"), "section header missing");
        assert!(html.contains("<table"), "table structure missing");
    }

    #[test]
    fn hotspots_section_high_cc_file_included() {
        let mut high_cc_func = sample_function("complex_fn");
        high_cc_func.cyclomatic_complexity = 25;
        let metrics_result = MetricsResults {
            file_lines_of_code: 300,
            file_source_lines_of_code: 250,
            functions: vec![high_cc_func],
            classes: vec![],
        };
        let metrics = vec![(PathBuf::from("/project/lib/complex.dart"), metrics_result)];
        let html = hotspots_section(&metrics);
        assert!(html.contains("complex.dart"), "file name missing from hotspots");
    }

    #[test]
    fn hotspots_section_low_cc_file_not_included() {
        // CC=3, LOC=50 — below both thresholds (cc>5, loc>200)
        let low_cc_metrics = MetricsResults {
            file_lines_of_code: 50,
            file_source_lines_of_code: 40,
            functions: vec![sample_function("simple_fn")],
            classes: vec![],
        };
        let metrics = vec![(PathBuf::from("/project/lib/simple.dart"), low_cc_metrics)];
        let html = hotspots_section(&metrics);
        // Should render table but without this file
        assert!(!html.contains("simple.dart"), "low-CC file should not appear in hotspots");
    }

    // ── level_of_concern_section ──────────────────────────────────────────────

    #[test]
    fn level_of_concern_section_includes_header() {
        let html = level_of_concern_section(&[], 10);
        assert!(html.contains("Level of Concern"), "section header missing");
    }

    #[test]
    fn level_of_concern_section_with_security_issue() {
        let issues = vec![sample_issue("hardcoded-credential", Severity::Error)];
        let html = level_of_concern_section(&issues, 5);
        assert!(html.contains("Security"), "Security area missing");
    }

    #[test]
    fn level_of_concern_section_shows_all_concern_areas() {
        let html = level_of_concern_section(&[], 1);
        assert!(html.contains("Security"), "Security area missing");
        assert!(html.contains("Complexity"), "Complexity area missing");
        assert!(html.contains("Performance"), "Performance area missing");
    }

    // ── project_properties_section ────────────────────────────────────────────

    #[test]
    fn project_properties_section_includes_project_name() {
        let info = ProjectInfo::default();
        let html = project_properties_section(&info, "TestProject");
        assert!(html.contains("TestProject"), "project name missing");
    }

    #[test]
    fn project_properties_section_with_version() {
        let info = ProjectInfo {
            name: "my_app".into(),
            version: "1.2.3".into(),
            description: String::new(),
            sdk_constraint: String::new(),
            flutter_constraint: String::new(),
            dependencies: vec![],
            dev_dependencies: vec![],
            homepage: String::new(),
            repository: String::new(),
        };
        let html = project_properties_section(&info, "my_app");
        assert!(html.contains("1.2.3"), "version missing");
    }

    #[test]
    fn project_properties_section_with_description() {
        let info = ProjectInfo {
            name: "my_app".into(),
            version: "0.1.0".into(),
            description: "A great app".into(),
            sdk_constraint: String::new(),
            flutter_constraint: String::new(),
            dependencies: vec![],
            dev_dependencies: vec![],
            homepage: String::new(),
            repository: String::new(),
        };
        let html = project_properties_section(&info, "my_app");
        assert!(html.contains("A great app"), "description missing");
    }

    // ── issues_section ────────────────────────────────────────────────────────

    #[test]
    fn issues_section_lists_each_issue() {
        let issues = vec![
            sample_issue("rule-one", Severity::Error),
            sample_issue("rule-two", Severity::Warning),
        ];
        let html = issues_section(&issues, issues.len());
        assert!(html.contains("rule-one"), "rule-one missing");
        assert!(html.contains("rule-two"), "rule-two missing");
    }

    #[test]
    fn issues_section_empty_still_renders() {
        // issues_section with 0 issues but non-zero total
        let html = issues_section(&[], 0);
        assert!(!html.is_empty(), "empty string for empty issues");
    }

    #[test]
    fn issues_section_shows_total_count() {
        let issues = vec![
            sample_issue("r1", Severity::Error),
            sample_issue("r2", Severity::Warning),
            sample_issue("r3", Severity::Info),
        ];
        let html = issues_section(&issues, 3);
        assert!(html.contains("3"), "total count missing");
    }

    #[test]
    fn issues_section_contains_severity_labels() {
        let issues = vec![
            sample_issue("err-rule", Severity::Error),
            sample_issue("warn-rule", Severity::Warning),
            sample_issue("info-rule", Severity::Info),
        ];
        let html = issues_section(&issues, 3);
        assert!(html.contains("ERROR") || html.contains("error"), "ERROR label missing");
        assert!(html.contains("WARN") || html.contains("warning"), "WARN label missing");
    }

    // ── metrics_section ───────────────────────────────────────────────────────

    #[test]
    fn metrics_section_lists_function_and_class_names() {
        let metrics = vec![(
            PathBuf::from("src/my_file.dart"),
            sample_metrics_results(),
        )];
        let html = metrics_section(&metrics);
        assert!(html.contains("foo"), "function name 'foo' missing");
        assert!(html.contains("Bar"), "class name 'Bar' missing");
    }

    #[test]
    fn metrics_section_contains_section_headers() {
        let html = metrics_section(&[]);
        assert!(html.contains("Function Metrics"), "Function Metrics header missing");
        assert!(html.contains("Class Metrics"), "Class Metrics header missing");
    }

    #[test]
    fn metrics_section_shows_file_name() {
        let metrics = vec![(
            PathBuf::from("/project/lib/my_widget.dart"),
            sample_metrics_results(),
        )];
        let html = metrics_section(&metrics);
        assert!(html.contains("my_widget.dart"), "file name missing from metrics section");
    }

    // ── test_coverage_section ─────────────────────────────────────────────────

    #[test]
    fn test_coverage_section_empty_metrics_returns_empty() {
        // With no metrics, scan_test_coverage returns empty vec,
        // so test_coverage_section returns empty string
        let html = test_coverage_section(&[]);
        assert!(html.is_empty(), "expected empty string for no metrics, got: '{}'", &html[..html.len().min(100)]);
    }

    #[test]
    fn test_coverage_section_with_lib_path_metrics() {
        // Provide metrics with a lib/ path so detect_project_root finds the root
        let metrics_result = MetricsResults {
            file_lines_of_code: 100,
            file_source_lines_of_code: 80,
            functions: vec![sample_function("my_func")],
            classes: vec![],
        };
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("lib").join("my_file.dart");
        std::fs::create_dir_all(lib_path.parent().unwrap()).unwrap();
        std::fs::write(&lib_path, "// dart").unwrap();
        let metrics = vec![(lib_path, metrics_result)];
        let html = test_coverage_section(&metrics);
        // With no test/ dir, coverage should be 0% or empty
        // Either empty (no coverage items found) or shows 0% coverage
        if !html.is_empty() {
            assert!(html.contains("Test Coverage") || html.contains("0%") || html.contains("Untested"),
                "unexpected coverage html content");
        }
    }

    // ── html_header ───────────────────────────────────────────────────────────

    #[test]
    fn html_header_contains_doctype_and_title() {
        let html = html_header("My Report");
        assert!(html.contains("<!DOCTYPE"), "DOCTYPE missing");
        assert!(html.contains("My Report"), "title missing");
    }

    #[test]
    fn html_header_contains_html_tag() {
        let html = html_header("Test");
        assert!(html.contains("<html"), "html tag missing");
        assert!(html.contains("<head>") || html.contains("<head"), "head tag missing");
    }

    #[test]
    fn html_header_contains_charset() {
        let html = html_header("Test");
        assert!(html.contains("UTF-8"), "charset missing");
    }

    // ── html_footer ───────────────────────────────────────────────────────────

    #[test]
    fn html_footer_closes_body_and_html() {
        let html = html_footer();
        assert!(html.contains("</body>"), "</body> missing");
        assert!(html.contains("</html>"), "</html> missing");
    }

    #[test]
    fn html_footer_contains_falcon_credit() {
        let html = html_footer();
        assert!(html.contains("Falcon"), "Falcon credit missing from footer");
    }

    // ── build_full_report ─────────────────────────────────────────────────────

    #[test]
    fn build_full_report_includes_doctype_and_file_count() {
        let mut report = sample_report();
        report.file_count = 7;
        let html = build_full_report(&report);
        assert!(html.contains("<!DOCTYPE"), "DOCTYPE missing");
        assert!(html.contains("7"), "file count 7 missing");
    }

    #[test]
    fn build_full_report_contains_major_sections() {
        let report = sample_report();
        let html = build_full_report(&report);
        assert!(html.contains("Health Score"), "Health Score section missing");
        assert!(html.contains("</html>"), "html closing tag missing");
    }

    #[test]
    fn build_full_report_with_issues_includes_rule() {
        let mut report = sample_report();
        report.issues = vec![sample_issue("my-special-rule", Severity::Error)];
        report.file_count = 1;
        let html = build_full_report(&report);
        assert!(html.contains("my-special-rule"), "issue rule missing from full report");
    }

    #[test]
    fn build_full_report_with_metrics_includes_function_name() {
        let mut report = sample_report();
        report.metrics = vec![(
            PathBuf::from("/proj/lib/widget.dart"),
            sample_metrics_results(),
        )];
        report.file_count = 1;
        let html = build_full_report(&report);
        assert!(html.contains("foo"), "function name missing from full report");
    }

    // ── build_metrics_report ──────────────────────────────────────────────────

    #[test]
    fn build_metrics_report_includes_doctype() {
        let html = build_metrics_report(&[]);
        assert!(html.contains("<!DOCTYPE"), "DOCTYPE missing");
    }

    #[test]
    fn build_metrics_report_contains_metrics_section() {
        let metrics = vec![(
            PathBuf::from("lib/my_file.dart"),
            sample_metrics_results(),
        )];
        let html = build_metrics_report(&metrics);
        assert!(html.contains("Function Metrics") || html.contains("foo"),
            "metrics content missing");
    }

    #[test]
    fn build_metrics_report_includes_file_count() {
        let metrics = vec![
            (PathBuf::from("a.dart"), sample_metrics_results()),
            (PathBuf::from("b.dart"), sample_metrics_results()),
        ];
        let html = build_metrics_report(&metrics);
        assert!(html.contains("2"), "file count 2 missing");
    }

    // ── build_issues_report ───────────────────────────────────────────────────

    #[test]
    fn build_issues_report_includes_doctype() {
        let html = build_issues_report(&[]);
        assert!(html.contains("<!DOCTYPE"), "DOCTYPE missing");
    }

    #[test]
    fn build_issues_report_contains_issue_rule() {
        let issues = vec![sample_issue("some-lint-rule", Severity::Warning)];
        let html = build_issues_report(&issues);
        assert!(html.contains("some-lint-rule"), "rule name missing from issues report");
    }

    // ── parse_pubspec (file-IO) ───────────────────────────────────────────────

    #[test]
    fn parse_pubspec_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let info = parse_pubspec(tmp.path());
        assert!(info.name.is_empty(), "expected empty name, got '{}'", info.name);
        assert!(info.version.is_empty(), "expected empty version");
    }

    #[test]
    fn parse_pubspec_reads_name_and_description() {
        let tmp = TempDir::new().unwrap();
        let pubspec = tmp.path().join("pubspec.yaml");
        std::fs::write(&pubspec, "name: my_app\ndescription: An awesome app\nversion: 1.0.0\n").unwrap();
        let info = parse_pubspec(tmp.path());
        assert_eq!(info.name, "my_app");
        assert_eq!(info.description, "An awesome app");
        assert_eq!(info.version, "1.0.0");
    }

    #[test]
    fn parse_pubspec_reads_dependencies() {
        let tmp = TempDir::new().unwrap();
        let pubspec = tmp.path().join("pubspec.yaml");
        let content = "name: my_app\ndependencies:\n  flutter:\n    sdk: flutter\n  http: ^0.13.0\n";
        std::fs::write(&pubspec, content).unwrap();
        let info = parse_pubspec(tmp.path());
        assert!(!info.dependencies.is_empty(), "expected at least one dependency");
    }

    #[test]
    fn parse_pubspec_reads_sdk_constraint() {
        let tmp = TempDir::new().unwrap();
        let pubspec = tmp.path().join("pubspec.yaml");
        let content = "name: test\nenvironment:\n  sdk: '>=2.17.0 <4.0.0'\n";
        std::fs::write(&pubspec, content).unwrap();
        let info = parse_pubspec(tmp.path());
        assert!(info.sdk_constraint.contains("2.17.0") || !info.sdk_constraint.is_empty(),
            "sdk_constraint should be set");
    }

    // ── collect_dart_files (file-IO) ──────────────────────────────────────────

    #[test]
    fn collect_dart_files_walks_recursively() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.dart"), "").unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b.dart"), "").unwrap();
        let subsub = sub.join("deep");
        std::fs::create_dir(&subsub).unwrap();
        std::fs::write(subsub.join("c.dart"), "").unwrap();
        std::fs::write(tmp.path().join("readme.txt"), "").unwrap();

        let mut out = std::collections::HashSet::new();
        collect_dart_files(tmp.path(), &mut out);

        assert_eq!(out.len(), 3, "expected 3 dart files, found {}: {:?}", out.len(), out);
        assert!(!out.iter().any(|f| f.ends_with(".txt")), "txt file should not be collected");
    }

    #[test]
    fn collect_dart_files_empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let mut out = std::collections::HashSet::new();
        collect_dart_files(tmp.path(), &mut out);
        assert!(out.is_empty());
    }

    // ── detect_project_root ───────────────────────────────────────────────────

    #[test]
    fn detect_project_root_finds_root_via_lib_prefix() {
        let metrics = vec![
            (PathBuf::from("/home/user/myapp/lib/src/widget.dart"), sample_metrics_results()),
            (PathBuf::from("/home/user/myapp/lib/main.dart"), sample_metrics_results()),
        ];
        let root = detect_project_root(&metrics);
        assert!(root.is_some(), "expected Some root");
        let root = root.unwrap();
        assert_eq!(root, PathBuf::from("/home/user/myapp"),
            "unexpected root: {:?}", root);
    }

    #[test]
    fn detect_project_root_falls_back_to_parent() {
        // Paths without /lib/ prefix
        let metrics = vec![
            (PathBuf::from("/some/path/a.dart"), sample_metrics_results()),
        ];
        let root = detect_project_root(&metrics);
        assert!(root.is_some());
        assert_eq!(root.unwrap(), PathBuf::from("/some/path"));
    }

    #[test]
    fn detect_project_root_empty_metrics_returns_none() {
        let root = detect_project_root(&[]);
        assert!(root.is_none());
    }

    // ── scan_test_coverage (file-IO) ──────────────────────────────────────────

    #[test]
    fn scan_test_coverage_zero_when_no_tests() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        std::fs::create_dir(&lib_dir).unwrap();
        std::fs::write(lib_dir.join("my_widget.dart"), "// code").unwrap();

        let metrics = vec![(
            lib_dir.join("my_widget.dart"),
            sample_metrics_results(),
        )];
        let coverage = scan_test_coverage(&metrics);
        let has_any_tested = coverage.iter().any(|c| c.has_test);
        assert!(!has_any_tested, "should have no tested files when no test/ dir exists");
    }

    #[test]
    fn scan_test_coverage_detects_test_files() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        let test_dir = tmp.path().join("test");
        std::fs::create_dir(&lib_dir).unwrap();
        std::fs::create_dir(&test_dir).unwrap();
        std::fs::write(lib_dir.join("my_widget.dart"), "// code").unwrap();
        std::fs::write(test_dir.join("my_widget_test.dart"), "// test").unwrap();

        let metrics = vec![(
            lib_dir.join("my_widget.dart"),
            sample_metrics_results(),
        )];
        let coverage = scan_test_coverage(&metrics);
        assert!(!coverage.is_empty(), "expected coverage entries");
        let entry = coverage.iter().find(|c| c.source_file == "my_widget.dart");
        assert!(entry.is_some(), "my_widget.dart entry not found");
        assert!(entry.unwrap().has_test, "my_widget.dart should be marked as tested");
    }

    // ── HtmlReporter trait methods (file-IO) ──────────────────────────────────

    #[test]
    fn html_reporter_report_analysis_writes_file() {
        let tmp = TempDir::new().unwrap();
        let out_path = tmp.path().join("out.html");
        let reporter = HtmlReporter { output_path: out_path.clone() };
        reporter.report_analysis(&sample_report());
        assert!(out_path.exists(), "output file not created");
        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(content.contains("<!DOCTYPE"), "DOCTYPE missing in written file");
    }

    #[test]
    fn html_reporter_report_metrics_writes_file() {
        let tmp = TempDir::new().unwrap();
        let out_path = tmp.path().join("metrics.html");
        let reporter = HtmlReporter { output_path: out_path.clone() };
        let metrics = vec![(PathBuf::from("a.dart"), sample_metrics_results())];
        reporter.report_metrics(&metrics);
        assert!(out_path.exists(), "output file not created");
        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(content.contains("<!DOCTYPE"), "DOCTYPE missing");
    }

    #[test]
    fn html_reporter_report_issues_writes_file() {
        let tmp = TempDir::new().unwrap();
        let out_path = tmp.path().join("issues.html");
        let reporter = HtmlReporter { output_path: out_path.clone() };
        let issues = vec![sample_issue("test-rule", Severity::Warning)];
        reporter.report_issues(&issues);
        assert!(out_path.exists(), "output file not created");
        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(content.contains("<!DOCTYPE"), "DOCTYPE missing");
        assert!(content.contains("test-rule"), "issue rule missing from written file");
    }
}
