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

        /// Apply a named rule preset (recommended, strict, flutter, riverpod, bloc, performance, ai-generated)
        #[arg(long)]
        preset: Option<String>,

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

    /// Explain a rule with examples and context
    Explain {
        /// Rule name to explain (or 'list' to show all rules)
        rule: String,
    },

    /// AI configuration and tools
    #[command(name = "ai")]
    Ai {
        #[command(subcommand)]
        action: AiAction,
    },

    /// Auto-fix lint issues
    Fix {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Preview fixes without applying
        #[arg(long)]
        preview: bool,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Score unused code issues with confidence levels
    #[command(name = "check-unused-confidence")]
    CheckUnusedConfidence {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Minimum confidence threshold (0-100)
        #[arg(long, default_value = "0")]
        min_confidence: u8,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Enforce clean architecture layer dependencies
    #[command(name = "check-layers")]
    CheckLayers {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Check import restriction rules
    #[command(name = "check-imports")]
    CheckImports {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Calculate cognitive complexity for all functions
    #[command(name = "cognitive-complexity")]
    CognitiveComplexity {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Threshold above which functions are flagged
        #[arg(long, default_value = "15")]
        threshold: u32,
    },

    /// Detect widget rebuild issues and build method complexity
    #[command(name = "check-widgets")]
    CheckWidgets {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Detect async/await anti-patterns
    #[command(name = "check-async")]
    CheckAsync {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Review code changes (pattern consistency, naming, error handling)
    Review {
        /// Path to project root
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Git ref to diff against (e.g., HEAD~1, main, origin/main)
        #[arg(long, default_value = "HEAD~1")]
        diff: String,

        /// Review strictness level
        #[arg(long, default_value = "standard")]
        strictness: falcon::review::pr_review::ReviewStrictness,
    },

    /// Analyze codebase health, god files, tech debt, and hotspots
    #[command(name = "codebase-intel")]
    CodebaseIntel {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Plugin management
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Rule presets (recommended, strict, flutter, riverpod, bloc, performance)
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },

    /// Dashboard and analytics
    Dashboard {
        #[command(subcommand)]
        action: DashboardAction,
    },

    /// Show quality trends from analysis history
    Trends {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Number of recent snapshots to compare
        #[arg(long, default_value = "10")]
        last: usize,
    },

    /// Analyze rule impact and get auto-tune recommendations
    #[command(name = "rule-impact")]
    RuleImpact {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Export metrics (prometheus, json, webhook)
    Export {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Export format
        #[arg(long, default_value = "json")]
        format: ExportFormat,

        /// Output file (optional, prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Webhook URL (for webhook format)
        #[arg(long)]
        webhook_url: Option<String>,
    },

    /// Migrate from DCM (Dart Code Metrics) to Falcon
    #[command(name = "migrate-from-dcm")]
    MigrateFromDcm {
        /// Path to DCM analysis_options.yaml
        #[arg(default_value = "analysis_options.yaml")]
        config_path: PathBuf,

        /// Output path for falcon.yaml
        #[arg(long, default_value = ".")]
        output: PathBuf,
    },

    /// Show DCM to Falcon feature gap report
    #[command(name = "feature-gap")]
    FeatureGap,

    /// Run performance benchmark on a project
    Benchmark {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Generate rule documentation
    #[command(name = "rule-docs")]
    RuleDocs {
        /// Output format (console or markdown)
        #[arg(long, default_value = "console")]
        format: DocFormat,

        /// Output file for markdown format
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Compare Falcon analysis with dart analyze
    Compare {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Analyze projects for a showcase report
    Showcase {
        /// Paths to projects to analyze
        paths: Vec<PathBuf>,

        /// Output format (console or markdown)
        #[arg(long, default_value = "console")]
        format: DocFormat,

        /// Output file for markdown format
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show Falcon's stability contract and guarantees
    #[command(name = "stability-contract")]
    StabilityContract,

    /// Show rule deprecation status
    #[command(name = "deprecation-status")]
    DeprecationStatus,

    /// Track performance over time
    #[command(name = "perf-track")]
    PerfTrack {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show history instead of recording a new snapshot
        #[arg(long)]
        history: bool,

        /// Number of recent entries to show
        #[arg(long, default_value = "20")]
        last: usize,
    },

    /// Manage issue suppressions and false-positive tracking
    Suppress {
        #[command(subcommand)]
        action: SuppressAction,
    },

    /// Community features — rule requests, voting, contributed rules
    Community {
        #[command(subcommand)]
        action: CommunityAction,
    },

    /// Calculate AI Code Quality Score (0-100) with 6-dimension breakdown
    #[command(name = "ai-score")]
    AiScore {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show README badge markdown
        #[arg(long)]
        badge: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate a State of AI-Generated Flutter Code report
    #[command(name = "ai-report")]
    AiReport {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format (console or markdown)
        #[arg(long, default_value = "console")]
        format: DocFormat,

        /// Output file for markdown format
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Analyze code provenance — detect AI-generated vs human-written code
    Provenance {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show per-file details
        #[arg(long)]
        verbose: bool,
    },

    /// Start Falcon MCP server (stdio) for AI tool integration
    #[command(name = "mcp")]
    Mcp,

    /// Post analysis results as a GitHub PR comment
    #[command(name = "pr-comment")]
    PrComment {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// GitHub repository owner
        #[arg(long)]
        owner: Option<String>,

        /// GitHub repository name
        #[arg(long)]
        repo: Option<String>,

        /// PR number
        #[arg(long)]
        pr: Option<u32>,

        /// Only print the comment body without posting
        #[arg(long)]
        dry_run: bool,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Send webhook notification with analysis results
    Webhook {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Webhook URL
        #[arg(long)]
        url: String,

        /// Event type (analysis, score, drift)
        #[arg(long, default_value = "analysis")]
        event: String,
    },

    /// Record and view AI tool benchmark comparisons
    #[command(name = "benchmark-db")]
    BenchmarkDb {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// AI tool name (cursor, copilot, claude, gemini, human)
        #[arg(long)]
        tool: Option<String>,

        /// Show benchmark summary instead of recording
        #[arg(long)]
        summary: bool,
    },

    /// Analyze platform channel code (Kotlin/Swift)
    #[command(name = "check-platform")]
    CheckPlatform {
        /// Path to Flutter project root
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Analyze code generation quality (.g.dart, .freezed.dart, etc.)
    #[command(name = "check-codegen")]
    CheckCodegen {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Run DevTools-style performance analysis
    #[command(name = "check-perf")]
    CheckPerf {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Start the Falcon HTTP API server
    #[command(name = "api")]
    Api {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(long, default_value = "8090")]
        port: u16,
    },

    /// Detect convention drift in new or changed code
    #[command(name = "drift")]
    Drift {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Only analyze files changed since this git ref (e.g. HEAD~1, main)
        #[arg(long)]
        since: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Self-tune rules based on usage patterns and suppression history
    #[command(name = "self-tune")]
    SelfTune {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Track AI Code Quality Score over time
    #[command(name = "score-track")]
    ScoreTrack {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show history instead of recording a new snapshot
        #[arg(long)]
        history: bool,

        /// Number of recent entries to show
        #[arg(long, default_value = "20")]
        last: usize,
    },

    /// Auto-detect team conventions (naming, architecture, state management)
    Conventions {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SuppressAction {
    /// Add a suppression for a rule
    Add {
        /// Rule name
        #[arg(long)]
        rule: String,

        /// File path
        #[arg(long)]
        file: String,

        /// Line number (optional)
        #[arg(long)]
        line: Option<usize>,

        /// Reason for suppression
        #[arg(long)]
        reason: String,

        /// Category: false-positive, wont-fix, acknowledged, deferred
        #[arg(long, default_value = "acknowledged")]
        category: String,

        /// Project root
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },

    /// Show suppression statistics
    Stats {
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// List all suppressions
    List {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum CommunityAction {
    /// Submit a rule request
    Request {
        /// Rule name
        #[arg(long)]
        name: String,

        /// Description
        #[arg(long)]
        desc: String,

        /// Category (dart, flutter, riverpod, bloc, etc.)
        #[arg(long, default_value = "dart")]
        category: String,

        /// Project root for storing requests
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },

    /// Vote on a rule request
    Vote {
        /// Request ID
        id: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,
    },

    /// List rule requests
    Requests {
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Show community-contributed rules
    Contributed,
}

#[derive(Subcommand)]
enum PluginAction {
    /// Create a new plugin scaffold
    Create {
        /// Plugin name
        name: String,

        /// Plugin type (wasm or preset)
        #[arg(long, default_value = "wasm")]
        r#type: String,

        /// Directory to create plugin in
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
    },

    /// List installed plugins
    List,

    /// Install a plugin from a local path
    Install {
        /// Path to plugin directory
        path: PathBuf,
    },

    /// Search the plugin registry
    Search {
        /// Search query
        #[arg(default_value = "*")]
        query: String,
    },

    /// Test a plugin
    Test {
        /// Path to plugin directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum PresetAction {
    /// List available presets
    List,

    /// Show details for a preset
    Show {
        /// Preset name
        name: String,
    },

    /// Apply a preset to falcon.yaml
    Apply {
        /// Preset name
        name: String,

        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum DashboardAction {
    /// Capture an analysis snapshot to history
    Snapshot {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Start the local web dashboard
    Serve {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Port number
        #[arg(long, default_value = "8080")]
        port: u16,
    },

    /// Show snapshot history
    History {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Number of entries to show
        #[arg(long, default_value = "10")]
        last: usize,
    },
}

#[derive(Subcommand)]
enum AiAction {
    /// Interactive AI configuration setup
    Setup {
        /// Path containing falcon.yaml
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Show AI configuration status
    Status {
        /// Path containing falcon.yaml
        #[arg(default_value = ".")]
        path: PathBuf,
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

#[derive(Clone, Debug, clap::ValueEnum)]
enum ExportFormat {
    Prometheus,
    Json,
    Webhook,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum DocFormat {
    Console,
    Markdown,
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
            preset,
            exclude_public_api: _,
        } => {
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
        Commands::Explain { rule } => {
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
        }
        Commands::Ai { action } => match action {
            AiAction::Setup { path } => {
                falcon::ai::config::generate_ai_setup(&path)?;
                println!(
                    "{} AI configuration added to falcon.yaml",
                    "✓".green().bold()
                );
                println!("  Edit falcon.yaml to set your provider and API key.");
                println!("  Supported providers: openai, anthropic, gemini, local (Ollama)");
            }
            AiAction::Status { path } => {
                let config = FalconConfig::load(&path)?;
                let ai = &config.ai;
                println!();
                println!("  {} AI Configuration Status", "falcon".bright_cyan().bold());
                println!();
                println!("  Enabled:   {}", if ai.enabled { "yes".green() } else { "no".red() });
                println!("  Provider:  {:?}", ai.provider);
                println!("  Model:     {}", ai.effective_model());
                println!("  API Key:   {}", if ai.resolve_api_key().is_some() { "configured".green() } else { "not set".yellow() });
                println!("  Available: {}", if ai.is_available() { "yes".green() } else { "no".red() });
                println!();
                println!("  Feature Toggles:");
                println!("    Confidence scoring:      {}", if ai.features.confidence_scoring { "on" } else { "off" });
                println!("    Smart fixes:             {}", if ai.features.smart_fixes { "on" } else { "off" });
                println!("    Explanations:            {}", if ai.features.explanations { "on" } else { "off" });
                println!("    False positive reduction: {}", if ai.features.false_positive_reduction { "on" } else { "off" });
                println!();
            }
        },
        Commands::Fix { path, preview, config } => {
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
                println!(
                    "  {} Applied {} fix(es).",
                    "✓".green().bold(),
                    applied
                );
            }
        }
        Commands::CheckUnusedConfidence {
            path,
            min_confidence,
            config,
        } => {
            let config_path = config.as_deref().unwrap_or(&path);
            let falcon_config = FalconConfig::load(config_path)?;
            let falcon = Falcon::new(falcon_config)?;
            let report = falcon.analyze(&path)?;

            let results = falcon::ai::confidence::score_unused_issues(&report.issues, &path);
            falcon::ai::confidence::print_confidence_results(
                &results,
                Some(min_confidence),
            );
        }
        Commands::CheckLayers { path } => {
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

                    let issues = falcon::analysis::layer_enforcement::enforce_layers(&path, &layers, &exclude);
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
        }
        Commands::CheckImports { path } => {
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
        }
        Commands::CognitiveComplexity { path, threshold } => {
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
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
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
                println!("\n  {} {} function(s) exceed threshold of {}.", "⚠".yellow(), flagged, threshold);
                process::exit(1);
            }
        }
        Commands::CheckWidgets { path } => {
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
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
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
        }
        Commands::CheckAsync { path } => {
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
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "dart"))
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
        }
        Commands::Review { path, diff, strictness } => {
            let config = FalconConfig::load(&path)?;
            let report = falcon::review::pr_review::review_diff(&path, &diff, &config, strictness)?;
            falcon::review::pr_review::print_review(&report);
        }
        Commands::CodebaseIntel { path } => {
            let config = FalconConfig::load(&path)?;
            let report = falcon::review::codebase_intel::analyze_codebase(&path, &config)?;
            falcon::review::codebase_intel::print_codebase_report(&report, &path);
        }
        Commands::Plugin { action } => match action {
            PluginAction::Create { name, r#type, dir } => {
                let plugin_type = match r#type.as_str() {
                    "wasm" => falcon::plugins::manifest::PluginType::Wasm,
                    "native" => falcon::plugins::manifest::PluginType::Native,
                    "preset" => falcon::plugins::manifest::PluginType::Preset,
                    _ => {
                        eprintln!("Invalid plugin type '{}'. Use: wasm, native, preset", r#type);
                        process::exit(1);
                    }
                };
                falcon::plugins::scaffold::create_plugin(&name, &dir, plugin_type)?;
            }
            PluginAction::List => {
                let plugin_dir = get_plugin_dir();
                let plugins = falcon::plugins::scaffold::list_plugins(&plugin_dir)?;
                falcon::plugins::scaffold::print_plugins(
                    &plugins
                        .iter()
                        .map(|m| m.clone())
                        .collect::<Vec<_>>(),
                );
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
        },
        Commands::Preset { action } => match action {
            PresetAction::List => {
                let presets = falcon::plugins::presets::list_presets();
                falcon::plugins::presets::print_presets(&presets);
            }
            PresetAction::Show { name } => {
                match falcon::plugins::presets::get_preset(&name) {
                    Some(preset) => falcon::plugins::presets::print_preset_detail(&preset),
                    None => {
                        eprintln!("Unknown preset '{}'. Use: falcon preset list", name);
                        process::exit(1);
                    }
                }
            }
            PresetAction::Apply { name, path } => {
                match falcon::plugins::presets::get_preset(&name) {
                    Some(preset) => falcon::plugins::presets::apply_preset(&preset, &path)?,
                    None => {
                        eprintln!("Unknown preset '{}'. Use: falcon preset list", name);
                        process::exit(1);
                    }
                }
            }
        },
        Commands::Dashboard { action } => match action {
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
        },
        Commands::Trends { path, last } => {
            let history = falcon::dashboard::snapshot::load_history(&path)?;
            match falcon::dashboard::trends::analyze_trends(&history, last) {
                Some(report) => falcon::dashboard::trends::print_trend_report(&report),
                None => {
                    println!("  Need at least 2 snapshots for trends. Run: falcon dashboard snapshot");
                }
            }
        }
        Commands::RuleImpact { path } => {
            let history = falcon::dashboard::snapshot::load_history(&path)?;
            if history.is_empty() {
                println!("  No snapshots yet. Run: falcon dashboard snapshot");
            } else {
                let impacts = falcon::dashboard::rule_impact::measure_rule_impact(&history);
                falcon::dashboard::rule_impact::print_rule_impact(&impacts);
                let recs = falcon::dashboard::rule_impact::auto_tune_recommendations(&impacts);
                falcon::dashboard::rule_impact::print_recommendations(&recs);
            }
        }
        Commands::Export { path, format, output, webhook_url } => {
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
                            println!("  {} Prometheus metrics saved to {}", "✓".green().bold(), out.display());
                        }
                        None => print!("{}", metrics),
                    }
                }
                ExportFormat::Json => {
                    let json = falcon::dashboard::exports::export_json(&snapshot)?;
                    match output {
                        Some(out) => {
                            std::fs::write(&out, &json)?;
                            println!("  {} JSON export saved to {}", "✓".green().bold(), out.display());
                        }
                        None => println!("{}", json),
                    }
                }
                ExportFormat::Webhook => {
                    let url = webhook_url.as_deref().unwrap_or("http://localhost:9000/webhook");
                    let project = path.file_name().and_then(|f| f.to_str()).unwrap_or("project");
                    let payload = falcon::dashboard::exports::WebhookPayload::from_snapshot(&snapshot, project);
                    let json = payload.to_json()?;
                    println!("{}", json);
                    println!("  Webhook payload generated for {}", url.bright_blue());
                }
            }
        }
        Commands::MigrateFromDcm { config_path, output } => {
            let result = falcon::migration::dcm::migrate_from_dcm(&config_path)?;
            falcon::migration::dcm::print_migration_result(&result);

            let output_path = output.join("falcon.yaml");
            std::fs::write(&output_path, &result.falcon_yaml_content)?;
            println!(
                "  {} falcon.yaml written to {}",
                "✓".green().bold(),
                output_path.display()
            );
        }
        Commands::FeatureGap => {
            let report = falcon::migration::dcm::feature_gap_report();
            println!("{}", report);
        }
        Commands::Benchmark { path } => {
            let result = falcon::benchmark::run_benchmark(&path)?;
            falcon::benchmark::print_benchmark(&result);
        }
        Commands::RuleDocs { format, output } => {
            match format {
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
            }
        }
        Commands::Compare { path } => {
            let result = falcon::benchmark_compare::compare_with_dart_analyze(&path)?;
            falcon::benchmark_compare::print_compare_result(&result);
        }
        Commands::Showcase { paths, format, output } => {
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
                    Err(e) => eprintln!("  {} Failed to analyze {}: {}", "✗".red(), path.display(), e),
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
        }
        Commands::StabilityContract => {
            let contract = falcon::stability::contract::StabilityContract::default();
            falcon::stability::contract::print_stability_contract(&contract);
        }
        Commands::DeprecationStatus => {
            falcon::stability::deprecation::print_deprecation_status();
        }
        Commands::PerfTrack { path, history, last } => {
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
        }
        Commands::Suppress { action } => match action {
            SuppressAction::Add {
                rule,
                file,
                line,
                reason,
                category,
                path,
            } => {
                let cat = match category.as_str() {
                    "false-positive" | "fp" => falcon::stability::suppression::SuppressionCategory::FalsePositive,
                    "wont-fix" | "wf" => falcon::stability::suppression::SuppressionCategory::WontFix,
                    "acknowledged" | "ack" => falcon::stability::suppression::SuppressionCategory::Acknowledged,
                    "deferred" | "defer" => falcon::stability::suppression::SuppressionCategory::Deferred,
                    _ => {
                        eprintln!("Unknown category '{}'. Use: false-positive, wont-fix, acknowledged, deferred", category);
                        process::exit(1);
                    }
                };
                falcon::stability::suppression::add_suppression(&path, &falcon::stability::suppression::SuppressionRequest {
                    rule: &rule,
                    file: &file,
                    line,
                    reason: &reason,
                    category: cat,
                })?;
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
        },
        Commands::Mcp => {
            falcon::mcp::server::run_mcp_server()?;
        }
        Commands::PrComment { path, owner, repo, pr, dry_run, config } => {
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
                    owner, repo, pr
                );
            } else {
                falcon::ci::pr_comment::post_comment_auto(&comment)?;
                println!(
                    "  {} Posted analysis to PR (auto-detected)",
                    "✓".green().bold()
                );
            }

            falcon::ci::pr_comment::write_github_step_summary(&report, &path)?;
        }
        Commands::Webhook { path, url, event } => {
            let config = FalconConfig::load(&path)?;
            let falcon_inst = Falcon::new(config)?;
            let report = falcon_inst.analyze(&path)?;
            let project = path.file_name().and_then(|f| f.to_str()).unwrap_or("project");

            match event.as_str() {
                "analysis" => {
                    falcon::ci::webhook::send_analysis_webhook(&url, project, &report)?;
                    println!(
                        "  {} Sent analysis webhook to {}",
                        "✓".green().bold(),
                        url
                    );
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
        }
        Commands::BenchmarkDb { path, tool, summary } => {
            if summary {
                let db = falcon::ai_score::benchmark_db::load_benchmark_db(&path)?;
                let stats = falcon::ai_score::benchmark_db::compute_tool_stats(&db);
                falcon::ai_score::benchmark_db::print_benchmark_summary(&stats);
            } else if let Some(tool_name) = tool {
                let project = path.file_name().and_then(|f| f.to_str()).unwrap_or("project");
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
        }
        Commands::CheckPlatform { path } => {
            let issues = falcon::analysis::platform_channels::analyze_platform_channels(&path);
            falcon::analysis::platform_channels::print_platform_summary(&issues);
            if !issues.is_empty() {
                get_reporter(&OutputFormat::Console, &PathBuf::from("")).report_issues(&issues);
            }
        }
        Commands::CheckCodegen { path } => {
            let report = falcon::analysis::codegen_quality::analyze_codegen(&path);
            falcon::analysis::codegen_quality::print_codegen_report(&report);
        }
        Commands::CheckPerf { path } => {
            let report = falcon::analysis::devtools_bridge::analyze_performance(&path);
            falcon::analysis::devtools_bridge::print_perf_report(&report);
        }
        Commands::Api { host, port } => {
            falcon::api::server::start_api_server(&host, port)?;
        }
        Commands::Drift { path, since, json } => {
            let report = falcon::ai_score::drift::detect_drift(&path, since.as_deref())?;
            if json {
                let j = serde_json::to_string_pretty(&report)?;
                println!("{}", j);
            } else {
                falcon::ai_score::drift::print_drift_report(&report);
            }
        }
        Commands::SelfTune { path } => {
            let history = falcon::ai_score::self_tune::record_analysis(&path)?;
            let recs = falcon::ai_score::self_tune::generate_recommendations(&history);
            falcon::ai_score::self_tune::print_tune_recommendations(&recs, &history);
        }
        Commands::ScoreTrack { path, history, last } => {
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
        }
        Commands::AiScore { path, badge, json } => {
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
        }
        Commands::AiReport { path, format, output } => {
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
        }
        Commands::Provenance { path, verbose } => {
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
        }
        Commands::Conventions { path, json } => {
            let report = falcon::ai_score::convention::detect_conventions(&path)?;
            if json {
                let j = serde_json::to_string_pretty(&report)?;
                println!("{}", j);
            } else {
                falcon::ai_score::convention::print_convention_report(&report);
            }
        }
        Commands::Community { action } => match action {
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
        },
    }

    Ok(())
}

fn get_plugin_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"));
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
