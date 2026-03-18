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
    let mut config = CloudConfig::default();
    config.team_name = team_name.to_string();
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
                score: 0, grade: "?".to_string(), issues: 0, files: 0,
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
                    if !alert.enabled { continue; }
                    match alert.condition {
                        AlertCondition::ScoreDropBelow(threshold) => {
                            if score.overall < threshold {
                                alerts.push(format!("[{}] {} — score {}/100 below threshold {}",
                                    project.name, alert.name, score.overall, threshold));
                            }
                        }
                        AlertCondition::NewErrors(threshold) => {
                            if score.total_issues > threshold {
                                alerts.push(format!("[{}] {} — {} issues exceed threshold {}",
                                    project.name, alert.name, score.total_issues, threshold));
                            }
                        }
                        AlertCondition::CriticalVulnerability => {
                            if score.security.score < 80 {
                                alerts.push(format!("[{}] {} — security score {}/100, check for hardcoded credentials",
                                    project.name, alert.name, score.security.score));
                            }
                        }
                        AlertCondition::DriftAboveThreshold(max_drift) => {
                            if let Ok(drift) = crate::ai_score::drift::detect_drift(&project_path, None) {
                                let drift_pct = 100.0 - drift.drift_score;
                                if drift_pct > max_drift {
                                    alerts.push(format!("[{}] {} — {:.0}% drift exceeds threshold {:.0}%",
                                        project.name, alert.name, drift_pct, max_drift));
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
                    score: 0, grade: "?".to_string(), issues: 0, files: 0,
                    trend: "error".to_string(),
                });
            }
        }
    }

    save_cloud_config(root, &config)?;

    let scores: Vec<f64> = summaries.iter().filter(|s| s.score > 0).map(|s| s.score as f64).collect();
    let avg = if scores.is_empty() { 0.0 } else { scores.iter().sum::<f64>() / scores.len() as f64 };

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
            "  {:<25} {:<8} {:<6} {:<8} {:<8} {}",
            "Project", "Score", "Grade", "Issues", "Files", "Trend"
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
                p.name.bright_white(), score_color, p.grade, p.issues, p.files, p.trend
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
