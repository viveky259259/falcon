# Falcon Coverage Uplift — Session 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up coverage tooling, capture the baseline, and complete the first module's tests so that subsequent sessions can repeat the per-module loop deterministically.

**Architecture:** Install `cargo-llvm-cov`. Add `scripts/coverage.sh` wrapper. Create `docs/coverage-progress.md` ledger. Run baseline. Identify the lowest-covered library module. Add `#[cfg(test)] mod tests` colocated tests until that module hits ≥90% line coverage. Commit per module.

**Tech Stack:** Rust 2021 (stable), `cargo-llvm-cov`, `llvm-tools-preview` rustup component, existing test framework (`#[test]`, `tempfile`, `serde_json`).

**Spec:** `docs/superpowers/specs/2026-05-25-coverage-uplift-design.md` — authoritative on scope, exclusions, refactor budget, exit criteria.

---

## Phase A — Tooling Setup

### Task 1: Install cargo-llvm-cov and llvm-tools-preview

**Files:**
- None (toolchain only)

- [ ] **Step 1: Install the llvm-tools rustup component**

Run: `rustup component add llvm-tools-preview`
Expected: `info: component 'llvm-tools-preview' for target '...' is up to date` (or successful install).

- [ ] **Step 2: Install cargo-llvm-cov**

Run: `cargo install cargo-llvm-cov --locked`
Expected: `Installed package 'cargo-llvm-cov vX.Y.Z'` or `already exists`.

- [ ] **Step 3: Smoke-test the tool**

Run: `cargo llvm-cov --version`
Expected: prints a version like `cargo-llvm-cov 0.6.x`.

- [ ] **Step 4: No commit yet (toolchain install only — nothing tracked).**

---

### Task 2: Add the coverage wrapper script

**Files:**
- Create: `scripts/coverage.sh`

- [ ] **Step 1: Write `scripts/coverage.sh`**

Create the file with these contents exactly:

```bash
#!/usr/bin/env bash
# Coverage helper for Falcon.
# Wraps cargo-llvm-cov with the invocations used by the coverage-uplift initiative.
# See docs/superpowers/specs/2026-05-25-coverage-uplift-design.md.

set -euo pipefail

CMD="${1:-help}"
shift || true

case "$CMD" in
  summary)
    # Per-module table from cargo-llvm-cov. To get a sorted view, run:
    #   scripts/coverage.sh summary | sort -k2 -n   # adjust column index as needed
    cargo llvm-cov --workspace --summary-only "$@"
    ;;
  html)
    cargo llvm-cov --workspace --html --output-dir target/llvm-cov/html "$@"
    echo "Report: target/llvm-cov/html/index.html"
    ;;
  lcov)
    cargo llvm-cov --workspace --lcov --output-path target/llvm-cov/lcov.info "$@"
    echo "LCOV: target/llvm-cov/lcov.info"
    ;;
  ci)
    cargo llvm-cov --workspace --fail-under-lines 90 "$@"
    ;;
  module)
    if [ -z "${1:-}" ]; then
      echo "usage: $0 module <path/to/file.rs>" >&2
      exit 2
    fi
    cargo llvm-cov --workspace --summary-only "$@" | grep -E "^($1|Filename|---)"
    ;;
  help|*)
    cat <<'USAGE'
Usage: scripts/coverage.sh <command> [extra cargo llvm-cov args]

Commands:
  summary           Per-module table, sorted ascending by line coverage.
  html              Generate HTML report at target/llvm-cov/html/index.html.
  lcov              Emit LCOV file at target/llvm-cov/lcov.info.
  ci                Run with --fail-under-lines 90 (used in CI once stable).
  module <path.rs>  Show coverage for a single file.
  help              Show this message.
USAGE
    ;;
esac
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x scripts/coverage.sh`
Expected: no output.

- [ ] **Step 3: Sanity-check the help output**

Run: `scripts/coverage.sh help`
Expected: prints the usage block above.

- [ ] **Step 4: Commit**

```bash
git add scripts/coverage.sh
git commit -m "chore(coverage): add scripts/coverage.sh wrapper for cargo-llvm-cov"
```

---

### Task 3: Create the coverage ledger

**Files:**
- Create: `docs/coverage-progress.md`

- [ ] **Step 1: Write the ledger skeleton**

Create `docs/coverage-progress.md` with these contents exactly:

```markdown
# Coverage Progress Ledger

Append-only record of the coverage-uplift initiative.

Spec: `docs/superpowers/specs/2026-05-25-coverage-uplift-design.md`
Target: ≥90% line coverage (library crate)

## Baseline

_To be filled in by Task 4._

## Completed Modules

| Date       | Module | Before | After | Commit |
|------------|--------|--------|-------|--------|

## Deferred Modules

_Module → reason → blocking issue._
```

- [ ] **Step 2: Commit**

```bash
git add docs/coverage-progress.md
git commit -m "docs(coverage): add coverage-progress ledger skeleton"
```

---

## Phase B — Baseline Measurement

### Task 4: Capture the baseline coverage report

**Files:**
- Modify: `docs/coverage-progress.md` (fill in the `## Baseline` section)

- [ ] **Step 1: Run the full baseline**

Run: `scripts/coverage.sh summary | tee target/llvm-cov/baseline.txt`
(create `target/llvm-cov/` if missing: `mkdir -p target/llvm-cov`)
Expected: a sorted table with one row per `.rs` file plus an overall total at the bottom.

- [ ] **Step 2: Capture the overall line-coverage %**

From the last row of the table, note the `Lines %` column. Record it.

- [ ] **Step 3: Capture the bottom 20 modules**

The `cargo llvm-cov --summary-only` output uses fixed-width columns. Identify the column index for "Lines %" in the header (it varies by version — usually the last numeric column). Then:

```bash
# Sort ascending by the Lines % column. Replace <COL> with the column number.
scripts/coverage.sh summary | grep -E '\.rs\s' | sort -k<COL> -n | head -20
```

If the column index is unclear, fall back to opening `target/llvm-cov/html/index.html` and reading the table — it's sortable by clicking the header. Note the 20 lowest-covered library files (skip anything under the spec's hard-exclusion list: `src/main.rs`, server entry points, generated code).
Note: these are the work queue for subsequent sessions.

- [ ] **Step 4: Fill in `## Baseline` in `docs/coverage-progress.md`**

Replace `_To be filled in by Task 4._` with:

```markdown
- Date: 2026-05-25
- Tool: cargo-llvm-cov vX.Y.Z (output of `cargo llvm-cov --version`)
- Overall line coverage: <%>
- Total library files counted: <N>
- Bottom 20 modules (work queue):

| Module | Lines | Covered | % |
|--------|-------|---------|---|
| <path/to/file.rs> | <N> | <N> | <%> |
| ...                                  |
```

Use the actual numbers from Step 1's output.

- [ ] **Step 5: Generate the HTML report for drill-down**

Run: `scripts/coverage.sh html`
Expected: `Report: target/llvm-cov/html/index.html`. Open it to confirm it renders.

- [ ] **Step 6: Commit the filled ledger**

```bash
git add docs/coverage-progress.md
git commit -m "docs(coverage): record baseline line coverage and work queue"
```

---

## Phase C — First Module

> The lowest-covered library module is unknown until Task 4 runs. Tasks 5–8 are **parameterized**: substitute `<MODULE_PATH>` (e.g. `src/rules/common/dead_code.rs`) and `<MODULE_NAME>` (e.g. `dead_code`) with the actual values picked from the baseline.
>
> **Module selection rule:** Pick the lowest-covered file from the baseline's bottom-20 list that **is not** on the spec's hard-exclusion list (`src/main.rs`, server entry points, generated code). If the lowest is excluded, skip and take the next.

### Task 5: Read the target module and inventory testable units

**Files:**
- Read-only: `<MODULE_PATH>` and any sibling files it depends on.

- [ ] **Step 1: Read the source**

Run: `cat <MODULE_PATH>` (or use the IDE / Read tool).
Expected: you can list every public and `pub(crate)` function in the file.

- [ ] **Step 2: List existing tests for this module**

Run: `grep -rn "<MODULE_NAME>" tests/ src/ --include='*.rs' | grep -E '#\[test\]|mod tests' | head -50`
Note any tests already covering parts of this module.

- [ ] **Step 3: Drill into uncovered lines in the HTML report**

Open `target/llvm-cov/html/index.html`, navigate to `<MODULE_PATH>`, and list the line ranges shown as red (uncovered).

- [ ] **Step 4: Write a one-paragraph inventory in a scratch file**

Create or overwrite `target/llvm-cov/notes-<MODULE_NAME>.txt` with:

```
Module: <MODULE_PATH>
Public/pub(crate) functions: <list with arity>
Uncovered line ranges: <list>
Testability concerns: <pure functions / IO / global state / etc.>
Refactor needed (must stay within budget per spec): <none | extract pure helper | inject trait>
```

No commit — this is a working note.

---

### Task 6: Add the first failing test

**Files:**
- Modify: `<MODULE_PATH>` (add a `#[cfg(test)] mod tests` block at the bottom if one does not exist; otherwise append a test inside the existing block).

- [ ] **Step 1: Pick the easiest uncovered function**

From the inventory in Task 5, pick the function with the smallest signature and clearest single behavior (often a pure helper or a parser of a small struct).

- [ ] **Step 2: Write a failing test for it**

Append to `<MODULE_PATH>` (skeleton — replace `<FUNC>`, `<input>`, `<expected>` with the real values, and adapt the imports if `mod tests` already exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn <FUNC>_<scenario>_<expectation>() {
        let result = <FUNC>(<input>);
        assert_eq!(result, <expected>);
    }
}
```

If the function returns `Result<T, E>`, follow the `_ok` / `_err` naming convention:

```rust
#[test]
fn <FUNC>_ok_for_valid_input() {
    let result = <FUNC>(<valid_input>).expect("should be Ok");
    assert_eq!(result, <expected>);
}

#[test]
fn <FUNC>_err_for_invalid_input() {
    let err = <FUNC>(<invalid_input>).unwrap_err();
    assert!(matches!(err, <ErrVariant>(_)));
}
```

- [ ] **Step 3: Run the test and watch it fail (or pass for the wrong reason)**

Run: `cargo test --lib <FUNC>_ -- --exact --nocapture` (substitute the test name).
Expected: either FAIL (good — meaningful test) or PASS (suspect — the test may be tautological; rewrite it to actually assert behavior).

If the test passes on first run because the function already works as you expected, that's still a valid coverage test. Move on. Only rewrite if the test would pass regardless of the function's correctness.

- [ ] **Step 4: No commit yet — bundle commits at end of module.**

---

### Task 7: Cover the remaining uncovered branches

**Files:**
- Modify: `<MODULE_PATH>` (more tests inside `mod tests`).

- [ ] **Step 1: Open the HTML report for this module after Task 6**

Run: `scripts/coverage.sh html` and reopen the module's page.
Expected: lines covered by Task 6's test go from red → green.

- [ ] **Step 2: Add one test per remaining uncovered branch**

For each red line group, add a test that exercises that branch. Reuse the naming pattern from Task 6. Examples:

```rust
#[test]
fn <FUNC>_empty_input_returns_default() { ... }

#[test]
fn <FUNC>_with_unicode_handles_correctly() { ... }

#[test]
fn <FUNC>_with_nested_value_recurses() { ... }
```

Filesystem fixtures use `tempfile::TempDir`:

```rust
#[test]
fn loader_reads_file_from_disk() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("input.dart");
    std::fs::write(&path, "void main() {}").unwrap();
    let result = loader::load(&path).unwrap();
    assert_eq!(result.kind(), FileKind::Dart);
}
```

JSON fixtures use `serde_json::json!`:

```rust
#[test]
fn parse_config_accepts_minimal_object() {
    let value = serde_json::json!({"name": "foo"});
    let cfg: Config = serde_json::from_value(value).unwrap();
    assert_eq!(cfg.name, "foo");
}
```

- [ ] **Step 3: Run the module's tests**

Run: `cargo test --lib <MODULE_NAME>:: -- --nocapture`
Expected: all green.

- [ ] **Step 4: Run the full test suite**

Run: `cargo test`
Expected: all green, no regressions in the other 286 tests.

- [ ] **Step 5: Re-measure module coverage**

Run: `scripts/coverage.sh module <MODULE_PATH>`
Expected: line coverage ≥ 90% for `<MODULE_PATH>`.

If under 90%:
- Re-open the HTML report and identify remaining red lines.
- Add tests for them and repeat Step 3.
- If a line cannot be reached without exceeding the refactor budget (per spec), add a `// LCOV_EXCL_LINE` comment with a one-line reason and proceed.

---

### Task 8: Commit and update the ledger

**Files:**
- Modify: `docs/coverage-progress.md` (append a row).
- Implicit: `<MODULE_PATH>` (already modified in Tasks 6–7).

- [ ] **Step 1: Confirm `cargo fmt` and `cargo clippy` are clean**

Run: `cargo fmt --check && cargo clippy --all-targets --workspace -- -D warnings`
Expected: exit 0. If clippy complains about test code, fix it (don't `#[allow]` unless the spec or an existing module already does so).

- [ ] **Step 2: Stage and commit the module's tests**

```bash
git add <MODULE_PATH>
git commit -m "test(<MODULE_NAME>): cover <MODULE_NAME> to <after>% (was <before>%)"
```

Use the real before/after numbers from Task 4's baseline and Task 7 Step 5.

- [ ] **Step 3: Append the ledger row**

Open `docs/coverage-progress.md` and add one row under `## Completed Modules`:

```markdown
| 2026-05-25 | <MODULE_PATH> | <before>% | <after>% | <commit-sha> |
```

Use `git rev-parse --short HEAD` for the commit SHA from Step 2.

- [ ] **Step 4: Commit the ledger update**

```bash
git add docs/coverage-progress.md
git commit -m "docs(coverage): record <MODULE_NAME> uplift in ledger"
```

- [ ] **Step 5: Re-run the overall coverage to confirm a positive delta**

Run: `scripts/coverage.sh summary | tail -3`
Expected: overall line % ≥ baseline % captured in Task 4.

---

## Phase D — Session Wrap-Up

### Task 9: Confirm session exit state

- [ ] **Step 1: All tests green**

Run: `cargo test`
Expected: all pass (existing 286 + new tests).

- [ ] **Step 2: Working tree clean**

Run: `git status -sb`
Expected: clean tree, no untracked files outside `target/`.

- [ ] **Step 3: Print summary for the user**

Echo to chat:

```
Session 1 complete.
Baseline: <baseline>%
After first module: <after>%
Modules completed: 1 (<MODULE_PATH>)
Next session: repeat Tasks 5–8 with the next lowest-covered module from the ledger's work queue.
```

- [ ] **Step 4: No additional commit (Tasks 1–8 already committed individually).**

---

## Subsequent Sessions

Session 2+ repeat **Tasks 5–8** with the next lowest-covered module from the baseline work queue, refreshed by re-running `scripts/coverage.sh summary` at the start of each session.

When the overall library line coverage reaches ≥90% (per `cargo llvm-cov --fail-under-lines 90`):

1. Add `.github/workflows/coverage.yml` running `scripts/coverage.sh ci` on every PR.
2. Mark the initiative complete in `docs/coverage-progress.md` with a final summary row.
3. Update the spec's status from "Draft, awaiting approval" → "Completed".

These follow-up tasks will be planned in a short closing spec at that time — not now.
