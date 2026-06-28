use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
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
use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Parser)]
#[command(
    name = "falcon",
    version,
    about = "Falcon — Rust-powered static analysis for Flutter/Dart",
    long_about = "A blazing-fast static analysis tool for Flutter and Dart projects.\nAnalyzes code metrics, enforces lint rules, and detects unused code.",
    after_long_help = GROUPED_HELP,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

const CUTOVER_HELP: &str = r#"Falcon — Rust-powered static analysis for Flutter/Dart

Usage: falcon <COMMAND>

Commands:
  review  Review changed Dart files for pull requests
  check   Run project checks and static analysis
  fix     Apply safe automated fixes
  score   Calculate the AI Code Quality Score
  x       Advanced, legacy, and experimental commands

Options:
  -h, --help         Print help
  -V, --version      Print version
      --legacy-help  Print the full legacy command list for this release

Run `falcon <command> --help` for command-specific options.
"#;

const GROUPED_HELP: &str = r#"
SEMANTIC COMMAND GROUPS:

  Analysis          analyze, metrics, score, cognitive-complexity, codebase-intel
  Code Checks       check-unused-code, check-unused-files, check-cycles, check-async,
                    check-widgets, check-dead-code, check-layers, check-imports,
                    check-platform, check-codegen, check-perf, check-unused-params,
                    check-unused-l10n, check-dependencies, check-promoted-deps
  Comparison        compare-branches, compare-reports, compare, history
  AI Intelligence   score, ai-report, provenance, conventions, drift, predict,
                    discover-rules, refactor-sim, test-gen, vuln-scan
  CI/CD             pr-comment, webhook, export, fix
  Tracking          trends, history, benchmark, score-track, perf-track, fix-track
  Configuration     init, validate, explain, preset, suppress, baseline, self-tune
  App Management    manage, review, watch, runtime-check, live, devtools, workspace
  Flutter Quality   asset-audit, theme-audit, l10n-coverage, deeplink-validate,
                    animation-audit, golden-gen
  Integration       mcp, api
  Flutter SDK       flutter (passthrough — every flutter subcommand: run, build, test, pub, doctor, …)
  Enterprise        cloud, enterprise, certify, marketplace
  Setup             init, update

Use 'falcon <command> --help' for details on any command.
"#;

#[derive(Subcommand)]
enum Commands {
    /// Run full analysis (metrics + rules + unused detection)
    #[command(display_order = 1)]
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

    /// Run project checks and static analysis
    #[command(display_order = 1)]
    Check {
        /// Path to check (defaults to current directory)
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

        /// Minimum severity to fail on (error, warning, info)
        #[arg(long, default_value = "error")]
        fail_on: FailLevel,

        /// Apply a named rule preset (recommended, strict, flutter, riverpod, bloc, performance, ai-generated)
        #[arg(long)]
        preset: Option<String>,

        /// Only report findings not present in this baseline file
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,

        /// Rewrite the baseline file with the current findings
        #[arg(long, value_name = "FILE")]
        update_baseline: Option<PathBuf>,
    },

    /// Categorized smells report: Dead Code, Code Smells, Security Smells.
    #[command(display_order = 1)]
    Smells {
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

        /// Maximum issues to print per category in console output
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    /// Calculate code metrics only
    #[command(display_order = 1)]
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
    #[command(name = "check-unused-code", display_order = 2)]
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
    #[command(name = "check-unused-files", display_order = 2)]
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
    #[command(name = "check-dependencies", display_order = 2)]
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
    #[command(display_order = 7)]
    Init {
        /// Path where to create falcon.yaml
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Watch for file changes and re-analyze continuously
    #[command(display_order = 8)]
    Watch {
        /// Path to watch
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Run a Flutter app and capture any errors to numbered error_N.md files
    #[command(name = "run", display_order = 7)]
    Run {
        /// Path to the Flutter project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Directory where error_N.md files are written (defaults to project root)
        #[arg(long, default_value = ".")]
        output_dir: PathBuf,

        /// Target device ID passed to `flutter run -d`
        #[arg(long)]
        device: Option<String>,

        /// Build flavor passed to `flutter run --flavor`
        #[arg(long)]
        flavor: Option<String>,

        /// Send an OS desktop notification when errors are found
        #[arg(long)]
        notify: bool,

        /// POST a JSON summary to this webhook URL when errors are found
        #[arg(long)]
        webhook: Option<String>,

        /// Live monitor interval in seconds (memory + issue snapshots).
        /// Min 5, max 600, default 30.
        #[arg(
            long,
            default_value_t = falcon::flutter_run::monitor::DEFAULT_MONITOR_INTERVAL_SECS,
            value_parser = clap::value_parser!(u64)
                .range(falcon::flutter_run::monitor::MIN_MONITOR_INTERVAL_SECS
                    ..=falcon::flutter_run::monitor::MAX_MONITOR_INTERVAL_SECS),
        )]
        monitor_interval: u64,

        /// Disable the live memory / issues monitor entirely
        #[arg(long)]
        no_monitor: bool,
    },

    /// Proxy to the Flutter SDK — exposes every `flutter` subcommand (run, build, test, pub, doctor, …)
    #[command(
        name = "flutter",
        display_order = 7,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Flutter {
        /// Arguments forwarded verbatim to the `flutter` CLI (e.g. `falcon flutter build apk --release`)
        #[arg(num_args = 0.., value_name = "ARGS")]
        args: Vec<String>,
    },

    /// Generate AGENTS.md files (root + per-feature) so Codex/Cursor/Aider follow the same rules
    #[command(display_order = 7)]
    Agents {
        #[command(subcommand)]
        action: AgentsAction,
    },

    /// Proxy to FVM — manage Flutter SDK versions (`falcon fvm install 3.24.0`, `falcon fvm use stable`, …)
    #[command(
        name = "fvm",
        display_order = 7,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        disable_help_flag = true,
        disable_help_subcommand = true
    )]
    Fvm {
        /// Arguments forwarded verbatim to the `fvm` CLI
        #[arg(num_args = 0.., value_name = "ARGS")]
        args: Vec<String>,
    },

    /// Run runtime diagnostics on a Flutter app (memory, rendering, network, CPU)
    #[command(name = "runtime-check", display_order = 8)]
    RuntimeCheck {
        /// Path to the Flutter project (used for `flutter run`)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Duration in seconds to collect diagnostics
        #[arg(short, long, default_value = "30")]
        duration: u64,

        /// Output file path for the HTML dashboard report
        #[arg(short, long, default_value = "falcon-runtime-report.html")]
        output: PathBuf,

        /// Memory warning threshold in MB
        #[arg(long, default_value = "150")]
        memory_warn_mb: f64,

        /// Frame build time warning threshold in ms
        #[arg(long, default_value = "16")]
        frame_warn_ms: f64,

        /// Skip HTML report generation (CLI output only)
        #[arg(long)]
        no_html: bool,
    },

    /// Watch a running Flutter app and surface runtime issues in realtime
    #[command(name = "live", display_order = 8)]
    Live {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Total session duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,

        /// Collection window in seconds for each iteration
        #[arg(long, default_value = "10")]
        interval: u64,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Query DevTools-backed runtime APIs directly
    #[command(name = "devtools", display_order = 8)]
    Devtools {
        #[command(subcommand)]
        action: DevtoolsAction,
    },

    /// Audit Flutter project assets — find unused, oversized, and WebP-convertible files
    #[command(name = "asset-audit", display_order = 9)]
    AssetAudit {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-asset-report.html")]
        output: PathBuf,

        /// Size threshold in KB above which an image is flagged (default 200)
        #[arg(long, default_value = "200")]
        size_threshold_kb: u64,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Audit Flutter theme consistency — hardcoded colors, fonts, missing dark mode
    #[command(name = "theme-audit", display_order = 9)]
    ThemeAudit {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-theme-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Analyze localization coverage across all ARB locales
    #[command(name = "l10n-coverage", display_order = 9)]
    L10nCoverage {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-l10n-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Validate deep link configuration across Android, iOS, and Flutter routes
    #[command(name = "deeplink-validate", display_order = 9)]
    DeeplinkValidate {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-deeplink-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Audit Flutter animations for anti-patterns, missing disposal, and jank risks
    #[command(name = "animation-audit", display_order = 9)]
    AnimationAudit {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-animation-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Generate golden (snapshot) test stubs for all discoverable widgets
    #[command(name = "golden-gen", display_order = 9)]
    GoldenGen {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Directory to write generated test files into
        #[arg(short, long, default_value = "test/golden_generated")]
        output_dir: PathBuf,

        /// Preview what would be generated without writing files
        #[arg(long)]
        dry_run: bool,

        /// Output HTML report path
        #[arg(long, default_value = "falcon-golden-report.html")]
        html_output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Manage analysis baselines
    #[command(display_order = 7)]
    Baseline {
        #[command(subcommand)]
        action: BaselineAction,
    },

    /// Show file dependency graph
    #[command(name = "dep-graph", display_order = 12)]
    DepGraph {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show dependents of a specific file
        #[arg(long)]
        file: Option<PathBuf>,
    },

    /// Analyze all packages in a monorepo workspace
    #[command(name = "workspace", display_order = 8)]
    Workspace {
        /// Workspace root path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Generate rule documentation
    #[command(display_order = 12)]
    Docs {
        /// Output directory for generated docs
        #[arg(default_value = "docs")]
        output: PathBuf,
    },

    /// Validate falcon.yaml configuration
    #[command(display_order = 7)]
    Validate {
        /// Path containing falcon.yaml
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Detect cyclic import dependencies
    #[command(name = "check-cycles", display_order = 2)]
    CheckCycles {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Detect unused method/function parameters
    #[command(name = "check-unused-params", display_order = 2)]
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
    #[command(name = "check-dead-code", display_order = 2)]
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
    #[command(name = "check-unused-l10n", display_order = 2)]
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
    #[command(name = "check-promoted-deps", display_order = 2)]
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
    #[command(display_order = 7)]
    Explain {
        /// Rule name to explain (or 'list' to show all rules)
        rule: String,
    },

    /// AI configuration and tools
    #[command(name = "ai", display_order = 4)]
    Ai {
        #[command(subcommand)]
        action: AiAction,
    },

    /// Auto-fix lint issues
    #[command(display_order = 5)]
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
    #[command(name = "check-unused-confidence", display_order = 2)]
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
    #[command(name = "check-layers", display_order = 2)]
    CheckLayers {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Check import restriction rules
    #[command(name = "check-imports", display_order = 2)]
    CheckImports {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Calculate cognitive complexity for all functions
    #[command(name = "cognitive-complexity", display_order = 1)]
    CognitiveComplexity {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Threshold above which functions are flagged
        #[arg(long, default_value = "15")]
        threshold: u32,
    },

    /// Detect widget rebuild issues and build method complexity
    #[command(name = "check-widgets", display_order = 2)]
    CheckWidgets {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Detect async/await anti-patterns
    #[command(name = "check-async", display_order = 2)]
    CheckAsync {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Review code changes (pattern consistency, naming, error handling)
    #[command(display_order = 8)]
    Review {
        /// Path to project root
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Git ref to diff against (e.g., HEAD~1, main, origin/main)
        #[arg(long = "base-ref", alias = "diff", default_value = "origin/main")]
        base_ref: String,

        /// Output format
        #[arg(long, default_value = "text")]
        format: ReviewOutputFormat,

        /// Review strictness level
        #[arg(long, default_value = "standard")]
        strictness: falcon::review::pr_review::ReviewStrictness,

        /// Run dart analyze as a co-pilot and defer same-line Falcon findings
        #[arg(long = "analyzer-copilot")]
        analyzer_copilot: bool,

        /// Keep Falcon findings even when dart analyze reports the same line
        #[arg(long = "no-defer-to-analyzer")]
        no_defer_to_analyzer: bool,

        /// Only report findings not present in this baseline file
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,

        /// Rewrite the baseline file with the current findings
        #[arg(long, value_name = "FILE")]
        update_baseline: Option<PathBuf>,
    },

    /// Analyze codebase health, god files, tech debt, and hotspots
    #[command(name = "codebase-intel", display_order = 1)]
    CodebaseIntel {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Plugin management
    #[command(display_order = 12)]
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Rule presets (recommended, strict, flutter, riverpod, bloc, performance)
    #[command(display_order = 7)]
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },

    /// Dashboard and analytics
    #[command(display_order = 6)]
    Dashboard {
        #[command(subcommand)]
        action: DashboardAction,
    },

    /// Show quality trends from analysis history
    #[command(display_order = 6)]
    Trends {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Number of recent snapshots to compare
        #[arg(long, default_value = "10")]
        last: usize,
    },

    /// Analyze rule impact and get auto-tune recommendations
    #[command(name = "rule-impact", display_order = 7)]
    RuleImpact {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Export metrics (prometheus, json, webhook)
    #[command(display_order = 5)]
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
    #[command(name = "migrate-from-dcm", display_order = 11)]
    MigrateFromDcm {
        /// Path to DCM analysis_options.yaml
        #[arg(default_value = "analysis_options.yaml")]
        config_path: PathBuf,

        /// Output path for falcon.yaml
        #[arg(long, default_value = ".")]
        output: PathBuf,
    },

    /// Show DCM to Falcon feature gap report
    #[command(name = "feature-gap", display_order = 11)]
    FeatureGap,

    /// Run performance benchmark on a project
    #[command(display_order = 6)]
    Benchmark {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Generate rule documentation
    #[command(name = "rule-docs", display_order = 12)]
    RuleDocs {
        /// Output format (console or markdown)
        #[arg(long, default_value = "console")]
        format: DocFormat,

        /// Output file for markdown format
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Compare Falcon analysis with dart analyze
    #[command(display_order = 3)]
    Compare {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Compare two stored analysis runs
    #[command(display_order = 3)]
    CompareReports {
        /// Path to project (where .falcon-data/ lives)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Run number for baseline (1-based, from `falcon history`)
        #[arg(long, default_value = "0")]
        run1: usize,

        /// Run number for comparison (1-based, 0 = latest)
        #[arg(long, default_value = "0")]
        run2: usize,

        /// Output HTML comparison report
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Compare analysis results between two git branches
    #[command(display_order = 3)]
    CompareBranches {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Base branch (e.g. main)
        #[arg(long)]
        base: String,

        /// Branch to compare against base
        #[arg(long)]
        branch: String,

        /// Output HTML comparison report
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Path to falcon.yaml config
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Show analysis run history
    #[command(display_order = 3)]
    History {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Update Falcon to the latest or a specific version
    #[command(display_order = 11)]
    Update {
        /// Target version (e.g. 0.2.0). Omit for latest.
        #[arg(long)]
        version: Option<String>,

        /// List all available versions
        #[arg(long)]
        list: bool,
    },

    /// Analyze projects for a showcase report
    #[command(display_order = 12)]
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
    #[command(name = "stability-contract", display_order = 11)]
    StabilityContract,

    /// Show rule deprecation status
    #[command(name = "deprecation-status", display_order = 11)]
    DeprecationStatus,

    /// Track performance over time
    #[command(name = "perf-track", display_order = 6)]
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
    #[command(display_order = 7)]
    Suppress {
        #[command(subcommand)]
        action: SuppressAction,
    },

    /// Community features — rule requests, voting, contributed rules
    #[command(display_order = 10)]
    Community {
        #[command(subcommand)]
        action: CommunityAction,
    },

    /// Calculate AI Code Quality Score (0-100) with 6-dimension breakdown
    #[command(name = "score", alias = "ai-score", display_order = 1)]
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
    #[command(name = "ai-report", display_order = 4)]
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
    #[command(display_order = 4)]
    Provenance {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show per-file details
        #[arg(long)]
        verbose: bool,
    },

    /// Manage Flutter app — health, deps, architecture, maintenance, build
    #[command(display_order = 8)]
    Manage {
        #[command(subcommand)]
        action: ManageAction,
    },

    /// Start Falcon MCP server (stdio) for AI tool integration
    #[command(name = "mcp", display_order = 9)]
    Mcp,

    /// Post analysis results as a GitHub PR comment
    #[command(name = "pr-comment", display_order = 5)]
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
    #[command(display_order = 5)]
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
    #[command(name = "benchmark-db", display_order = 6)]
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

    /// Simulate a refactoring and analyze impact
    #[command(name = "refactor-sim", display_order = 4)]
    RefactorSim {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Refactoring scenario
        #[arg(long)]
        scenario: falcon::analysis::refactor_sim::RefactorScenario,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate test stubs from code analysis
    #[command(name = "test-gen", display_order = 4)]
    TestGen {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Write test files to disk
        #[arg(long)]
        write: bool,
    },

    /// Scan for security vulnerabilities and anti-patterns
    #[command(name = "vuln-scan", display_order = 4)]
    VulnScan {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Profile AI tools based on benchmark data
    #[command(name = "ai-profile", display_order = 4)]
    AiProfile {
        /// Path to project (reads benchmark-db)
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Discover patterns that could become new rules
    #[command(name = "discover-rules", display_order = 4)]
    DiscoverRules {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Track fix acceptance/rejection effectiveness
    #[command(name = "fix-track", display_order = 6)]
    FixTrack {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Rule name (for recording)
        #[arg(long)]
        rule: Option<String>,

        /// Fix outcome (accepted, rejected, modified)
        #[arg(long)]
        outcome: Option<String>,

        /// File where fix was applied
        #[arg(long)]
        file: Option<String>,

        /// Show effectiveness report
        #[arg(long)]
        report: bool,
    },

    /// Record a project into the cross-project learning database
    #[command(display_order = 12)]
    Learn {
        /// Path to project to record
        #[arg(default_value = ".")]
        project: PathBuf,

        /// Path to the learning database (defaults to current dir)
        #[arg(long, default_value = ".")]
        db: PathBuf,

        /// Show insights instead of recording
        #[arg(long)]
        insights: bool,
    },

    /// Predict production risks based on code patterns
    #[command(name = "predict", display_order = 4)]
    Predict {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Check Flutter upgrade compatibility (deprecated APIs)
    #[command(name = "upgrade-check")]
    UpgradeCheck {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Falcon Cloud — team dashboards and multi-project tracking
    #[command(display_order = 10)]
    Cloud {
        #[command(subcommand)]
        action: CloudAction,
    },

    /// Enterprise features — policies, audit, compliance
    #[command(display_order = 10)]
    Enterprise {
        #[command(subcommand)]
        action: EnterpriseAction,
    },

    /// Browse the Falcon marketplace
    #[command(display_order = 10)]
    Marketplace {
        /// Search query
        #[arg(default_value = "")]
        query: String,
    },

    /// Evaluate project for Falcon certification
    #[command(display_order = 10)]
    Certify {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// View Falcon partner integrations
    #[command(display_order = 10)]
    Partners,

    /// Analyze platform channel code (Kotlin/Swift)
    #[command(name = "check-platform", display_order = 2)]
    CheckPlatform {
        /// Path to Flutter project root
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Analyze code generation quality (.g.dart, .freezed.dart, etc.)
    #[command(name = "check-codegen", display_order = 2)]
    CheckCodegen {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Run DevTools-style performance analysis
    #[command(name = "check-perf", display_order = 2)]
    CheckPerf {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Start the Falcon HTTP API server
    #[command(name = "api", display_order = 9)]
    Api {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(long, default_value = "8090")]
        port: u16,
    },

    /// Detect convention drift in new or changed code
    #[command(name = "drift", display_order = 4)]
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
    #[command(name = "self-tune", display_order = 7)]
    SelfTune {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Track AI Code Quality Score over time
    #[command(name = "score-track", display_order = 6)]
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
    #[command(display_order = 4)]
    Conventions {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Experimental / extended commands (see council roadmap)
    #[command(
        name = "x",
        about = "Experimental / extended commands (see council roadmap)",
        display_order = 99
    )]
    X {
        #[command(subcommand)]
        action: XAction,
    },
}

/// Subcommands exposed under the `falcon x` namespace.
///
/// These all re-dispatch to existing top-level commands without changing
/// behavior or argument shapes. Landing this enum now lets the Sept 1
/// cutover to the 4-verb story (`review`, `check`, `fix`, `score`) flip
/// only the default help surface — no command bodies move.
#[derive(Subcommand)]
enum XAction {
    /// Audit Flutter project assets — find unused, oversized, and WebP-convertible files
    #[command(name = "asset-audit")]
    AssetAudit {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-asset-report.html")]
        output: PathBuf,

        /// Size threshold in KB above which an image is flagged (default 200)
        #[arg(long, default_value = "200")]
        size_threshold_kb: u64,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Audit Flutter theme consistency — hardcoded colors, fonts, missing dark mode
    #[command(name = "theme-audit")]
    ThemeAudit {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-theme-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Analyze localization coverage across all ARB locales
    #[command(name = "l10n-coverage")]
    L10nCoverage {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-l10n-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Validate deep link configuration across Android, iOS, and Flutter routes
    #[command(name = "deeplink-validate")]
    DeeplinkValidate {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-deeplink-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Audit Flutter animations for anti-patterns, missing disposal, and jank risks
    #[command(name = "animation-audit")]
    AnimationAudit {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-animation-report.html")]
        output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
    },

    /// Generate golden (snapshot) test stubs for all discoverable widgets
    #[command(name = "golden-gen")]
    GoldenGen {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Directory to write generated test files into
        #[arg(short, long, default_value = "test/golden_generated")]
        output_dir: PathBuf,

        /// Preview what would be generated without writing files
        #[arg(long)]
        dry_run: bool,

        /// Output HTML report path
        #[arg(long, default_value = "falcon-golden-report.html")]
        html_output: PathBuf,

        /// Skip HTML report
        #[arg(long)]
        no_html: bool,
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
    #[command(name = "docs")]
    Docs {
        /// Output directory for generated docs
        #[arg(default_value = "docs")]
        output: PathBuf,
    },

    /// Generate the static pub.dev leaderboard site
    #[command(name = "leaderboard")]
    Leaderboard {
        /// JSON manifest with package metadata and Falcon score payloads
        #[arg(long)]
        input: PathBuf,

        /// Output directory for the generated static site
        #[arg(long, default_value = "site/leaderboard")]
        output: PathBuf,
    },

    /// Scan for security vulnerabilities and anti-patterns
    #[command(name = "vuln-scan")]
    VulnScan {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Simulate a refactoring and analyze impact
    #[command(name = "refactor-sim")]
    RefactorSim {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Refactoring scenario
        #[arg(long)]
        scenario: falcon::analysis::refactor_sim::RefactorScenario,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate test stubs from code analysis
    #[command(name = "test-gen")]
    TestGen {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Write test files to disk
        #[arg(long)]
        write: bool,
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

    /// Triage findings with embedded AI false-positive review
    Triage {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(long, default_value = "text")]
        format: TriageOutputFormat,
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

#[derive(Subcommand)]
enum AgentsAction {
    /// Write AGENTS.md at the given path (and per-feature files inside Flutter apps)
    Init {
        /// Project root (Flutter app or generic repo)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Overwrite existing AGENTS.md files
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum DevtoolsAction {
    /// Fetch a memory snapshot and allocation summary
    Memory {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Collect network activity through the DevTools HTTP/socket APIs
    Network {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Duration in seconds to record network traffic
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Collect timeline-based performance data
    Performance {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Duration in seconds to record timeline data
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Collect CPU profiler data
    Profiler {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Duration in seconds to record CPU samples
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Inspect debugger state and optionally trigger a debugger action
    Debugger {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Optional debugger action: pause, resume, step-over, step-in, step-out
        #[arg(long)]
        action: Option<DebuggerActionArg>,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Collect stdout, stderr, developer log, GC, and extension stream events
    Logging {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Duration in seconds to collect log events
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Show widget rebuild counts (which widgets rebuild most often)
    Rebuilds {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        attach: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Inspect the live widget tree (depth, root, currently-selected widget)
    Inspector {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        attach: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Trigger a hot reload of the running Flutter app
    Reload {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        attach: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Trigger a forced reload of all sources (state-preserving "hot restart")
    Restart {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        attach: Option<String>,
        #[arg(long)]
        json: bool,
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
enum ReviewOutputFormat {
    Text,
    Json,
    Gh,
    Sarif,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum TriageOutputFormat {
    Text,
    Json,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum FailLevel {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum DebuggerActionArg {
    Pause,
    Resume,
    StepOver,
    StepIn,
    StepOut,
}

#[derive(Subcommand)]
enum CloudAction {
    /// Initialize cloud config for a team
    Init {
        /// Team name
        #[arg(long)]
        team: String,

        /// Config root path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Register a project
    #[command(name = "add-project")]
    AddProject {
        /// Project name
        #[arg(long)]
        name: String,

        /// Path to the project
        #[arg(long)]
        project_path: String,

        /// Config root path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Show team dashboard
    Dashboard {
        /// Config root path
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum EnterpriseAction {
    /// Initialize enterprise policies
    Init {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Check policies against project
    Check {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Generate compliance report
    Compliance {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show audit log
    Audit {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Number of entries to show
        #[arg(long, default_value = "20")]
        last: usize,
    },
}

#[derive(Subcommand)]
enum ManageAction {
    /// Project health dashboard (unified score across all dimensions)
    Health {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Dependency health analysis (unused, outdated, security)
    Deps {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Architecture governance (layer enforcement, violations, hotspots)
    Arch {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Maintenance advisor (cleanup tasks, tech debt, auto-fix pipeline)
    Maint {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Build optimization (assets, config, code size)
    Build {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Run ALL management analyses at once
    All {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },
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
    let args: Vec<String> = std::env::args().collect();
    if handle_root_help(&args) {
        return;
    }
    if let Some(command) = args.get(1) {
        if let Some(new) = falcon::cli::deprecation::aliased_target(command) {
            falcon::cli::deprecation::warn_aliased(command, new);
        }
    }
    let cli = Cli::parse_from(args);

    if let Err(e) = run(cli) {
        eprintln!("{}: {}", "error".red(), e);
        process::exit(1);
    }
}

fn handle_root_help(args: &[String]) -> bool {
    if args.len() != 2 {
        return false;
    }

    match args[1].as_str() {
        "--help" | "-h" => {
            print!("{}", CUTOVER_HELP);
            true
        }
        "--legacy-help" => {
            let mut command = Cli::command();
            command.print_help().expect("write legacy help");
            println!();
            true
        }
        _ => false,
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
            run_analysis_command(AnalysisCommandOptions {
                path,
                format,
                output,
                config,
                since,
                baseline,
                baseline_path: None,
                update_baseline_path: None,
                fail_on,
                preset,
            })?;
        }
        Commands::Check {
            path,
            format,
            output,
            config,
            fail_on,
            preset,
            baseline,
            update_baseline,
        } => {
            run_analysis_command(AnalysisCommandOptions {
                path,
                format,
                output,
                config,
                since: None,
                baseline: false,
                baseline_path: baseline,
                update_baseline_path: update_baseline,
                fail_on,
                preset,
            })?;
        }
        Commands::Smells {
            path,
            format,
            output,
            config,
            limit,
        } => {
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
        Commands::Run {
            path,
            output_dir,
            device,
            flavor,
            notify,
            webhook,
            monitor_interval,
            no_monitor,
        } => {
            // Resolve output_dir relative to path when it is the default "."
            let resolved_output = if output_dir == Path::new(".") {
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
                monitor_interval: std::time::Duration::from_secs(monitor_interval),
                monitor_enabled: !no_monitor,
            };

            let report = falcon::flutter_run::run_flutter_app(&config)?;

            if report.has_errors() {
                process::exit(1);
            }
        }

        Commands::Flutter { args } => {
            let status = std::process::Command::new("flutter")
                .args(&args)
                .status()
                .map_err(|e| {
                    anyhow::anyhow!(
                        "failed to invoke `flutter`: {e}. Is the Flutter SDK on your PATH?"
                    )
                })?;
            process::exit(status.code().unwrap_or(1));
        }

        Commands::Fvm { args } => {
            let status = std::process::Command::new("fvm")
                .args(&args)
                .status()
                .map_err(|e| anyhow::anyhow!("failed to invoke `fvm`: {e}. Install FVM (https://fvm.app) or ensure it is on your PATH."))?;
            process::exit(status.code().unwrap_or(1));
        }

        Commands::Agents { action } => match action {
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
        },

        Commands::RuntimeCheck {
            path,
            attach,
            duration,
            output,
            memory_warn_mb,
            frame_warn_ms,
            no_html,
        } => {
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
        }
        Commands::Live {
            path,
            attach,
            duration,
            interval,
            json,
        } => {
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
        }
        Commands::Devtools { action } => {
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
                    let report =
                        rt.block_on(falcon::runtime::tools::collect_performance_report(
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
                        DebuggerActionArg::StepOver => {
                            falcon::runtime::tools::DebuggerAction::StepOver
                        }
                        DebuggerActionArg::StepIn => falcon::runtime::tools::DebuggerAction::StepIn,
                        DebuggerActionArg::StepOut => {
                            falcon::runtime::tools::DebuggerAction::StepOut
                        }
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
            }
        }
        Commands::AssetAudit {
            path,
            output,
            size_threshold_kb,
            no_html,
        } => {
            eprintln!(
                "  {} Scanning assets in {} …",
                "▸".bright_cyan(),
                path.display()
            );
            let report =
                falcon::asset_audit::audit_assets_with_threshold(&path, size_threshold_kb)?;
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
        }

        Commands::ThemeAudit {
            path,
            output,
            no_html,
        } => {
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
        }

        Commands::L10nCoverage {
            path,
            output,
            no_html,
        } => {
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
        }

        Commands::DeeplinkValidate {
            path,
            output,
            no_html,
        } => {
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
        }

        Commands::AnimationAudit {
            path,
            output,
            no_html,
        } => {
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
        }

        Commands::GoldenGen {
            path,
            output_dir,
            dry_run,
            html_output,
            no_html,
        } => {
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
                stats.sort_by_key(|(count, _)| Reverse(*count));

                println!(
                    "  {} (by number of dependents):",
                    "Most depended-on files".bright_green()
                );
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
            if errors.iter().any(|e| {
                matches!(
                    e.severity,
                    falcon::config::validator::ConfigErrorSeverity::Error
                )
            }) {
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
        }
        Commands::CheckUnusedParams {
            path,
            format,
            output,
        } => {
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
        Commands::CheckDeadCode {
            path,
            format,
            output,
        } => {
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
        Commands::CheckUnusedL10n {
            path,
            format,
            output,
        } => {
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
        Commands::CheckPromotedDeps {
            path,
            format,
            output,
        } => {
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
        Commands::Ai { action } => {
            match action {
                AiAction::Setup { path } => {
                    falcon::ai::config::generate_ai_setup(&path)?;
                    println!(
                        "{} AI configuration added to falcon.yaml",
                        "✓".green().bold()
                    );
                    println!("  Edit falcon.yaml to set your provider and API key.");
                    println!("  Supported providers: openai, anthropic, gemini, local (Ollama), embedded");
                    println!("  Embedded builds run in-process triage with --features ai-local.");
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
                    if let Some(ref embedded) = ai.embedded {
                        println!();
                        println!("  Embedded model:");
                        println!("    Model id:   {}", embedded.model_id);
                        println!("    Model file: {}", embedded.model_file);
                        println!("    Max issues: {}", embedded.max_issues);
                        #[cfg(feature = "ai-local")]
                        println!("    Engine:     compiled in (ai-local)");
                        #[cfg(not(feature = "ai-local"))]
                        println!("    Engine:     NOT compiled (rebuild with --features ai-local)");
                    }
                    println!();
                }
                AiAction::Triage { path, format } => {
                    run_ai_triage(&path, format)?;
                }
            }
        }
        Commands::Fix {
            path,
            preview,
            config,
        } => {
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
            falcon::ai::confidence::print_confidence_results(&results, Some(min_confidence));
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

                    let issues = falcon::analysis::layer_enforcement::enforce_layers(
                        &path, &layers, &exclude,
                    );
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

            let boundary_issues =
                falcon::analysis::import_rules::check_package_boundaries(&path, &exclude);
            get_reporter(&OutputFormat::Console, &PathBuf::from(""))
                .report_issues(&boundary_issues);

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
        }
        Commands::Review {
            path,
            base_ref,
            format,
            strictness,
            analyzer_copilot,
            no_defer_to_analyzer,
            baseline,
            update_baseline,
        } => {
            let config = FalconConfig::load(&path)?;
            let changed_files = falcon::review::pr_review::changed_dart_files(&path, &base_ref)?;
            let review_observations = (!matches!(
                strictness,
                falcon::review::pr_review::ReviewStrictness::Quick
            ))
            .then(|| {
                falcon::review::pr_review::review_diff(&path, &base_ref, &config, strictness)
                    .map(|report| report.observations)
            })
            .transpose()?;
            let falcon = Falcon::new(config)?;
            let mut report = if changed_files.is_empty() {
                falcon::reporters::AnalysisReport {
                    issues: Vec::new(),
                    metrics: Vec::new(),
                    file_count: 0,
                    project_path: Some(path.clone()),
                }
            } else {
                falcon.analyze_files_with_project_context(&path, &changed_files)?
            };

            apply_review_strictness(&mut report, strictness, review_observations);

            let should_run_analyzer_copilot =
                analyzer_copilot || path.join(".dart_tool/package_config.json").is_file();
            if should_run_analyzer_copilot {
                if let Some(analyzer_diagnostics) =
                    falcon::analyzer_bridge::run_dart_analyze(&path)?
                {
                    let (issues, _) = falcon::analyzer_bridge::defer_to_analyzer(
                        &report.issues,
                        &analyzer_diagnostics,
                        no_defer_to_analyzer,
                    );
                    report.issues = issues;
                }
            }

            let unfiltered_issues = report.issues.clone();
            if let Some(ref baseline_path) = baseline {
                let baseline = Baseline::load_path(baseline_path)?;
                let file_aliases = review_baseline_file_aliases(&path, &base_ref)?;
                report.issues = baseline.filter_new_issues_with_file_aliases(
                    report.issues,
                    &path,
                    &file_aliases,
                );
            }
            if let Some(ref baseline_path) = update_baseline {
                let update_report = falcon::reporters::AnalysisReport {
                    issues: unfiltered_issues,
                    metrics: report.metrics.clone(),
                    file_count: report.file_count,
                    project_path: report.project_path.clone(),
                };
                let path_written =
                    Baseline::create_at_path(&update_report.issues, &path, baseline_path)?;
                eprintln!("Baseline updated: {}", path_written.display());
            }

            match format {
                ReviewOutputFormat::Text => ConsoleReporter.report_analysis(&report),
                ReviewOutputFormat::Json => JsonReporter.report_analysis(&report),
                ReviewOutputFormat::Sarif => {
                    SarifReporter { output_path: None }.report_analysis(&report)
                }
                ReviewOutputFormat::Gh => {
                    println!(
                        "{}",
                        falcon::ci::pr_comment::format_pr_comment(&report, &path)
                    );
                }
            }

            if report.has_errors() {
                process::exit(1);
            }
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
                falcon::plugins::scaffold::print_plugins(&plugins);
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
            PresetAction::Show { name } => match falcon::plugins::presets::get_preset(&name) {
                Some(preset) => falcon::plugins::presets::print_preset_detail(&preset),
                None => {
                    eprintln!("Unknown preset '{}'. Use: falcon preset list", name);
                    process::exit(1);
                }
            },
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
                let snapshot =
                    falcon::dashboard::snapshot::AnalysisSnapshot::capture(&report, &path);
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
                    println!(
                        "  Need at least 2 snapshots for trends. Run: falcon dashboard snapshot"
                    );
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
        Commands::Export {
            path,
            format,
            output,
            webhook_url,
        } => {
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
                    let payload = falcon::dashboard::exports::WebhookPayload::from_snapshot(
                        &snapshot, project,
                    );
                    let json = payload.to_json()?;
                    println!("{}", json);
                    println!("  Webhook payload generated for {}", url.bright_blue());
                }
            }
        }
        Commands::MigrateFromDcm {
            config_path,
            output,
        } => {
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
        Commands::Compare { path } => {
            let result = falcon::benchmark_compare::compare_with_dart_analyze(&path)?;
            falcon::benchmark_compare::print_compare_result(&result);
        }
        Commands::CompareReports {
            path,
            run1,
            run2,
            output,
        } => {
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
        }
        Commands::CompareBranches {
            path,
            base,
            branch,
            output,
            config,
        } => {
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
        }
        Commands::History { path } => {
            falcon::dashboard::compare_reports::list_history(&path)?;
        }
        Commands::Update { version, list } => {
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
        }
        Commands::Showcase {
            paths,
            format,
            output,
        } => {
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
        }
        Commands::StabilityContract => {
            let contract = falcon::stability::contract::StabilityContract::default();
            falcon::stability::contract::print_stability_contract(&contract);
        }
        Commands::DeprecationStatus => {
            falcon::stability::deprecation::print_deprecation_status();
        }
        Commands::PerfTrack {
            path,
            history,
            last,
        } => {
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
                    "false-positive" | "fp" => {
                        falcon::stability::suppression::SuppressionCategory::FalsePositive
                    }
                    "wont-fix" | "wf" => {
                        falcon::stability::suppression::SuppressionCategory::WontFix
                    }
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
        },
        Commands::Manage { action } => match action {
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
        },
        Commands::Mcp => {
            falcon::mcp::server::run_mcp_server()?;
        }
        Commands::PrComment {
            path,
            owner,
            repo,
            pr,
            dry_run,
            config,
        } => {
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
        }
        Commands::Webhook { path, url, event } => {
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
        }
        Commands::BenchmarkDb {
            path,
            tool,
            summary,
        } => {
            if summary {
                let db = falcon::ai_score::benchmark_db::load_benchmark_db(&path)?;
                let stats = falcon::ai_score::benchmark_db::compute_tool_stats(&db);
                falcon::ai_score::benchmark_db::print_benchmark_summary(&stats);
            } else if let Some(tool_name) = tool {
                let project = path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("project");
                let entry =
                    falcon::ai_score::benchmark_db::record_benchmark(&path, project, &tool_name)?;
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
        Commands::RefactorSim {
            path,
            scenario,
            json,
        } => {
            let impact = falcon::analysis::refactor_sim::simulate_refactor(&path, &scenario)?;
            if json {
                let j = serde_json::to_string_pretty(&impact)?;
                println!("{}", j);
            } else {
                falcon::analysis::refactor_sim::print_refactor_impact(&impact);
            }
        }
        Commands::TestGen { path, write } => {
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
        }
        Commands::VulnScan { path } => {
            let findings = falcon::analysis::vuln_radar::scan_vulnerabilities(&path);
            falcon::analysis::vuln_radar::print_vuln_report(&findings);
            if findings
                .iter()
                .any(|f| f.risk_level == falcon::analysis::vuln_radar::RiskLevel::Critical)
            {
                process::exit(1);
            }
        }
        Commands::AiProfile { path } => {
            let db = falcon::ai_score::benchmark_db::load_benchmark_db(&path)?;
            let profiles = falcon::ai_score::ai_profiling::build_tool_profiles(&db);
            falcon::ai_score::ai_profiling::print_tool_profiles(&profiles);
        }
        Commands::DiscoverRules { path } => {
            let rules = falcon::ai_score::auto_rules::discover_patterns(&path);
            falcon::ai_score::auto_rules::print_proposed_rules(&rules);
        }
        Commands::FixTrack {
            path,
            rule,
            outcome,
            file,
            report,
        } => {
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
        }
        Commands::Learn {
            project,
            db,
            insights,
        } => {
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
        }
        Commands::Predict { path, json } => {
            let predictions = falcon::ai_score::regression_predict::predict_risks(&path)?;
            if json {
                let j = serde_json::to_string_pretty(&predictions)?;
                println!("{}", j);
            } else {
                falcon::ai_score::regression_predict::print_risk_predictions(&predictions);
            }
        }
        Commands::UpgradeCheck { path } => {
            let findings = falcon::analysis::upgrade_check::check_upgrade_compatibility(&path);
            falcon::analysis::upgrade_check::print_compat_report(&findings);
            if findings.iter().any(|f| f.removed_in.is_some()) {
                process::exit(1);
            }
        }
        Commands::Cloud { action } => match action {
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
        },
        Commands::Enterprise { action } => match action {
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
        },
        Commands::Marketplace { query } => {
            let q = if query.is_empty() {
                None
            } else {
                Some(query.as_str())
            };
            let listings = falcon::platform::marketplace::browse_marketplace(q);
            falcon::platform::marketplace::print_marketplace(&listings, q);
        }
        Commands::Certify { path } => {
            let result = falcon::platform::certification::evaluate_certification(&path)?;
            falcon::platform::certification::print_certification(&result);
        }
        Commands::Partners => {
            let partners = falcon::platform::partner::list_partners();
            falcon::platform::partner::print_partners(&partners);
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
        Commands::ScoreTrack {
            path,
            history,
            last,
        } => {
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
        Commands::AiReport {
            path,
            format,
            output,
        } => {
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
                    .filter(|r| {
                        r.origin == falcon::ai_score::provenance::CodeOrigin::LikelyAiGenerated
                    })
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
        Commands::X { action } => {
            // The `x` namespace is the new home for extended commands. Every
            // entry here re-dispatches into the matching top-level command
            // without changing behavior or argument shapes. Legacy warnings
            // are emitted before clap parsing so `falcon x ...` stays quiet.
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
                XAction::Leaderboard { input, output } => {
                    falcon::leaderboard::build_from_file(&input, &output)?;
                    eprintln!("Leaderboard written to {}", output.display());
                    return Ok(());
                }
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

fn apply_review_strictness(
    report: &mut falcon::reporters::AnalysisReport,
    strictness: falcon::review::pr_review::ReviewStrictness,
    observations: Option<Vec<falcon::review::pr_review::ReviewObservation>>,
) {
    match strictness {
        falcon::review::pr_review::ReviewStrictness::Quick => {
            report
                .issues
                .retain(|issue| issue.severity == falcon::config::Severity::Error);
        }
        falcon::review::pr_review::ReviewStrictness::Standard
        | falcon::review::pr_review::ReviewStrictness::Thorough => {
            if let Some(observations) = observations {
                report
                    .issues
                    .extend(observations.into_iter().map(review_observation_to_issue));
            }
        }
    }
}

fn review_observation_to_issue(
    observation: falcon::review::pr_review::ReviewObservation,
) -> falcon::reporters::Issue {
    let severity = match observation.severity {
        falcon::review::pr_review::ObservationSeverity::Critical => falcon::config::Severity::Error,
        falcon::review::pr_review::ObservationSeverity::Suggestion => {
            falcon::config::Severity::Warning
        }
        falcon::review::pr_review::ObservationSeverity::Nitpick => falcon::config::Severity::Info,
    };
    let category = match observation.category {
        falcon::review::pr_review::ObservationCategory::PatternConsistency => "pattern-consistency",
        falcon::review::pr_review::ObservationCategory::NamingConvention => "naming-convention",
        falcon::review::pr_review::ObservationCategory::ErrorHandling => "error-handling",
        falcon::review::pr_review::ObservationCategory::MissingTest => "missing-test",
        falcon::review::pr_review::ObservationCategory::CodeStyle => "code-style",
        falcon::review::pr_review::ObservationCategory::Performance => "performance",
    };
    let message = match observation.suggestion {
        Some(suggestion) => format!("{} Suggestion: {}", observation.message, suggestion),
        None => observation.message,
    };

    falcon::reporters::Issue {
        rule: format!("review-{}", category),
        message,
        severity,
        file: observation.file,
        line: observation.line,
        column: 1,
    }
}

fn review_baseline_file_aliases(
    root: &Path,
    base_ref: &str,
) -> Result<std::collections::HashMap<String, Vec<String>>> {
    let renames = falcon::review::pr_review::renamed_dart_files(root, base_ref)?;
    let mut aliases: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for (new_path, old_path) in renames {
        let new_rel = new_path
            .strip_prefix(root)
            .unwrap_or(&new_path)
            .to_string_lossy()
            .to_string();
        let old_rel = old_path
            .strip_prefix(root)
            .unwrap_or(&old_path)
            .to_string_lossy()
            .to_string();
        aliases.entry(new_rel).or_default().push(old_rel);
    }

    Ok(aliases)
}

struct AnalysisCommandOptions {
    path: PathBuf,
    format: OutputFormat,
    output: PathBuf,
    config: Option<PathBuf>,
    since: Option<String>,
    baseline: bool,
    baseline_path: Option<PathBuf>,
    update_baseline_path: Option<PathBuf>,
    fail_on: FailLevel,
    preset: Option<String>,
}

fn run_analysis_command(options: AnalysisCommandOptions) -> Result<()> {
    let AnalysisCommandOptions {
        path,
        format,
        output,
        config,
        since,
        baseline,
        baseline_path,
        update_baseline_path,
        fail_on,
        preset,
    } = options;

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

    let unfiltered_issues = issues.clone();
    if let Some(ref baseline_path) = baseline_path {
        let bl = Baseline::load_path(baseline_path)?;
        issues = bl.filter_new_issues(issues, &path);
    }
    if let Some(ref baseline_path) = update_baseline_path {
        let path_written = Baseline::create_at_path(&unfiltered_issues, &path, baseline_path)?;
        eprintln!("Baseline updated: {}", path_written.display());
    }

    let final_report = falcon::reporters::AnalysisReport {
        issues,
        metrics: report.metrics,
        file_count: report.file_count,
        project_path: report.project_path,
    };

    get_reporter(&format, &output).report_analysis(&final_report);

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
    let report = falcon.analyze_files_with_project_context(path, &affected_vec)?;

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

fn run_ai_triage(path: &Path, format: TriageOutputFormat) -> Result<()> {
    #[cfg(feature = "ai-local")]
    {
        use falcon::ai::local::engine::LocalEngine;
        use falcon::ai::local::triage::{print_triage_run, triage_issues, triage_run_to_json};

        let config = FalconConfig::load(path)?;
        let embedded_cfg = config.ai.embedded.clone().unwrap_or_default();
        let falcon = Falcon::new(config)?;
        let report = falcon.analyze(path)?;
        if report.issues.is_empty() {
            let run = falcon::ai::local::triage::TriageRun {
                verdicts: Vec::new(),
                truncated_from: None,
            };
            match format {
                TriageOutputFormat::Text => {
                    print_triage_run(&run);
                }
                TriageOutputFormat::Json => {
                    println!("{}", triage_run_to_json(&run));
                }
            }
            return Ok(());
        }

        let mut engine = LocalEngine::load(&embedded_cfg)?;
        let run = triage_issues(&report.issues, path, &mut engine, &embedded_cfg);

        match format {
            TriageOutputFormat::Text => {
                print_triage_run(&run);
            }
            TriageOutputFormat::Json => {
                println!("{}", triage_run_to_json(&run));
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "ai-local"))]
    {
        match format {
            TriageOutputFormat::Text => {
                println!("falcon ai triage requires a build compiled with --features ai-local.");
                println!("Path: {}", path.display());
            }
            TriageOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "available": false,
                        "path": path,
                        "reason": "rebuild falcon with --features ai-local to enable embedded AI triage"
                    })
                );
            }
        }
        Ok(())
    }
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
