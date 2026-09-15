//! Shared semantic-version comparison and pubspec constraint satisfaction.

use std::cmp::Ordering;

/// Compare two dotted version strings numerically. Any `-suffix` or
/// `+suffix` is ignored, so `3.24.0-1.2.pre` equals `3.24.0`.
/// Leading 'v' is also stripped, so `v3.24.0` equals `3.24.0`.
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
        let major = base.split('.').next().unwrap_or("0");
        let next_major = major.parse::<u64>().map(|m| m + 1).unwrap_or(0);
        return version_cmp(version, &format!("{}.0.0", next_major)) == Ordering::Less;
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
}
