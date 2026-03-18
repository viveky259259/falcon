use anyhow::Result;
use clap::{Parser, Subcommand};
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
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(
    name = "falcon",
    version,
    about = "Falcon — Rust-powered static analysis for Flutter/Dart",
    long_about = "A blazing-fast static analysis tool for Flutter and Dart projects.\nAnalyzes code metrics, enforces lint rules, and detects unused code."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run full analysis (metrics + rules + unused detection)
    Analyze {
        /// Path to analyze (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path (for file-based formats)
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Only analyze files changed since this git ref (e.g. HEAD~1, main)
        #[arg(long)]
        since: Option<String>,

        /// Use baseline — only report new violations
        #[arg(long)]
        baseline: bool,

        /// Minimum severity to fail on (error, warning, info)
        #[arg(long, default_value = "error")]
        fail_on: FailLevel,

        /// Exclude public API from analysis
        #[arg(long)]
        exclude_public_api: bool,
    },

    /// Calculate code metrics only
    Metrics {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path (for file-based formats)
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Check for unused code declarations
    #[command(name = "check-unused-code")]
    CheckUnusedCode {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path (for file-based formats)
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },

    /// Check for unused Dart files
    #[command(name = "check-unused-files")]
    CheckUnusedFiles {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path (for file-based formats)
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },

    /// Check for unused dependencies in pubspec.yaml
    #[command(name = "check-dependencies")]
    CheckDependencies {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path (for file-based formats)
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },

    /// Generate a default falcon.yaml configuration file
    Init {
        /// Path where to create falcon.yaml
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Watch for file changes and re-analyze continuously
    Watch {
        /// Path to watch
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Manage analysis baselines
    Baseline {
        #[command(subcommand)]
        action: BaselineAction,
    },

    /// Show file dependency graph
    #[command(name = "dep-graph")]
    DepGraph {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show dependents of a specific file
        #[arg(long)]
        file: Option<PathBuf>,
    },

    /// Analyze all packages in a monorepo workspace
    #[command(name = "workspace")]
    Workspace {
        /// Workspace root path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Generate rule documentation
    Docs {
        /// Output directory for generated docs
        #[arg(default_value = "docs")]
        output: PathBuf,
    },

    /// Validate falcon.yaml configuration
    Validate {
        /// Path containing falcon.yaml
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Detect cyclic import dependencies
    #[command(name = "check-cycles")]
    CheckCycles {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Detect unused method/function parameters
    #[command(name = "check-unused-params")]
    CheckUnusedParams {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },

    /// Detect dead code paths (unreachable code after return/throw)
    #[command(name = "check-dead-code")]
    CheckDeadCode {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },

    /// Detect unused localization keys in ARB files
    #[command(name = "check-unused-l10n")]
    CheckUnusedL10n {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },

    /// Detect over-promoted and under-promoted dependencies
    #[command(name = "check-promoted-deps")]
    CheckPromotedDeps {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum BaselineAction {
    /// Create a baseline from current analysis
    Create {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    Console,
    Json,
    Html,
    Sarif,
    Codeclimate,
    Checkstyle,
    Sonar,
    Gitlab,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum FailLevel {
    Error,
    Warning,
    Info,
}

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
            exclude_public_api: _,
        } => {
            let config_path = config.as_deref().unwrap_or(&path);
            let falcon_config = FalconConfig::load(config_path)?;
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
            };

            get_reporter(&format, &output).report_analysis(&final_report);

            if should_fail(&final_report, &fail_on) {
                process::exit(1);
            }
        }
        Commands::Metrics {
            path,
            format,
            output,
            config,
        } => {
            let config_path = config.as_deref().unwrap_or(&path);
            let falcon_config = FalconConfig::load(config_path)?;
            let falcon = Falcon::new(falcon_config)?;
            let metrics = falcon.calculate_metrics(&path)?;

            get_reporter(&format, &output).report_metrics(&metrics);
        }
        Commands::CheckUnusedCode {
            path,
            format,
            output,
        } => {
            let config = FalconConfig::load(&path)?;
            let falcon = Falcon::new(config)?;
            let issues = falcon.check_unused_code(&path)?;

            get_reporter(&format, &output).report_issues(&issues);

            if !issues.is_empty() {
                process::exit(1);
            }
        }
        Commands::CheckUnusedFiles {
            path,
            format,
            output,
        } => {
            let config = FalconConfig::load(&path)?;
            let falcon = Falcon::new(config)?;
            let issues = falcon.check_unused_files(&path)?;

            get_reporter(&format, &output).report_issues(&issues);

            if !issues.is_empty() {
                process::exit(1);
            }
        }
        Commands::CheckDependencies {
            path,
            format,
            output,
        } => {
            let config = FalconConfig::load(&path)?;
            let falcon = Falcon::new(config)?;
            let issues = falcon.check_dependencies(&path)?;

            get_reporter(&format, &output).report_issues(&issues);

            if !issues.is_empty() {
                process::exit(1);
            }
        }
        Commands::Init { path } => {
            falcon::init_config(&path)?;
            println!("Created falcon.yaml in {}", path.display());
        }
        Commands::Watch { path, config } => {
            let config_path = config.as_deref().unwrap_or(&path);
            let falcon_config = FalconConfig::load(config_path)?;
            falcon::incremental::watcher::watch(&path, falcon_config)?;
        }
        Commands::Baseline { action } => match action {
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
        },
        Commands::DepGraph { path, file } => {
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

                println!("{} Dependencies for: {}", "→".bright_cyan(), target.display());

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
                stats.sort_by(|a, b| b.0.cmp(&a.0));

                println!("  {} (by number of dependents):", "Most depended-on files".bright_green());
                for (count, file) in stats.iter().take(20) {
                    let rel = file.strip_prefix(&path).unwrap_or(file);
                    println!("    {:>4} ← {}", count, rel.display());
                }
            }
        }
        Commands::Workspace { path } => {
            let report = falcon::workspace::analyze_workspace(&path)?;
            if report.total_errors > 0 {
                process::exit(1);
            }
        }
        Commands::Docs { output } => {
            falcon::docs::generate_rule_docs(&output)?;
        }
        Commands::Validate { path } => {
            let errors = falcon::config::validator::validate_config(&path);
            falcon::config::validator::print_validation_results(&errors);
            if errors.iter().any(|e| matches!(e.severity, falcon::config::validator::ConfigErrorSeverity::Error)) {
                process::exit(1);
            }
        }
        Commands::CheckCycles { path } => {
            let config = FalconConfig::load(&path)?;
            let exclude: Vec<glob::Pattern> = config
                .exclude
                .iter()
                .filter_map(|p| glob::Pattern::new(p).ok())
                .collect();

            let graph = DependencyGraph::build(&path, &exclude);
            let (issues, cycles) = falcon::resolver::cyclic::detect_cycles(&graph, &path);

            println!("{}", falcon::resolver::cyclic::format_cycles(&cycles, &path));

            if !issues.is_empty() {
                println!("{} {} files involved in cycles", "⚠".yellow().bold(), issues.len());
                process::exit(1);
            }
        }
        Commands::CheckUnusedParams { path, format, output } => {
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
        }
        Commands::CheckDeadCode { path, format, output } => {
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
        }
        Commands::CheckUnusedL10n { path, format, output } => {
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
        }
        Commands::CheckPromotedDeps { path, format, output } => {
            let issues = falcon::resolver::cyclic::detect_promoted_deps(&path);
            get_reporter(&format, &output).report_issues(&issues);

            if !issues.is_empty() {
                process::exit(1);
            }
        }
    }

    Ok(())
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
    let report = falcon.analyze_files(path, &affected_vec)?;

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

fn get_reporter(format: &OutputFormat, output: &PathBuf) -> Box<dyn Reporter> {
    match format {
        OutputFormat::Console => Box::new(ConsoleReporter),
        OutputFormat::Json => Box::new(JsonReporter),
        OutputFormat::Html => Box::new(HtmlReporter {
            output_path: output.clone(),
        }),
        OutputFormat::Sarif => Box::new(SarifReporter {
            output_path: Some(output.clone()),
        }),
        OutputFormat::Codeclimate | OutputFormat::Gitlab => Box::new(CodeClimateReporter {
            output_path: Some(output.clone()),
        }),
        OutputFormat::Checkstyle => Box::new(CheckstyleReporter {
            output_path: Some(output.clone()),
        }),
        OutputFormat::Sonar => Box::new(SonarReporter {
            output_path: Some(output.clone()),
        }),
    }
}
