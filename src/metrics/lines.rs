/// Returns (total_lines, source_lines_of_code).
/// Source lines exclude blank lines and comment-only lines.
pub fn count_lines(source: &str) -> (u32, u32) {
    let mut total = 0u32;
    let mut sloc = 0u32;
    let mut in_block_comment = false;

    for line in source.lines() {
        total += 1;
        let trimmed = line.trim();

        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("//") {
            continue;
        }

        if trimmed.starts_with("///") {
            continue;
        }

        if trimmed.starts_with("/*") {
            in_block_comment = !trimmed.contains("*/");
            continue;
        }

        sloc += 1;
    }

    (total, sloc)
}
