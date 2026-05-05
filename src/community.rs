use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

const COMMUNITY_FILE: &str = ".falcon-data/community.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleRequest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub votes: u32,
    pub status: RequestStatus,
    pub submitted_at: String,
    pub submitted_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RequestStatus {
    Open,
    Accepted,
    InProgress,
    Implemented,
    Declined,
}

impl std::fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestStatus::Open => write!(f, "open"),
            RequestStatus::Accepted => write!(f, "accepted"),
            RequestStatus::InProgress => write!(f, "in-progress"),
            RequestStatus::Implemented => write!(f, "implemented"),
            RequestStatus::Declined => write!(f, "declined"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommunityData {
    pub rule_requests: Vec<RuleRequest>,
    pub contributed_rules: Vec<ContributedRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributedRule {
    pub name: String,
    pub author: String,
    pub description: String,
    pub plugin_url: Option<String>,
    pub downloads: u32,
    pub rating: f32,
}

pub fn load_community(root: &Path) -> anyhow::Result<CommunityData> {
    let path = root.join(COMMUNITY_FILE);
    if !path.exists() {
        return Ok(CommunityData::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let data: CommunityData = serde_json::from_str(&content)?;
    Ok(data)
}

pub fn save_community(root: &Path, data: &CommunityData) -> anyhow::Result<()> {
    let data_dir = root.join(".falcon-data");
    std::fs::create_dir_all(&data_dir)?;

    let path = root.join(COMMUNITY_FILE);
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Submit a rule request.
pub fn submit_rule_request(
    root: &Path,
    name: &str,
    description: &str,
    category: &str,
) -> anyhow::Result<String> {
    let mut data = load_community(root)?;

    let id = format!("req-{:04}", data.rule_requests.len() + 1);

    let timestamp = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| {
            log::warn!("Failed to get timestamp: {}", e);
            "unknown".to_string()
        });

    data.rule_requests.push(RuleRequest {
        id: id.clone(),
        name: name.to_string(),
        description: description.to_string(),
        category: category.to_string(),
        votes: 1,
        status: RequestStatus::Open,
        submitted_at: timestamp,
        submitted_by: None,
    });

    save_community(root, &data)?;
    Ok(id)
}

/// Vote on a rule request.
pub fn vote_rule_request(root: &Path, request_id: &str) -> anyhow::Result<u32> {
    let mut data = load_community(root)?;

    let request = data
        .rule_requests
        .iter_mut()
        .find(|r| r.id == request_id)
        .ok_or_else(|| anyhow::anyhow!("Rule request '{}' not found", request_id))?;

    request.votes += 1;
    let votes = request.votes;

    save_community(root, &data)?;
    Ok(votes)
}

/// Get sample contributed rules.
pub fn sample_contributed_rules() -> Vec<ContributedRule> {
    vec![
        ContributedRule {
            name: "avoid-getx-obx-without-obs".to_string(),
            author: "flutter_community".to_string(),
            description: "Detect Obx widgets wrapping non-observable values in GetX".to_string(),
            plugin_url: Some("https://github.com/example/falcon-getx-rules".to_string()),
            downloads: 1250,
            rating: 4.5,
        },
        ContributedRule {
            name: "prefer-freezed-unions".to_string(),
            author: "dart_patterns".to_string(),
            description: "Prefer freezed unions over manual sealed class hierarchies".to_string(),
            plugin_url: Some("https://github.com/example/falcon-freezed-rules".to_string()),
            downloads: 890,
            rating: 4.2,
        },
        ContributedRule {
            name: "ensure-go-router-redirect".to_string(),
            author: "nav_expert".to_string(),
            description: "Ensure GoRouter routes have proper redirect guards for auth".to_string(),
            plugin_url: Some("https://github.com/example/falcon-router-rules".to_string()),
            downloads: 2100,
            rating: 4.8,
        },
        ContributedRule {
            name: "avoid-provider-in-dispose".to_string(),
            author: "riverpod_team".to_string(),
            description: "Don't read providers in dispose — ref may already be invalidated"
                .to_string(),
            plugin_url: Some("https://github.com/example/falcon-provider-rules".to_string()),
            downloads: 3400,
            rating: 4.9,
        },
    ]
}

pub fn print_rule_requests(requests: &[RuleRequest]) {
    println!();
    println!("  {} Rule Requests", "falcon".bright_cyan().bold());
    println!();

    if requests.is_empty() {
        println!("  No rule requests yet.");
        println!("  Submit one: falcon community request --name <rule-name> --desc <description>");
        println!();
        return;
    }

    let mut sorted = requests.to_vec();
    sorted.sort_by(|a, b| b.votes.cmp(&a.votes));

    println!(
        "  {:<10} {:<6} {:<35} {:<15} {}",
        "ID".bright_white().bold(),
        "Votes".bright_white().bold(),
        "Rule".bright_white().bold(),
        "Status".bright_white().bold(),
        "Category".bright_white().bold()
    );
    println!("  {}", "─".repeat(80));

    for req in &sorted {
        let status_color = match req.status {
            RequestStatus::Open => req.status.to_string().blue(),
            RequestStatus::Accepted => req.status.to_string().green(),
            RequestStatus::InProgress => req.status.to_string().yellow(),
            RequestStatus::Implemented => req.status.to_string().bright_green(),
            RequestStatus::Declined => req.status.to_string().red(),
        };

        println!(
            "  {:<10} {:<6} {:<35} {:<15} {}",
            req.id.dimmed(),
            format!("▲ {}", req.votes).bright_white(),
            req.name.bright_white(),
            status_color,
            req.category.dimmed()
        );
    }
    println!();
}

pub fn print_contributed_rules(rules: &[ContributedRule]) {
    println!();
    println!(
        "  {} Community-Contributed Rules",
        "falcon".bright_cyan().bold()
    );
    println!();

    for rule in rules {
        let stars = "★".repeat(rule.rating.round() as usize);
        let empty = "☆".repeat(5 - rule.rating.round() as usize);
        println!(
            "  {} {} by {} ({}{} {:.1})",
            "•".bright_white(),
            rule.name.bright_white().bold(),
            rule.author.dimmed(),
            stars.bright_yellow(),
            empty.dimmed(),
            rule.rating
        );
        println!("    {}", rule.description);
        println!(
            "    {} downloads",
            rule.downloads.to_string().bright_white()
        );
        if let Some(ref url) = rule.plugin_url {
            println!("    {}", url.dimmed());
        }
        println!();
    }
}
