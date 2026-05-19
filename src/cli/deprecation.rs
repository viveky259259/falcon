//! Deprecation / alias warning helpers.
//!
//! On Sept 1 the CLI cuts over to four primary verbs (`review`, `check`,
//! `fix`, `score`) and everything else moves behind `falcon x <thing>`.
//! Until then, both forms must keep working. After the cutover, each
//! aliased command will call [`warn_aliased`] so users see a one-line
//! migration hint before the legacy form is removed in v1.0.
//!
//! This helper is intentionally not wired into any command yet — landing
//! it now keeps the Sept 1 flip a one-line change per call site.

/// Emit a one-line deprecation warning to stderr telling the user that
/// `falcon <old>` is now aliased to `falcon <new>` and will be removed
/// in v1.0.
pub fn warn_aliased(old: &str, new: &str) {
    eprintln!(
        "warning: `falcon {old}` is being aliased to `falcon {new}` and will be removed in v1.0. Run `falcon {new}` instead."
    );
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
}
