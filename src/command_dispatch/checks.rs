use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::CheckUnusedCode {
            path,
            format,
            output,
        } => handle_check_unused_code(path, format, output)?,
        Commands::CheckUnusedFiles {
            path,
            format,
            output,
        } => handle_check_unused_files(path, format, output)?,
        Commands::CheckDependencies {
            path,
            format,
            output,
        } => handle_check_dependencies(path, format, output)?,
        Commands::CheckAssets { path, format } => handle_check_assets_preflight(path, format)?,
        Commands::CheckA11y { path, format } => handle_check_a11y(path, format)?,
        Commands::CheckPods {
            path,
            format,
            refresh,
        } => handle_check_pods(path, format, refresh)?,
        Commands::CheckPlatformDeps {
            path,
            format,
            refresh,
            platform,
        } => handle_check_platform_deps(path, format, refresh, platform)?,
        Commands::CheckCycles { path } => handle_check_cycles(path)?,
        Commands::CheckUnusedParams {
            path,
            format,
            output,
        } => handle_check_unused_params(path, format, output)?,
        Commands::CheckDeadCode {
            path,
            format,
            output,
        } => handle_check_dead_code(path, format, output)?,
        Commands::CheckUnusedL10n {
            path,
            format,
            output,
        } => handle_check_unused_l10n(path, format, output)?,
        Commands::CheckPromotedDeps {
            path,
            format,
            output,
        } => handle_check_promoted_deps(path, format, output)?,
        Commands::CheckUnusedConfidence {
            path,
            min_confidence,
            config,
        } => handle_check_unused_confidence(path, min_confidence, config)?,
        Commands::CheckLayers { path } => handle_check_layers(path)?,
        Commands::CheckImports { path } => handle_check_imports(path)?,
        Commands::CheckWidgets { path } => handle_check_widgets(path)?,
        Commands::CheckAsync { path } => handle_check_async(path)?,
        Commands::CheckPlatform { path } => handle_check_platform(path)?,
        Commands::CheckCodegen { path } => handle_check_codegen(path)?,
        Commands::CheckPerf { path } => handle_check_perf(path)?,
        _ => unreachable!("check command routed to the wrong handler"),
    }

    Ok(())
}
