use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use falcon::config::{FalconConfig, Severity};
use falcon::incremental::baseline::Baseline;
use falcon::incremental::cache::AnalysisCache;
use falcon::incremental::dep_graph::DependencyGraph;
use falcon::reporters::checkstyle::CheckstyleReporter;
use falcon::reporters::codeclimate::CodeClimateReporter;
use falcon::reporters::console::ConsoleReporter;
use falcon::reporters::html::HtmlReporter;
use falcon::reporters::json::JsonReporter;
use falcon::reporters::sarif::SarifReporter;
use falcon::reporters::sonar::SonarReporter;
use falcon::reporters::Reporter;
use falcon::Falcon;
use std::path::{Path, PathBuf};
use std::process;

mod cli_args;
use cli_args::*;

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{}: {}", "error".red(), e);
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Analyze {
            path,
            format,
            output,
            config,
            since,
            baseline,
            fail_on,
            preset,
            exclude_public_api: _,
        } => handle_analyze(
            path, format, output, config, since, baseline, fail_on, preset,
        )?,
        Commands::Smells {
            path,
            format,
            output,
            config,
            limit,
        } => handle_smells(path, format, output, config, limit)?,
        Commands::Metrics {
            path,
            format,
            output,
            config,
        } => handle_metrics(path, format, output, config)?,
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
        Commands::Init { path } => handle_init(path)?,
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

        Commands::Baseline { action } => handle_baseline(action)?,
        Commands::DepGraph { path, file } => handle_dep_graph(path, file)?,
        Commands::Workspace { path } => handle_workspace(path)?,
        Commands::Docs { output } => handle_docs(output)?,
        Commands::Validate { path } => handle_validate(path)?,
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
        Commands::Explain { rule } => handle_explain(rule)?,
        Commands::Ai { action } => handle_ai(action)?,
        Commands::Fix {
            path,
            preview,
            config,
        } => handle_fix(path, preview, config)?,
        Commands::CheckUnusedConfidence {
            path,
            min_confidence,
            config,
        } => handle_check_unused_confidence(path, min_confidence, config)?,
        Commands::CheckLayers { path } => handle_check_layers(path)?,
        Commands::CheckImports { path } => handle_check_imports(path)?,
        Commands::CognitiveComplexity { path, threshold } => {
            handle_cognitive_complexity(path, threshold)?
        }
        Commands::CheckWidgets { path } => handle_check_widgets(path)?,
        Commands::CheckAsync { path } => handle_check_async(path)?,
        Commands::Review {
            path,
            diff,
            strictness,
        } => handle_review(path, diff, strictness)?,
        Commands::CodebaseIntel { path } => handle_codebase_intel(path)?,
        Commands::Plugin { action } => handle_plugin(action)?,
        Commands::Preset { action } => handle_preset(action)?,
        Commands::Dashboard { action } => handle_dashboard(action)?,
        Commands::Trends { path, last } => handle_trends(path, last)?,
        Commands::RuleImpact { path } => handle_rule_impact(path)?,
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
        Commands::Benchmark { path } => handle_benchmark(path)?,
        Commands::RuleDocs { format, output } => match format {
            DocFormat::Console => {
                let docs = falcon::docs::rule_docs::generate_rule_docs();
                falcon::docs::rule_docs::print_rule_docs(&docs);
            }
            DocFormat::Markdown => {
                let docs = falcon::docs::rule_docs::generate_rule_docs();
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
        },
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
        Commands::History { path } => handle_history(path)?,
        Commands::Update { version, list } => handle_update(version, list)?,
        Commands::Showcase {
            paths,
            format,
            output,
        } => handle_showcase(paths, format, output)?,
        Commands::StabilityContract => handle_stability_contract()?,
        Commands::DeprecationStatus => handle_deprecation_status()?,
        Commands::PerfTrack {
            path,
            history,
            last,
        } => handle_perf_track(path, history, last)?,
        Commands::Suppress { action } => handle_suppress(action)?,
        Commands::Manage { action } => handle_manage(action)?,
        Commands::Mcp => handle_mcp()?,
        Commands::PrComment {
            path,
            owner,
            repo,
            pr,
            dry_run,
            config,
        } => handle_pr_comment(path, owner, repo, pr, dry_run, config)?,
        Commands::Webhook { path, url, event } => handle_webhook(path, url, event)?,
        Commands::BenchmarkDb {
            path,
            tool,
            summary,
        } => handle_benchmark_db(path, tool, summary)?,
        Commands::RefactorSim {
            path,
            scenario,
            json,
        } => handle_refactor_sim(path, scenario, json)?,
        Commands::TestGen { path, write } => handle_test_gen(path, write)?,
        Commands::VulnScan { path } => handle_vuln_scan(path)?,
        Commands::AiProfile { path } => handle_ai_profile(path)?,
        Commands::DiscoverRules { path } => handle_discover_rules(path)?,
        Commands::FixTrack {
            path,
            rule,
            outcome,
            file,
            report,
        } => handle_fix_track(path, rule, outcome, file, report)?,
        Commands::Learn {
            project,
            db,
            insights,
        } => handle_learn(project, db, insights)?,
        Commands::Predict { path, json } => handle_predict(path, json)?,
        Commands::UpgradeCheck { path } => handle_upgrade_check(path)?,
        Commands::Cloud { action } => handle_cloud(action)?,
        Commands::Enterprise { action } => handle_enterprise(action)?,
        Commands::Marketplace { query } => handle_marketplace(query)?,
        Commands::Certify { path } => handle_certify(path)?,
        Commands::Partners => handle_partners()?,
        Commands::CheckPlatform { path } => handle_check_platform(path)?,
        Commands::CheckCodegen { path } => handle_check_codegen(path)?,
        Commands::CheckPerf { path } => handle_check_perf(path)?,
        Commands::Api { host, port } => handle_api(host, port)?,
        Commands::Drift { path, since, json } => handle_drift(path, since, json)?,
        Commands::SelfTune { path } => handle_self_tune(path)?,
        Commands::ScoreTrack {
            path,
            history,
            last,
        } => handle_score_track(path, history, last)?,
        Commands::AiScore { path, badge, json } => handle_ai_score(path, badge, json)?,
        Commands::AiReport {
            path,
            format,
            output,
        } => handle_ai_report(path, format, output)?,
        Commands::Provenance { path, verbose } => handle_provenance(path, verbose)?,
        Commands::Conventions { path, json } => handle_conventions(path, json)?,
        Commands::Community { action } => handle_community(action)?,
        Commands::X { action } => {
            // The `x` namespace is the Sept 1 cutover scaffold: every entry
            // here re-dispatches into the matching legacy top-level command
            // without changing behavior or argument shapes. When the cutover
            // flips, the legacy variants will start calling
            // `falcon::cli::deprecation::warn_aliased` and eventually go away.
            let legacy = match action {
                XAction::AssetAudit {
                    path,
                    output,
                    size_threshold_kb,
                    no_html,
                } => Commands::AssetAudit {
                    path,
                    output,
                    size_threshold_kb,
                    no_html,
                },
                XAction::ThemeAudit {
                    path,
                    output,
                    no_html,
                } => Commands::ThemeAudit {
                    path,
                    output,
                    no_html,
                },
                XAction::L10nCoverage {
                    path,
                    output,
                    no_html,
                } => Commands::L10nCoverage {
                    path,
                    output,
                    no_html,
                },
                XAction::DeeplinkValidate {
                    path,
                    output,
                    no_html,
                } => Commands::DeeplinkValidate {
                    path,
                    output,
                    no_html,
                },
                XAction::AnimationAudit {
                    path,
                    output,
                    no_html,
                } => Commands::AnimationAudit {
                    path,
                    output,
                    no_html,
                },
                XAction::GoldenGen {
                    path,
                    output_dir,
                    dry_run,
                    html_output,
                    no_html,
                } => Commands::GoldenGen {
                    path,
                    output_dir,
                    dry_run,
                    html_output,
                    no_html,
                },
                XAction::DepGraph { path, file } => Commands::DepGraph { path, file },
                XAction::Workspace { path } => Commands::Workspace { path },
                XAction::Docs { output } => Commands::Docs { output },
                XAction::VulnScan { path } => Commands::VulnScan { path },
                XAction::RefactorSim {
                    path,
                    scenario,
                    json,
                } => Commands::RefactorSim {
                    path,
                    scenario,
                    json,
                },
                XAction::TestGen { path, write } => Commands::TestGen { path, write },
            };
            return run(Cli { command: legacy });
        }
    }

    Ok(())
}

fn handle_devtools(action: DevtoolsAction) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    match action {
        DevtoolsAction::Memory { path, attach, json } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_memory_report(
                &client, &vm_uri,
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_memory_report(&report);
            }
        }
        DevtoolsAction::Network {
            path,
            attach,
            duration,
            json,
        } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_network_report(
                &client,
                &vm_uri,
                std::time::Duration::from_secs(duration),
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_network_report(&report);
            }
        }
        DevtoolsAction::Performance {
            path,
            attach,
            duration,
            json,
        } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_performance_report(
                &client,
                &vm_uri,
                std::time::Duration::from_secs(duration),
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_performance_report(&report);
            }
        }
        DevtoolsAction::Profiler {
            path,
            attach,
            duration,
            json,
        } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_profiler_report(
                &client,
                &vm_uri,
                std::time::Duration::from_secs(duration),
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_profiler_report(&report);
            }
        }
        DevtoolsAction::Debugger {
            path,
            attach,
            action,
            json,
        } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let action = action.map(|action| match action {
                DebuggerActionArg::Pause => falcon::runtime::tools::DebuggerAction::Pause,
                DebuggerActionArg::Resume => falcon::runtime::tools::DebuggerAction::Resume,
                DebuggerActionArg::StepOver => falcon::runtime::tools::DebuggerAction::StepOver,
                DebuggerActionArg::StepIn => falcon::runtime::tools::DebuggerAction::StepIn,
                DebuggerActionArg::StepOut => falcon::runtime::tools::DebuggerAction::StepOut,
            });
            let report = rt.block_on(falcon::runtime::tools::collect_debugger_report(
                &client, &vm_uri, action,
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_debugger_report(&report);
            }
        }
        DevtoolsAction::Logging {
            path,
            attach,
            duration,
            json,
        } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_logging_report(
                &client,
                &vm_uri,
                std::time::Duration::from_secs(duration),
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_logging_report(&report);
            }
        }
        DevtoolsAction::Rebuilds { path, attach, json } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_rebuilds_report(
                &client, &vm_uri,
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_rebuilds_report(&report);
            }
        }
        DevtoolsAction::Inspector { path, attach, json } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_inspector_report(
                &client, &vm_uri,
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_inspector_report(&report);
            }
        }
        DevtoolsAction::Reload { path, attach, json } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_reload_report(
                &client,
                &vm_uri,
                falcon::runtime::tools::ReloadMode::HotReload,
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_reload_report(&report);
            }
            if !report.success {
                process::exit(1);
            }
        }
        DevtoolsAction::Restart { path, attach, json } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_reload_report(
                &client,
                &vm_uri,
                falcon::runtime::tools::ReloadMode::HotRestart,
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_reload_report(&report);
            }
            if !report.success {
                process::exit(1);
            }
        }
        DevtoolsAction::Screenshot {
            path,
            attach,
            out,
            device,
            json,
        } => {
            let (vm_uri, client) = rt.block_on(falcon::runtime::tools::connect_client(
                &path,
                attach.as_deref(),
            ))?;
            let report = rt.block_on(falcon::runtime::tools::collect_screenshot(
                &client,
                &vm_uri,
                &out,
                &path,
                device.as_deref(),
            ))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::runtime::tools::print_screenshot_report(&report);
            }
        }
    }
    Ok(())
}

// CLI command handlers take all their flags as parameters by design.
#[allow(clippy::too_many_arguments)]
fn handle_analyze(
    path: PathBuf,
    format: OutputFormat,
    output: PathBuf,
    config: Option<PathBuf>,
    since: Option<String>,
    baseline: bool,
    fail_on: FailLevel,
    preset: Option<String>,
) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let mut falcon_config = FalconConfig::load(config_path)?;

    if let Some(ref preset_name) = preset {
        match falcon::plugins::presets::get_preset(preset_name) {
            Some(p) => {
                falcon_config.rules = p.rules;
                eprintln!(
                    "  {} Using preset '{}' ({} rules)",
                    "▸".bright_cyan(),
                    preset_name.bright_white(),
                    falcon_config.rules.len()
                );
            }
            None => {
                eprintln!(
                            "Unknown preset '{}'. Available: recommended, strict, flutter, riverpod, bloc, performance, ai-generated",
                            preset_name
                        );
                process::exit(1);
            }
        }
    }

    let falcon = Falcon::new(falcon_config.clone())?;

    let report = if let Some(ref git_ref) = since {
        run_incremental(&falcon, &path, git_ref, &falcon_config)?
    } else {
        falcon.analyze(&path)?
    };

    let mut issues = report.issues;

    if baseline {
        let bl = Baseline::load(&path)?;
        issues = bl.filter_new_issues(issues, &path);
    }

    let final_report = falcon::reporters::AnalysisReport {
        issues,
        metrics: report.metrics,
        file_count: report.file_count,
        project_path: report.project_path,
    };

    get_reporter(&format, &output).report_analysis(&final_report);

    // Auto-save snapshot for history tracking
    let snap_root = if path.is_dir() {
        path.clone()
    } else {
        path.parent().unwrap_or(&path).to_path_buf()
    };
    let snapshot =
        falcon::dashboard::snapshot::AnalysisSnapshot::capture(&final_report, &snap_root);
    if let Err(e) = falcon::dashboard::snapshot::save_snapshot(&snap_root, &snapshot) {
        log::debug!("Could not save snapshot: {}", e);
    }

    if should_fail(&final_report, &fail_on) {
        process::exit(1);
    }
    Ok(())
}

fn handle_smells(
    path: PathBuf,
    format: OutputFormat,
    output: PathBuf,
    config: Option<PathBuf>,
    limit: usize,
) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let falcon_config = FalconConfig::load(config_path)?;
    let falcon = Falcon::new(falcon_config.clone())?;

    // 1. Full analysis (rules + metrics)
    let report = falcon.analyze(&path)?;
    let mut all_issues = report.issues;

    // 2. Dead-code-path detector (in case the rule is gated off)
    let exclude: Vec<glob::Pattern> = falcon_config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();
    all_issues.extend(falcon::resolver::dead_code::detect_dead_code(
        &path, &exclude,
    ));

    // 3. Unused files (so dead-folder rollup has data to work from)
    let resolver = falcon::resolver::ProjectResolver::new(&path, &falcon_config)?;
    let unused_file_issues = resolver.find_unused_files().unwrap_or_default();
    let unused_set: std::collections::HashSet<std::path::PathBuf> =
        unused_file_issues.iter().map(|i| i.file.clone()).collect();
    all_issues.extend(unused_file_issues);

    // 4. Dead-folder rollup
    let dead_folders = falcon::smells::dead_folders::find_dead_folders(&path, &unused_set);

    // 5. Categorize and print
    let summary = falcon::smells::SmellsSummary::from_issues(&all_issues, dead_folders);
    print_smells_summary(&summary, &path, limit, &format, &output);

    if !summary.security_smells.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_metrics(
    path: PathBuf,
    format: OutputFormat,
    output: PathBuf,
    config: Option<PathBuf>,
) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let falcon_config = FalconConfig::load(config_path)?;
    let falcon = Falcon::new(falcon_config)?;
    let metrics = falcon.calculate_metrics(&path)?;

    get_reporter(&format, &output).report_metrics(&metrics);
    Ok(())
}

fn handle_check_unused_code(path: PathBuf, format: OutputFormat, output: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let falcon = Falcon::new(config)?;
    let issues = falcon.check_unused_code(&path)?;

    get_reporter(&format, &output).report_issues(&issues);

    if !issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_check_unused_files(path: PathBuf, format: OutputFormat, output: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let falcon = Falcon::new(config)?;
    let issues = falcon.check_unused_files(&path)?;

    get_reporter(&format, &output).report_issues(&issues);

    if !issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_check_dependencies(path: PathBuf, format: OutputFormat, output: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let falcon = Falcon::new(config)?;
    let issues = falcon.check_dependencies(&path)?;

    get_reporter(&format, &output).report_issues(&issues);

    if !issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_init(path: PathBuf) -> Result<()> {
    falcon::init_config(&path)?;
    println!("Created falcon.yaml in {}", path.display());
    Ok(())
}

fn handle_watch(path: PathBuf, config: Option<PathBuf>) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let falcon_config = FalconConfig::load(config_path)?;
    falcon::incremental::watcher::watch(&path, falcon_config)?;
    Ok(())
}

fn handle_run(
    path: PathBuf,
    output_dir: PathBuf,
    device: Option<String>,
    flavor: Option<String>,
    notify: bool,
    webhook: Option<String>,
) -> Result<()> {
    // Resolve output_dir relative to path when it is the default "."
    let resolved_output = if output_dir.as_os_str() == "." {
        path.clone()
    } else {
        output_dir
    };

    let config = falcon::flutter_run::FlutterRunConfig {
        project_path: path,
        output_dir: resolved_output,
        device,
        flavor,
        notify,
        webhook,
    };

    let report = falcon::flutter_run::run_flutter_app(&config)?;

    if report.has_errors() {
        process::exit(1);
    }
    Ok(())
}

fn handle_flutter(args: Vec<String>) -> Result<()> {
    let status = std::process::Command::new("flutter")
        .args(&args)
        .status()
        .map_err(|e| {
            anyhow::anyhow!("failed to invoke `flutter`: {e}. Is the Flutter SDK on your PATH?")
        })?;
    process::exit(status.code().unwrap_or(1));
}

fn handle_fvm(args: Vec<String>) -> Result<()> {
    let status = std::process::Command::new("fvm")
                .args(&args)
                .status()
                .map_err(|e| anyhow::anyhow!("failed to invoke `fvm`: {e}. Install FVM (https://fvm.app) or ensure it is on your PATH."))?;
    process::exit(status.code().unwrap_or(1));
}

fn handle_runtime_check(
    path: PathBuf,
    attach: Option<String>,
    duration: u64,
    output: PathBuf,
    memory_warn_mb: f64,
    frame_warn_ms: f64,
    no_html: bool,
) -> Result<()> {
    let config = falcon::runtime::RuntimeCheckConfig {
        project_path: path.clone(),
        duration: std::time::Duration::from_secs(duration),
        attach_uri: attach,
        html_output: if no_html { None } else { Some(output.clone()) },
        thresholds: falcon::runtime::RuntimeThresholds {
            memory_warn_mb,
            frame_warn_ms,
            ..Default::default()
        },
    };

    let rt = tokio::runtime::Runtime::new()?;
    let report = rt.block_on(falcon::runtime::run_runtime_check(&config))?;

    // Console output.
    falcon::runtime::print_console_report(&report);

    // HTML output.
    if !no_html {
        falcon::runtime::write_html_report(&report, &output)?;
        eprintln!(
            "  {} HTML report written to {}",
            "✓".green().bold(),
            output.display().to_string().bright_white()
        );
    }

    if report.error_count() > 0 {
        process::exit(1);
    }
    Ok(())
}

fn handle_live(
    path: PathBuf,
    attach: Option<String>,
    duration: u64,
    interval: u64,
    json: bool,
) -> Result<()> {
    let config = falcon::runtime::live::LiveConfig {
        project_path: path,
        attach_uri: attach,
        duration: std::time::Duration::from_secs(duration),
        interval: std::time::Duration::from_secs(interval.max(1)),
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new()?;
    let report = rt.block_on(falcon::runtime::live::run_live_session(&config))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    if report
        .issues
        .iter()
        .any(|issue| issue.severity == falcon::runtime::live::LiveIssueSeverity::Error)
    {
        process::exit(1);
    }
    Ok(())
}

fn handle_asset_audit(
    path: PathBuf,
    output: PathBuf,
    size_threshold_kb: u64,
    no_html: bool,
) -> Result<()> {
    eprintln!(
        "  {} Scanning assets in {} …",
        "▸".bright_cyan(),
        path.display()
    );
    let report = falcon::asset_audit::audit_assets_with_threshold(&path, size_threshold_kb)?;
    falcon::asset_audit::print_asset_report(&report);
    if !no_html {
        falcon::asset_audit::write_asset_html_report(&report, &output)?;
        eprintln!(
            "  {} HTML report → {}",
            "✓".green().bold(),
            output.display()
        );
    }
    if report.score < 60 {
        process::exit(1);
    }
    Ok(())
}

fn handle_theme_audit(path: PathBuf, output: PathBuf, no_html: bool) -> Result<()> {
    eprintln!(
        "  {} Auditing theme consistency in {} …",
        "▸".bright_cyan(),
        path.display()
    );
    let report = falcon::theme_audit::audit_theme(&path)?;
    falcon::theme_audit::print_theme_report(&report);
    if !no_html {
        falcon::theme_audit::write_theme_html_report(&report, &output)?;
        eprintln!(
            "  {} HTML report → {}",
            "✓".green().bold(),
            output.display()
        );
    }
    if report.score < 60 {
        process::exit(1);
    }
    Ok(())
}

fn handle_l10n_coverage(path: PathBuf, output: PathBuf, no_html: bool) -> Result<()> {
    eprintln!(
        "  {} Analysing localization coverage in {} …",
        "▸".bright_cyan(),
        path.display()
    );
    let report = falcon::l10n_coverage::analyze_l10n_coverage(&path)?;
    falcon::l10n_coverage::print_l10n_report(&report);
    if !no_html {
        falcon::l10n_coverage::write_l10n_html_report(&report, &output)?;
        eprintln!(
            "  {} HTML report → {}",
            "✓".green().bold(),
            output.display()
        );
    }
    if report.score < 60 {
        process::exit(1);
    }
    Ok(())
}

fn handle_deeplink_validate(path: PathBuf, output: PathBuf, no_html: bool) -> Result<()> {
    eprintln!(
        "  {} Validating deep links in {} …",
        "▸".bright_cyan(),
        path.display()
    );
    let report = falcon::deeplink::validate_deeplinks(&path)?;
    falcon::deeplink::print_deeplink_report(&report);
    if !no_html {
        falcon::deeplink::write_deeplink_html_report(&report, &output)?;
        eprintln!(
            "  {} HTML report → {}",
            "✓".green().bold(),
            output.display()
        );
    }
    if report.score < 60 {
        process::exit(1);
    }
    Ok(())
}

fn handle_animation_audit(path: PathBuf, output: PathBuf, no_html: bool) -> Result<()> {
    eprintln!(
        "  {} Auditing animations in {} …",
        "▸".bright_cyan(),
        path.display()
    );
    let report = falcon::animation_audit::audit_animations(&path)?;
    falcon::animation_audit::print_animation_report(&report);
    if !no_html {
        falcon::animation_audit::write_animation_html_report(&report, &output)?;
        eprintln!(
            "  {} HTML report → {}",
            "✓".green().bold(),
            output.display()
        );
    }
    if report.score < 60 {
        process::exit(1);
    }
    Ok(())
}

fn handle_golden_gen(
    path: PathBuf,
    output_dir: PathBuf,
    dry_run: bool,
    html_output: PathBuf,
    no_html: bool,
) -> Result<()> {
    if dry_run {
        eprintln!(
            "  {} Dry-run: discovering widgets in {} …",
            "▸".bright_cyan(),
            path.display()
        );
    } else {
        eprintln!(
            "  {} Generating golden tests in {} …",
            "▸".bright_cyan(),
            path.display()
        );
    }
    let report = falcon::golden_gen::generate_golden_tests(&path, &output_dir, dry_run)?;
    falcon::golden_gen::print_golden_report(&report);
    if !no_html {
        falcon::golden_gen::write_golden_html_report(&report, &html_output)?;
        eprintln!(
            "  {} HTML report → {}",
            "✓".green().bold(),
            html_output.display()
        );
    }
    Ok(())
}

fn handle_dep_graph(path: PathBuf, file: Option<PathBuf>) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let graph = DependencyGraph::build(&path, &exclude);

    if let Some(target) = file {
        let abs = if target.is_absolute() {
            target.clone()
        } else {
            path.join(&target)
        };

        println!(
            "{} Dependencies for: {}",
            "→".bright_cyan(),
            target.display()
        );

        if let Some(imports) = graph.imports.get(&abs) {
            println!("\n  {} ({}):", "Imports".bright_green(), imports.len());
            for imp in imports {
                let rel = imp.strip_prefix(&path).unwrap_or(imp);
                println!("    {}", rel.display());
            }
        }

        if let Some(deps) = graph.dependents.get(&abs) {
            println!("\n  {} ({}):", "Depended on by".bright_yellow(), deps.len());
            for dep in deps {
                let rel = dep.strip_prefix(&path).unwrap_or(dep);
                println!("    {}", rel.display());
            }
        }

        let affected = graph.affected_files(&[abs]);
        println!(
            "\n  {} {} file(s) would need re-analysis if changed",
            "Impact:".bright_red(),
            affected.len()
        );
    } else {
        println!(
            "{} Dependency graph: {} files tracked\n",
            "falcon".bright_cyan().bold(),
            graph.imports.len()
        );

        let mut stats: Vec<(usize, &PathBuf)> = graph
            .dependents
            .iter()
            .map(|(file, deps)| (deps.len(), file))
            .collect();
        stats.sort_by_key(|e| std::cmp::Reverse(e.0));

        println!(
            "  {} (by number of dependents):",
            "Most depended-on files".bright_green()
        );
        for (count, file) in stats.iter().take(20) {
            let rel = file.strip_prefix(&path).unwrap_or(file);
            println!("    {:>4} ← {}", count, rel.display());
        }
    }
    Ok(())
}

fn handle_workspace(path: PathBuf) -> Result<()> {
    let report = falcon::workspace::analyze_workspace(&path)?;
    if report.total_errors > 0 {
        process::exit(1);
    }
    Ok(())
}

fn handle_docs(output: PathBuf) -> Result<()> {
    falcon::docs::generate_rule_docs(&output)?;
    Ok(())
}

fn handle_validate(path: PathBuf) -> Result<()> {
    let errors = falcon::config::validator::validate_config(&path);
    falcon::config::validator::print_validation_results(&errors);
    if errors.iter().any(|e| {
        matches!(
            e.severity,
            falcon::config::validator::ConfigErrorSeverity::Error
        )
    }) {
        process::exit(1);
    }
    Ok(())
}

fn handle_check_cycles(path: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let graph = DependencyGraph::build(&path, &exclude);
    let (issues, cycles) = falcon::resolver::cyclic::detect_cycles(&graph, &path);

    println!(
        "{}",
        falcon::resolver::cyclic::format_cycles(&cycles, &path)
    );

    if !issues.is_empty() {
        println!(
            "{} {} files involved in cycles",
            "⚠".yellow().bold(),
            issues.len()
        );
        process::exit(1);
    }
    Ok(())
}

fn handle_check_unused_params(path: PathBuf, format: OutputFormat, output: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let issues = falcon::resolver::unused_params::detect_unused_params(&path, &exclude);
    get_reporter(&format, &output).report_issues(&issues);

    if !issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_check_dead_code(path: PathBuf, format: OutputFormat, output: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let issues = falcon::resolver::dead_code::detect_dead_code(&path, &exclude);
    get_reporter(&format, &output).report_issues(&issues);

    if !issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_check_unused_l10n(path: PathBuf, format: OutputFormat, output: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let issues = falcon::resolver::unused_l10n::detect_unused_l10n(&path, &exclude);
    get_reporter(&format, &output).report_issues(&issues);

    if !issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_check_promoted_deps(path: PathBuf, format: OutputFormat, output: PathBuf) -> Result<()> {
    let issues = falcon::resolver::cyclic::detect_promoted_deps(&path);
    get_reporter(&format, &output).report_issues(&issues);

    if !issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_explain(rule: String) -> Result<()> {
    if rule == "list" || rule == "all" {
        falcon::ai::explain::list_all_rules();
    } else if let Some(explanation) = falcon::ai::explain::explain_rule(&rule) {
        falcon::ai::explain::print_explanation(&explanation);
    } else {
        eprintln!(
            "{}: Unknown rule '{}'. Use 'falcon explain list' to see all rules.",
            "error".red(),
            rule
        );
        process::exit(1);
    }
    Ok(())
}

fn handle_fix(path: PathBuf, preview: bool, config: Option<PathBuf>) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let falcon_config = FalconConfig::load(config_path)?;
    let falcon = Falcon::new(falcon_config)?;
    let report = falcon.analyze(&path)?;

    let fixes = falcon::ai::fix::generate_fixes(&report.issues, &path);

    if preview {
        falcon::ai::fix::preview_fixes(&fixes);
    } else {
        falcon::ai::fix::preview_fixes(&fixes);
        let applied = falcon::ai::fix::apply_fixes(&fixes);
        println!("  {} Applied {} fix(es).", "✓".green().bold(), applied);
    }
    Ok(())
}

fn handle_check_unused_confidence(
    path: PathBuf,
    min_confidence: u8,
    config: Option<PathBuf>,
) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let falcon_config = FalconConfig::load(config_path)?;
    let falcon = Falcon::new(falcon_config)?;
    let report = falcon.analyze(&path)?;

    let results = falcon::ai::confidence::score_unused_issues(&report.issues, &path);
    falcon::ai::confidence::print_confidence_results(&results, Some(min_confidence));
    Ok(())
}

fn handle_check_layers(path: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let layers = falcon::analysis::layer_enforcement::detect_architecture(&path);
    match layers {
        Some(layers) => {
            println!(
                "{} Detected architecture: {} layers",
                "falcon".bright_cyan().bold(),
                layers.len()
            );
            for l in &layers {
                println!(
                    "  {} → can import: [{}]",
                    l.name.bright_white(),
                    if l.allowed_imports.is_empty() {
                        "none".to_string()
                    } else {
                        l.allowed_imports.join(", ")
                    }
                );
            }
            println!();

            let issues =
                falcon::analysis::layer_enforcement::enforce_layers(&path, &layers, &exclude);
            get_reporter(&OutputFormat::Console, &PathBuf::from("")).report_issues(&issues);

            if !issues.is_empty() {
                process::exit(1);
            }
        }
        None => {
            println!(
                        "{} No recognized architecture pattern detected (domain/, data/, presentation/ or core/, features/).",
                        "info".bright_blue()
                    );
        }
    }
    Ok(())
}

fn handle_check_imports(path: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let boundary_issues = falcon::analysis::import_rules::check_package_boundaries(&path, &exclude);
    get_reporter(&OutputFormat::Console, &PathBuf::from("")).report_issues(&boundary_issues);

    if !boundary_issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_cognitive_complexity(path: PathBuf, threshold: u32) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let mut flagged = 0;
    for entry in walkdir::WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(&path).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut parser = match falcon::parser::DartParser::new() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let tree = match parser.parse(&source) {
            Some(t) => t,
            None => continue,
        };

        let results = falcon::analysis::cognitive_complexity::file_cognitive_complexity(
            tree.root_node(),
            &source,
        );

        for (name, complexity, line) in &results {
            if *complexity > threshold {
                let rel = entry.path().strip_prefix(&path).unwrap_or(entry.path());
                println!(
                    "  {} {}:{} {} — cognitive complexity {}",
                    "⚠".yellow(),
                    rel.display(),
                    line,
                    name.bright_white(),
                    complexity.to_string().red().bold()
                );
                flagged += 1;
            }
        }
    }

    if flagged == 0 {
        println!(
            "  {} All functions below cognitive complexity threshold of {}.",
            "✓".green().bold(),
            threshold
        );
    } else {
        println!(
            "\n  {} {} function(s) exceed threshold of {}.",
            "⚠".yellow(),
            flagged,
            threshold
        );
        process::exit(1);
    }
    Ok(())
}

fn handle_check_widgets(path: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let mut all_issues = Vec::new();
    for entry in walkdir::WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(&path).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut parser = match falcon::parser::DartParser::new() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let tree = match parser.parse(&source) {
            Some(t) => t,
            None => continue,
        };

        let issues = falcon::analysis::widget_rebuild::detect_widget_issues(
            tree.root_node(),
            &source,
            entry.path(),
        );
        all_issues.extend(issues);
    }

    get_reporter(&OutputFormat::Console, &PathBuf::from("")).report_issues(&all_issues);
    if !all_issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_check_async(path: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let mut all_issues = Vec::new();
    for entry in walkdir::WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "dart"))
        .filter(|e| {
            let rel = e.path().strip_prefix(&path).unwrap_or(e.path());
            !exclude.iter().any(|p| p.matches_path(rel))
        })
    {
        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut parser = match falcon::parser::DartParser::new() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let tree = match parser.parse(&source) {
            Some(t) => t,
            None => continue,
        };

        let issues = falcon::analysis::async_antipatterns::detect_async_antipatterns(
            tree.root_node(),
            &source,
            entry.path(),
        );
        all_issues.extend(issues);
    }

    get_reporter(&OutputFormat::Console, &PathBuf::from("")).report_issues(&all_issues);
    if !all_issues.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn handle_review(
    path: PathBuf,
    diff: String,
    strictness: falcon::review::pr_review::ReviewStrictness,
) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let report = falcon::review::pr_review::review_diff(&path, &diff, &config, strictness)?;
    falcon::review::pr_review::print_review(&report);
    Ok(())
}

fn handle_codebase_intel(path: PathBuf) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let report = falcon::review::codebase_intel::analyze_codebase(&path, &config)?;
    falcon::review::codebase_intel::print_codebase_report(&report, &path);
    Ok(())
}

fn handle_trends(path: PathBuf, last: usize) -> Result<()> {
    let history = falcon::dashboard::snapshot::load_history(&path)?;
    match falcon::dashboard::trends::analyze_trends(&history, last) {
        Some(report) => falcon::dashboard::trends::print_trend_report(&report),
        None => {
            println!("  Need at least 2 snapshots for trends. Run: falcon dashboard snapshot");
        }
    }
    Ok(())
}

fn handle_rule_impact(path: PathBuf) -> Result<()> {
    let history = falcon::dashboard::snapshot::load_history(&path)?;
    if history.is_empty() {
        println!("  No snapshots yet. Run: falcon dashboard snapshot");
    } else {
        let impacts = falcon::dashboard::rule_impact::measure_rule_impact(&history);
        falcon::dashboard::rule_impact::print_rule_impact(&impacts);
        let recs = falcon::dashboard::rule_impact::auto_tune_recommendations(&impacts);
        falcon::dashboard::rule_impact::print_recommendations(&recs);
    }
    Ok(())
}

fn handle_export(
    path: PathBuf,
    format: ExportFormat,
    output: Option<PathBuf>,
    webhook_url: Option<String>,
) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let falcon_inst = Falcon::new(config)?;
    let report = falcon_inst.analyze(&path)?;
    let snapshot = falcon::dashboard::snapshot::AnalysisSnapshot::capture(&report, &path);

    match format {
        ExportFormat::Prometheus => {
            let metrics = falcon::dashboard::exports::export_prometheus(&snapshot);
            match output {
                Some(out) => {
                    std::fs::write(&out, &metrics)?;
                    println!(
                        "  {} Prometheus metrics saved to {}",
                        "✓".green().bold(),
                        out.display()
                    );
                }
                None => print!("{}", metrics),
            }
        }
        ExportFormat::Json => {
            let json = falcon::dashboard::exports::export_json(&snapshot)?;
            match output {
                Some(out) => {
                    std::fs::write(&out, &json)?;
                    println!(
                        "  {} JSON export saved to {}",
                        "✓".green().bold(),
                        out.display()
                    );
                }
                None => println!("{}", json),
            }
        }
        ExportFormat::Webhook => {
            let url = webhook_url
                .as_deref()
                .unwrap_or("http://localhost:9000/webhook");
            let project = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("project");
            let payload =
                falcon::dashboard::exports::WebhookPayload::from_snapshot(&snapshot, project);
            let json = payload.to_json()?;
            println!("{}", json);
            println!("  Webhook payload generated for {}", url.bright_blue());
        }
    }
    Ok(())
}

fn handle_migrate_from_dcm(config_path: PathBuf, output: PathBuf) -> Result<()> {
    let result = falcon::migration::dcm::migrate_from_dcm(&config_path)?;
    falcon::migration::dcm::print_migration_result(&result);

    let output_path = output.join("falcon.yaml");
    std::fs::write(&output_path, &result.falcon_yaml_content)?;
    println!(
        "  {} falcon.yaml written to {}",
        "✓".green().bold(),
        output_path.display()
    );
    Ok(())
}

fn handle_feature_gap() -> Result<()> {
    let report = falcon::migration::dcm::feature_gap_report();
    println!("{}", report);
    Ok(())
}

fn handle_benchmark(path: PathBuf) -> Result<()> {
    let result = falcon::benchmark::run_benchmark(&path)?;
    falcon::benchmark::print_benchmark(&result);
    Ok(())
}

fn handle_compare(path: PathBuf) -> Result<()> {
    let result = falcon::benchmark_compare::compare_with_dart_analyze(&path)?;
    falcon::benchmark_compare::print_compare_result(&result);
    Ok(())
}

fn handle_compare_reports(
    path: PathBuf,
    run1: usize,
    run2: usize,
    output: Option<PathBuf>,
) -> Result<()> {
    let history = falcon::dashboard::snapshot::load_history(&path)?;
    if history.len() < 2 {
        eprintln!(
            "  ❌ {} Need at least 2 analysis runs to compare. Run {} first.",
            "error:".bright_red(),
            "falcon analyze".bright_blue()
        );
        process::exit(1);
    }

    let idx1 = if run1 == 0 {
        history.len() - 2
    } else {
        (run1 - 1).min(history.len() - 1)
    };
    let idx2 = if run2 == 0 {
        history.len() - 1
    } else {
        (run2 - 1).min(history.len() - 1)
    };

    let snap1 = &history[idx1];
    let snap2 = &history[idx2];

    let result = falcon::dashboard::compare_reports::compare_snapshots(snap1, snap2);
    falcon::dashboard::compare_reports::print_comparison(&result);

    if let Some(out) = output {
        falcon::dashboard::compare_reports::generate_html_comparison(&result, &out)?;
    }
    Ok(())
}

fn handle_compare_branches(
    path: PathBuf,
    base: String,
    branch: String,
    output: Option<PathBuf>,
    config: Option<PathBuf>,
) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let falcon_config = FalconConfig::load(config_path)?;

    let html_out = output.unwrap_or_else(|| path.join("falcon-branch-comparison.html"));

    println!();
    println!(
        "  🦅 {} {}",
        "falcon".bright_blue().bold(),
        "Branch Comparison".bold()
    );
    println!(
        "  🌿 {} {} {}",
        base.bright_cyan(),
        "vs".dimmed(),
        branch.bright_cyan()
    );
    println!();

    match falcon::dashboard::compare_reports::compare_branches(
        &path,
        &base,
        &branch,
        &falcon_config,
        &html_out,
    ) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("  ❌ {} {}", "error:".bright_red(), e);
            process::exit(1);
        }
    }
    Ok(())
}

fn handle_history(path: PathBuf) -> Result<()> {
    falcon::dashboard::compare_reports::list_history(&path)?;
    Ok(())
}

fn handle_update(version: Option<String>, list: bool) -> Result<()> {
    if list {
        falcon::self_update::print_version_info();
        if let Err(e) = falcon::self_update::print_available_versions() {
            eprintln!("  ❌ {} {}", "error:".bright_red(), e);
        }
    } else {
        if let Err(e) = falcon::self_update::run_update(version.as_deref()) {
            eprintln!("  ❌ {} {}", "error:".bright_red(), e);
            process::exit(1);
        }
    }
    Ok(())
}

fn handle_showcase(paths: Vec<PathBuf>, format: DocFormat, output: Option<PathBuf>) -> Result<()> {
    if paths.is_empty() {
        eprintln!("Provide at least one project path to analyze.");
        process::exit(1);
    }

    let mut analyses = Vec::new();
    for path in &paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        match falcon::showcase::analyze_local_project(path, &name) {
            Ok(analysis) => analyses.push(analysis),
            Err(e) => eprintln!(
                "  {} Failed to analyze {}: {}",
                "✗".red(),
                path.display(),
                e
            ),
        }
    }

    let report = falcon::showcase::generate_showcase_report(analyses);

    match format {
        DocFormat::Console => falcon::showcase::print_showcase_report(&report),
        DocFormat::Markdown => {
            let md = falcon::showcase::generate_markdown_report(&report);
            match output {
                Some(out) => {
                    std::fs::write(&out, &md)?;
                    println!(
                        "  {} Showcase report written to {}",
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

fn handle_stability_contract() -> Result<()> {
    let contract = falcon::stability::contract::StabilityContract::default();
    falcon::stability::contract::print_stability_contract(&contract);
    Ok(())
}

fn handle_deprecation_status() -> Result<()> {
    falcon::stability::deprecation::print_deprecation_status();
    Ok(())
}

fn handle_perf_track(path: PathBuf, history: bool, last: usize) -> Result<()> {
    if history {
        let hist = falcon::stability::perf_track::load_perf_history(&path)?;
        falcon::stability::perf_track::print_perf_history(&hist, last);
    } else {
        let snapshot = falcon::stability::perf_track::capture_perf_snapshot(&path)?;
        falcon::stability::perf_track::save_perf_snapshot(&path, &snapshot)?;
        println!(
            "  {} Performance snapshot recorded: {} files, {} lines, {}ms",
            "✓".green().bold(),
            snapshot.file_count,
            snapshot.total_lines,
            snapshot.analysis_time_ms
        );

        let hist = falcon::stability::perf_track::load_perf_history(&path)?;
        if let Some(regression) = falcon::stability::perf_track::check_regression(&hist) {
            if regression.is_regression {
                eprintln!(
                    "  {} Performance regression: {:.1}% slower",
                    "⚠".yellow(),
                    regression.time_change_pct
                );
            }
        }
    }
    Ok(())
}

fn handle_mcp() -> Result<()> {
    falcon::mcp::server::run_mcp_server()?;
    Ok(())
}

fn handle_pr_comment(
    path: PathBuf,
    owner: Option<String>,
    repo: Option<String>,
    pr: Option<u32>,
    dry_run: bool,
    config: Option<PathBuf>,
) -> Result<()> {
    let config_path = config.as_deref().unwrap_or(&path);
    let falcon_config = FalconConfig::load(config_path)?;
    let falcon = Falcon::new(falcon_config)?;
    let report = falcon.analyze(&path)?;

    let comment = falcon::ci::pr_comment::format_pr_comment(&report, &path);

    if dry_run {
        println!("{}", comment);
    } else if let (Some(owner), Some(repo), Some(pr)) = (owner, repo, pr) {
        falcon::ci::pr_comment::post_pr_comment(&owner, &repo, pr, &comment)?;
        println!(
            "  {} Posted analysis to {}/{}#{}",
            "✓".green().bold(),
            owner,
            repo,
            pr
        );
    } else {
        falcon::ci::pr_comment::post_comment_auto(&comment)?;
        println!(
            "  {} Posted analysis to PR (auto-detected)",
            "✓".green().bold()
        );
    }

    falcon::ci::pr_comment::write_github_step_summary(&report, &path)?;
    Ok(())
}

fn handle_webhook(path: PathBuf, url: String, event: String) -> Result<()> {
    let config = FalconConfig::load(&path)?;
    let falcon_inst = Falcon::new(config)?;
    let report = falcon_inst.analyze(&path)?;
    let project = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("project");

    match event.as_str() {
        "analysis" => {
            falcon::ci::webhook::send_analysis_webhook(&url, project, &report)?;
            println!("  {} Sent analysis webhook to {}", "✓".green().bold(), url);
        }
        "score" => {
            let score = falcon::ai_score::score::score_from_report(&report)?;
            falcon::ci::webhook::send_score_webhook(&url, project, &score, None)?;
            println!(
                "  {} Sent score webhook ({}/100) to {}",
                "✓".green().bold(),
                score.overall,
                url
            );
        }
        "drift" => {
            let drift = falcon::ai_score::drift::detect_drift(&path, None)?;
            falcon::ci::webhook::send_drift_webhook(&url, project, &drift)?;
            println!(
                "  {} Sent drift webhook ({:.0}% adherence) to {}",
                "✓".green().bold(),
                drift.drift_score,
                url
            );
        }
        other => {
            eprintln!("Unknown event '{}'. Use: analysis, score, drift", other);
            process::exit(1);
        }
    }
    Ok(())
}

fn handle_benchmark_db(path: PathBuf, tool: Option<String>, summary: bool) -> Result<()> {
    if summary {
        let db = falcon::ai_score::benchmark_db::load_benchmark_db(&path)?;
        let stats = falcon::ai_score::benchmark_db::compute_tool_stats(&db);
        falcon::ai_score::benchmark_db::print_benchmark_summary(&stats);
    } else if let Some(tool_name) = tool {
        let project = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("project");
        let entry = falcon::ai_score::benchmark_db::record_benchmark(&path, project, &tool_name)?;
        println!(
            "  {} Recorded benchmark: {} (tool: {}) — score {}/100",
            "✓".green().bold(),
            entry.project_name,
            entry.ai_tool,
            entry.score
        );
    } else {
        eprintln!("Use --tool <name> to record, or --summary to view benchmarks");
        process::exit(1);
    }
    Ok(())
}

fn handle_refactor_sim(
    path: PathBuf,
    scenario: falcon::analysis::refactor_sim::RefactorScenario,
    json: bool,
) -> Result<()> {
    let impact = falcon::analysis::refactor_sim::simulate_refactor(&path, &scenario)?;
    if json {
        let j = serde_json::to_string_pretty(&impact)?;
        println!("{}", j);
    } else {
        falcon::analysis::refactor_sim::print_refactor_impact(&impact);
    }
    Ok(())
}

fn handle_test_gen(path: PathBuf, write: bool) -> Result<()> {
    let stubs = falcon::analysis::test_gen::generate_test_stubs(&path);
    falcon::analysis::test_gen::print_test_gen_summary(&stubs);

    if write {
        let mut written = 0;
        for stub in &stubs {
            let test_path = path.join(&stub.test_file);
            if !test_path.exists() && !stub.test_cases.is_empty() {
                if let Some(parent) = test_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let content = falcon::analysis::test_gen::render_test_file(stub);
                if std::fs::write(&test_path, &content).is_ok() {
                    written += 1;
                }
            }
        }
        println!("  {} Wrote {} test file(s)", "✓".green().bold(), written);
    }
    Ok(())
}

fn handle_vuln_scan(path: PathBuf) -> Result<()> {
    let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(&path);
    falcon::analysis::vuln_radar::print_vuln_report(&findings);
    if findings
        .iter()
        .any(|f| f.risk_level == falcon::analysis::vuln_radar::RiskLevel::Critical)
    {
        process::exit(1);
    }
    Ok(())
}

fn handle_ai_profile(path: PathBuf) -> Result<()> {
    let db = falcon::ai_score::benchmark_db::load_benchmark_db(&path)?;
    let profiles = falcon::ai_score::ai_profiling::build_tool_profiles(&db);
    falcon::ai_score::ai_profiling::print_tool_profiles(&profiles);
    Ok(())
}

fn handle_discover_rules(path: PathBuf) -> Result<()> {
    let rules = falcon::ai_score::auto_rules::discover_patterns(&path);
    falcon::ai_score::auto_rules::print_proposed_rules(&rules);
    Ok(())
}

fn handle_fix_track(
    path: PathBuf,
    rule: Option<String>,
    outcome: Option<String>,
    file: Option<String>,
    report: bool,
) -> Result<()> {
    if report {
        let history = falcon::ai_score::fix_tracking::load_fix_history(&path)?;
        let eff = falcon::ai_score::fix_tracking::compute_effectiveness(&history);
        falcon::ai_score::fix_tracking::print_fix_effectiveness(&eff);
    } else if let (Some(rule), Some(outcome_str), Some(file)) = (rule, outcome, file) {
        let outcome = match outcome_str.as_str() {
            "accepted" | "accept" => falcon::ai_score::fix_tracking::FixOutcome::Accepted,
            "rejected" | "reject" => falcon::ai_score::fix_tracking::FixOutcome::Rejected,
            "modified" | "modify" => falcon::ai_score::fix_tracking::FixOutcome::Modified,
            _ => {
                eprintln!(
                    "Unknown outcome '{}'. Use: accepted, rejected, modified",
                    outcome_str
                );
                process::exit(1);
            }
        };
        falcon::ai_score::fix_tracking::record_fix(&path, &rule, &file, outcome)?;
        println!(
            "  {} Recorded fix outcome for '{}' in {}",
            "✓".green().bold(),
            rule,
            file
        );
    } else {
        eprintln!("Use --report to view, or --rule/--outcome/--file to record");
        process::exit(1);
    }
    Ok(())
}

fn handle_learn(project: PathBuf, db: PathBuf, insights: bool) -> Result<()> {
    if insights {
        let learning_db = falcon::ai_score::cross_project::load_learning_db(&db)?;
        let ins = falcon::ai_score::cross_project::derive_insights(&learning_db);
        falcon::ai_score::cross_project::print_insights(&ins);
    } else {
        let profile = falcon::ai_score::cross_project::record_project(&db, &project)?;
        println!(
            "  {} Recorded project '{}' — {} files, arch: {}, score: {}",
            "✓".green().bold(),
            profile.project_id,
            profile.file_count,
            profile.architecture,
            profile
                .ai_score
                .map_or("N/A".to_string(), |s| format!("{}/100", s))
        );
    }
    Ok(())
}

fn handle_predict(path: PathBuf, json: bool) -> Result<()> {
    let predictions = falcon::ai_score::regression_predict::predict_risks(&path)?;
    if json {
        let j = serde_json::to_string_pretty(&predictions)?;
        println!("{}", j);
    } else {
        falcon::ai_score::regression_predict::print_risk_predictions(&predictions);
    }
    Ok(())
}

fn handle_upgrade_check(path: PathBuf) -> Result<()> {
    let findings = falcon::analysis::upgrade_check::check_upgrade_compatibility(&path);
    falcon::analysis::upgrade_check::print_compat_report(&findings);
    if findings.iter().any(|f| f.removed_in.is_some()) {
        process::exit(1);
    }
    Ok(())
}

fn handle_marketplace(query: String) -> Result<()> {
    let q = if query.is_empty() {
        None
    } else {
        Some(query.as_str())
    };
    let listings = falcon::platform::marketplace::browse_marketplace(q);
    falcon::platform::marketplace::print_marketplace(&listings, q);
    Ok(())
}

fn handle_certify(path: PathBuf) -> Result<()> {
    let result = falcon::platform::certification::evaluate_certification(&path)?;
    falcon::platform::certification::print_certification(&result);
    Ok(())
}

fn handle_partners() -> Result<()> {
    let partners = falcon::platform::partner::list_partners();
    falcon::platform::partner::print_partners(&partners);
    Ok(())
}

fn handle_check_platform(path: PathBuf) -> Result<()> {
    let issues = falcon::analysis::platform_channels::analyze_platform_channels(&path);
    falcon::analysis::platform_channels::print_platform_summary(&issues);
    if !issues.is_empty() {
        get_reporter(&OutputFormat::Console, &PathBuf::from("")).report_issues(&issues);
    }
    Ok(())
}

fn handle_check_codegen(path: PathBuf) -> Result<()> {
    let report = falcon::analysis::codegen_quality::analyze_codegen(&path);
    falcon::analysis::codegen_quality::print_codegen_report(&report);
    Ok(())
}

fn handle_check_perf(path: PathBuf) -> Result<()> {
    let report = falcon::analysis::devtools_bridge::analyze_performance(&path);
    falcon::analysis::devtools_bridge::print_perf_report(&report);
    Ok(())
}

fn handle_api(host: String, port: u16) -> Result<()> {
    falcon::api::server::start_api_server(&host, port)?;
    Ok(())
}

fn handle_drift(path: PathBuf, since: Option<String>, json: bool) -> Result<()> {
    let report = falcon::ai_score::drift::detect_drift(&path, since.as_deref())?;
    if json {
        let j = serde_json::to_string_pretty(&report)?;
        println!("{}", j);
    } else {
        falcon::ai_score::drift::print_drift_report(&report);
    }
    Ok(())
}

fn handle_self_tune(path: PathBuf) -> Result<()> {
    let history = falcon::ai_score::self_tune::record_analysis(&path)?;
    let recs = falcon::ai_score::self_tune::generate_recommendations(&history);
    falcon::ai_score::self_tune::print_tune_recommendations(&recs, &history);
    Ok(())
}

fn handle_score_track(path: PathBuf, history: bool, last: usize) -> Result<()> {
    if history {
        let hist = falcon::ai_score::score_trends::load_score_history(&path)?;
        falcon::ai_score::score_trends::print_score_history(&hist, last);
    } else {
        let snapshot = falcon::ai_score::score_trends::record_score(&path)?;
        println!(
            "  {} Score snapshot recorded: {}/100 (Grade: {}), {} issues",
            "✓".green().bold(),
            snapshot.overall,
            snapshot.grade,
            snapshot.total_issues
        );

        let hist = falcon::ai_score::score_trends::load_score_history(&path)?;
        if hist.snapshots.len() >= 2 {
            let prev = &hist.snapshots[hist.snapshots.len() - 2];
            let delta = falcon::ai_score::score_trends::compare_scores(prev, &snapshot);
            if delta.overall > 0 {
                println!(
                    "    {} Score improved by {} points",
                    "↑".bright_green(),
                    delta.overall
                );
            } else if delta.overall < 0 {
                println!(
                    "    {} Score dropped by {} points",
                    "↓".red(),
                    delta.overall.abs()
                );
            }
        }
    }
    Ok(())
}

fn handle_ai_score(path: PathBuf, badge: bool, json: bool) -> Result<()> {
    let score = falcon::ai_score::score::calculate_ai_score(&path)?;
    if json {
        let j = serde_json::to_string_pretty(&score)?;
        println!("{}", j);
    } else {
        falcon::ai_score::score::print_ai_score(&score);
    }
    if badge {
        println!("{}", falcon::ai_score::score::generate_badge(&score));
    }
    Ok(())
}

fn handle_ai_report(path: PathBuf, format: DocFormat, output: Option<PathBuf>) -> Result<()> {
    let report = falcon::ai_score::report::generate_ai_report(&path)?;
    match format {
        DocFormat::Console => falcon::ai_score::report::print_ai_report(&report),
        DocFormat::Markdown => {
            let md = falcon::ai_score::report::generate_markdown_report(&report);
            match output {
                Some(out) => {
                    std::fs::write(&out, &md)?;
                    println!(
                        "  {} AI report written to {}",
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

fn handle_provenance(path: PathBuf, verbose: bool) -> Result<()> {
    let results = falcon::ai_score::provenance::analyze_project_provenance(&path)?;
    let summary = falcon::ai_score::provenance::summarize_provenance(&results);
    falcon::ai_score::provenance::print_provenance_summary(&summary);

    if verbose {
        let ai_files: Vec<_> = results
            .iter()
            .filter(|r| r.origin == falcon::ai_score::provenance::CodeOrigin::LikelyAiGenerated)
            .collect();
        if !ai_files.is_empty() {
            println!("  Files with AI-generation signals:");
            for f in &ai_files {
                let rel = std::path::Path::new(&f.file)
                    .strip_prefix(&path)
                    .unwrap_or(std::path::Path::new(&f.file));
                println!(
                    "    {} {} ({:.0}% confidence)",
                    "→".bright_yellow(),
                    rel.display(),
                    f.confidence * 100.0
                );
                for signal in &f.signals {
                    println!("      · {}", signal);
                }
            }
            println!();
        }
    }
    Ok(())
}

fn handle_conventions(path: PathBuf, json: bool) -> Result<()> {
    let report = falcon::ai_score::convention::detect_conventions(&path)?;
    if json {
        let j = serde_json::to_string_pretty(&report)?;
        println!("{}", j);
    } else {
        falcon::ai_score::convention::print_convention_report(&report);
    }
    Ok(())
}

fn handle_agents(action: AgentsAction) -> Result<()> {
    match action {
        AgentsAction::Init { path, force } => {
            let report = falcon::agents::run_init(&path, force)?;
            println!();
            println!("  {} Agents", "falcon".bright_cyan().bold());
            println!();
            if let Some(root) = &report.root_written {
                println!("    {} {}", "✓".green(), root.display());
            }
            for f in &report.feature_files {
                println!("    {} {}", "✓".green(), f.display());
            }
            for s in &report.skipped {
                println!(
                    "    {} {} (already exists — pass --force to overwrite)",
                    "·".dimmed(),
                    s.display()
                );
            }
            println!();
            println!(
                "  {} root: {}, features: {}, skipped: {}",
                "■".bright_white(),
                if report.root_written.is_some() { 1 } else { 0 },
                report.feature_files.len(),
                report.skipped.len()
            );
            println!();
        }
    }
    Ok(())
}

fn handle_baseline(action: BaselineAction) -> Result<()> {
    match action {
        BaselineAction::Create { path, config } => {
            let config_path = config.as_deref().unwrap_or(&path);
            let falcon_config = FalconConfig::load(config_path)?;
            let falcon = Falcon::new(falcon_config)?;
            let report = falcon.analyze(&path)?;

            let baseline_path = Baseline::create(&report.issues, &path)?;
            println!(
                "{} Baseline created with {} issues at {}",
                "✓".green().bold(),
                report.issues.len(),
                baseline_path.display()
            );
        }
    }
    Ok(())
}

fn handle_ai(action: AiAction) -> Result<()> {
    match action {
        AiAction::Setup { path } => {
            falcon::ai::config::generate_ai_setup(&path)?;
            println!(
                "{} AI configuration added to falcon.yaml",
                "✓".green().bold()
            );
            println!("  Edit falcon.yaml to set your provider and API key.");
            println!("  Supported providers: openai, anthropic, gemini, local (Ollama), embedded (in-process SLM)");
        }
        AiAction::Status { path } => {
            let config = FalconConfig::load(&path)?;
            let ai = &config.ai;
            println!();
            println!(
                "  {} AI Configuration Status",
                "falcon".bright_cyan().bold()
            );
            println!();
            println!(
                "  Enabled:   {}",
                if ai.enabled {
                    "yes".green()
                } else {
                    "no".red()
                }
            );
            println!("  Provider:  {:?}", ai.provider);
            println!("  Model:     {}", ai.effective_model());
            println!(
                "  API Key:   {}",
                if ai.resolve_api_key().is_some() {
                    "configured".green()
                } else {
                    "not set".yellow()
                }
            );
            println!(
                "  Available: {}",
                if ai.is_available() {
                    "yes".green()
                } else {
                    "no".red()
                }
            );
            println!();
            println!("  Feature Toggles:");
            println!(
                "    Confidence scoring:      {}",
                if ai.features.confidence_scoring {
                    "on"
                } else {
                    "off"
                }
            );
            println!(
                "    Smart fixes:             {}",
                if ai.features.smart_fixes { "on" } else { "off" }
            );
            println!(
                "    Explanations:            {}",
                if ai.features.explanations {
                    "on"
                } else {
                    "off"
                }
            );
            println!(
                "    False positive reduction: {}",
                if ai.features.false_positive_reduction {
                    "on"
                } else {
                    "off"
                }
            );
            if let Some(ref e) = ai.embedded {
                println!();
                println!("  Embedded model:");
                println!("    Model id:   {}", e.model_id);
                println!("    Max issues: {}", e.max_issues);
                #[cfg(feature = "ai-local")]
                println!("    Engine:     compiled in (ai-local)");
                #[cfg(not(feature = "ai-local"))]
                println!("    Engine:     NOT compiled (rebuild with --features ai-local)");
            }
            println!();
        }
        AiAction::Triage { path, format } => {
            #[cfg(not(feature = "ai-local"))]
            {
                let _ = (&path, &format);
                println!("{} embedded AI is not compiled in.", "✗".red().bold());
                println!("  Rebuild with: cargo build --release --features ai-local");
            }
            #[cfg(feature = "ai-local")]
            {
                use falcon::ai::local::engine::LocalEngine;
                use falcon::ai::local::triage::{
                    print_triage_run, triage_issues, triage_run_to_json,
                };

                let falcon_config = FalconConfig::load(&path)?;
                let embedded_cfg = falcon_config.ai.embedded.clone().unwrap_or_default();
                let falcon = Falcon::new(falcon_config)?;
                let report = falcon.analyze(&path)?;

                let mut engine = LocalEngine::load(&embedded_cfg)?;
                let run = triage_issues(&report.issues, &path, &mut engine, &embedded_cfg);

                if format == "json" {
                    println!("{}", triage_run_to_json(&run));
                } else {
                    print_triage_run(&run);
                }
            }
        }
    }
    Ok(())
}

fn handle_plugin(action: PluginAction) -> Result<()> {
    match action {
        PluginAction::Create { name, r#type, dir } => {
            let plugin_type = match r#type.as_str() {
                "wasm" => falcon::plugins::manifest::PluginType::Wasm,
                "native" => falcon::plugins::manifest::PluginType::Native,
                "preset" => falcon::plugins::manifest::PluginType::Preset,
                _ => {
                    eprintln!(
                        "Invalid plugin type '{}'. Use: wasm, native, preset",
                        r#type
                    );
                    process::exit(1);
                }
            };
            falcon::plugins::scaffold::create_plugin(&name, &dir, plugin_type)?;
        }
        PluginAction::List => {
            let plugin_dir = get_plugin_dir();
            let plugins = falcon::plugins::scaffold::list_plugins(&plugin_dir)?;
            falcon::plugins::scaffold::print_plugins(&plugins.to_vec());
        }
        PluginAction::Install { path } => {
            let plugin_dir = get_plugin_dir();
            std::fs::create_dir_all(&plugin_dir)?;
            let name = falcon::plugins::scaffold::install_plugin(&path, &plugin_dir)?;
            println!(
                "  {} Installed plugin '{}'",
                "✓".green().bold(),
                name.bright_cyan()
            );
        }
        PluginAction::Search { query } => {
            let results = falcon::plugins::registry::search_registry(&query);
            falcon::plugins::registry::print_search_results(&results, &query);
        }
        PluginAction::Test { path } => {
            let manifest = falcon::plugins::manifest::PluginManifest::load(&path)?;
            println!(
                "  {} Plugin '{}' v{} — manifest valid, {} rule(s) defined",
                "✓".green().bold(),
                manifest.name.bright_cyan(),
                manifest.version,
                manifest.rules.len()
            );

            let rules_path = path.join("rules/rules.yaml");
            if rules_path.exists() {
                let rules = falcon::plugins::wasm_runtime::load_wasm_rules(&rules_path)?;
                println!(
                    "  {} Loaded {} rule definition(s) from rules.yaml",
                    "✓".green().bold(),
                    rules.len()
                );
            }

            let test_path = path.join("test/test_cases.yaml");
            if test_path.exists() {
                println!(
                    "  {} Test cases file found at test/test_cases.yaml",
                    "✓".green().bold()
                );
            }
        }
    }
    Ok(())
}

fn handle_preset(action: PresetAction) -> Result<()> {
    match action {
        PresetAction::List => {
            let presets = falcon::plugins::presets::list_presets();
            falcon::plugins::presets::print_presets(&presets);
        }
        PresetAction::Show { name } => match falcon::plugins::presets::get_preset(&name) {
            Some(preset) => falcon::plugins::presets::print_preset_detail(&preset),
            None => {
                eprintln!("Unknown preset '{}'. Use: falcon preset list", name);
                process::exit(1);
            }
        },
        PresetAction::Apply { name, path } => match falcon::plugins::presets::get_preset(&name) {
            Some(preset) => falcon::plugins::presets::apply_preset(&preset, &path)?,
            None => {
                eprintln!("Unknown preset '{}'. Use: falcon preset list", name);
                process::exit(1);
            }
        },
    }
    Ok(())
}

fn handle_dashboard(action: DashboardAction) -> Result<()> {
    match action {
        DashboardAction::Snapshot { path } => {
            let config = FalconConfig::load(&path)?;
            let falcon = Falcon::new(config)?;
            let report = falcon.analyze(&path)?;
            let snapshot = falcon::dashboard::snapshot::AnalysisSnapshot::capture(&report, &path);
            let saved = falcon::dashboard::snapshot::save_snapshot(&path, &snapshot)?;
            println!(
                "  {} Snapshot saved — health {:.0}/100, {} issues, {} files",
                "✓".green().bold(),
                snapshot.health_score,
                snapshot.issues.total,
                snapshot.file_count,
            );
            println!("    → {}", saved.display());
        }
        DashboardAction::Serve { path, port } => {
            falcon::dashboard::server::start_dashboard(&path, port)?;
        }
        DashboardAction::History { path, last } => {
            let history = falcon::dashboard::snapshot::load_history(&path)?;
            if history.is_empty() {
                println!("  No snapshots yet. Run: falcon dashboard snapshot");
            } else {
                println!();
                println!(
                    "  {} Analysis History ({} total, showing last {})",
                    "falcon".bright_cyan().bold(),
                    history.len(),
                    last,
                );
                println!();
                for snap in history.iter().rev().take(last) {
                    let commit = snap.commit_hash.as_deref().unwrap_or("—");
                    println!(
                        "  {} │ {} │ health {:.0} │ {} issues │ {} files",
                        snap.timestamp,
                        commit.bright_blue(),
                        snap.health_score,
                        snap.issues.total,
                        snap.file_count,
                    );
                }
                println!();
            }
        }
    }
    Ok(())
}

fn handle_suppress(action: SuppressAction) -> Result<()> {
    match action {
        SuppressAction::Add {
            rule,
            file,
            line,
            reason,
            category,
            path,
        } => {
            let cat = match category.as_str() {
                "false-positive" | "fp" => {
                    falcon::stability::suppression::SuppressionCategory::FalsePositive
                }
                "wont-fix" | "wf" => falcon::stability::suppression::SuppressionCategory::WontFix,
                "acknowledged" | "ack" => {
                    falcon::stability::suppression::SuppressionCategory::Acknowledged
                }
                "deferred" | "defer" => {
                    falcon::stability::suppression::SuppressionCategory::Deferred
                }
                _ => {
                    eprintln!("Unknown category '{}'. Use: false-positive, wont-fix, acknowledged, deferred", category);
                    process::exit(1);
                }
            };
            falcon::stability::suppression::add_suppression(
                &path,
                &falcon::stability::suppression::SuppressionRequest {
                    rule: &rule,
                    file: &file,
                    line,
                    reason: &reason,
                    category: cat,
                },
            )?;
            println!(
                "  {} Suppression added for '{}' in {}",
                "✓".green().bold(),
                rule,
                file
            );
        }
        SuppressAction::Stats { path } => {
            let db = falcon::stability::suppression::load_suppressions(&path)?;
            let stats = falcon::stability::suppression::suppression_stats(&db);
            falcon::stability::suppression::print_suppression_stats(&stats);
        }
        SuppressAction::List { path } => {
            let db = falcon::stability::suppression::load_suppressions(&path)?;
            falcon::stability::suppression::print_suppression_list(&db);
        }
    }
    Ok(())
}

fn handle_manage(action: ManageAction) -> Result<()> {
    match action {
        ManageAction::Health { path, json } => {
            let report = falcon::manage::health::generate_health_report(&path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::manage::health::print_health_report(&report);
            }
        }
        ManageAction::Deps { path } => {
            let report = falcon::manage::deps::analyze_dependencies(&path)?;
            falcon::manage::deps::print_dep_report(&report);
        }
        ManageAction::Arch { path, json } => {
            let report = falcon::manage::architect::analyze_architecture(&path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                falcon::manage::architect::print_arch_report(&report);
            }
        }
        ManageAction::Maint { path } => {
            let report = falcon::manage::maintenance::analyze_maintenance(&path)?;
            falcon::manage::maintenance::print_maintenance_report(&report);
        }
        ManageAction::Build { path } => {
            let report = falcon::manage::build_opt::analyze_build(&path)?;
            falcon::manage::build_opt::print_build_report(&report);
        }
        ManageAction::All { path } => {
            let health = falcon::manage::health::generate_health_report(&path)?;
            falcon::manage::health::print_health_report(&health);

            let deps = falcon::manage::deps::analyze_dependencies(&path)?;
            falcon::manage::deps::print_dep_report(&deps);

            let arch = falcon::manage::architect::analyze_architecture(&path)?;
            falcon::manage::architect::print_arch_report(&arch);

            let maint = falcon::manage::maintenance::analyze_maintenance(&path)?;
            falcon::manage::maintenance::print_maintenance_report(&maint);

            let build = falcon::manage::build_opt::analyze_build(&path)?;
            falcon::manage::build_opt::print_build_report(&build);
        }
    }
    Ok(())
}

fn handle_cloud(action: CloudAction) -> Result<()> {
    match action {
        CloudAction::Init { team, path } => {
            falcon::platform::cloud::init_cloud(&path, &team)?;
            println!(
                "  {} Cloud initialized for team '{}'",
                "✓".green().bold(),
                team
            );
        }
        CloudAction::AddProject {
            name,
            project_path,
            path,
        } => {
            falcon::platform::cloud::register_project(&path, &name, &project_path)?;
            println!("  {} Project '{}' registered", "✓".green().bold(), name);
        }
        CloudAction::Dashboard { path } => {
            let dashboard = falcon::platform::cloud::generate_dashboard(&path)?;
            falcon::platform::cloud::print_dashboard(&dashboard);
        }
    }
    Ok(())
}

fn handle_enterprise(action: EnterpriseAction) -> Result<()> {
    match action {
        EnterpriseAction::Init { path } => {
            let policies = falcon::platform::enterprise::default_policies();
            falcon::platform::enterprise::save_policies(&path, &policies)?;
            println!(
                "  {} Enterprise policies initialized ({} policies)",
                "✓".green().bold(),
                policies.policies.len()
            );
            falcon::platform::enterprise::record_audit(
                &path,
                "system",
                "init",
                "policies",
                "Default enterprise policies created",
            )?;
        }
        EnterpriseAction::Check { path } => {
            let results = falcon::platform::enterprise::check_policies(&path)?;
            falcon::platform::enterprise::print_policy_results(&results);
            falcon::platform::enterprise::record_audit(
                &path,
                "system",
                "policy-check",
                "project",
                &format!(
                    "{} passed, {} failed",
                    results.iter().filter(|r| r.passed).count(),
                    results.iter().filter(|r| !r.passed).count()
                ),
            )?;
            if results.iter().any(|r| !r.passed) {
                process::exit(1);
            }
        }
        EnterpriseAction::Compliance { path, output } => {
            let report = falcon::platform::enterprise::generate_compliance_report(&path)?;
            match output {
                Some(out) => {
                    std::fs::write(&out, &report)?;
                    println!(
                        "  {} Compliance report written to {}",
                        "✓".green().bold(),
                        out.display()
                    );
                }
                None => print!("{}", report),
            }
        }
        EnterpriseAction::Audit { path, last } => {
            let log = falcon::platform::enterprise::load_audit_log(&path)?;
            println!();
            println!(
                "  {} Audit Log ({} entries)",
                "falcon".bright_cyan().bold(),
                log.entries.len()
            );
            println!();
            for entry in log.entries.iter().rev().take(last) {
                println!(
                    "  {} {} {} → {} ({})",
                    entry.timestamp.dimmed(),
                    entry.user.bright_white(),
                    entry.action.bright_yellow(),
                    entry.target,
                    entry.details.dimmed()
                );
            }
            println!();
        }
    }
    Ok(())
}

fn handle_community(action: CommunityAction) -> Result<()> {
    match action {
        CommunityAction::Request {
            name,
            desc,
            category,
            path,
        } => {
            let id = falcon::community::submit_rule_request(&path, &name, &desc, &category)?;
            println!(
                "  {} Rule request submitted: {} ({})",
                "✓".green().bold(),
                name,
                id
            );
        }
        CommunityAction::Vote { id, path } => {
            let votes = falcon::community::vote_rule_request(&path, &id)?;
            println!(
                "  {} Voted on {}. Total votes: {}",
                "✓".green().bold(),
                id,
                votes
            );
        }
        CommunityAction::Requests { path } => {
            let data = falcon::community::load_community(&path)?;
            falcon::community::print_rule_requests(&data.rule_requests);
        }
        CommunityAction::Contributed => {
            let rules = falcon::community::sample_contributed_rules();
            falcon::community::print_contributed_rules(&rules);
        }
    }
    Ok(())
}

fn get_plugin_dir() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
    match home {
        Ok(dir) => PathBuf::from(dir).join(".falcon").join("plugins"),
        Err(_) => {
            log::warn!("HOME/USERPROFILE not set, using current directory for plugins");
            PathBuf::from(".falcon").join("plugins")
        }
    }
}

fn run_incremental(
    falcon: &Falcon,
    path: &std::path::Path,
    git_ref: &str,
    config: &FalconConfig,
) -> Result<falcon::reporters::AnalysisReport> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", git_ref])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let changed_files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(".dart"))
        .map(|line| path.join(line))
        .filter(|p| p.exists())
        .collect();

    if changed_files.is_empty() {
        eprintln!(
            "{} No Dart files changed since {}",
            "info".bright_blue(),
            git_ref
        );
        return Ok(falcon::reporters::AnalysisReport {
            issues: Vec::new(),
            metrics: Vec::new(),
            file_count: 0,
            project_path: None,
        });
    }

    let exclude: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let graph = DependencyGraph::build(path, &exclude);
    let affected = graph.affected_files(&changed_files);
    let affected_vec: Vec<PathBuf> = affected.into_iter().collect();

    eprintln!(
        "{} Incremental: {} changed, {} affected (since {})",
        "info".bright_blue(),
        changed_files.len(),
        affected_vec.len(),
        git_ref,
    );

    let mut cache = AnalysisCache::load(path);
    let report = falcon.analyze_files(&affected_vec)?;

    for (file, _) in &report.metrics {
        let file_issues = report.issues.iter().filter(|i| i.file == *file).count();
        let has_errors = report
            .issues
            .iter()
            .any(|i| i.file == *file && i.severity == Severity::Error);
        cache.update_entry(file, file_issues, has_errors);
    }
    let _ = cache.save(path);

    Ok(report)
}

fn should_fail(report: &falcon::reporters::AnalysisReport, level: &FailLevel) -> bool {
    match level {
        FailLevel::Error => report.has_errors(),
        FailLevel::Warning => report.error_count() > 0 || report.warning_count() > 0,
        FailLevel::Info => !report.issues.is_empty(),
    }
}

fn print_smells_summary(
    summary: &falcon::smells::SmellsSummary,
    root: &Path,
    limit: usize,
    format: &OutputFormat,
    output: &PathBuf,
) {
    if matches!(format, OutputFormat::Json) {
        let payload = serde_json::json!({
            "dead_code": summary.dead_code.iter().map(issue_to_json).collect::<Vec<_>>(),
            "code_smells": summary.code_smells.iter().map(issue_to_json).collect::<Vec<_>>(),
            "security_smells": summary.security_smells.iter().map(issue_to_json).collect::<Vec<_>>(),
            "other": summary.other.iter().map(issue_to_json).collect::<Vec<_>>(),
            "dead_folders": summary.dead_folders.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            "total": summary.total(),
        });
        let s = serde_json::to_string_pretty(&payload).unwrap_or_default();
        if matches!(format, OutputFormat::Console) {
            println!("{}", s);
        } else {
            let _ = std::fs::write(output, s);
        }
        return;
    }

    println!();
    println!("  {} Smells", "falcon".bright_cyan().bold());
    println!();
    println!(
        "  {} {}",
        "▸".bright_red(),
        format!("Security Smells ({})", summary.security_smells.len())
            .bright_white()
            .bold()
    );
    print_issue_block(&summary.security_smells, root, limit);

    println!(
        "  {} {}",
        "▸".bright_yellow(),
        format!(
            "Dead Code ({} issues, {} dead folders)",
            summary.dead_code.len(),
            summary.dead_folders.len()
        )
        .bright_white()
        .bold()
    );
    print_issue_block(&summary.dead_code, root, limit);
    if !summary.dead_folders.is_empty() {
        println!("    {} Dead folders:", "└".dimmed());
        for folder in summary.dead_folders.iter().take(limit) {
            let rel = folder.strip_prefix(root).unwrap_or(folder);
            println!("      {}/", rel.display().to_string().bright_yellow());
        }
        if summary.dead_folders.len() > limit {
            println!("      ... and {} more", summary.dead_folders.len() - limit);
        }
        println!();
    }

    println!(
        "  {} {}",
        "▸".bright_blue(),
        format!("Code Smells ({})", summary.code_smells.len())
            .bright_white()
            .bold()
    );
    print_issue_block(&summary.code_smells, root, limit);

    if !summary.other.is_empty() {
        println!(
            "  {} {}",
            "▸".dimmed(),
            format!("Other ({})", summary.other.len()).dimmed()
        );
    }

    println!();
    println!(
        "  {} {} total smells",
        "■".bright_white(),
        summary.total().to_string().bright_white().bold()
    );
    println!();
}

fn print_issue_block(issues: &[falcon::reporters::Issue], root: &Path, limit: usize) {
    if issues.is_empty() {
        println!("    {} none", "✓".green());
        println!();
        return;
    }
    for issue in issues.iter().take(limit) {
        let rel = issue.file.strip_prefix(root).unwrap_or(&issue.file);
        println!(
            "    {}:{}  [{}] {}",
            rel.display().to_string().bright_white(),
            issue.line,
            issue.rule.dimmed(),
            issue.message
        );
    }
    if issues.len() > limit {
        println!("    ... and {} more", issues.len() - limit);
    }
    println!();
}

fn issue_to_json(issue: &falcon::reporters::Issue) -> serde_json::Value {
    serde_json::json!({
        "rule": issue.rule,
        "message": issue.message,
        "severity": format!("{:?}", issue.severity),
        "file": issue.file.to_string_lossy(),
        "line": issue.line,
        "column": issue.column,
    })
}

fn get_reporter(format: &OutputFormat, output: &Path) -> Box<dyn Reporter> {
    match format {
        OutputFormat::Console => Box::new(ConsoleReporter),
        OutputFormat::Json => Box::new(JsonReporter),
        OutputFormat::Html => Box::new(HtmlReporter {
            output_path: output.to_path_buf(),
        }),
        OutputFormat::Sarif => Box::new(SarifReporter {
            output_path: Some(output.to_path_buf()),
        }),
        OutputFormat::Codeclimate | OutputFormat::Gitlab => Box::new(CodeClimateReporter {
            output_path: Some(output.to_path_buf()),
        }),
        OutputFormat::Checkstyle => Box::new(CheckstyleReporter {
            output_path: Some(output.to_path_buf()),
        }),
        OutputFormat::Sonar => Box::new(SonarReporter {
            output_path: Some(output.to_path_buf()),
        }),
    }
}
