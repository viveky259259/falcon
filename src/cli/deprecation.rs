//! Deprecation / alias warning helpers.
//!
//! On Sept 1 the CLI cuts over to four primary verbs (`review`, `check`,
//! `fix`, `score`) and everything else moves behind `falcon x <thing>`.
//! Until then, both forms must keep working. After the cutover, each
//! legacy top-level command emits a one-line migration hint before the
//! legacy form is removed in v1.0.

/// Emit a one-line deprecation warning to stderr telling the user that
/// `falcon <old>` is now aliased to `falcon <new>` and will be removed
/// in v1.0.
pub fn warn_aliased(old: &str, new: &str) {
    eprintln!("{}", aliased_message(old, new));
}

/// Return the replacement command for a deprecated top-level command.
pub fn aliased_target(command: &str) -> Option<&'static str> {
    match command {
        "ai-score" => Some("score"),
        "check-unused-code" => Some("x check-unused-code"),
        "check-unused-files" => Some("x check-unused-files"),
        "check-dependencies" => Some("x check-dependencies"),
        "check-cycles" => Some("x check-cycles"),
        "check-unused-params" => Some("x check-unused-params"),
        "check-dead-code" => Some("x check-dead-code"),
        "check-unused-l10n" => Some("x check-unused-l10n"),
        "check-promoted-deps" => Some("x check-promoted-deps"),
        "upgrade-check" => Some("x upgrade-check"),
        "check-platform" => Some("x check-platform"),
        "check-codegen" => Some("x check-codegen"),
        "check-perf" => Some("x check-perf"),
        "check-unused-confidence" => Some("x check-unused-confidence"),
        "check-layers" => Some("x check-layers"),
        "check-imports" => Some("x check-imports"),
        "cognitive-complexity" => Some("x cognitive-complexity"),
        "check-widgets" => Some("x check-widgets"),
        "check-async" => Some("x check-async"),
        "codebase-intel" => Some("x codebase-intel"),
        "ai-report" => Some("x ai-report"),
        "provenance" => Some("x provenance"),
        "ai-profile" => Some("x ai-profile"),
        "discover-rules" => Some("x discover-rules"),
        "predict" => Some("x predict"),
        "drift" => Some("x drift"),
        "conventions" => Some("x conventions"),
        "compare" => Some("x compare"),
        "compare-reports" => Some("x compare-reports"),
        "compare-branches" => Some("x compare-branches"),
        "history" => Some("x history"),
        "baseline" => Some("x baseline"),
        "validate" => Some("x validate"),
        "explain" => Some("x explain"),
        "preset" => Some("x preset"),
        "rule-docs" => Some("x rule-docs"),
        "stability-contract" => Some("x stability-contract"),
        "deprecation-status" => Some("x deprecation-status"),
        "suppress" => Some("x suppress"),
        "dashboard" => Some("x dashboard"),
        "trends" => Some("x trends"),
        "rule-impact" => Some("x rule-impact"),
        "benchmark" => Some("x benchmark"),
        "benchmark-db" => Some("x benchmark-db"),
        "score-track" => Some("x score-track"),
        "perf-track" => Some("x perf-track"),
        "fix-track" => Some("x fix-track"),
        "self-tune" => Some("x self-tune"),
        "learn" => Some("x learn"),
        "cloud" => Some("x cloud"),
        "enterprise" => Some("x enterprise"),
        "marketplace" => Some("x marketplace"),
        "certify" => Some("x certify"),
        "partners" => Some("x partners"),
        "smells" => Some("x smells"),
        "metrics" => Some("x metrics"),
        "asset-audit" => Some("x asset-audit"),
        "theme-audit" => Some("x theme-audit"),
        "l10n-coverage" => Some("x l10n-coverage"),
        "deeplink-validate" => Some("x deeplink-validate"),
        "animation-audit" => Some("x animation-audit"),
        "golden-gen" => Some("x golden-gen"),
        "dep-graph" => Some("x dep-graph"),
        "workspace" => Some("x workspace"),
        "docs" => Some("x docs"),
        "vuln-scan" => Some("x vuln-scan"),
        "refactor-sim" => Some("x refactor-sim"),
        "test-gen" => Some("x test-gen"),
        _ => None,
    }
}

/// Return the replacement command for deprecated commands that cannot be
/// mechanically rewritten yet because their current option shapes differ.
pub fn warning_target(command: &str) -> Option<&'static str> {
    match command {
        "analyze" => Some("check"),
        "pr-comment" => Some("review --format gh"),
        _ => None,
    }
}

/// Render the warning string without printing it. Used by tests and by
/// any caller that wants to route the message through their own logger.
pub fn aliased_message(old: &str, new: &str) -> String {
    format!(
        "warning: `falcon {old}` is being aliased to `falcon {new}` and will be removed in v1.0. Run `falcon {new}` instead."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliased_message_has_expected_shape() {
        let msg = aliased_message("asset-audit", "x asset-audit");
        assert_eq!(
            msg,
            "warning: `falcon asset-audit` is being aliased to `falcon x asset-audit` and will be removed in v1.0. Run `falcon x asset-audit` instead."
        );
    }

    #[test]
    fn aliased_message_mentions_both_commands() {
        let msg = aliased_message("vuln-scan", "x vuln-scan");
        assert!(msg.contains("`falcon vuln-scan`"));
        assert!(msg.contains("`falcon x vuln-scan`"));
    }

    #[test]
    fn aliased_message_mentions_v1_removal() {
        let msg = aliased_message("foo", "bar");
        assert!(msg.contains("v1.0"));
        assert!(msg.contains("removed"));
    }

    #[test]
    fn aliased_message_starts_with_warning_prefix() {
        let msg = aliased_message("foo", "bar");
        assert!(msg.starts_with("warning: "));
    }

    #[test]
    fn aliased_target_maps_ai_score() {
        assert_eq!(aliased_target("ai-score"), Some("score"));
    }

    #[test]
    fn warning_target_maps_non_rewritable_commands() {
        assert_eq!(warning_target("analyze"), Some("check"));
        assert_eq!(warning_target("pr-comment"), Some("review --format gh"));
    }

    #[test]
    fn aliased_target_maps_moved_x_commands() {
        assert_eq!(aliased_target("ai-profile"), Some("x ai-profile"));
        assert_eq!(aliased_target("ai-report"), Some("x ai-report"));
        assert_eq!(aliased_target("asset-audit"), Some("x asset-audit"));
        assert_eq!(aliased_target("baseline"), Some("x baseline"));
        assert_eq!(aliased_target("benchmark"), Some("x benchmark"));
        assert_eq!(aliased_target("benchmark-db"), Some("x benchmark-db"));
        assert_eq!(aliased_target("check-cycles"), Some("x check-cycles"));
        assert_eq!(aliased_target("check-dead-code"), Some("x check-dead-code"));
        assert_eq!(
            aliased_target("check-promoted-deps"),
            Some("x check-promoted-deps")
        );
        assert_eq!(aliased_target("check-codegen"), Some("x check-codegen"));
        assert_eq!(aliased_target("check-async"), Some("x check-async"));
        assert_eq!(aliased_target("check-imports"), Some("x check-imports"));
        assert_eq!(aliased_target("check-layers"), Some("x check-layers"));
        assert_eq!(aliased_target("check-perf"), Some("x check-perf"));
        assert_eq!(aliased_target("check-platform"), Some("x check-platform"));
        assert_eq!(
            aliased_target("check-unused-code"),
            Some("x check-unused-code")
        );
        assert_eq!(
            aliased_target("check-unused-confidence"),
            Some("x check-unused-confidence")
        );
        assert_eq!(
            aliased_target("check-unused-l10n"),
            Some("x check-unused-l10n")
        );
        assert_eq!(
            aliased_target("check-unused-params"),
            Some("x check-unused-params")
        );
        assert_eq!(aliased_target("check-widgets"), Some("x check-widgets"));
        assert_eq!(aliased_target("certify"), Some("x certify"));
        assert_eq!(aliased_target("cloud"), Some("x cloud"));
        assert_eq!(aliased_target("codebase-intel"), Some("x codebase-intel"));
        assert_eq!(aliased_target("compare"), Some("x compare"));
        assert_eq!(
            aliased_target("compare-branches"),
            Some("x compare-branches")
        );
        assert_eq!(aliased_target("compare-reports"), Some("x compare-reports"));
        assert_eq!(aliased_target("conventions"), Some("x conventions"));
        assert_eq!(
            aliased_target("cognitive-complexity"),
            Some("x cognitive-complexity")
        );
        assert_eq!(aliased_target("dashboard"), Some("x dashboard"));
        assert_eq!(
            aliased_target("deprecation-status"),
            Some("x deprecation-status")
        );
        assert_eq!(aliased_target("discover-rules"), Some("x discover-rules"));
        assert_eq!(aliased_target("docs"), Some("x docs"));
        assert_eq!(aliased_target("drift"), Some("x drift"));
        assert_eq!(aliased_target("enterprise"), Some("x enterprise"));
        assert_eq!(aliased_target("explain"), Some("x explain"));
        assert_eq!(aliased_target("fix-track"), Some("x fix-track"));
        assert_eq!(aliased_target("history"), Some("x history"));
        assert_eq!(aliased_target("learn"), Some("x learn"));
        assert_eq!(aliased_target("marketplace"), Some("x marketplace"));
        assert_eq!(aliased_target("metrics"), Some("x metrics"));
        assert_eq!(aliased_target("partners"), Some("x partners"));
        assert_eq!(aliased_target("perf-track"), Some("x perf-track"));
        assert_eq!(aliased_target("predict"), Some("x predict"));
        assert_eq!(aliased_target("preset"), Some("x preset"));
        assert_eq!(aliased_target("provenance"), Some("x provenance"));
        assert_eq!(aliased_target("rule-docs"), Some("x rule-docs"));
        assert_eq!(aliased_target("rule-impact"), Some("x rule-impact"));
        assert_eq!(aliased_target("score-track"), Some("x score-track"));
        assert_eq!(aliased_target("self-tune"), Some("x self-tune"));
        assert_eq!(
            aliased_target("stability-contract"),
            Some("x stability-contract")
        );
        assert_eq!(aliased_target("smells"), Some("x smells"));
        assert_eq!(aliased_target("suppress"), Some("x suppress"));
        assert_eq!(aliased_target("test-gen"), Some("x test-gen"));
        assert_eq!(aliased_target("trends"), Some("x trends"));
        assert_eq!(aliased_target("upgrade-check"), Some("x upgrade-check"));
        assert_eq!(aliased_target("validate"), Some("x validate"));
    }

    #[test]
    fn aliased_target_ignores_new_namespace_and_unknown_commands() {
        assert_eq!(aliased_target("x"), None);
        assert_eq!(aliased_target("score"), None);
        assert_eq!(aliased_target("unknown"), None);
    }
}
