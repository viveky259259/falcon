/// Extract a window of source lines centered on `line` (1-based), with `radius`
/// lines of context on each side. Clamps to file bounds. Returns the window text.
pub fn extract_code_window(source: &str, line: usize, radius: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let center = line.saturating_sub(1).min(lines.len() - 1);
    let start = center.saturating_sub(radius);
    let end = (center + radius).min(lines.len() - 1);
    lines[start..=end].join("\n")
}

#[cfg(test)]
mod window_tests {
    use super::*;

    const SRC: &str = "l1\nl2\nl3\nl4\nl5";

    #[test]
    fn window_centered_in_middle() {
        assert_eq!(extract_code_window(SRC, 3, 1), "l2\nl3\nl4");
    }

    #[test]
    fn window_clamps_at_start() {
        assert_eq!(extract_code_window(SRC, 1, 2), "l1\nl2\nl3");
    }

    #[test]
    fn window_clamps_at_end() {
        assert_eq!(extract_code_window(SRC, 5, 2), "l3\nl4\nl5");
    }

    #[test]
    fn empty_source_yields_empty() {
        assert_eq!(extract_code_window("", 1, 3), "");
    }

    #[test]
    fn line_beyond_eof_clamps_to_last() {
        assert_eq!(extract_code_window(SRC, 999, 1), "l4\nl5");
    }
}
