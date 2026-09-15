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
                json: json_mode(&format)?,
            };
            let code = falcon::doctor::run(&opts)?;
            std::process::exit(code);
        }
        _ => unreachable!("environment group received a non-environment command"),
    }
}

/// A toolchain diagnosis is not a set of static-analysis findings, so SARIF has
/// nothing to describe here. Reject it by name rather than quietly rendering
/// text under a format the caller did not ask for.
fn json_mode(format: &falcon::preflight::OutputFormat) -> Result<bool> {
    use falcon::preflight::OutputFormat;
    match format {
        OutputFormat::Text => Ok(false),
        OutputFormat::Json => Ok(true),
        OutputFormat::Sarif => Err(anyhow::anyhow!(
            "falcon doctor does not support --format sarif: SARIF describes \
             static-analysis findings, not a toolchain diagnosis. \
             Use --format text or --format json."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon::preflight::OutputFormat;

    #[test]
    fn text_is_the_human_format_and_json_the_machine_one() {
        assert!(!json_mode(&OutputFormat::Text).unwrap());
        assert!(json_mode(&OutputFormat::Json).unwrap());
    }

    #[test]
    fn sarif_is_refused_with_the_formats_that_do_work() {
        let err = json_mode(&OutputFormat::Sarif).unwrap_err().to_string();
        assert!(
            err.contains("sarif"),
            "the error must name the format: {}",
            err
        );
        assert!(
            err.contains("--format text") && err.contains("--format json"),
            "the error must name the supported values: {}",
            err
        );
    }
}
