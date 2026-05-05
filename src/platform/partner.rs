//! Falcon Partner Program — registry for AI tools, IDEs, and CI/CD platforms.

use colored::Colorize;
use serde::{Deserialize, Serialize};

/// A registered partner integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partner {
    pub name: String,
    pub category: PartnerCategory,
    pub integration_type: IntegrationType,
    pub description: String,
    pub status: PartnerStatus,
    pub docs_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartnerCategory {
    AiTool,
    Ide,
    CiCd,
    CloudPlatform,
    QualityTool,
}

impl std::fmt::Display for PartnerCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AiTool => write!(f, "AI Tool"),
            Self::Ide => write!(f, "IDE"),
            Self::CiCd => write!(f, "CI/CD"),
            Self::CloudPlatform => write!(f, "Cloud Platform"),
            Self::QualityTool => write!(f, "Quality Tool"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrationType {
    Mcp,
    Sdk,
    Api,
    Cli,
    Plugin,
}

impl std::fmt::Display for IntegrationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mcp => write!(f, "MCP"),
            Self::Sdk => write!(f, "SDK"),
            Self::Api => write!(f, "API"),
            Self::Cli => write!(f, "CLI"),
            Self::Plugin => write!(f, "Plugin"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PartnerStatus {
    Certified,
    InProgress,
    Planned,
}

impl std::fmt::Display for PartnerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Certified => write!(f, "Certified"),
            Self::InProgress => write!(f, "In Progress"),
            Self::Planned => write!(f, "Planned"),
        }
    }
}

/// List all partner integrations.
pub fn list_partners() -> Vec<Partner> {
    vec![
        Partner {
            name: "Cursor".to_string(),
            category: PartnerCategory::AiTool,
            integration_type: IntegrationType::Mcp,
            description: "MCP server integration — Cursor calls Falcon tools during Flutter code generation".to_string(),
            status: PartnerStatus::Certified,
            docs_url: "https://falcon.dev/partners/cursor".to_string(),
        },
        Partner {
            name: "VS Code".to_string(),
            category: PartnerCategory::Ide,
            integration_type: IntegrationType::Plugin,
            description: "LSP-based extension with real-time diagnostics, code actions, and AI provenance hints".to_string(),
            status: PartnerStatus::Certified,
            docs_url: "https://falcon.dev/partners/vscode".to_string(),
        },
        Partner {
            name: "GitHub Actions".to_string(),
            category: PartnerCategory::CiCd,
            integration_type: IntegrationType::Cli,
            description: "PR comment bot, step summaries, SARIF upload, exit code gating".to_string(),
            status: PartnerStatus::Certified,
            docs_url: "https://falcon.dev/partners/github-actions".to_string(),
        },
        Partner {
            name: "Windsurf".to_string(),
            category: PartnerCategory::AiTool,
            integration_type: IntegrationType::Mcp,
            description: "MCP server integration for in-flight Flutter code analysis".to_string(),
            status: PartnerStatus::InProgress,
            docs_url: "https://falcon.dev/partners/windsurf".to_string(),
        },
        Partner {
            name: "GitHub Copilot".to_string(),
            category: PartnerCategory::AiTool,
            integration_type: IntegrationType::Api,
            description: "HTTP API integration for Copilot-generated Flutter code validation".to_string(),
            status: PartnerStatus::InProgress,
            docs_url: "https://falcon.dev/partners/copilot".to_string(),
        },
        Partner {
            name: "GitLab CI".to_string(),
            category: PartnerCategory::CiCd,
            integration_type: IntegrationType::Cli,
            description: "GitLab CI template with CodeClimate report format".to_string(),
            status: PartnerStatus::Certified,
            docs_url: "https://falcon.dev/partners/gitlab".to_string(),
        },
        Partner {
            name: "SonarQube".to_string(),
            category: PartnerCategory::QualityTool,
            integration_type: IntegrationType::Api,
            description: "Export Falcon results to SonarQube dashboards".to_string(),
            status: PartnerStatus::Planned,
            docs_url: "https://falcon.dev/partners/sonarqube".to_string(),
        },
        Partner {
            name: "IntelliJ / Android Studio".to_string(),
            category: PartnerCategory::Ide,
            integration_type: IntegrationType::Plugin,
            description: "LSP-based plugin for JetBrains IDEs with inspections and quick fixes".to_string(),
            status: PartnerStatus::Planned,
            docs_url: "https://falcon.dev/partners/intellij".to_string(),
        },
    ]
}

/// Print partner registry.
pub fn print_partners(partners: &[Partner]) {
    println!();
    println!("  {} Partner Program", "falcon".bright_cyan().bold());
    println!();

    let certified: Vec<&Partner> = partners
        .iter()
        .filter(|p| p.status == PartnerStatus::Certified)
        .collect();
    let in_progress: Vec<&Partner> = partners
        .iter()
        .filter(|p| p.status == PartnerStatus::InProgress)
        .collect();
    let planned: Vec<&Partner> = partners
        .iter()
        .filter(|p| p.status == PartnerStatus::Planned)
        .collect();

    if !certified.is_empty() {
        println!("  {} Certified Partners:", "✓".green().bold());
        for p in &certified {
            println!(
                "    {} {:<25} [{}] via {}",
                "●".green(),
                p.name.bright_white().bold(),
                p.category.to_string().bright_cyan(),
                p.integration_type
            );
            println!("      {}", p.description.dimmed());
        }
        println!();
    }

    if !in_progress.is_empty() {
        println!("  {} In Progress:", "▸".yellow());
        for p in &in_progress {
            println!(
                "    {} {:<25} [{}] via {}",
                "●".yellow(),
                p.name.bright_white(),
                p.category.to_string().bright_cyan(),
                p.integration_type
            );
        }
        println!();
    }

    if !planned.is_empty() {
        println!("  {} Planned:", "○".dimmed());
        for p in &planned {
            println!("    {} {:<25} [{}]", "○".dimmed(), p.name, p.category);
        }
        println!();
    }
}
