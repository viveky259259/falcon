use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Analyze {
            path,
            format,
            output,
            config,
            since,
            baseline,
            update_baseline,
            semantic,
            no_defer_to_analyzer,
            fail_on,
            preset,
            exclude_public_api: _,
        } => handle_analyze(
            path,
            format,
            output,
            config,
            since,
            baseline,
            update_baseline,
            semantic,
            no_defer_to_analyzer,
            fail_on,
            preset,
        )?,
        Commands::Smells {
            path,
            format,
            output,
            config,
            limit,
        } => handle_smells(path, format, output, config, limit)?,
        Commands::Metrics {
            path,
            format,
            output,
            config,
        } => handle_metrics(path, format, output, config)?,
        Commands::CognitiveComplexity { path, threshold } => {
            handle_cognitive_complexity(path, threshold)?
        }
        Commands::CodebaseIntel { path } => handle_codebase_intel(path)?,
        Commands::DepGraph { path, file } => handle_dep_graph(path, file)?,
        Commands::ArchMap {
            path,
            output,
            no_html,
            json,
        } => handle_arch_map(path, output, no_html, json)?,
        Commands::AiScore {
            path,
            badge,
            json,
            format,
        } => handle_ai_score(
            path,
            badge,
            json || matches!(format, Some(ScoreFormat::Json)),
        )?,
        _ => unreachable!("analysis command routed to the wrong handler"),
    }

    Ok(())
}
