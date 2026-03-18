use anyhow::Result;
use clap::{Parser, Subcommand};
use falcon::config::FalconConfig;
use falcon::reporters::console::ConsoleReporter;
use falcon::reporters::html::HtmlReporter;
use falcon::reporters::json::JsonReporter;
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

        /// Output file path (for HTML format)
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Calculate code metrics only
    Metrics {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "console")]
        format: OutputFormat,

        /// Output file path (for HTML format)
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

        /// Output file path (for HTML format)
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

        /// Output file path (for HTML format)
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

        /// Output file path (for HTML format)
        #[arg(short, long, default_value = "falcon-report.html")]
        output: PathBuf,
    },

    /// Generate a default falcon.yaml configuration file
    Init {
        /// Path where to create falcon.yaml
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    Console,
    Json,
    Html,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{}: {}", "error".to_string(), e);
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
        } => {
            let config_path = config.as_deref().unwrap_or(&path);
            let falcon_config = FalconConfig::load(config_path)?;
            let falcon = Falcon::new(falcon_config)?;
            let report = falcon.analyze(&path)?;
            let has_errors = report.has_errors();

            get_reporter(&format, &output).report_analysis(&report);

            if has_errors {
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
    }

    Ok(())
}

fn get_reporter(format: &OutputFormat, output: &PathBuf) -> Box<dyn Reporter> {
    match format {
        OutputFormat::Console => Box::new(ConsoleReporter),
        OutputFormat::Json => Box::new(JsonReporter),
        OutputFormat::Html => Box::new(HtmlReporter {
            output_path: output.clone(),
        }),
    }
}
