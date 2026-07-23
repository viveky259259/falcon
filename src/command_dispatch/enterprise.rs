use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Cloud { action } => handle_cloud(action)?,
        Commands::Enterprise { action } => handle_enterprise(action)?,
        Commands::Marketplace { query } => handle_marketplace(query)?,
        Commands::Certify { path } => handle_certify(path)?,
        Commands::Partners => handle_partners()?,
        Commands::Drift { path, since, json } => handle_drift(path, since, json)?,
        _ => unreachable!("enterprise command routed to the wrong handler"),
    }

    Ok(())
}
