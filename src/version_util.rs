//! Shared semantic-version comparison and pubspec constraint satisfaction.

use std::cmp::Ordering;

/// Compare two dotted version strings numerically.
///
/// Contract: Split each version by dots, parse each segment as u64 (treating
/// unparseable segments as 0), ignore any `-` or `+` suffix and everything after,
/// strip leading 'v', and compare the resulting numeric vectors element-wise.
/// Missing trailing segments are treated as 0.
///
/// Examples:
/// - `3.24.0-1.2.pre` equals `3.24.0` (suffix ignored)
/// - `v3.24.0` equals `3.24.0` (v-prefix stripped)
/// - `3.24` equals `3.24.0` (missing segment treated as 0)
/// - `3.x.0` equals `3.0.0` (unparseable segment treated as 0)
///
/// This algorithm differs from the prior private implementation: it zero-pads
/// missing and unparseable segments rather than dropping them. This fixes
/// a bug where `3.27.0-1.2.pre` would incorrectly compare as greater than
/// `3.27.0` (the suffix would be parsed as segments [2], making it [3,27,2]).
pub fn version_cmp(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> Vec<u64> {
        v.strip_prefix('v')
            .unwrap_or(v)
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (pa, pb) = (parts(a), parts(b));
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// Does `version` satisfy a pubspec-style `constraint`?
///
/// Supports `any`, the empty string, `^x.y.z`, and space-separated
/// comparator lists such as `>=3.4.0 <4.0.0`. An unrecognised constraint
/// returns `false` — callers must treat that as "cannot determine".
///
/// Caret constraint rules (pubspec-style):
/// - `^x.y.z` where x > 0: allows `>=x.y.z <(x+1).0.0`
/// - `^0.y.z` where y > 0: allows `>=0.y.z <0.(y+1).0`
/// - `^0.0.z`: allows `>=0.0.z <0.0.(z+1)`
pub fn satisfies(version: &str, constraint: &str) -> bool {
    let c = constraint.trim().trim_matches(['\'', '"']);
    if c.is_empty() || c == "any" {
        return true;
    }
    if let Some(base) = c.strip_prefix('^') {
        let base = base.trim();
        if version_cmp(version, base) == Ordering::Less {
            return false;
        }
        // Parse major.minor.patch for caret constraint logic
        let mut parts = base.split('.');
        let major = parts.next().unwrap_or("0").parse::<u64>().unwrap_or(0);
        let minor = parts.next().unwrap_or("0").parse::<u64>().unwrap_or(0);
        let patch = parts.next().unwrap_or("0").parse::<u64>().unwrap_or(0);

        let upper = if major > 0 {
            // ^x.y.z where x > 0: upper bound is (x+1).0.0
            format!("{}.0.0", major + 1)
        } else if minor > 0 {
            // ^0.y.z where y > 0: upper bound is 0.(y+1).0
            format!("0.{}.0", minor + 1)
        } else {
            // ^0.0.z: upper bound is 0.0.(z+1)
            format!("0.0.{}", patch + 1)
        };

        return version_cmp(version, &upper) == Ordering::Less;
    }
    let mut saw_comparator = false;
    for token in c.split_whitespace() {
        let (op, rhs) = split_comparator(token);
        let Some(op) = op else { return false };
        saw_comparator = true;
        let ord = version_cmp(version, rhs);
        let ok = match op {
            ">=" => ord != Ordering::Less,
            "<=" => ord != Ordering::Greater,
            ">" => ord == Ordering::Greater,
            "<" => ord == Ordering::Less,
            "=" => ord == Ordering::Equal,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    saw_comparator
}

fn split_comparator(token: &str) -> (Option<&str>, &str) {
    for op in [">=", "<=", ">", "<", "="] {
        if let Some(rest) = token.strip_prefix(op) {
            return (Some(op), rest.trim());
        }
    }
    (None, token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn compares_numeric_segments_not_strings() {
        assert_eq!(version_cmp("3.10.0", "3.9.0"), Ordering::Greater);
    }

    #[test]
    fn equal_versions_compare_equal() {
        assert_eq!(version_cmp("3.24.5", "3.24.5"), Ordering::Equal);
    }

    #[test]
    fn caret_constraint_allows_same_major() {
        assert!(satisfies("3.24.5", "^3.22.0"));
        assert!(!satisfies("4.0.0", "^3.22.0"));
        assert!(!satisfies("3.21.0", "^3.22.0"));
    }

    #[test]
    fn range_constraint_respects_both_bounds() {
        assert!(satisfies("3.5.0", ">=3.4.0 <4.0.0"));
        assert!(!satisfies("4.0.0", ">=3.4.0 <4.0.0"));
        assert!(!satisfies("3.3.0", ">=3.4.0 <4.0.0"));
    }

    #[test]
    fn any_constraint_accepts_everything() {
        assert!(satisfies("1.0.0", "any"));
        assert!(satisfies("1.0.0", ""));
    }

    #[test]
    fn prerelease_suffix_is_ignored_for_ordering() {
        assert_eq!(version_cmp("3.24.0-1.2.pre", "3.24.0"), Ordering::Equal);
    }

    #[test]
    fn unparseable_constraint_is_not_satisfied() {
        assert!(!satisfies("3.24.5", "wat"));
    }

    #[test]
    fn missing_trailing_segments_treated_as_zero() {
        assert_eq!(version_cmp("3.24", "3.24.0"), Ordering::Equal);
        assert_eq!(version_cmp("3", "3.0.0"), Ordering::Equal);
    }

    #[test]
    fn unparseable_segments_treated_as_zero() {
        assert_eq!(version_cmp("3.x.0", "3.0.0"), Ordering::Equal);
        assert_eq!(version_cmp("3.abc", "3"), Ordering::Equal);
    }

    #[test]
    fn prerelease_beta_suffix_does_not_corrupt_comparison() {
        // This is the bug that motivated the algorithm change:
        // Old: "3.27.0-1.2.pre".split('.') = ["3","27","0-1","2","pre"]
        //      After filter_map, this becomes [3,27,2], which is > [3,27,0]
        // New: We strip the suffix before splitting, so it's [3,27,0] == [3,27,0]
        assert_eq!(version_cmp("3.27.0-1.2.pre", "3.27.0"), Ordering::Equal);
    }

    #[test]
    fn v_prefix_is_stripped() {
        assert_eq!(version_cmp("v3.24.0", "3.24.0"), Ordering::Equal);
        assert_eq!(version_cmp("v3.24.0", "v3.24.0"), Ordering::Equal);
        assert_eq!(version_cmp("v3.24.0", "3.23.0"), Ordering::Greater);
    }

    #[test]
    fn caret_major_greater_than_zero() {
        // ^3.22.0 -> >=3.22.0 <4.0.0
        assert!(satisfies("3.22.0", "^3.22.0"));
        assert!(satisfies("3.24.5", "^3.22.0"));
        assert!(satisfies("3.999.999", "^3.22.0"));
        assert!(!satisfies("4.0.0", "^3.22.0"));
        assert!(!satisfies("3.21.0", "^3.22.0"));
    }

    #[test]
    fn caret_major_zero_minor_greater_than_zero() {
        // ^0.5.0 -> >=0.5.0 <0.6.0 (pubspec-style, not semver)
        assert!(satisfies("0.5.0", "^0.5.0"));
        assert!(satisfies("0.5.5", "^0.5.0"));
        assert!(satisfies("0.5.999", "^0.5.0"));
        assert!(!satisfies("0.6.0", "^0.5.0"));
        assert!(!satisfies("0.4.999", "^0.5.0"));
    }

    #[test]
    fn caret_major_zero_minor_zero() {
        // ^0.0.3 -> >=0.0.3 <0.0.4 (patch-bounded for pre-alpha)
        assert!(satisfies("0.0.3", "^0.0.3"));
        assert!(!satisfies("0.0.4", "^0.0.3"));
        assert!(!satisfies("0.0.2", "^0.0.3"));
        assert!(!satisfies("0.1.0", "^0.0.3"));
    }
}
