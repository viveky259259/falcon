use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::AssetAudit {
            path,
            output,
            size_threshold_kb,
            no_html,
        } => handle_asset_audit(path, output, size_threshold_kb, no_html)?,
        Commands::ThemeAudit {
            path,
            output,
            no_html,
        } => handle_theme_audit(path, output, no_html)?,
        Commands::L10nCoverage {
            path,
            output,
            no_html,
        } => handle_l10n_coverage(path, output, no_html)?,
        Commands::DeeplinkValidate {
            path,
            output,
            no_html,
        } => handle_deeplink_validate(path, output, no_html)?,
        Commands::AnimationAudit {
            path,
            output,
            no_html,
        } => handle_animation_audit(path, output, no_html)?,
        Commands::GoldenGen {
            path,
            output_dir,
            dry_run,
            html_output,
            no_html,
        } => handle_golden_gen(path, output_dir, dry_run, html_output, no_html)?,
        _ => unreachable!("Flutter quality command routed to the wrong handler"),
    }

    Ok(())
}
