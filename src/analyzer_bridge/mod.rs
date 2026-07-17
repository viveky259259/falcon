//! Analyzer bridge — co-pilot mode skeleton.
//!
//! Lets Falcon ingest `dart analyze --format=json` output and defer to it on
//! same-line collisions. The Dart analyzer is the source of truth for things
//! like `unused_import`, `invalid_assignment`, etc.; Falcon's role in
//! co-pilot mode is to add AI-specific checks on top, not to duplicate
//! analyzer diagnostics.
//!
use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_DART_ANALYZE_TIMEOUT: Duration = Duration::from_secs(10);
const ANALYZER_TIMEOUT_ENV: &str = "FALCON_ANALYZER_TIMEOUT_MS";

/// One diagnostic reported by `dart analyze --format=json`.
#[derive(Debug, Clone)]
pub struct AnalyzerDiagnostic {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub code: String,     // e.g. "unused_import"
    pub severity: String, // "ERROR" | "WARNING" | "INFO"
    pub message: String,
}

/// A trait Falcon findings already implement (or can be adapted to).
/// Anything with (file, line, rule_id) is enough.
pub trait FalconFindingLike {
    fn file(&self) -> &Path;
    fn line(&self) -> usize;
    fn rule_id(&self) -> &str;
}

impl FalconFindingLike for crate::reporters::Issue {
    fn file(&self) -> &Path {
        &self.file
    }

    fn line(&self) -> usize {
        self.line
    }

    fn rule_id(&self) -> &str {
        &self.rule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleClass {
    Style,
    Type,
    Unused,
    Behavioral,
    Security,
    Other,
}

pub fn falcon_rule_class(rule_id: &str) -> RuleClass {
    let rule = rule_id.to_ascii_lowercase();

    if rule.starts_with("unused-")
        || rule.contains("unused-")
        || rule.contains("dead-code")
        || rule.contains("dead-folder")
    {
        RuleClass::Unused
    } else if rule.contains("hardcoded-credential")
        || rule.contains("credential")
        || rule.contains("secret")
        || rule.contains("token")
        || rule.contains("password")
        || rule.contains("security")
        || rule.contains("vuln")
        || rule.contains("cert")
        || rule.contains("insecure")
        || rule.contains("sql-injection")
    {
        RuleClass::Security
    } else if matches!(
        rule.as_str(),
        "set-state-after-dispose"
            | "dispose-not-called"
            | "unawaited-future-in-build"
            | "fake-mounted-check"
            | "silent-catch"
            | "riverpod-scope-leak"
            | "ensure-dispose-lifecycle"
            | "ensure-stream-subscription-cancel"
            | "avoid-unawaited-futures"
            | "avoid-unnecessary-setstate"
            | "avoid-empty-catch"
            | "avoid-global-state"
            | "prefer-specific-catch-type"
            | "avoid-throw-in-catch"
            | "avoid-throw-in-catch-block"
    ) {
        RuleClass::Behavioral
    } else if rule.contains("dynamic")
        || rule.contains("type")
        || rule.contains("cast")
        || rule.contains("assertion")
        || rule.contains("collection-methods-unrelated-types")
        || rule.contains("collection-methods-with-unrelated-types")
        || rule == "avoid-missing-enum-constant-in-map"
    {
        RuleClass::Type
    } else if rule.starts_with("avoid-")
        || rule.starts_with("prefer-")
        || rule.starts_with("ensure-")
        || rule.contains("naming")
        || rule.contains("identifier")
        || rule.contains("format")
        || rule.contains("style")
        || rule.contains("print")
        || rule.contains("magic-number")
        || rule.contains("long-")
        || rule.contains("nested")
        || rule.contains("duplicate-export")
    {
        RuleClass::Style
    } else {
        RuleClass::Other
    }
}

pub fn analyzer_rule_class(code: &str) -> RuleClass {
    let code = code.to_ascii_lowercase();

    if code.starts_with("unused_") || code == "dead_code" {
        RuleClass::Unused
    } else if matches!(
        code.as_str(),
        "invalid_assignment"
            | "argument_type_not_assignable"
            | "return_of_invalid_type"
            | "undefined_identifier"
            | "undefined_method"
            | "undefined_class"
            | "uri_does_not_exist"
            | "missing_required_argument"
            | "not_enough_positional_arguments"
            | "extra_positional_arguments"
            | "undefined_named_parameter"
            | "wrong_number_of_type_arguments"
            | "unchecked_use_of_nullable_value"
    ) {
        RuleClass::Type
    } else if code.contains("insecure")
        || code.contains("security")
        || code.contains("secret")
        || code.contains("credential")
        || code.contains("password")
        || code.contains("token")
    {
        RuleClass::Security
    } else if matches!(
        code.as_str(),
        "use_build_context_synchronously"
            | "unawaited_futures"
            | "discarded_futures"
            | "cancel_subscriptions"
            | "close_sinks"
    ) {
        RuleClass::Behavioral
    } else if code.starts_with("avoid_")
        || code.starts_with("prefer_")
        || code.starts_with("unnecessary_")
        || code.starts_with("camel_case")
        || code.starts_with("non_constant")
        || code == "file_names"
        || code == "sort_child_properties_last"
    {
        RuleClass::Style
    } else {
        RuleClass::Other
    }
}

// ---------------------------------------------------------------------------
// Internal serde shapes. `dart analyze --format=json` has shifted shape across
// SDK versions; we tolerate both the per-file/per-line NDJSON form and the
// newer combined-JSON form. Unknown fields are ignored.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawLocation {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    range: Option<RawRange>,
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    column: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RawRange {
    #[serde(default)]
    start: Option<RawPos>,
}

#[derive(Debug, Deserialize)]
struct RawPos {
    #[serde(default)]
    line: Option<usize>,
    #[serde(default, rename = "column")]
    col: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RawDiagnostic {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default, rename = "problemMessage")]
    problem_message: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    location: Option<RawLocation>,
    // Legacy / alternate field names that some Dart SDKs emit.
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    column: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RawEnvelope {
    #[serde(default)]
    diagnostics: Vec<RawDiagnostic>,
}

fn raw_to_diag(raw: RawDiagnostic) -> Option<AnalyzerDiagnostic> {
    // Resolve file path from any of the supported shapes.
    let file_str = raw
        .location
        .as_ref()
        .and_then(|l| l.file.clone())
        .or(raw.file.clone())?;

    // Resolve line/column from any of the supported shapes.
    let (line, column) = if let Some(loc) = &raw.location {
        if let Some(range) = &loc.range {
            if let Some(start) = &range.start {
                (start.line.unwrap_or(0), start.col.unwrap_or(0))
            } else {
                (loc.line.unwrap_or(0), loc.column.unwrap_or(0))
            }
        } else {
            (loc.line.unwrap_or(0), loc.column.unwrap_or(0))
        }
    } else {
        (raw.line.unwrap_or(0), raw.column.unwrap_or(0))
    };

    let message = raw.problem_message.or(raw.message).unwrap_or_default();
    let code = raw.code.unwrap_or_default();
    let severity = raw.severity.unwrap_or_else(|| "INFO".to_string());

    Some(AnalyzerDiagnostic {
        file: PathBuf::from(file_str),
        line,
        column,
        code,
        severity,
        message,
    })
}

/// Parse a JSON blob from `dart analyze --format=json`. Public for testability.
///
/// Tolerates two shapes:
/// - A single combined JSON document (object with `diagnostics`, or an array
///   of diagnostics).
/// - NDJSON, where each non-empty line is either an envelope with
///   `diagnostics` or a bare diagnostic object.
pub fn parse_dart_analyze_json(blob: &str) -> Result<Vec<AnalyzerDiagnostic>> {
    let trimmed = blob.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // First, try to parse the entire blob as one JSON value.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(diags) = diagnostics_from_value(&value) {
            return Ok(diags);
        }
    }

    // Fall back to line-by-line NDJSON parsing.
    let mut out = Vec::new();
    for (idx, line) in blob.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(value) => {
                if let Some(mut diags) = diagnostics_from_value(&value) {
                    out.append(&mut diags);
                }
            }
            Err(e) => {
                log::debug!(
                    "analyzer_bridge: skipping line {} (parse error): {}",
                    idx + 1,
                    e
                );
            }
        }
    }

    Ok(out)
}

/// Extract diagnostics from an already-parsed JSON value.
/// Returns `Some(_)` only if the value matched a known shape (envelope,
/// array of diagnostics, or bare diagnostic), even if the resulting Vec is
/// empty. Returns `None` if the shape is entirely unrecognized so callers
/// can fall back to line-by-line parsing.
fn diagnostics_from_value(value: &serde_json::Value) -> Option<Vec<AnalyzerDiagnostic>> {
    // Envelope form: { "diagnostics": [...] }
    if let Some(obj) = value.as_object() {
        if let Some(diags) = obj.get("diagnostics") {
            if let Ok(env) =
                serde_json::from_value::<RawEnvelope>(serde_json::Value::Object(obj.clone()))
            {
                return Some(
                    env.diagnostics
                        .into_iter()
                        .filter_map(raw_to_diag)
                        .collect(),
                );
            }
            // The "diagnostics" key existed but didn't parse as an envelope;
            // try interpreting it as a raw list of diagnostics.
            if let Some(arr) = diags.as_array() {
                return Some(
                    arr.iter()
                        .filter_map(|v| serde_json::from_value::<RawDiagnostic>(v.clone()).ok())
                        .filter_map(raw_to_diag)
                        .collect(),
                );
            }
        }
        // Bare single-diagnostic object.
        if obj.contains_key("code") || obj.contains_key("location") {
            if let Ok(raw) = serde_json::from_value::<RawDiagnostic>(value.clone()) {
                return Some(raw_to_diag(raw).into_iter().collect());
            }
        }
        return None;
    }

    // Top-level array of diagnostics.
    if let Some(arr) = value.as_array() {
        return Some(
            arr.iter()
                .filter_map(|v| serde_json::from_value::<RawDiagnostic>(v.clone()).ok())
                .filter_map(raw_to_diag)
                .collect(),
        );
    }

    None
}

/// Best-effort canonicalization. Falls back to the original path if the
/// filesystem call fails (e.g. the file doesn't exist on disk yet).
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn normalize_diagnostic_paths(project_root: &Path, diagnostics: &mut [AnalyzerDiagnostic]) {
    for diagnostic in diagnostics {
        if diagnostic.file.is_relative() {
            diagnostic.file = project_root.join(&diagnostic.file);
        }
    }
}

/// Run `dart analyze --format=json` in `project_root` and parse results.
///
/// Returns `Ok(None)` if `dart` is not on PATH or there is no
/// `.dart_tool/package_config.json` — the bridge is best-effort, never
/// required.
pub fn run_dart_analyze(project_root: &Path) -> Result<Option<Vec<AnalyzerDiagnostic>>> {
    let timeout = configured_analyzer_timeout();

    // 1. Is dart on PATH?
    let mut probe = Command::new("dart");
    probe.arg("--version");
    match command_output_with_timeout(&mut probe, timeout) {
        Ok(Some(out)) if out.status.success() => {}
        Ok(None) => {
            log::warn!(
                "analyzer_bridge: `dart --version` timed out after {:?}, skipping",
                timeout
            );
            return Ok(None);
        }
        _ => {
            log::debug!("analyzer_bridge: `dart` not available, skipping");
            return Ok(None);
        }
    }

    // 2. Is this a resolved Dart/Flutter project?
    if !crate::paths::has_package_config(project_root) {
        log::debug!(
            "analyzer_bridge: {} missing, skipping",
            project_root
                .join(".dart_tool")
                .join("package_config.json")
                .display()
        );
        return Ok(None);
    }

    // 3. Run analyzer. `dart analyze` exits non-zero on findings; that's
    // fine — we care about stdout, not the exit code.
    let mut analyze = Command::new("dart");
    analyze
        .arg("analyze")
        .arg("--format=json")
        .current_dir(project_root);
    let Some(output) = command_output_with_timeout(&mut analyze, timeout)
        .context("failed to invoke `dart analyze`")?
    else {
        log::warn!(
            "analyzer_bridge: `dart analyze --format=json` timed out after {:?}, skipping",
            timeout
        );
        return Ok(None);
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut diags = parse_dart_analyze_json(&stdout)?;
    normalize_diagnostic_paths(project_root, &mut diags);

    // `dart analyze` exits non-zero when it finds diagnostics — that is
    // normal.  However, if the command failed *and* produced no parseable
    // diagnostics, it likely crashed or the `--format=json` flag is
    // unsupported.  Surface stderr so callers see the bridge failure rather
    // than silently treating it as "no findings" (which would disable deferral
    // without any indication that the bridge is broken).
    if !output.status.success() && diags.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "dart analyze failed (exit {:?}): {}",
                output.status.code(),
                stderr.trim()
            ));
        }
    }

    Ok(Some(diags))
}

fn configured_analyzer_timeout() -> Duration {
    std::env::var(ANALYZER_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_DART_ANALYZE_TIMEOUT)
}

fn command_output_with_timeout(command: &mut Command, timeout: Duration) -> Result<Option<Output>> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout was configured as piped");
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    let stdout_reader = read_pipe(stdout);
    let stderr_reader = read_pipe(stderr);

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let stdout = join_reader(stdout_reader)?;
            let stderr = join_reader(stderr_reader)?;
            return Ok(Some(Output {
                status,
                stdout,
                stderr,
            }));
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            drop(stdout_reader);
            drop(stderr_reader);
            return Ok(None);
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn read_pipe<R: Read + Send + 'static>(
    mut pipe: R,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_reader(handle: thread::JoinHandle<std::io::Result<Vec<u8>>>) -> Result<Vec<u8>> {
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("analyzer subprocess reader panicked"))?
        .context("failed to read analyzer subprocess output")
}

/// Filter Falcon findings, dropping any whose (file, line, rule class) collides
/// with an analyzer diagnostic — UNLESS `no_defer` is true.
///
/// Returns `(surviving, suppressed)`. `suppressed` is empty when
/// `no_defer` is true.
pub fn defer_to_analyzer<F: FalconFindingLike + Clone>(
    falcon: &[F],
    analyzer: &[AnalyzerDiagnostic],
    no_defer: bool,
) -> (Vec<F>, Vec<F>) {
    if no_defer {
        return (falcon.to_vec(), Vec::new());
    }

    if analyzer.is_empty() {
        return (falcon.to_vec(), Vec::new());
    }

    // Pre-compute analyzer collision keys.
    let mut keys: std::collections::HashSet<(PathBuf, usize, RuleClass)> =
        std::collections::HashSet::with_capacity(analyzer.len());
    for diag in analyzer {
        keys.insert((
            canonical(&diag.file),
            diag.line,
            analyzer_rule_class(&diag.code),
        ));
    }

    let mut surviving = Vec::with_capacity(falcon.len());
    let mut suppressed = Vec::new();
    for finding in falcon {
        let key = (
            canonical(finding.file()),
            finding.line(),
            falcon_rule_class(finding.rule_id()),
        );
        if keys.contains(&key) {
            suppressed.push(finding.clone());
        } else {
            surviving.push(finding.clone());
        }
    }

    if !suppressed.is_empty() {
        log::info!(
            "analyzer_bridge: deferred {} finding(s) to dart analyze",
            suppressed.len()
        );
    }

    (surviving, suppressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestFinding {
        file: PathBuf,
        line: usize,
        rule_id: String,
    }

    impl FalconFindingLike for TestFinding {
        fn file(&self) -> &Path {
            &self.file
        }
        fn line(&self) -> usize {
            self.line
        }
        fn rule_id(&self) -> &str {
            &self.rule_id
        }
    }

    fn finding(file: &str, line: usize, rule: &str) -> TestFinding {
        TestFinding {
            file: PathBuf::from(file),
            line,
            rule_id: rule.to_string(),
        }
    }

    fn diag(file: &str, line: usize, code: &str) -> AnalyzerDiagnostic {
        AnalyzerDiagnostic {
            file: PathBuf::from(file),
            line,
            column: 1,
            code: code.to_string(),
            severity: "WARNING".to_string(),
            message: format!("{} at {}:{}", code, file, line),
        }
    }

    #[test]
    fn parse_combined_envelope_json() {
        let blob = r#"{
            "version": 1,
            "diagnostics": [
                {
                    "code": "unused_import",
                    "severity": "INFO",
                    "type": "HINT",
                    "problemMessage": "Unused import: 'dart:async'.",
                    "location": {
                        "file": "/tmp/proj/lib/main.dart",
                        "range": { "start": { "line": 3, "column": 1 } }
                    }
                }
            ]
        }"#;
        let diags = parse_dart_analyze_json(blob).expect("parse ok");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "unused_import");
        assert_eq!(diags[0].line, 3);
        assert_eq!(diags[0].column, 1);
        assert_eq!(diags[0].file, PathBuf::from("/tmp/proj/lib/main.dart"));
        assert!(diags[0].message.contains("Unused import"));
    }

    #[test]
    fn parse_ndjson_multiple_lines() {
        // Two envelopes on separate lines, plus a blank line and a junk line
        // that should be skipped gracefully.
        let blob = "\
{\"diagnostics\":[{\"code\":\"a\",\"location\":{\"file\":\"/a.dart\",\"range\":{\"start\":{\"line\":1,\"column\":1}}}}]}

not-json-at-all
{\"diagnostics\":[{\"code\":\"b\",\"location\":{\"file\":\"/b.dart\",\"range\":{\"start\":{\"line\":7,\"column\":2}}}}]}
";
        let diags = parse_dart_analyze_json(blob).expect("parse ok");
        assert_eq!(diags.len(), 2);
        let codes: Vec<_> = diags.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&"a"));
        assert!(codes.contains(&"b"));
    }

    #[test]
    fn parse_empty_blob_returns_empty() {
        assert!(parse_dart_analyze_json("").unwrap().is_empty());
        assert!(parse_dart_analyze_json("   \n\n").unwrap().is_empty());
    }

    #[test]
    fn parse_top_level_array() {
        let blob = r#"[
            {"code":"x","location":{"file":"/x.dart","range":{"start":{"line":2,"column":3}}}},
            {"code":"y","location":{"file":"/y.dart","range":{"start":{"line":4,"column":5}}}}
        ]"#;
        let diags = parse_dart_analyze_json(blob).expect("parse ok");
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].file, PathBuf::from("/x.dart"));
        assert_eq!(diags[1].line, 4);
    }

    #[test]
    fn parse_ignores_unknown_fields() {
        let blob = r#"{
            "extra_top_level": "ignored",
            "diagnostics": [
                {
                    "code": "z",
                    "weird_field": 42,
                    "location": {
                        "file": "/z.dart",
                        "range": { "start": { "line": 5, "column": 6 } }
                    }
                }
            ]
        }"#;
        let diags = parse_dart_analyze_json(blob).expect("parse ok");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "z");
    }

    #[test]
    fn defer_empty_inputs() {
        let (surviving, suppressed) = defer_to_analyzer::<TestFinding>(&[], &[], false);
        assert!(surviving.is_empty());
        assert!(suppressed.is_empty());
    }

    #[test]
    fn defer_no_overlap_keeps_all() {
        let falcon = vec![finding("/proj/lib/a.dart", 10, "avoid-unused-parameters")];
        let analyzer = vec![diag("/proj/lib/b.dart", 10, "unused_import")];
        let (surviving, suppressed) = defer_to_analyzer(&falcon, &analyzer, false);
        assert_eq!(surviving.len(), 1);
        assert!(suppressed.is_empty());
    }

    #[test]
    fn defer_exact_line_and_class_overlap_suppresses() {
        let falcon = vec![
            finding("/proj/lib/a.dart", 10, "avoid-print-in-production"),
            finding("/proj/lib/a.dart", 12, "avoid-print-in-production"),
        ];
        let analyzer = vec![diag("/proj/lib/a.dart", 10, "avoid_print")];
        let (surviving, suppressed) = defer_to_analyzer(&falcon, &analyzer, false);
        assert_eq!(surviving.len(), 1);
        assert_eq!(surviving[0].line, 12);
        assert_eq!(suppressed.len(), 1);
        assert_eq!(suppressed[0].line, 10);
    }

    #[test]
    fn defer_same_line_different_class_keeps_behavioral_finding() {
        let falcon = vec![finding("/proj/lib/a.dart", 10, "set-state-after-dispose")];
        let analyzer = vec![diag("/proj/lib/a.dart", 10, "avoid_print")];
        let (surviving, suppressed) = defer_to_analyzer(&falcon, &analyzer, false);
        assert_eq!(surviving.len(), 1);
        assert!(suppressed.is_empty());
    }

    #[test]
    fn defer_unused_falcon_finding_to_unused_analyzer_diagnostic() {
        let falcon = vec![finding("/proj/lib/a.dart", 10, "unused-code")];
        let analyzer = vec![diag("/proj/lib/a.dart", 10, "unused_import")];
        let (surviving, suppressed) = defer_to_analyzer(&falcon, &analyzer, false);
        assert!(surviving.is_empty());
        assert_eq!(suppressed.len(), 1);
    }

    #[test]
    fn defer_same_file_different_line_keeps_both() {
        let falcon = vec![finding("/proj/lib/a.dart", 9, "avoid-unused-parameters")];
        let analyzer = vec![diag("/proj/lib/a.dart", 10, "unused_import")];
        let (surviving, suppressed) = defer_to_analyzer(&falcon, &analyzer, false);
        assert_eq!(surviving.len(), 1);
        assert!(suppressed.is_empty());
    }

    #[test]
    fn defer_no_defer_flag_bypasses_suppression() {
        let falcon = vec![finding("/proj/lib/a.dart", 10, "avoid-unused-parameters")];
        let analyzer = vec![diag("/proj/lib/a.dart", 10, "unused_import")];
        let (surviving, suppressed) = defer_to_analyzer(&falcon, &analyzer, true);
        assert_eq!(surviving.len(), 1);
        assert!(suppressed.is_empty());
    }

    #[test]
    fn rule_class_mappings_cover_core_cases() {
        assert_eq!(
            falcon_rule_class("avoid-print-in-production"),
            RuleClass::Style
        );
        assert_eq!(falcon_rule_class("unused-code"), RuleClass::Unused);
        assert_eq!(
            falcon_rule_class("set-state-after-dispose"),
            RuleClass::Behavioral
        );
        assert_eq!(
            falcon_rule_class("avoid-throw-in-catch-block"),
            RuleClass::Behavioral
        );
        assert_eq!(
            falcon_rule_class("avoid-collection-methods-with-unrelated-types"),
            RuleClass::Type
        );
        assert_eq!(
            falcon_rule_class("avoid-missing-enum-constant-in-map"),
            RuleClass::Type
        );
        assert_eq!(
            falcon_rule_class("avoid-hardcoded-credentials"),
            RuleClass::Security
        );
        assert_eq!(analyzer_rule_class("avoid_print"), RuleClass::Style);
        assert_eq!(analyzer_rule_class("unused_import"), RuleClass::Unused);
        assert_eq!(analyzer_rule_class("invalid_assignment"), RuleClass::Type);
        assert_eq!(
            analyzer_rule_class("use_build_context_synchronously"),
            RuleClass::Behavioral
        );
    }

    #[test]
    fn normalize_relative_diagnostic_paths_against_project_root() {
        let project_root = PathBuf::from("/tmp/project");
        let mut diagnostics = vec![
            diag("lib/main.dart", 2, "unused_import"),
            diag("/outside/file.dart", 3, "invalid_assignment"),
        ];

        normalize_diagnostic_paths(&project_root, &mut diagnostics);

        assert_eq!(
            diagnostics[0].file,
            PathBuf::from("/tmp/project/lib/main.dart")
        );
        assert_eq!(diagnostics[1].file, PathBuf::from("/outside/file.dart"));
    }

    #[test]
    fn issue_implements_finding_like_for_deferral() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("lib").join("main.dart");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "void main() {}\n").unwrap();

        let falcon = vec![crate::reporters::Issue {
            rule: "avoid-print-in-production".to_string(),
            message: "print found".to_string(),
            severity: crate::config::Severity::Warning,
            file: file.clone(),
            line: 1,
            column: 1,
        }];
        let analyzer = vec![AnalyzerDiagnostic {
            file,
            line: 1,
            column: 1,
            code: "avoid_print".to_string(),
            severity: "INFO".to_string(),
            message: "avoid print".to_string(),
        }];

        let (surviving, suppressed) = defer_to_analyzer(&falcon, &analyzer, false);

        assert!(surviving.is_empty());
        assert_eq!(suppressed.len(), 1);
        assert_eq!(suppressed[0].rule, "avoid-print-in-production");
    }
}
