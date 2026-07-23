use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Trends { path, last } => handle_trends(path, last)?,
        Commands::RuleImpact { path } => handle_rule_impact(path)?,
        Commands::Benchmark { path } => handle_benchmark(path)?,
        Commands::History { path } => handle_history(path)?,
        Commands::PerfTrack {
            path,
            history,
            last,
        } => handle_perf_track(path, history, last)?,
        Commands::BenchmarkDb {
            path,
            tool,
            summary,
        } => handle_benchmark_db(path, tool, summary)?,
        Commands::FixTrack {
            path,
            rule,
            outcome,
            file,
            report,
        } => handle_fix_track(path, rule, outcome, file, report)?,
        Commands::ScoreTrack {
            path,
            history,
            last,
        } => handle_score_track(path, history, last)?,
        Commands::Fix {
            path,
            preview,
            config,
        } => handle_fix(path, preview, config)?,
        _ => unreachable!("tracking command routed to the wrong handler"),
    }

    Ok(())
}
