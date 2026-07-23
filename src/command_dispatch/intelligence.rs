use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Review {
            path,
            diff,
            format,
            strictness,
            baseline,
            update_baseline,
            semantic,
            no_defer_to_analyzer,
        } => handle_review(
            path,
            diff,
            format,
            strictness,
            baseline,
            update_baseline,
            semantic,
            no_defer_to_analyzer,
        )?,
        Commands::Ai { action } => handle_ai(action)?,
        Commands::Plugin { action } => handle_plugin(action)?,
        Commands::Dashboard { action } => handle_dashboard(action)?,
        Commands::Export {
            path,
            format,
            output,
            webhook_url,
        } => handle_export(path, format, output, webhook_url)?,
        Commands::MigrateFromDcm {
            config_path,
            output,
        } => handle_migrate_from_dcm(config_path, output)?,
        Commands::FeatureGap => handle_feature_gap()?,
        Commands::RuleDocs { format, output } => handle_rule_docs(format, output)?,
        Commands::Compare { path } => handle_compare(path)?,
        Commands::CompareReports {
            path,
            run1,
            run2,
            output,
        } => handle_compare_reports(path, run1, run2, output)?,
        Commands::CompareBranches {
            path,
            base,
            branch,
            output,
            config,
        } => handle_compare_branches(path, base, branch, output, config)?,
        Commands::Showcase {
            paths,
            format,
            output,
        } => handle_showcase(paths, format, output)?,
        Commands::PrComment {
            path,
            owner,
            repo,
            pr,
            dry_run,
            config,
        } => handle_pr_comment(path, owner, repo, pr, dry_run, config)?,
        Commands::Webhook { path, url, event } => handle_webhook(path, url, event)?,
        Commands::RefactorSim {
            path,
            scenario,
            json,
        } => handle_refactor_sim(path, scenario, json)?,
        Commands::TestGen { path, write } => handle_test_gen(path, write)?,
        Commands::VulnScan { path } => handle_vuln_scan(path)?,
        Commands::AiProfile { path } => handle_ai_profile(path)?,
        Commands::DiscoverRules { path } => handle_discover_rules(path)?,
        Commands::Learn {
            project,
            db,
            insights,
        } => handle_learn(project, db, insights)?,
        Commands::Predict { path, json } => handle_predict(path, json)?,
        Commands::UpgradeCheck { path } => handle_upgrade_check(path)?,
        Commands::AiReport {
            path,
            format,
            output,
        } => handle_ai_report(path, format, output)?,
        Commands::Provenance { path, verbose } => handle_provenance(path, verbose)?,
        Commands::Conventions { path, json } => handle_conventions(path, json)?,
        Commands::Community { action } => handle_community(action)?,
        _ => unreachable!("intelligence command routed to the wrong handler"),
    }

    Ok(())
}

fn handle_rule_docs(format: DocFormat, output: Option<PathBuf>) -> Result<()> {
    let docs = falcon::docs::rule_docs::generate_rule_docs();
    match format {
        DocFormat::Console => falcon::docs::rule_docs::print_rule_docs(&docs),
        DocFormat::Markdown => {
            let md = falcon::docs::rule_docs::generate_markdown_docs(&docs);
            match output {
                Some(out) => {
                    std::fs::write(&out, &md)?;
                    println!(
                        "  {} Rule docs written to {}",
                        "✓".green().bold(),
                        out.display()
                    );
                }
                None => print!("{}", md),
            }
        }
    }
    Ok(())
}
