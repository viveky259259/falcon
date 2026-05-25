//! Parse `ios/Podfile` (and `macos/Podfile`) for the `platform` directive.

/// Result of scanning a Podfile for its platform directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodfilePlatform {
    /// The major.minor version string declared (e.g. "13.0"). None when absent.
    pub version: Option<String>,
}

/// Parse the contents of a `Podfile` and return the iOS platform version.
pub fn parse_ios_platform(podfile: &str) -> PodfilePlatform {
    parse_platform(podfile, "ios")
}

/// Same for macOS Podfile.
pub fn parse_osx_platform(podfile: &str) -> PodfilePlatform {
    parse_platform(podfile, "osx")
}

fn parse_platform(podfile: &str, kind: &str) -> PodfilePlatform {
    let needle_lower = format!("platform :{kind}");
    for raw in podfile.lines() {
        let line = strip_comment(raw);
        let trimmed = line.trim();
        if !trimmed.contains(&needle_lower) {
            continue;
        }
        if let Some(version) = extract_first_quoted(trimmed) {
            return PodfilePlatform { version: Some(version) };
        }
    }
    PodfilePlatform { version: None }
}

fn strip_comment(line: &str) -> &str {
    for (idx, ch) in line.char_indices() {
        if ch == '#' {
            return &line[..idx];
        }
    }
    line
}

fn extract_first_quoted(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\'' || c == b'"' {
            let quote = c;
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j > start {
                return std::str::from_utf8(&bytes[start..j]).ok().map(|s| s.to_string());
            }
        }
        i += 1;
    }
    None
}

/// Compare two `MAJOR.MINOR` version strings. Returns negative if `a < b`,
/// zero if equal, positive if `a > b`. Unparseable segments compare as 0.
pub fn cmp_version(a: &str, b: &str) -> i32 {
    let parse = |v: &str| -> (u32, u32) {
        let mut it = v.split('.').filter_map(|s| s.parse::<u32>().ok());
        (it.next().unwrap_or(0), it.next().unwrap_or(0))
    };
    let (am, an) = parse(a);
    let (bm, bn) = parse(b);
    if am != bm {
        am as i32 - bm as i32
    } else {
        an as i32 - bn as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ios_platform_basic() {
        let pf = "platform :ios, '13.0'\n";
        assert_eq!(parse_ios_platform(pf).version, Some("13.0".to_string()));
    }

    #[test]
    fn parse_ios_platform_double_quoted() {
        let pf = "platform :ios, \"14.5\"\n";
        assert_eq!(parse_ios_platform(pf).version, Some("14.5".to_string()));
    }

    #[test]
    fn parse_ios_platform_absent() {
        let pf = "target 'Runner' do\n  use_frameworks!\nend\n";
        assert_eq!(parse_ios_platform(pf).version, None);
    }

    #[test]
    fn parse_ios_platform_commented_line_ignored() {
        let pf = "# platform :ios, '14.0'\n";
        assert_eq!(parse_ios_platform(pf).version, None);
    }

    #[test]
    fn parse_ios_platform_inline_comment_ignored_after_directive() {
        let pf = "platform :ios, '12.0' # legacy\n";
        assert_eq!(parse_ios_platform(pf).version, Some("12.0".to_string()));
    }

    #[test]
    fn parse_osx_platform_basic() {
        let pf = "platform :osx, '10.15'\n";
        assert_eq!(parse_osx_platform(pf).version, Some("10.15".to_string()));
    }

    #[test]
    fn cmp_version_ordering() {
        assert!(cmp_version("12.0", "13.0") < 0);
        assert!(cmp_version("13.0", "12.0") > 0);
        assert_eq!(cmp_version("13.0", "13.0"), 0);
        assert!(cmp_version("13.5", "13.0") > 0);
        assert!(cmp_version("13.0", "13.5") < 0);
        assert_eq!(cmp_version("foo.bar", "0.0"), 0);
    }
}
