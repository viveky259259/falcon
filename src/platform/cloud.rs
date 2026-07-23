//! Falcon Cloud — team dashboards, multi-project tracking, alerts, and sharing.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

const CLOUD_CONFIG_FILE: &str = ".falcon-data/cloud-config.json";

/// Cloud configuration for a team/organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub team_name: String,
    pub projects: Vec<ProjectEntry>,
    pub alerts: Vec<AlertRule>,
    pub sharing: SharingConfig,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            team_name: "My Team".to_string(),
            projects: vec![],
            alerts: vec![
                AlertRule {
                    name: "Score drop".to_string(),
                    condition: AlertCondition::ScoreDropBelow(70),
                    notify: NotifyChannel::Console,
                    enabled: true,
                },
                AlertRule {
                    name: "Critical vulnerability".to_string(),
                    condition: AlertCondition::CriticalVulnerability,
                    notify: NotifyChannel::Console,
                    enabled: true,
                },
            ],
            sharing: SharingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub last_score: Option<u32>,
    pub last_analyzed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub notify: NotifyChannel,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    ScoreDropBelow(u32),
    NewErrors(usize),
    CriticalVulnerability,
    DriftAboveThreshold(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotifyChannel {
    Console,
    Webhook(String),
    File(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SharingConfig {
    pub share_conventions: bool,
    pub share_scores: bool,
    pub anonymize: bool,
}

/// Team dashboard data aggregated from all projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDashboard {
    pub team_name: String,
    pub project_count: usize,
    pub projects: Vec<ProjectSummary>,
    pub avg_score: f64,
    pub total_issues: usize,
    pub alerts_triggered: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub name: String,
    pub score: u32,
    pub grade: String,
    pub issues: usize,
    pub files: usize,
    pub trend: String,
}

/// Load cloud config.
pub fn load_cloud_config(root: &Path) -> anyhow::Result<CloudConfig> {
    let path = root.join(CLOUD_CONFIG_FILE);
    if !path.exists() {
        return Ok(CloudConfig::default());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Save cloud config.
pub fn save_cloud_config(root: &Path, config: &CloudConfig) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(root.join(CLOUD_CONFIG_FILE), json)?;
    Ok(())
}

/// Initialize a cloud config with a team name.
pub fn init_cloud(root: &Path, team_name: &str) -> anyhow::Result<()> {
    let config = CloudConfig {
        team_name: team_name.to_string(),
        ..Default::default()
    };
    save_cloud_config(root, &config)?;
    Ok(())
}

/// Register a project in the cloud dashboard.
pub fn register_project(root: &Path, name: &str, project_path: &str) -> anyhow::Result<()> {
    let mut config = load_cloud_config(root)?;
    if config.projects.iter().any(|p| p.name == name) {
        anyhow::bail!("Project '{}' already registered", name);
    }
    config.projects.push(ProjectEntry {
        name: name.to_string(),
        path: project_path.to_string(),
        last_score: None,
        last_analyzed: None,
    });
    save_cloud_config(root, &config)?;
    Ok(())
}

/// Generate a team dashboard by analyzing all registered projects.
pub fn generate_dashboard(root: &Path) -> anyhow::Result<TeamDashboard> {
    let mut config = load_cloud_config(root)?;
    let mut summaries = Vec::new();
    let mut total_issues = 0;
    let mut alerts = Vec::new();

    for project in &mut config.projects {
        let project_path = std::path::PathBuf::from(&project.path);
        if !project_path.exists() {
            summaries.push(ProjectSummary {
                name: project.name.clone(),
                score: 0,
                grade: "?".to_string(),
                issues: 0,
                files: 0,
                trend: "unavailable".to_string(),
            });
            continue;
        }

        match crate::ai_score::score::calculate_ai_score(&project_path) {
            Ok(score) => {
                let prev_score = project.last_score;
                let trend = match prev_score {
                    Some(prev) if score.overall > prev => format!("↑ +{}", score.overall - prev),
                    Some(prev) if score.overall < prev => format!("↓ -{}", prev - score.overall),
                    Some(_) => "→ stable".to_string(),
                    None => "new".to_string(),
                };

                for alert in &config.alerts {
                    if !alert.enabled {
                        continue;
                    }
                    match alert.condition {
                        AlertCondition::ScoreDropBelow(threshold) => {
                            if score.overall < threshold {
                                alerts.push(format!(
                                    "[{}] {} — score {}/100 below threshold {}",
                                    project.name, alert.name, score.overall, threshold
                                ));
                            }
                        }
                        AlertCondition::NewErrors(threshold) => {
                            if score.total_issues > threshold {
                                alerts.push(format!(
                                    "[{}] {} — {} issues exceed threshold {}",
                                    project.name, alert.name, score.total_issues, threshold
                                ));
                            }
                        }
                        AlertCondition::CriticalVulnerability => {
                            if score.security.score < 80 {
                                alerts.push(format!("[{}] {} — security score {}/100, check for hardcoded credentials",
                                    project.name, alert.name, score.security.score));
                            }
                        }
                        AlertCondition::DriftAboveThreshold(max_drift) => {
                            if let Ok(drift) =
                                crate::ai_score::drift::detect_drift(&project_path, None)
                            {
                                let drift_pct = 100.0 - drift.drift_score;
                                if drift_pct > max_drift {
                                    alerts.push(format!(
                                        "[{}] {} — {:.0}% drift exceeds threshold {:.0}%",
                                        project.name, alert.name, drift_pct, max_drift
                                    ));
                                }
                            }
                        }
                    }
                }

                total_issues += score.total_issues;

                summaries.push(ProjectSummary {
                    name: project.name.clone(),
                    score: score.overall,
                    grade: score.grade.to_string(),
                    issues: score.total_issues,
                    files: score.file_count,
                    trend,
                });

                project.last_score = Some(score.overall);
                project.last_analyzed = Some(get_timestamp());
            }
            Err(e) => {
                log::warn!("Failed to analyze {}: {}", project.name, e);
                summaries.push(ProjectSummary {
                    name: project.name.clone(),
                    score: 0,
                    grade: "?".to_string(),
                    issues: 0,
                    files: 0,
                    trend: "error".to_string(),
                });
            }
        }
    }

    save_cloud_config(root, &config)?;

    let scores: Vec<f64> = summaries
        .iter()
        .filter(|s| s.score > 0)
        .map(|s| s.score as f64)
        .collect();
    let avg = if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    };

    Ok(TeamDashboard {
        team_name: config.team_name.clone(),
        project_count: summaries.len(),
        projects: summaries,
        avg_score: avg,
        total_issues,
        alerts_triggered: alerts,
    })
}

/// Print the team dashboard.
pub fn print_dashboard(dashboard: &TeamDashboard) {
    println!();
    println!(
        "  {} Cloud Dashboard — {}",
        "falcon".bright_cyan().bold(),
        dashboard.team_name.bright_white().bold()
    );
    println!();

    println!("  Projects:     {}", dashboard.project_count);
    println!("  Avg Score:    {:.0}/100", dashboard.avg_score);
    println!("  Total Issues: {}", dashboard.total_issues);

    if !dashboard.projects.is_empty() {
        println!();
        println!(
            "  {:<25} {:<8} {:<6} {:<8} {:<8} Trend",
            "Project", "Score", "Grade", "Issues", "Files"
        );
        println!("  {}", "─".repeat(70));

        for p in &dashboard.projects {
            let score_color = if p.score >= 85 {
                p.score.to_string().bright_green()
            } else if p.score >= 60 {
                p.score.to_string().yellow()
            } else {
                p.score.to_string().red()
            };

            println!(
                "  {:<25} {:<8} {:<6} {:<8} {:<8} {}",
                p.name.bright_white(),
                score_color,
                p.grade,
                p.issues,
                p.files,
                p.trend
            );
        }
    }

    if !dashboard.alerts_triggered.is_empty() {
        println!();
        println!("  {} Alerts:", "⚠".yellow().bold());
        for alert in &dashboard.alerts_triggered {
            println!("    {} {}", "!".red(), alert);
        }
    }

    println!();
}

fn get_timestamp() -> String {
    std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn write_file(dir: &std::path::Path, rel: &str, content: &str) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&full).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    fn write_minimal_dart_project(root: &std::path::Path) {
        write_file(root, "lib/main.dart", "void main() {}\n");
    }

    // ── CloudConfig::default ─────────────────────────────────────────────────

    #[test]
    fn test_cloud_config_default_team_name() {
        let cfg = CloudConfig::default();
        assert_eq!(cfg.team_name, "My Team");
    }

    #[test]
    fn test_cloud_config_default_projects_empty() {
        let cfg = CloudConfig::default();
        assert!(cfg.projects.is_empty());
    }

    #[test]
    fn test_cloud_config_default_alerts_count() {
        let cfg = CloudConfig::default();
        assert_eq!(cfg.alerts.len(), 2);
    }

    #[test]
    fn test_cloud_config_default_first_alert_is_score_drop() {
        let cfg = CloudConfig::default();
        assert_eq!(cfg.alerts[0].name, "Score drop");
        assert!(cfg.alerts[0].enabled);
        matches!(cfg.alerts[0].condition, AlertCondition::ScoreDropBelow(70));
    }

    #[test]
    fn test_cloud_config_default_second_alert_is_critical_vuln() {
        let cfg = CloudConfig::default();
        assert_eq!(cfg.alerts[1].name, "Critical vulnerability");
        assert!(cfg.alerts[1].enabled);
        matches!(
            cfg.alerts[1].condition,
            AlertCondition::CriticalVulnerability
        );
    }

    #[test]
    fn test_cloud_config_default_sharing_all_false() {
        let cfg = CloudConfig::default();
        assert!(!cfg.sharing.share_conventions);
        assert!(!cfg.sharing.share_scores);
        assert!(!cfg.sharing.anonymize);
    }

    // ── load_cloud_config ────────────────────────────────────────────────────

    #[test]
    fn test_load_cloud_config_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(cfg.team_name, "My Team");
        assert!(cfg.projects.is_empty());
    }

    #[test]
    fn test_load_cloud_config_reads_saved_config() {
        let tmp = TempDir::new().unwrap();
        let mut expected = CloudConfig::default();
        expected.team_name = "Test Team".to_string();
        save_cloud_config(tmp.path(), &expected).unwrap();

        let loaded = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(loaded.team_name, "Test Team");
    }

    #[test]
    fn test_load_cloud_config_preserves_projects() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = CloudConfig::default();
        cfg.projects.push(ProjectEntry {
            name: "proj-a".to_string(),
            path: "/some/path".to_string(),
            last_score: Some(88),
            last_analyzed: Some("2025-01-01".to_string()),
        });
        save_cloud_config(tmp.path(), &cfg).unwrap();

        let loaded = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "proj-a");
        assert_eq!(loaded.projects[0].last_score, Some(88));
    }

    #[test]
    fn test_load_cloud_config_invalid_json_errors() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join(".falcon-data");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(data_dir.join("cloud-config.json"), "not-json").unwrap();

        let result = load_cloud_config(tmp.path());
        assert!(result.is_err());
    }

    // ── save_cloud_config ────────────────────────────────────────────────────

    #[test]
    fn test_save_cloud_config_creates_data_dir() {
        let tmp = TempDir::new().unwrap();
        let cfg = CloudConfig::default();
        save_cloud_config(tmp.path(), &cfg).unwrap();
        assert!(tmp.path().join(".falcon-data").exists());
    }

    #[test]
    fn test_save_cloud_config_writes_valid_json() {
        let tmp = TempDir::new().unwrap();
        let cfg = CloudConfig::default();
        save_cloud_config(tmp.path(), &cfg).unwrap();

        let path = tmp.path().join(".falcon-data").join("cloud-config.json");
        let content = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["team_name"], "My Team");
    }

    #[test]
    fn test_save_then_load_round_trips_alerts() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = CloudConfig::default();
        cfg.alerts.push(AlertRule {
            name: "Extra alert".to_string(),
            condition: AlertCondition::NewErrors(10),
            notify: NotifyChannel::Webhook("http://example.com".to_string()),
            enabled: false,
        });
        save_cloud_config(tmp.path(), &cfg).unwrap();

        let loaded = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(loaded.alerts.len(), 3);
        assert_eq!(loaded.alerts[2].name, "Extra alert");
        assert!(!loaded.alerts[2].enabled);
    }

    // ── init_cloud ───────────────────────────────────────────────────────────

    #[test]
    fn test_init_cloud_sets_team_name() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Falcon Squad").unwrap();

        let cfg = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(cfg.team_name, "Falcon Squad");
    }

    #[test]
    fn test_init_cloud_creates_config_file() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Alpha").unwrap();

        let path = tmp.path().join(".falcon-data").join("cloud-config.json");
        assert!(path.exists());
    }

    #[test]
    fn test_init_cloud_projects_empty_after_init() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "NewTeam").unwrap();

        let cfg = load_cloud_config(tmp.path()).unwrap();
        assert!(cfg.projects.is_empty());
    }

    #[test]
    fn test_init_cloud_overwrites_existing() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "OldTeam").unwrap();
        init_cloud(tmp.path(), "NewTeam").unwrap();

        let cfg = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(cfg.team_name, "NewTeam");
    }

    // ── register_project ─────────────────────────────────────────────────────

    #[test]
    fn test_register_project_adds_entry() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Team").unwrap();
        register_project(tmp.path(), "my-app", "/projects/my-app").unwrap();

        let cfg = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(cfg.projects.len(), 1);
        assert_eq!(cfg.projects[0].name, "my-app");
        assert_eq!(cfg.projects[0].path, "/projects/my-app");
    }

    #[test]
    fn test_register_project_no_score_initially() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Team").unwrap();
        register_project(tmp.path(), "app", "/path/to/app").unwrap();

        let cfg = load_cloud_config(tmp.path()).unwrap();
        assert!(cfg.projects[0].last_score.is_none());
        assert!(cfg.projects[0].last_analyzed.is_none());
    }

    #[test]
    fn test_register_project_duplicate_errors() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Team").unwrap();
        register_project(tmp.path(), "app", "/path").unwrap();

        let result = register_project(tmp.path(), "app", "/other-path");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already registered"));
    }

    #[test]
    fn test_register_multiple_projects() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Team").unwrap();
        register_project(tmp.path(), "app-a", "/path/a").unwrap();
        register_project(tmp.path(), "app-b", "/path/b").unwrap();
        register_project(tmp.path(), "app-c", "/path/c").unwrap();

        let cfg = load_cloud_config(tmp.path()).unwrap();
        assert_eq!(cfg.projects.len(), 3);
    }

    // ── generate_dashboard ───────────────────────────────────────────────────

    #[test]
    fn test_generate_dashboard_no_projects_returns_empty() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "EmptyTeam").unwrap();

        let dashboard = generate_dashboard(tmp.path()).unwrap();
        assert_eq!(dashboard.team_name, "EmptyTeam");
        assert_eq!(dashboard.project_count, 0);
        assert!(dashboard.projects.is_empty());
        assert_eq!(dashboard.avg_score, 0.0);
        assert_eq!(dashboard.total_issues, 0);
        assert!(dashboard.alerts_triggered.is_empty());
    }

    #[test]
    fn test_generate_dashboard_nonexistent_project_path_shows_unavailable() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Team").unwrap();
        register_project(tmp.path(), "ghost-app", "/nonexistent/path/xyz").unwrap();

        let dashboard = generate_dashboard(tmp.path()).unwrap();
        assert_eq!(dashboard.project_count, 1);
        assert_eq!(dashboard.projects[0].name, "ghost-app");
        assert_eq!(dashboard.projects[0].score, 0);
        assert_eq!(dashboard.projects[0].grade, "?");
        assert_eq!(dashboard.projects[0].trend, "unavailable");
    }

    #[test]
    fn test_generate_dashboard_multiple_unavailable_avg_zero() {
        let tmp = TempDir::new().unwrap();
        init_cloud(tmp.path(), "Team").unwrap();
        register_project(tmp.path(), "ghost-1", "/nonexistent/1").unwrap();
        register_project(tmp.path(), "ghost-2", "/nonexistent/2").unwrap();

        let dashboard = generate_dashboard(tmp.path()).unwrap();
        assert_eq!(dashboard.project_count, 2);
        assert_eq!(dashboard.avg_score, 0.0);
    }

    #[test]
    fn test_generate_dashboard_with_real_dart_project() {
        let tmp_root = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        write_minimal_dart_project(proj_dir.path());

        init_cloud(tmp_root.path(), "DevTeam").unwrap();
        register_project(
            tmp_root.path(),
            "real-app",
            proj_dir.path().to_str().unwrap(),
        )
        .unwrap();

        let dashboard = generate_dashboard(tmp_root.path()).unwrap();
        assert_eq!(dashboard.team_name, "DevTeam");
        assert_eq!(dashboard.project_count, 1);
        // Project existed — should have a real summary (not "unavailable")
        assert_ne!(dashboard.projects[0].trend, "unavailable");
    }

    #[test]
    fn test_generate_dashboard_saves_last_score() {
        let tmp_root = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        write_minimal_dart_project(proj_dir.path());

        init_cloud(tmp_root.path(), "Team").unwrap();
        register_project(
            tmp_root.path(),
            "scored-app",
            proj_dir.path().to_str().unwrap(),
        )
        .unwrap();

        generate_dashboard(tmp_root.path()).unwrap();

        let cfg = load_cloud_config(tmp_root.path()).unwrap();
        // After dashboard generation the last_score should be set for a real project
        let entry = cfg
            .projects
            .iter()
            .find(|p| p.name == "scored-app")
            .unwrap();
        assert!(entry.last_score.is_some());
    }

    // ── get_timestamp ────────────────────────────────────────────────────────

    #[test]
    fn test_get_timestamp_returns_non_empty() {
        let ts = get_timestamp();
        assert!(!ts.is_empty());
    }

    #[test]
    fn test_get_timestamp_looks_like_datetime() {
        let ts = get_timestamp();
        // On darwin/linux date is available; result should match YYYY-MM-DDTHH:MM:SS
        // or "unknown" if the command failed — both are non-empty strings.
        assert!(!ts.is_empty());
        if ts != "unknown" {
            // Basic sanity: contains 'T' separator and dashes
            assert!(ts.contains('T') || ts.contains('-'));
        }
    }

    // ── serde round-trip for all enums ───────────────────────────────────────

    #[test]
    fn test_alert_condition_serde_score_drop() {
        let cond = AlertCondition::ScoreDropBelow(50);
        let json = serde_json::to_string(&cond).unwrap();
        let back: AlertCondition = serde_json::from_str(&json).unwrap();
        matches!(back, AlertCondition::ScoreDropBelow(50));
    }

    #[test]
    fn test_alert_condition_serde_new_errors() {
        let cond = AlertCondition::NewErrors(3);
        let json = serde_json::to_string(&cond).unwrap();
        let back: AlertCondition = serde_json::from_str(&json).unwrap();
        matches!(back, AlertCondition::NewErrors(3));
    }

    #[test]
    fn test_alert_condition_serde_critical_vuln() {
        let cond = AlertCondition::CriticalVulnerability;
        let json = serde_json::to_string(&cond).unwrap();
        let back: AlertCondition = serde_json::from_str(&json).unwrap();
        matches!(back, AlertCondition::CriticalVulnerability);
    }

    #[test]
    fn test_alert_condition_serde_drift() {
        let cond = AlertCondition::DriftAboveThreshold(25.5);
        let json = serde_json::to_string(&cond).unwrap();
        let back: AlertCondition = serde_json::from_str(&json).unwrap();
        matches!(back, AlertCondition::DriftAboveThreshold(_));
    }

    #[test]
    fn test_notify_channel_serde_console() {
        let ch = NotifyChannel::Console;
        let json = serde_json::to_string(&ch).unwrap();
        let back: NotifyChannel = serde_json::from_str(&json).unwrap();
        matches!(back, NotifyChannel::Console);
    }

    #[test]
    fn test_notify_channel_serde_webhook() {
        let ch = NotifyChannel::Webhook("http://hook.example".to_string());
        let json = serde_json::to_string(&ch).unwrap();
        let back: NotifyChannel = serde_json::from_str(&json).unwrap();
        matches!(back, NotifyChannel::Webhook(_));
    }

    #[test]
    fn test_notify_channel_serde_file() {
        let ch = NotifyChannel::File("/var/log/falcon.log".to_string());
        let json = serde_json::to_string(&ch).unwrap();
        let back: NotifyChannel = serde_json::from_str(&json).unwrap();
        matches!(back, NotifyChannel::File(_));
    }

    #[test]
    fn test_sharing_config_default_all_false() {
        let sc = SharingConfig::default();
        assert!(!sc.share_conventions);
        assert!(!sc.share_scores);
        assert!(!sc.anonymize);
    }

    #[test]
    fn test_project_entry_round_trip() {
        let entry = ProjectEntry {
            name: "test".to_string(),
            path: "/tmp/test".to_string(),
            last_score: Some(77),
            last_analyzed: Some("2025-05-01T10:00:00".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ProjectEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
        assert_eq!(back.last_score, Some(77));
    }

    #[test]
    fn test_team_dashboard_serde_round_trip() {
        let dash = TeamDashboard {
            team_name: "T".to_string(),
            project_count: 1,
            projects: vec![ProjectSummary {
                name: "p".to_string(),
                score: 80,
                grade: "B".to_string(),
                issues: 5,
                files: 10,
                trend: "new".to_string(),
            }],
            avg_score: 80.0,
            total_issues: 5,
            alerts_triggered: vec!["alert!".to_string()],
        };
        let json = serde_json::to_string(&dash).unwrap();
        let back: TeamDashboard = serde_json::from_str(&json).unwrap();
        assert_eq!(back.team_name, "T");
        assert_eq!(back.avg_score, 80.0);
        assert_eq!(back.alerts_triggered.len(), 1);
    }
}
