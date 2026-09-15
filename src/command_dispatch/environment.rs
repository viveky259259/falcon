use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Doctor {
            path,
            fix,
            yes,
            dry_run,
            only,
            skip,
            channel,
            flutter_version,
            dir,
            format,
        } => {
            let opts = falcon::doctor::DoctorOptions {
                root: path,
                fix,
                yes,
                dry_run,
                only,
                skip,
                channel,
                flutter_version,
                dir,
                json: matches!(format, DoctorFormat::Json),
            };
            let code = falcon::doctor::run(&opts)?;
            std::process::exit(code);
        }
        _ => unreachable!("environment group received a non-environment command"),
    }
}
