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
            offline,
        } => {
            let opts = falcon::doctor::DoctorOptions {
                root: path,
                fix,
                yes,
                dry_run,
                only,
                skip,
                channel: channel.map(|c| c.as_str().to_string()),
                flutter_version,
                dir,
                json: matches!(format, DoctorFormat::Json),
                // The CLI always has a terminal to print to.
                silent: false,
                offline,
            };
            let code = falcon::doctor::run(&opts)?;
            std::process::exit(code);
        }
        _ => unreachable!("environment group received a non-environment command"),
    }
}
