use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::X { action } => {
            // The `x` namespace is the Sept 1 cutover scaffold: every entry
            // here re-dispatches into the matching legacy top-level command
            // without changing behavior or argument shapes. When the cutover
            // flips, the legacy variants will start calling
            // `falcon::cli::deprecation::warn_aliased` and eventually go away.
            let legacy = match action {
                XAction::Ai { action } => Commands::Ai { action },
                XAction::AssetAudit {
                    path,
                    output,
                    size_threshold_kb,
                    no_html,
                } => Commands::AssetAudit {
                    path,
                    output,
                    size_threshold_kb,
                    no_html,
                },
                XAction::ThemeAudit {
                    path,
                    output,
                    no_html,
                } => Commands::ThemeAudit {
                    path,
                    output,
                    no_html,
                },
                XAction::L10nCoverage {
                    path,
                    output,
                    no_html,
                } => Commands::L10nCoverage {
                    path,
                    output,
                    no_html,
                },
                XAction::DeeplinkValidate {
                    path,
                    output,
                    no_html,
                } => Commands::DeeplinkValidate {
                    path,
                    output,
                    no_html,
                },
                XAction::AnimationAudit {
                    path,
                    output,
                    no_html,
                } => Commands::AnimationAudit {
                    path,
                    output,
                    no_html,
                },
                XAction::GoldenGen {
                    path,
                    output_dir,
                    dry_run,
                    html_output,
                    no_html,
                } => Commands::GoldenGen {
                    path,
                    output_dir,
                    dry_run,
                    html_output,
                    no_html,
                },
                XAction::DepGraph { path, file } => Commands::DepGraph { path, file },
                XAction::Workspace { path } => Commands::Workspace { path },
                XAction::Docs { output } => Commands::Docs { output },
                XAction::VulnScan { path } => Commands::VulnScan { path },
                XAction::RefactorSim {
                    path,
                    scenario,
                    json,
                } => Commands::RefactorSim {
                    path,
                    scenario,
                    json,
                },
                XAction::TestGen { path, write } => Commands::TestGen { path, write },
            };
            run(Cli { command: legacy })
        }
        _ => unreachable!("experimental command routed to the wrong handler"),
    }
}
