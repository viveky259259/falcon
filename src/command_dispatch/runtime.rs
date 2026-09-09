use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Watch { path, config } => handle_watch(path, config)?,
        Commands::Run {
            path,
            output_dir,
            device,
            flavor,
            notify,
            webhook,
        } => handle_run(path, output_dir, device, flavor, notify, webhook)?,
        Commands::Flutter { args } => handle_flutter(args)?,
        Commands::Fvm { args } => handle_fvm(args)?,
        Commands::Agents { action } => handle_agents(action)?,
        Commands::RuntimeCheck {
            path,
            attach,
            duration,
            output,
            memory_warn_mb,
            frame_warn_ms,
            no_html,
        } => handle_runtime_check(
            path,
            attach,
            duration,
            output,
            memory_warn_mb,
            frame_warn_ms,
            no_html,
        )?,
        Commands::Live {
            path,
            attach,
            duration,
            interval,
            json,
        } => handle_live(path, attach, duration, interval, json)?,
        Commands::Devtools { action } => handle_devtools(action)?,
        Commands::Screenshot {
            path,
            attach,
            out,
            device,
            window,
            json,
        } => handle_screenshot(path, attach, out, device, window, json)?,
        Commands::Journey {
            path,
            attach,
            device,
            duration,
            interval,
            output_dir,
            no_html,
            json,
        } => handle_journey(
            path, attach, device, duration, interval, output_dir, no_html, json,
        )?,
        Commands::Trace {
            path,
            attach,
            duration,
            jank_ms,
            json,
        } => handle_trace(path, attach, duration, jank_ms, json)?,
        Commands::Workspace { path } => handle_workspace(path)?,
        Commands::Manage { action } => handle_manage(action)?,
        Commands::Mcp => handle_mcp()?,
        Commands::Api { host, port } => handle_api(host, port)?,
        _ => unreachable!("runtime command routed to the wrong handler"),
    }

    Ok(())
}
