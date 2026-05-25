# Preflight: `falcon check-a11y` — Implementation Plan (PR 2 of 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Ship `falcon check-a11y`: verify `SemanticsBinding.instance.ensureSemantics()` is called in `main()` AND a configurable % of interactive widgets carry `Semantics(identifier: ...)`. Emits an Error if `ensureSemantics()` is missing, a Warning if interactive-widget coverage is below threshold, plus per-widget Suggestion entries for unwrapped interactive widgets.

**Architecture:** Uses the existing `DartParser` (tree-sitter Dart). Two scanners — `main_check.rs` (single-file scan of `lib/main.dart`) and `widget_scan.rs` (walks `lib/` for `.dart` files). Reuses everything from PR1 (`preflight::PreflightIssue`, `OutputFormat`, `reporter`, `FalconConfig.preflight`).

**Tech Stack:** Rust 2021, existing `DartParser`, `tree_sitter::Node` walking, `tempfile` (dev-dep). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-05-25-preflight-checks-design.md` v2 (§check-a11y).

---

## File Structure

**Created:**
- `src/check_a11y/mod.rs` — `pub fn run(root, format, config) -> anyhow::Result<i32>` orchestrator.
- `src/check_a11y/main_check.rs` — `pub fn check_main_dart(source: &str) -> Option<MainCheckIssue>`.
- `src/check_a11y/widget_scan.rs` — interactive-widget enumeration + `Semantics(identifier:)` wrapping check.
- `tests/preflight_a11y_tests.rs` — integration test via the CLI binary.

**Modified:**
- `src/lib.rs` — register `pub mod check_a11y;`.
- `src/main.rs` — add `Commands::CheckA11y` variant + dispatch arm.

---

## Task 1: Scaffold `check_a11y` module + main_check.rs (ensureSemantics detection)

**Files:**
- Create: `src/check_a11y/mod.rs` (stub)
- Create: `src/check_a11y/main_check.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/check_a11y/main_check.rs`**

```rust
//! Detect whether `SemanticsBinding.instance.ensureSemantics()` is called in
//! the project's `main()` function.

/// Result of the main-file scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainCheckResult {
    /// `ensureSemantics()` is called (active, uncommented).
    Present,
    /// The call is in source but is part of a commented-out line.
    CommentedOut,
    /// No call is found anywhere in the file.
    Missing,
    /// The file could not be read or had no `main()` function.
    NoMainFunction,
}

/// Scan a `main.dart` source string and report whether `ensureSemantics()` is
/// present, commented out, or missing.
///
/// This is a textual scan rather than a tree-sitter walk — `ensureSemantics`
/// is a unique enough identifier that a substring search after stripping line
/// comments is both faster and more robust to tree-sitter's quirks with
/// Flutter's evolving syntax.
pub fn check_main_dart(source: &str) -> MainCheckResult {
    if !source.contains("main(") && !source.contains("main (") {
        return MainCheckResult::NoMainFunction;
    }

    // Look for any active line that calls ensureSemantics()
    let mut found_commented = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            if line.contains("ensureSemantics") {
                found_commented = true;
            }
            continue;
        }
        // Strip an inline `//` comment, if any.
        let code_only = match trimmed.find("//") {
            Some(idx) => &trimmed[..idx],
            None => trimmed,
        };
        if code_only.contains("ensureSemantics") {
            return MainCheckResult::Present;
        }
        if trimmed.contains("ensureSemantics") {
            // The call is in this line but the part before any `//` did not
            // contain it — so it's inside a trailing comment.
            found_commented = true;
        }
    }

    if found_commented {
        MainCheckResult::CommentedOut
    } else {
        MainCheckResult::Missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_active_ensure_semantics_call() {
        let src = r#"
import 'package:flutter/widgets.dart';
void main() {
  SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Present);
    }

    #[test]
    fn detects_commented_out_call_line_comment() {
        let src = r#"
void main() {
  // SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::CommentedOut);
    }

    #[test]
    fn detects_commented_out_call_trailing_comment() {
        let src = r#"
void main() {
  runApp(MyApp()); // SemanticsBinding.instance.ensureSemantics();
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::CommentedOut);
    }

    #[test]
    fn returns_missing_when_no_call_anywhere() {
        let src = r#"
void main() {
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Missing);
    }

    #[test]
    fn returns_no_main_function_when_main_absent() {
        let src = "class Foo {}\n";
        assert_eq!(check_main_dart(src), MainCheckResult::NoMainFunction);
    }

    #[test]
    fn active_call_takes_precedence_over_commented() {
        let src = r#"
void main() {
  // ensureSemantics() — legacy comment
  SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Present);
    }

    #[test]
    fn handles_arrow_body_main() {
        let src = "void main() => runApp(MyApp());\n";
        // No ensureSemantics anywhere → Missing (but main is present).
        assert_eq!(check_main_dart(src), MainCheckResult::Missing);
    }

    #[test]
    fn handles_async_main() {
        let src = r#"
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Present);
    }
}
```

- [ ] **Step 2: Create `src/check_a11y/mod.rs` (stub)**

```rust
//! `falcon check-a11y` — verify Flutter project is ready for Maestro UI testing.

pub mod main_check;
```

- [ ] **Step 3: Register the module in `src/lib.rs`**

Add `pub mod check_a11y;` near other `pub mod check_*` declarations (alphabetical, between `check_assets` and any other `check_*` if present).

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib check_a11y::main_check::tests`
Expected: 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/check_a11y/mod.rs src/check_a11y/main_check.rs src/lib.rs
git commit -m "feat(check-a11y): scaffold module + ensureSemantics() detection

Adds check_main_dart() returning MainCheckResult (Present /
CommentedOut / Missing / NoMainFunction). Uses a textual scan rather
than tree-sitter — ensureSemantics is unique enough that line-by-line
comment handling is faster and more robust.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: widget_scan.rs — interactive widget enumeration + Semantics wrap detection

**Files:**
- Create: `src/check_a11y/widget_scan.rs`
- Modify: `src/check_a11y/mod.rs` (add `pub mod widget_scan;`)

- [ ] **Step 1: Create `src/check_a11y/widget_scan.rs`**

```rust
//! Find interactive Flutter widgets and check whether each is wrapped in a
//! `Semantics(identifier: ...)` within 3 ancestor widgets.

use crate::parser::DartParser;
use tree_sitter::Node;

/// The set of widget identifiers Falcon considers "interactive" for Maestro
/// testing purposes. Wrapping any of these in `Semantics(identifier: ...)`
/// makes them targetable from Maestro flows.
pub const INTERACTIVE_WIDGETS: &[&str] = &[
    "ElevatedButton",
    "TextButton",
    "OutlinedButton",
    "IconButton",
    "FloatingActionButton",
    "TextField",
    "TextFormField",
    "GestureDetector",
    "InkWell",
    "InkResponse",
    "DropdownButton",
    "DropdownButtonFormField",
    "Checkbox",
    "CheckboxListTile",
    "Switch",
    "SwitchListTile",
    "Radio",
    "RadioListTile",
];

/// Maximum number of enclosing widgets we consider when looking for a wrapping
/// `Semantics(identifier:)`. The spec's "within 3 ancestors" rule.
const ANCESTOR_LOOKBACK: usize = 3;

/// A single interactive-widget occurrence found in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetOccurrence {
    /// Which interactive widget kind (e.g. "TextField").
    pub kind: String,
    /// 1-based line number where the widget appears.
    pub line: usize,
    /// Whether one of the nearest `ANCESTOR_LOOKBACK` enclosing widgets is
    /// `Semantics(identifier: ...)`.
    pub wrapped: bool,
}

/// Walk a Dart source string and return every interactive-widget occurrence
/// together with whether it is wrapped in `Semantics(identifier:)`.
pub fn scan_widgets(source: &str) -> Vec<WidgetOccurrence> {
    let mut parser = match DartParser::new() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let tree = match parser.parse(source) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut occurrences = Vec::new();
    walk(tree.root_node(), source, &mut Vec::new(), &mut occurrences);
    occurrences
}

fn walk<'a>(
    node: Node<'a>,
    source: &str,
    enclosing_widgets: &mut Vec<&'a str>,
    out: &mut Vec<WidgetOccurrence>,
) {
    // Tree-sitter's Dart grammar emits `identifier` nodes inside the function
    // position of a `function_expression_invocation` / `instance_creation_expression`.
    // For our purposes "the widget being constructed at this position" is the
    // name in the first identifier child.
    let widget_name = widget_name_at_node(node, source);

    if let Some(name) = widget_name {
        // Is this an interactive widget?
        if INTERACTIVE_WIDGETS.iter().any(|w| *w == name) {
            let wrapped = enclosing_widgets
                .iter()
                .rev()
                .take(ANCESTOR_LOOKBACK)
                .any(|ancestor| {
                    *ancestor == "Semantics"
                        && enclosing_widgets
                            .iter()
                            .rev()
                            .take(ANCESTOR_LOOKBACK)
                            .next()
                            .is_some()
                });
            // A simpler reliable check: was the nearest Semantics call (within
            // the window) given an `identifier:` argument? We re-scan the
            // ancestor lookup window's nodes here using the parent chain.
            let wrapped = nearest_semantics_with_identifier(node, source, ANCESTOR_LOOKBACK);

            out.push(WidgetOccurrence {
                kind: name.to_string(),
                line: node.start_position().row + 1,
                wrapped,
            });
        }
        enclosing_widgets.push(box_leak(name));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, enclosing_widgets, out);
    }

    if widget_name.is_some() {
        enclosing_widgets.pop();
    }
}

/// Look up at most `lookback` widget calls in the ancestor chain (via
/// tree-sitter parent links) and check whether any is `Semantics(...)` with
/// an `identifier:` argument.
fn nearest_semantics_with_identifier(node: Node<'_>, source: &str, lookback: usize) -> bool {
    let mut current = node.parent();
    let mut seen = 0usize;
    while let Some(n) = current {
        if seen >= lookback * 2 {
            // Walk a few more layers than the strict widget count to allow for
            // intermediate AST nodes that aren't themselves widgets.
            break;
        }
        if let Some(name) = widget_name_at_node(n, source) {
            seen += 1;
            if name == "Semantics" {
                if has_identifier_argument(n, source) {
                    return true;
                }
            }
            if seen >= lookback {
                break;
            }
        }
        current = n.parent();
    }
    false
}

/// Return the constructor/identifier name if `node` looks like a widget call
/// site (e.g. `Foo(...)`).
fn widget_name_at_node<'a>(node: Node<'a>, source: &str) -> Option<&'a str> {
    let kind = node.kind();
    // Common kinds Dart's tree-sitter grammar produces for `Foo(...)` calls.
    if kind != "function_expression_invocation"
        && kind != "instance_creation_expression"
        && kind != "new_expression"
    {
        return None;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return child.utf8_text(source.as_bytes()).ok();
        }
    }
    None
}

/// Heuristic: does this call node have an `identifier:` named argument?
fn has_identifier_argument(node: Node<'_>, source: &str) -> bool {
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");
    // We accept any of:  identifier:  Semantics.fromProperties(identifier: ...)
    text.contains("identifier:")
}

/// Leak `name` to `&'static str`. tree-sitter's borrowed identifiers don't
/// survive across recursive `walk` calls — we copy them into a Box::leak so
/// the enclosing-widgets stack is `'static`-friendly. The stack is small
/// (depth ≤ tens) and tests pay no measurable cost.
fn box_leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_widgets_finds_textfield() {
        let src = r#"
import 'package:flutter/material.dart';
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return TextField(controller: c);
  }
}
"#;
        let result = scan_widgets(src);
        assert!(result.iter().any(|o| o.kind == "TextField"));
    }

    #[test]
    fn scan_widgets_marks_wrapped_when_inside_semantics_with_identifier() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Semantics(
      identifier: 'email-field',
      child: TextField(controller: c),
    );
  }
}
"#;
        let result = scan_widgets(src);
        let tf = result.iter().find(|o| o.kind == "TextField").expect("TextField found");
        assert!(tf.wrapped, "expected wrapped TextField, got {tf:?}");
    }

    #[test]
    fn scan_widgets_marks_unwrapped_when_semantics_missing_identifier() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: 'email field',
      child: TextField(controller: c),
    );
  }
}
"#;
        let result = scan_widgets(src);
        let tf = result.iter().find(|o| o.kind == "TextField").expect("TextField found");
        assert!(!tf.wrapped, "expected unwrapped TextField (no identifier:), got {tf:?}");
    }

    #[test]
    fn scan_widgets_includes_all_interactive_kinds() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        ElevatedButton(onPressed: () {}, child: const Text('a')),
        TextButton(onPressed: () {}, child: const Text('b')),
        IconButton(onPressed: () {}, icon: const Icon(Icons.add)),
        TextField(controller: c),
        GestureDetector(onTap: () {}, child: const SizedBox()),
        InkWell(onTap: () {}, child: const SizedBox()),
        Checkbox(value: true, onChanged: (_) {}),
        Switch(value: false, onChanged: (_) {}),
      ],
    );
  }
}
"#;
        let result = scan_widgets(src);
        let kinds: std::collections::HashSet<_> = result.iter().map(|o| o.kind.as_str()).collect();
        assert!(kinds.contains("ElevatedButton"));
        assert!(kinds.contains("TextButton"));
        assert!(kinds.contains("IconButton"));
        assert!(kinds.contains("TextField"));
        assert!(kinds.contains("GestureDetector"));
        assert!(kinds.contains("InkWell"));
        assert!(kinds.contains("Checkbox"));
        assert!(kinds.contains("Switch"));
    }

    #[test]
    fn scan_widgets_empty_source_returns_empty() {
        let result = scan_widgets("");
        assert!(result.is_empty());
    }

    #[test]
    fn scan_widgets_non_interactive_widgets_ignored() {
        let src = r#"
class Foo extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(child: const Text('hi'));
  }
}
"#;
        let result = scan_widgets(src);
        assert!(result.is_empty(), "got: {result:?}");
    }

    #[test]
    fn scan_widgets_reports_line_numbers() {
        let src = "class Foo extends StatelessWidget {\n  @override\n  Widget build(BuildContext context) {\n    return TextField(controller: c);\n  }\n}\n";
        let result = scan_widgets(src);
        let tf = result.iter().find(|o| o.kind == "TextField").expect("TextField found");
        assert_eq!(tf.line, 4);
    }

    #[test]
    fn interactive_widgets_list_is_exhaustive() {
        // Guard test — if the list shrinks unintentionally, this fails loud.
        assert!(INTERACTIVE_WIDGETS.len() >= 18, "expected >=18 widgets");
    }
}
```

- [ ] **Step 2: Register the submodule**

Edit `src/check_a11y/mod.rs` (add to the existing `pub mod main_check;`):

```rust
pub mod widget_scan;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib check_a11y::widget_scan::tests`
Expected: 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/check_a11y/widget_scan.rs src/check_a11y/mod.rs
git commit -m "feat(check-a11y): scan interactive widgets + Semantics wrap check

Adds scan_widgets() returning Vec<WidgetOccurrence> for every
interactive-widget kind (18 kinds: buttons, TextField, GestureDetector,
InkWell, Checkbox, Switch, Radio, etc.) with a wrapped flag set when
the widget is inside Semantics(identifier:) within 3 ancestor widgets.

Uses the existing DartParser (tree-sitter) and walks ancestors via
parent links rather than mutable state, keeping the scan stateless
per call.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Implement `check_a11y::run` orchestrator

**Files:**
- Modify: `src/check_a11y/mod.rs` (currently has the two `pub mod` lines; add the run() function below them)

- [ ] **Step 1: Replace `src/check_a11y/mod.rs` with:**

```rust
//! `falcon check-a11y` — verify Flutter project is ready for Maestro UI testing.

pub mod main_check;
pub mod widget_scan;

use crate::config::{FalconConfig, Severity};
use crate::preflight::{exit_code_for_issues, reporter, OutputFormat, PreflightIssue};
use anyhow::Result;
use std::path::{Path, PathBuf};

const RULE_ID_MISSING_ENSURE_SEMANTICS: &str = "a11y/missing-ensure-semantics";
const RULE_ID_COMMENTED_ENSURE_SEMANTICS: &str = "a11y/commented-ensure-semantics";
const RULE_ID_LOW_COVERAGE: &str = "a11y/low-interactive-coverage";
const RULE_ID_UNWRAPPED_WIDGET: &str = "a11y/unwrapped-interactive-widget";

/// Run the check. Returns exit code: 0 clean, 1 warnings only, 2 errors.
pub fn run(root: &Path, format: OutputFormat, config: &FalconConfig) -> Result<i32> {
    let mut issues: Vec<PreflightIssue> = Vec::new();

    // Tunable thresholds.
    let threshold = config
        .preflight
        .config
        .get("check-a11y")
        .and_then(|c| c.get("interactive_semantics_coverage"))
        .and_then(|c| c.get("threshold"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.6);

    // 1) ensureSemantics() in main.dart
    let main_path = root.join("lib").join("main.dart");
    if let Ok(source) = std::fs::read_to_string(&main_path) {
        let rel = PathBuf::from("lib/main.dart");
        let result = main_check::check_main_dart(&source);
        match result {
            main_check::MainCheckResult::Present => {}
            main_check::MainCheckResult::Missing
            | main_check::MainCheckResult::NoMainFunction => {
                issues.push(PreflightIssue {
                    rule_id: RULE_ID_MISSING_ENSURE_SEMANTICS.into(),
                    severity: Severity::Error,
                    title: "Missing ensureSemantics() call".into(),
                    file: Some(rel.clone()),
                    line: None,
                    plugin: None,
                    message: "lib/main.dart does not call SemanticsBinding.instance.ensureSemantics(). \
Maestro UI tests on iOS will see an empty accessibility tree."
                        .into(),
                    suggestion: Some(
                        "Add `SemanticsBinding.instance.ensureSemantics();` after \
WidgetsFlutterBinding.ensureInitialized() and before runApp() in main()."
                            .into(),
                    ),
                });
            }
            main_check::MainCheckResult::CommentedOut => {
                issues.push(PreflightIssue {
                    rule_id: RULE_ID_COMMENTED_ENSURE_SEMANTICS.into(),
                    severity: Severity::Error,
                    title: "ensureSemantics() is commented out".into(),
                    file: Some(rel),
                    line: None,
                    plugin: None,
                    message:
                        "lib/main.dart contains a commented-out call to ensureSemantics(); \
Maestro UI tests on iOS will see an empty accessibility tree."
                            .into(),
                    suggestion: Some("Uncomment the SemanticsBinding.instance.ensureSemantics() call.".into()),
                });
            }
        }
    } else {
        // No lib/main.dart at all — emit Info; this is unusual but not a hard error.
        issues.push(PreflightIssue {
            rule_id: RULE_ID_MISSING_ENSURE_SEMANTICS.into(),
            severity: Severity::Info,
            title: "lib/main.dart not found".into(),
            file: Some(PathBuf::from("lib/main.dart")),
            line: None,
            plugin: None,
            message: "Falcon could not find lib/main.dart to verify the ensureSemantics() call.".into(),
            suggestion: None,
        });
    }

    // 2) Interactive widget coverage
    let mut total = 0usize;
    let mut wrapped = 0usize;
    let lib_dir = root.join("lib");
    if lib_dir.is_dir() {
        for entry in walkdir::WalkDir::new(&lib_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let path_str = path.to_string_lossy();
            if !path_str.ends_with(".dart")
                || path_str.ends_with(".g.dart")
                || path_str.ends_with(".freezed.dart")
            {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(path) else { continue };
            let occurrences = widget_scan::scan_widgets(&source);
            for occ in &occurrences {
                total += 1;
                if occ.wrapped {
                    wrapped += 1;
                } else {
                    // One Suggestion per unwrapped widget.
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(path)
                        .to_path_buf();
                    issues.push(PreflightIssue {
                        rule_id: RULE_ID_UNWRAPPED_WIDGET.into(),
                        severity: Severity::Info,
                        title: format!("Interactive widget without Semantics(identifier:)"),
                        file: Some(rel),
                        line: Some(occ.line),
                        plugin: None,
                        message: format!(
                            "{} is not wrapped in Semantics(identifier:) within 3 ancestor widgets.",
                            occ.kind
                        ),
                        suggestion: Some(format!(
                            "Wrap with Semantics(identifier: '<unique-id>', child: ...). \
For {}, the Container-then-Semantics pattern works around Flutter web bug #155323.",
                            occ.kind
                        )),
                    });
                }
            }
        }
    }

    if total > 0 {
        let coverage = wrapped as f64 / total as f64;
        if coverage < threshold {
            issues.push(PreflightIssue {
                rule_id: RULE_ID_LOW_COVERAGE.into(),
                severity: Severity::Warning,
                title: format!(
                    "Interactive widget Semantics coverage {:.0}% < {:.0}% threshold",
                    coverage * 100.0,
                    threshold * 100.0
                ),
                file: None,
                line: None,
                plugin: None,
                message: format!(
                    "{} of {} interactive widgets are wrapped in Semantics(identifier:). \
Maestro tests may fall back to fragile text-based selectors.",
                    wrapped, total
                ),
                suggestion: Some(format!(
                    "Add Semantics(identifier: '<unique-id>') around interactive widgets to \
reach at least {:.0}% coverage.",
                    threshold * 100.0
                )),
            });
        }
    }

    // Apply suppression.
    issues.retain(|issue| !is_suppressed(issue, config));

    let exit = exit_code_for_issues(&issues);
    let rendered = reporter::render(&issues, format);
    print!("{rendered}");
    Ok(exit)
}

fn is_suppressed(issue: &PreflightIssue, config: &FalconConfig) -> bool {
    config
        .preflight
        .suppress
        .iter()
        .any(|s| s.rule_id == issue.rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PreflightConfig, PreflightSuppression};
    use tempfile::TempDir;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn basic_main(with_ensure: bool) -> String {
        let mut src = String::from("void main() {\n");
        if with_ensure {
            src.push_str("  SemanticsBinding.instance.ensureSemantics();\n");
        }
        src.push_str("  runApp(MyApp());\n}\n");
        src
    }

    #[test]
    fn empty_project_no_main_emits_info_only() {
        let tmp = TempDir::new().unwrap();
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        // Info-only → exit 0
        assert_eq!(code, 0);
    }

    #[test]
    fn missing_ensure_semantics_exits_two() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(false));
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn present_ensure_semantics_with_no_widgets_exits_zero() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn unwrapped_widgets_below_threshold_exits_one() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        write(
            &tmp.path().join("lib/page.dart"),
            r#"
class Page extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(children: [
      TextField(controller: a),
      TextField(controller: b),
      TextField(controller: c),
    ]);
  }
}
"#,
        );
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        // ensureSemantics is present (no error). 3 unwrapped TextFields → coverage 0%.
        // 0% < 60% default threshold → Warning (exit 1). Suggestion items are Info (no escalate).
        assert_eq!(code, 1);
    }

    #[test]
    fn wrapped_widgets_above_threshold_exits_zero() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        write(
            &tmp.path().join("lib/page.dart"),
            r#"
class Page extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(children: [
      Semantics(identifier: 'a', child: TextField(controller: a)),
      Semantics(identifier: 'b', child: TextField(controller: b)),
      Semantics(identifier: 'c', child: TextField(controller: c)),
    ]);
  }
}
"#,
        );
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn threshold_configurable_via_falcon_yaml() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        // 50% wrapped (1 wrapped, 1 unwrapped) — below default 60% but above a 40% override.
        write(
            &tmp.path().join("lib/page.dart"),
            r#"
class Page extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(children: [
      Semantics(identifier: 'a', child: TextField(controller: a)),
      TextField(controller: b),
    ]);
  }
}
"#,
        );
        let mut cfg = FalconConfig::default();
        let mut tunings = std::collections::HashMap::new();
        let yaml: serde_yaml::Value =
            serde_yaml::from_str("interactive_semantics_coverage:\n  threshold: 0.4").unwrap();
        tunings.insert("check-a11y".to_string(), yaml);
        cfg.preflight = PreflightConfig { suppress: vec![], config: tunings };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        // 50% >= 40% → no warning. Unwrapped widget still emits Info Suggestion → exit 0.
        assert_eq!(code, 0);
    }

    #[test]
    fn suppression_silences_missing_ensure_semantics() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(false));
        let cfg = FalconConfig {
            preflight: PreflightConfig {
                suppress: vec![PreflightSuppression {
                    rule_id: RULE_ID_MISSING_ENSURE_SEMANTICS.into(),
                    plugin: None,
                    key: None,
                    reason: "App is not Maestro-tested.".into(),
                }],
                config: Default::default(),
            },
            ..FalconConfig::default()
        };
        let code = run(tmp.path(), OutputFormat::Text, &cfg).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn commented_out_ensure_semantics_exits_two() {
        let tmp = TempDir::new().unwrap();
        let src = "void main() {\n  // SemanticsBinding.instance.ensureSemantics();\n  runApp(MyApp());\n}\n";
        write(&tmp.path().join("lib/main.dart"), src);
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 2);
    }

    #[test]
    fn generated_files_skipped() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("lib/main.dart"), &basic_main(true));
        // .g.dart should be skipped — even if it had a million unwrapped widgets.
        write(
            &tmp.path().join("lib/generated.g.dart"),
            "class X { Widget build() => TextField(); }",
        );
        let code = run(tmp.path(), OutputFormat::Text, &FalconConfig::default()).unwrap();
        assert_eq!(code, 0);
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --lib check_a11y::tests`
Expected: 9 tests pass.

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add src/check_a11y/mod.rs
git commit -m "feat(check-a11y): implement run() orchestrator with TempDir tests

Wires ensureSemantics() detection in lib/main.dart, interactive-widget
coverage scan across lib/, suppression via preflight.suppress, and a
configurable threshold via preflight.config.check-a11y.
interactive_semantics_coverage.threshold (default 0.6).

Exit codes: 0 clean, 1 coverage warning, 2 missing/commented
ensureSemantics().

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Wire `Commands::CheckA11y` into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add the command variant**

Inside `enum Commands { ... }`, near `Commands::CheckAssets`:

```rust
    /// Verify the Flutter project is ready for Maestro UI testing (ensureSemantics + Semantics coverage).
    CheckA11y {
        /// Path to the Flutter project.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: PreflightOutputFormat,
    },
```

(Use `PreflightOutputFormat` — the alias established in PR1 to avoid collision with main.rs's existing `OutputFormat`.)

- [ ] **Step 2: Add the dispatch arm**

Near the `Commands::CheckAssets` arm:

```rust
        Commands::CheckA11y { path, format } => {
            let config = falcon::config::FalconConfig::load(&path).unwrap_or_default();
            let code = falcon::check_a11y::run(&path, format, &config)?;
            process::exit(code);
        }
```

- [ ] **Step 3: Build + smoke test**

Run: `cargo build 2>&1 | tail -5` — expect clean.
Run: `cargo run --bin falcon -- check-a11y --help 2>&1 | head -15` — expect help text.

- [ ] **Step 4: Run the full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): wire Commands::CheckA11y

Adds the \`falcon check-a11y [path] [--format text|json|sarif]\`
subcommand that delegates to falcon::check_a11y::run() and exits with
the returned code (0/1/2).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Integration test against the CLI binary

**Files:**
- Create: `tests/preflight_a11y_tests.rs`

- [ ] **Step 1: Create the file:**

```rust
//! End-to-end test for `falcon check-a11y`.

use std::process::Command;
use tempfile::TempDir;

fn falcon_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_falcon"))
}

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn check_a11y_exits_two_on_missing_ensure_semantics() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("lib/main.dart"),
        "void main() {\n  runApp(MyApp());\n}\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-a11y")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ensureSemantics"), "stdout: {stdout}");
}

#[test]
fn check_a11y_exits_zero_when_clean() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("lib/main.dart"),
        "void main() {\n  SemanticsBinding.instance.ensureSemantics();\n  runApp(MyApp());\n}\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-a11y")
        .arg(tmp.path())
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn check_a11y_json_format() {
    let tmp = TempDir::new().unwrap();
    write(
        &tmp.path().join("lib/main.dart"),
        "void main() {\n  runApp(MyApp());\n}\n",
    );

    let output = Command::new(falcon_bin())
        .arg("check-a11y")
        .arg(tmp.path())
        .args(["--format", "json"])
        .output()
        .expect("failed to execute falcon");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["schema_version"], serde_json::json!(1));
    assert!(parsed["issues"].as_array().unwrap().len() >= 1);
    assert_eq!(
        parsed["issues"][0]["rule_id"],
        "a11y/missing-ensure-semantics"
    );
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test preflight_a11y_tests 2>&1 | tail -8`
Expected: 3 tests pass.

- [ ] **Step 3: Full suite**

Run: `cargo test 2>&1 | grep -E "^test result" | awk '{passed += $4; failed += $6} END {print "TOTAL: " passed " passed, " failed " failed"}'`
Expected: 0 failed.

- [ ] **Step 4: Commit**

```bash
git add tests/preflight_a11y_tests.rs
git commit -m "test(check-a11y): integration test against the CLI binary

Three e2e tests: missing-ensureSemantics (exit 2 + substring),
present-ensureSemantics (exit 0), and --format json validity
(schema_version=1, rule_id matches).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Finalize + smoke test

- [ ] **Step 1: Smoke test on `falcon_dart`**

Run: `cargo run --bin falcon -- check-a11y falcon_dart 2>&1 | tail -10`
Expected: either exit 0 or exit 2 with a specific message — both are acceptable smoke results.

- [ ] **Step 2: Working tree clean**

Run: `git status -sb` — expect clean.

- [ ] **Step 3: Print summary**

Echo:

```
PR-2 (check-a11y) complete.
Files added:    src/check_a11y/{mod,main_check,widget_scan}.rs, tests/preflight_a11y_tests.rs
Files modified: src/lib.rs, src/main.rs
Tests added:    8 (main_check) + 8 (widget_scan) + 9 (orchestrator) + 3 (integration) = 28
Next PR:        check-pods per docs/superpowers/specs/2026-05-25-preflight-checks-design.md
```

---

## Self-review

1. **Spec coverage:** ensureSemantics() detection (Task 1), interactive-widget enumeration + Semantics wrap (Task 2), threshold config + suggestion items (Task 3), CLI wiring (Task 4), integration test (Task 5). All v2-spec acceptance criteria #4 hit.
2. **Placeholder scan:** no TBDs.
3. **Type consistency:** `MainCheckResult`, `WidgetOccurrence`, rule_ids referenced consistently. `PreflightOutputFormat` alias matches PR1.
