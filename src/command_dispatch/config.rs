use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Init { path } => handle_init(path)?,
        Commands::Baseline { action } => handle_baseline(action)?,
        Commands::Docs { output } => handle_docs(output)?,
        Commands::Validate { path } => handle_validate(path)?,
        Commands::Explain { rule } => handle_explain(rule)?,
        Commands::Preset { action } => handle_preset(action)?,
        Commands::Update { version, list } => handle_update(version, list)?,
        Commands::StabilityContract => handle_stability_contract()?,
        Commands::DeprecationStatus => handle_deprecation_status()?,
        Commands::Suppress { action } => handle_suppress(action)?,
        Commands::SelfTune { path } => handle_self_tune(path)?,
        _ => unreachable!("config command routed to the wrong handler"),
    }

    Ok(())
}
