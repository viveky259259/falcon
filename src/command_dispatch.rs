use super::*;

mod analysis;
mod checks;
mod config;
mod enterprise;
mod environment;
mod experimental;
mod flutter_quality;
mod intelligence;
mod runtime;
mod tracking;

pub(crate) fn run(cli: Cli) -> Result<()> {
    match command_group(&cli.command) {
        CommandGroup::Analysis => analysis::handle_command(cli.command),
        CommandGroup::Checks => checks::handle_command(cli.command),
        CommandGroup::Config => config::handle_command(cli.command),
        CommandGroup::Runtime => runtime::handle_command(cli.command),
        CommandGroup::FlutterQuality => flutter_quality::handle_command(cli.command),
        CommandGroup::Intelligence => intelligence::handle_command(cli.command),
        CommandGroup::Tracking => tracking::handle_command(cli.command),
        CommandGroup::Enterprise => enterprise::handle_command(cli.command),
        CommandGroup::Environment => environment::handle_command(cli.command),
        CommandGroup::Experimental => experimental::handle_command(cli.command),
    }
}

enum CommandGroup {
    Analysis,
    Checks,
    Config,
    Runtime,
    FlutterQuality,
    Intelligence,
    Tracking,
    Enterprise,
    Environment,
    Experimental,
}

fn command_group(command: &Commands) -> CommandGroup {
    match command {
        Commands::Analyze { .. }
        | Commands::Smells { .. }
        | Commands::Metrics { .. }
        | Commands::CognitiveComplexity { .. }
        | Commands::CodebaseIntel { .. }
        | Commands::DepGraph { .. }
        | Commands::ArchMap { .. }
        | Commands::AiScore { .. } => CommandGroup::Analysis,

        Commands::CheckUnusedCode { .. }
        | Commands::CheckUnusedFiles { .. }
        | Commands::CheckDependencies { .. }
        | Commands::CheckAssets { .. }
        | Commands::CheckA11y { .. }
        | Commands::CheckPods { .. }
        | Commands::CheckPlatformDeps { .. }
        | Commands::CheckCycles { .. }
        | Commands::CheckUnusedParams { .. }
        | Commands::CheckDeadCode { .. }
        | Commands::CheckUnusedL10n { .. }
        | Commands::CheckPromotedDeps { .. }
        | Commands::CheckUnusedConfidence { .. }
        | Commands::CheckLayers { .. }
        | Commands::CheckImports { .. }
        | Commands::CheckWidgets { .. }
        | Commands::CheckAsync { .. }
        | Commands::CheckPlatform { .. }
        | Commands::CheckCodegen { .. }
        | Commands::CheckPerf { .. } => CommandGroup::Checks,

        Commands::Init { .. }
        | Commands::Baseline { .. }
        | Commands::Validate { .. }
        | Commands::Explain { .. }
        | Commands::Preset { .. }
        | Commands::Docs { .. }
        | Commands::Update { .. }
        | Commands::StabilityContract
        | Commands::DeprecationStatus
        | Commands::Suppress { .. }
        | Commands::SelfTune { .. } => CommandGroup::Config,

        Commands::Watch { .. }
        | Commands::Run { .. }
        | Commands::Flutter { .. }
        | Commands::Fvm { .. }
        | Commands::Agents { .. }
        | Commands::RuntimeCheck { .. }
        | Commands::Live { .. }
        | Commands::Devtools { .. }
        | Commands::Screenshot { .. }
        | Commands::Journey { .. }
        | Commands::Trace { .. }
        | Commands::Workspace { .. }
        | Commands::Manage { .. }
        | Commands::Mcp
        | Commands::Api { .. } => CommandGroup::Runtime,

        Commands::AssetAudit { .. }
        | Commands::ThemeAudit { .. }
        | Commands::L10nCoverage { .. }
        | Commands::DeeplinkValidate { .. }
        | Commands::AnimationAudit { .. }
        | Commands::GoldenGen { .. } => CommandGroup::FlutterQuality,

        Commands::Review { .. }
        | Commands::Ai { .. }
        | Commands::Plugin { .. }
        | Commands::Dashboard { .. }
        | Commands::Export { .. }
        | Commands::MigrateFromDcm { .. }
        | Commands::FeatureGap
        | Commands::RuleDocs { .. }
        | Commands::Compare { .. }
        | Commands::CompareReports { .. }
        | Commands::CompareBranches { .. }
        | Commands::Showcase { .. }
        | Commands::PrComment { .. }
        | Commands::Webhook { .. }
        | Commands::RefactorSim { .. }
        | Commands::TestGen { .. }
        | Commands::VulnScan { .. }
        | Commands::AiProfile { .. }
        | Commands::DiscoverRules { .. }
        | Commands::Learn { .. }
        | Commands::Predict { .. }
        | Commands::UpgradeCheck { .. }
        | Commands::AiReport { .. }
        | Commands::Provenance { .. }
        | Commands::Conventions { .. }
        | Commands::Community { .. } => CommandGroup::Intelligence,

        Commands::Trends { .. }
        | Commands::RuleImpact { .. }
        | Commands::Benchmark { .. }
        | Commands::History { .. }
        | Commands::PerfTrack { .. }
        | Commands::BenchmarkDb { .. }
        | Commands::FixTrack { .. }
        | Commands::ScoreTrack { .. }
        | Commands::Fix { .. } => CommandGroup::Tracking,

        Commands::Cloud { .. }
        | Commands::Enterprise { .. }
        | Commands::Marketplace { .. }
        | Commands::Certify { .. }
        | Commands::Partners
        | Commands::Drift { .. } => CommandGroup::Enterprise,

        Commands::Doctor { .. } => CommandGroup::Environment,

        Commands::X { .. } => CommandGroup::Experimental,
    }
}
