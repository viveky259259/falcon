use super::snapshot::AnalysisSnapshot;
use colored::Colorize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RuleImpact {
    pub rule: String,
    pub total_triggers: usize,
    pub trend: ImpactTrend,
    pub signal_score: f64,
}

#[derive(Debug, Clone)]
pub enum ImpactTrend {
    Increasing,
    Stable,
    Decreasing,
}

#[derive(Debug)]
pub struct AutoTuneRecommendation {
    pub rule: String,
    pub action: TuneAction,
    pub reason: String,
    pub confidence: f64,
}

#[derive(Debug)]
pub enum TuneAction {
    Disable,
    ReduceSeverity,
    IncreaseSeverity,
    KeepAsIs,
}

impl TuneAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disable => "DISABLE",
            Self::ReduceSeverity => "REDUCE",
            Self::IncreaseSeverity => "INCREASE",
            Self::KeepAsIs => "KEEP",
        }
    }
}

/// Measure rule impact across snapshot history.
pub fn measure_rule_impact(history: &[AnalysisSnapshot]) -> Vec<RuleImpact> {
    if history.is_empty() {
        return Vec::new();
    }

    let mut all_rules: HashMap<String, Vec<usize>> = HashMap::new();

    for snap in history {
        for (rule, count) in &snap.rule_counts {
            all_rules.entry(rule.clone()).or_default().push(*count);
        }
    }

    let mut impacts: Vec<RuleImpact> = all_rules
        .into_iter()
        .map(|(rule, counts)| {
            let total: usize = counts.iter().sum();
            let trend = if counts.len() >= 2 {
                let first_half: usize = counts[..counts.len() / 2].iter().sum();
                let second_half: usize = counts[counts.len() / 2..].iter().sum();
                if second_half > first_half + 5 {
                    ImpactTrend::Increasing
                } else if first_half > second_half + 5 {
                    ImpactTrend::Decreasing
                } else {
                    ImpactTrend::Stable
                }
            } else {
                ImpactTrend::Stable
            };

            let avg = total as f64 / counts.len().max(1) as f64;
            let signal_score = if avg > 50.0 {
                20.0
            } else if avg > 20.0 {
                50.0
            } else if avg > 5.0 {
                80.0
            } else {
                95.0
            };

            RuleImpact {
                rule,
                total_triggers: total,
                trend,
                signal_score,
            }
        })
        .collect();

    impacts.sort_by_key(|e| std::cmp::Reverse(e.total_triggers));
    impacts
}

/// Generate auto-tune recommendations based on rule impact.
pub fn auto_tune_recommendations(impacts: &[RuleImpact]) -> Vec<AutoTuneRecommendation> {
    let mut recs = Vec::new();

    for impact in impacts {
        if impact.signal_score < 30.0 {
            recs.push(AutoTuneRecommendation {
                rule: impact.rule.clone(),
                action: TuneAction::Disable,
                reason: format!(
                    "Rule fires {} times on average — likely too noisy. Signal score: {:.0}%.",
                    impact.total_triggers, impact.signal_score
                ),
                confidence: 0.7,
            });
        } else if impact.signal_score < 50.0 {
            recs.push(AutoTuneRecommendation {
                rule: impact.rule.clone(),
                action: TuneAction::ReduceSeverity,
                reason: format!(
                    "Rule fires frequently ({} total). Consider reducing severity to info.",
                    impact.total_triggers
                ),
                confidence: 0.6,
            });
        } else if impact.total_triggers <= 3 && matches!(impact.trend, ImpactTrend::Stable) {
            recs.push(AutoTuneRecommendation {
                rule: impact.rule.clone(),
                action: TuneAction::IncreaseSeverity,
                reason: format!(
                    "Rule fires rarely ({} total). Low noise — consider promoting to error.",
                    impact.total_triggers
                ),
                confidence: 0.5,
            });
        }
    }

    recs
}

pub fn print_rule_impact(impacts: &[RuleImpact]) {
    println!();
    println!(
        "  {} Rule Impact Analysis ({} rules)",
        "falcon".bright_cyan().bold(),
        impacts.len()
    );
    println!();

    println!(
        "  {:<40} {:>6} {:>10} {:>8}",
        "Rule".bright_white().bold(),
        "Count".bright_white().bold(),
        "Trend".bright_white().bold(),
        "Signal".bright_white().bold()
    );
    println!("  {}", "─".repeat(68));

    for impact in impacts.iter().take(20) {
        let trend_icon = match impact.trend {
            ImpactTrend::Increasing => "↑".red(),
            ImpactTrend::Stable => "→".dimmed(),
            ImpactTrend::Decreasing => "↓".green(),
        };
        let _signal_color = if impact.signal_score >= 80.0 {
            colored::Color::Green
        } else if impact.signal_score >= 50.0 {
            colored::Color::Yellow
        } else {
            colored::Color::Red
        };
        println!(
            "  {:<40} {:>6} {:>10} {:>7.0}%",
            impact.rule, impact.total_triggers, trend_icon, impact.signal_score,
        );
    }
    println!();
}

pub fn print_recommendations(recs: &[AutoTuneRecommendation]) {
    if recs.is_empty() {
        println!(
            "  {} No auto-tune recommendations — current configuration looks good.",
            "✓".green().bold()
        );
        return;
    }

    println!();
    println!(
        "  {} Auto-Tune Recommendations ({} suggestions)",
        "falcon".bright_cyan().bold(),
        recs.len()
    );
    println!();

    for rec in recs {
        let action_color = match rec.action {
            TuneAction::Disable => colored::Color::Red,
            TuneAction::ReduceSeverity => colored::Color::Yellow,
            TuneAction::IncreaseSeverity => colored::Color::Green,
            TuneAction::KeepAsIs => colored::Color::White,
        };
        println!(
            "  {} {} — {} (confidence: {:.0}%)",
            rec.action.label().color(action_color).bold(),
            rec.rule.bright_white(),
            rec.reason,
            rec.confidence * 100.0
        );
    }
    println!();
}
