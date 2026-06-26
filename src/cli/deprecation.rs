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
    fn aliased_target_maps_moved_x_commands() {
        assert_eq!(aliased_target("asset-audit"), Some("x asset-audit"));
        assert_eq!(aliased_target("docs"), Some("x docs"));
        assert_eq!(aliased_target("test-gen"), Some("x test-gen"));
    }

    #[test]
    fn aliased_target_ignores_new_namespace_and_unknown_commands() {
        assert_eq!(aliased_target("x"), None);
        assert_eq!(aliased_target("score"), None);
        assert_eq!(aliased_target("unknown"), None);
    }
}
