/// Calculates the Maintainability Index (MI) using the standard formula:
/// MI = max(0, (171 - 5.2 * ln(HV) - 0.23 * CC - 16.2 * ln(LOC)) * 100 / 171)
///
/// Where:
///   HV = Halstead Volume (simplified to line-based estimate for MVP)
///   CC = Cyclomatic Complexity
///   LOC = Source Lines of Code
pub fn calculate(cyclomatic_complexity: u32, source_lines: u32, halstead_volume: f64) -> f64 {
    let cc = cyclomatic_complexity as f64;
    let loc = (source_lines.max(1)) as f64;

    let hv = if halstead_volume > 0.0 {
        halstead_volume
    } else {
        loc * (2.0_f64).log2() * 2.0
    };

    let mi = 171.0 - 5.2 * hv.max(1.0).ln() - 0.23 * cc - 16.2 * loc.ln();
    let normalized = (mi * 100.0 / 171.0).clamp(0.0, 100.0);

    (normalized * 100.0).round() / 100.0
}
