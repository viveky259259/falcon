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

#[cfg(test)]
mod tests {
    use super::*;

    // --- PartnerCategory::Display ---

    #[test]
    fn partner_category_display_ai_tool() {
        assert_eq!(PartnerCategory::AiTool.to_string(), "AI Tool");
    }

    #[test]
    fn partner_category_display_ide() {
        assert_eq!(PartnerCategory::Ide.to_string(), "IDE");
    }

    #[test]
    fn partner_category_display_cicd() {
        assert_eq!(PartnerCategory::CiCd.to_string(), "CI/CD");
    }

    #[test]
    fn partner_category_display_cloud_platform() {
        assert_eq!(PartnerCategory::CloudPlatform.to_string(), "Cloud Platform");
    }

    #[test]
    fn partner_category_display_quality_tool() {
        assert_eq!(PartnerCategory::QualityTool.to_string(), "Quality Tool");
    }

    // --- IntegrationType::Display ---

    #[test]
    fn integration_type_display_mcp() {
        assert_eq!(IntegrationType::Mcp.to_string(), "MCP");
    }

    #[test]
    fn integration_type_display_sdk() {
        assert_eq!(IntegrationType::Sdk.to_string(), "SDK");
    }

    #[test]
    fn integration_type_display_api() {
        assert_eq!(IntegrationType::Api.to_string(), "API");
    }

    #[test]
    fn integration_type_display_cli() {
        assert_eq!(IntegrationType::Cli.to_string(), "CLI");
    }

    #[test]
    fn integration_type_display_plugin() {
        assert_eq!(IntegrationType::Plugin.to_string(), "Plugin");
    }

    // --- PartnerStatus::Display ---

    #[test]
    fn partner_status_display_certified() {
        assert_eq!(PartnerStatus::Certified.to_string(), "Certified");
    }

    #[test]
    fn partner_status_display_in_progress() {
        assert_eq!(PartnerStatus::InProgress.to_string(), "In Progress");
    }

    #[test]
    fn partner_status_display_planned() {
        assert_eq!(PartnerStatus::Planned.to_string(), "Planned");
    }

    // --- list_partners ---

    #[test]
    fn list_partners_returns_eight_entries() {
        let partners = list_partners();
        assert_eq!(partners.len(), 8);
    }

    #[test]
    fn list_partners_first_is_cursor() {
        let partners = list_partners();
        assert_eq!(partners[0].name, "Cursor");
    }

    #[test]
    fn list_partners_cursor_is_ai_tool_mcp_certified() {
        let p = &list_partners()[0];
        assert_eq!(p.category.to_string(), "AI Tool");
        assert_eq!(p.integration_type.to_string(), "MCP");
        assert_eq!(p.status, PartnerStatus::Certified);
    }

    #[test]
    fn list_partners_has_certified_partners() {
        let partners = list_partners();
        let certified: Vec<_> = partners
            .iter()
            .filter(|p| p.status == PartnerStatus::Certified)
            .collect();
        assert!(!certified.is_empty());
    }

    #[test]
    fn list_partners_has_planned_partners() {
        let partners = list_partners();
        let planned: Vec<_> = partners
            .iter()
            .filter(|p| p.status == PartnerStatus::Planned)
            .collect();
        assert!(!planned.is_empty());
    }

    #[test]
    fn list_partners_docs_urls_non_empty() {
        for p in list_partners() {
            assert!(!p.docs_url.is_empty(), "docs_url empty for {}", p.name);
            assert!(
                p.docs_url.starts_with("https://"),
                "docs_url not https for {}",
                p.name
            );
        }
    }

    #[test]
    fn list_partners_descriptions_non_empty() {
        for p in list_partners() {
            assert!(
                !p.description.is_empty(),
                "description empty for {}",
                p.name
            );
        }
    }

    // --- Partner clone ---

    #[test]
    fn partner_clone_is_equal() {
        let original = list_partners().remove(0);
        let cloned = original.clone();
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.docs_url, cloned.docs_url);
        assert_eq!(original.status, cloned.status);
    }

    // --- Partner serde round-trip ---

    #[test]
    fn partner_serde_round_trip() {
        let partners = list_partners();
        let json = serde_json::to_string(&partners).expect("serialize failed");
        let decoded: Vec<Partner> = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(decoded.len(), partners.len());
        assert_eq!(decoded[0].name, partners[0].name);
        assert_eq!(decoded[0].status, partners[0].status);
    }

    #[test]
    fn partner_category_serde_round_trip() {
        let variants = vec![
            PartnerCategory::AiTool,
            PartnerCategory::Ide,
            PartnerCategory::CiCd,
            PartnerCategory::CloudPlatform,
            PartnerCategory::QualityTool,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let decoded: PartnerCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(v.to_string(), decoded.to_string());
        }
    }

    #[test]
    fn partner_status_serde_round_trip() {
        let variants = vec![
            PartnerStatus::Certified,
            PartnerStatus::InProgress,
            PartnerStatus::Planned,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let decoded: PartnerStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn integration_type_serde_round_trip() {
        let variants = vec![
            IntegrationType::Mcp,
            IntegrationType::Sdk,
            IntegrationType::Api,
            IntegrationType::Cli,
            IntegrationType::Plugin,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let decoded: IntegrationType = serde_json::from_str(&json).unwrap();
            assert_eq!(v.to_string(), decoded.to_string());
        }
    }
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
