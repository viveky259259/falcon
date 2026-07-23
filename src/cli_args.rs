//! Command-line argument and subcommand definitions (clap).
//!
//! Extracted from `main.rs` to keep the binary entry point thin. These are
//! purely declarative clap definitions; all dispatch logic lives in `main.rs`.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "falcon",
    version,
    about = "Falcon — Rust-powered static analysis for Flutter/Dart",
    long_about = "A blazing-fast static analysis tool for Flutter and Dart projects.\nAnalyzes code metrics, enforces lint rules, and detects unused code.",
    after_long_help = GROUPED_HELP,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

const GROUPED_HELP: &str = r#"
SEMANTIC COMMAND GROUPS:

  Analysis          analyze, metrics, ai-score, cognitive-complexity, codebase-intel
  Code Checks       check-unused-code, check-unused-files, check-cycles, check-async,
                    check-widgets, check-dead-code, check-layers, check-imports,
                    check-platform, check-codegen, check-perf, check-unused-params,
                    check-unused-l10n, check-dependencies, check-promoted-deps
  Comparison        compare-branches, compare-reports, compare, history
  AI Intelligence   ai-score, ai-report, provenance, conventions, drift, predict,
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
pub enum Commands {
    /// Run project checks and static analysis
    #[command(name = "check", alias = "analyze", display_order = 1)]
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

        /// Only report findings not present in this baseline file
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,

        /// Rewrite the baseline file with the current findings
        #[arg(long, value_name = "FILE")]
        update_baseline: Option<PathBuf>,

        /// Run dart analyze as a semantic co-pilot and defer same-line Falcon findings
        #[arg(long)]
        semantic: bool,

        /// Keep Falcon findings even when dart analyze reports the same line
        #[arg(long = "no-defer-to-analyzer", alias = "no-defer")]
        no_defer_to_analyzer: bool,

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

    /// Verify declared Flutter assets exist
    #[command(name = "check-assets", display_order = 2)]
    CheckAssets {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: falcon::preflight::OutputFormat,
    },

    /// Verify accessibility preflight requirements
    #[command(name = "check-a11y", display_order = 2)]
    CheckA11y {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: falcon::preflight::OutputFormat,
    },

    /// Verify iOS Podfile and podspec deployment targets
    #[command(name = "check-pods", display_order = 2)]
    CheckPods {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: falcon::preflight::OutputFormat,

        /// Refresh cached plugin metadata
        #[arg(long)]
        refresh: bool,
    },

    /// Verify platform dependency declarations
    #[command(name = "check-platform-deps", display_order = 2)]
    CheckPlatformDeps {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: falcon::preflight::OutputFormat,

        /// Refresh cached plugin metadata
        #[arg(long)]
        refresh: bool,

        /// Platform to check
        #[arg(long, value_enum)]
        platform: Option<falcon::preflight::TargetPlatform>,
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

    /// Trace an interaction window — correlate frame jank with hot-rebuilding widgets
    #[command(name = "trace", display_order = 8)]
    Trace {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Window in seconds to record frames + rebuilds
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Frame build time (ms) above which a frame counts as jank
        #[arg(long, default_value = "16")]
        jank_ms: f64,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Record a user journey through a running app (screenshots + screens + metrics)
    #[command(name = "journey", display_order = 8)]
    Journey {
        /// Path to the Flutter project (used for `flutter run` when not attaching)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Attach to an already-running app by VM Service URI instead of launching
        #[arg(long)]
        attach: Option<String>,

        /// Target device ID for the screenshot device-capture fallback
        #[arg(long)]
        device: Option<String>,

        /// Total recording duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,

        /// Seconds between captures
        #[arg(long, default_value = "3")]
        interval: u64,

        /// Directory for screenshots + journey.html
        #[arg(short, long, default_value = "falcon-journey")]
        output_dir: PathBuf,

        /// Skip HTML report generation
        #[arg(long)]
        no_html: bool,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
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

    /// Map the codebase architecture — modules, layers, and dependencies (Mermaid + HTML)
    #[command(name = "arch-map", display_order = 1)]
    ArchMap {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output HTML report path
        #[arg(short, long, default_value = "falcon-arch-map.html")]
        output: PathBuf,

        /// Skip HTML report (console only)
        #[arg(long)]
        no_html: bool,

        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
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
        #[arg(long, alias = "base-ref", default_value = "HEAD~1")]
        diff: String,

        /// Output format
        #[arg(long, default_value = "gh")]
        format: ReviewFormat,

        /// Review strictness level
        #[arg(long, default_value = "standard")]
        strictness: falcon::review::pr_review::ReviewStrictness,

        /// Only report findings not present in this baseline file
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,

        /// Rewrite the baseline file with the current review findings
        #[arg(long, value_name = "FILE")]
        update_baseline: Option<PathBuf>,

        /// Run dart analyze as a semantic co-pilot and defer same-line Falcon findings
        #[arg(long)]
        semantic: bool,

        /// Keep Falcon findings even when dart analyze reports the same line
        #[arg(long = "no-defer-to-analyzer", alias = "no-defer")]
        no_defer_to_analyzer: bool,
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

        /// Output format
        #[arg(long, value_enum)]
        format: Option<ScoreFormat>,
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
pub enum XAction {
    /// AI configuration and tools
    #[command(name = "ai")]
    Ai {
        #[command(subcommand)]
        action: AiAction,
    },

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
pub enum SuppressAction {
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
pub enum CommunityAction {
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
pub enum PluginAction {
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
pub enum PresetAction {
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
pub enum DashboardAction {
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
pub enum AiAction {
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

    /// Triage findings as real vs. false-positive using the embedded SLM
    Triage {
        /// Path to analyze
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum BaselineAction {
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
pub enum AgentsAction {
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
pub enum DevtoolsAction {
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

    /// Diff the widget tree across a settle window (added/removed subtrees)
    TreeDiff {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        attach: Option<String>,
        /// Seconds to wait between the before/after tree snapshots
        #[arg(short, long, default_value = "5")]
        settle: u64,
        #[arg(long)]
        json: bool,
    },

    /// Log navigation/route changes from a running app over a window
    RouteLog {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        attach: Option<String>,
        /// Duration in seconds to record navigation events
        #[arg(short, long, default_value = "15")]
        duration: u64,
        #[arg(long)]
        json: bool,
    },

    /// Capture a PNG screenshot of the running Flutter app
    Screenshot {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        attach: Option<String>,
        /// Output file path for the PNG
        #[arg(short, long, default_value = "falcon-screenshot.png")]
        out: PathBuf,
        /// Target device ID for the `flutter screenshot` device-capture fallback
        #[arg(long)]
        device: Option<String>,
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
pub enum OutputFormat {
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
pub enum ReviewFormat {
    Gh,
    Json,
    Sarif,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum ScoreFormat {
    Json,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum FailLevel {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum DebuggerActionArg {
    Pause,
    Resume,
    StepOver,
    StepIn,
    StepOut,
}

#[derive(Subcommand)]
pub enum CloudAction {
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
pub enum EnterpriseAction {
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
pub enum ManageAction {
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
pub enum ExportFormat {
    Prometheus,
    Json,
    Webhook,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum DocFormat {
    Console,
    Markdown,
}
