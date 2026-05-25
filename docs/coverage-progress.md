# Coverage Progress Ledger

Append-only record of the coverage-uplift initiative.

Spec: `docs/superpowers/specs/2026-05-25-coverage-uplift-design.md`
Target: ≥90% line coverage (library crate)

## Baseline

- Date: 2026-05-25
- Tool: cargo-llvm-cov 0.8.7
- Overall line coverage: 53.73%
- Total library files counted: 188
- Bottom 20 modules (work queue, lowest line-coverage first):

| Module                                       | Lines | Lines % |
|----------------------------------------------|-------|---------|
| src/main.rs                                  | 1928  | 0.00% (EXCLUDED — CLI dispatch) |
| src/dashboard/compare_reports.rs             | 625   | 0.00%   |
| src/reporters/html.rs                        | 871   | 0.00%   |
| src/self_update.rs                           | 343   | 0.00%   |
| src/flutter_run/mod.rs                       | 281   | 0.00%   |
| src/reporters/console.rs                     | 146   | 0.00%   |
| src/mcp/server.rs                            | 125   | 0.00% (EXCLUDED — server loop) |
| src/runtime/report/console.rs                | 110   | 0.00%   |
| src/reporters/json.rs                        | 87    | 0.00%   |
| src/lsp/server.rs                            | 59    | 0.00% (EXCLUDED — server loop) |
| src/incremental/watcher.rs                   | 51    | 0.00%   |
| src/resolver/scope.rs                        | 36    | 0.00%   |
| src/parser/visitor.rs                        | 35    | 0.00%   |
| src/mcp_main.rs                              | 7     | 0.00% (EXCLUDED — bin entry) |
| src/lsp_main.rs                              | 8     | 0.00% (EXCLUDED — bin entry) |
| src/review/pr_review.rs                      | 316   | 2.85%   |
| src/dashboard/server.rs                      | 97    | 5.15%   |
| src/rules/flutter/accessibility.rs           | 137   | 6.57%   |
| src/lsp/ai_extensions.rs                     | 60    | 11.67%  |
| src/runtime/connection.rs                    | 611   | 13.58%  |

Next module to work on (lowest-covered, not excluded, smallest): ~~visitor~~ ~~scope~~ ~~watcher~~ ~~reporters/json~~ ~~runtime/report/console~~ done — next: **`src/reporters/console.rs`** (146 lines, 0.00%).

## Completed Modules

| Date       | Module                             | Before  | After    | Commit  |
|------------|------------------------------------|---------|----------|---------|
| 2026-05-25 | src/parser/visitor.rs              | 0.00%   | 91.18%   | b39470b |
| 2026-05-25 | src/resolver/scope.rs              | 0.00%   | 100.00%  | 9c73ba0 |
| 2026-05-25 | src/incremental/watcher.rs         | 0.00%   | 68.11%*  | 313dcd7 |
| 2026-05-25 | src/reporters/json.rs              | 0.00%   | 94.38%   | 363789f |
| 2026-05-25 | src/runtime/report/console.rs      | 0.00%   | 98.04%   | 1089897 |
| 2026-05-25 | src/reporters/console.rs           | 0.00%   | 97.88%   | 03a1b13 |
| 2026-05-25 | src/flutter_run/mod.rs             | 0.00%   | 77.43%*  | 0afdb76 |
| 2026-05-25 | src/dashboard/compare_reports.rs   | 0.00%   | 60.85%*  | becc5bf |
| 2026-05-25 | src/reporters/html.rs              | 0.00%   | 95.50%   | 4e93f14 |
| 2026-05-25 | src/self_update.rs                 | 0.00%   | 44.65%*  | fff9e1f |
| 2026-05-25 | src/runtime/tools.rs               | 40.03%  | 71.54%   | 2ff21e1 |
| 2026-05-25 | src/runtime/connection.rs          | 13.58%  | 39.89%*  | da6b019 |
| 2026-05-25 | src/review/pr_review.rs            | 2.85%   | 78.62%   | 93ecea6 |
| 2026-05-25 | src/deeplink/mod.rs                | 46.76%  | 91.36%   | 0e75644 |
| 2026-05-25 | src/l10n_coverage/mod.rs           | 65.24%  | 89.02%   | 2281be0 |
| 2026-05-25 | src/runtime/live.rs                | 53.33%  | 86.86%   | 363bd82 |
| 2026-05-25 | src/animation_audit/mod.rs         | 68.72%  | 95.83%   | 67903c6 |
| 2026-05-25 | src/theme_audit/mod.rs             | 61.84%  | 92.36%   | fb46960 |
| 2026-05-25 | src/asset_audit/mod.rs             | 68.46%  | 93.72%   | d1dfed0 |
| 2026-05-25 | src/ai_score/drift.rs              | 46.20%  | 88.63%   | 340a3e5 |
| 2026-05-25 | src/platform/enterprise.rs         | 33.61%  | 94.69%   | 8a23521 |
| 2026-05-25 | src/platform/cloud.rs              | 27.86%  | 85.85%   | 2d513f2 |
| 2026-05-25 | src/api/server.rs                  | 25.91%  | 88.25%   | 0b1c2d3 |
| 2026-05-25 | src/analysis/vuln_radar.rs         | 47.31%  | 89.31%   | 82fee39 |
| 2026-05-25 | src/ai/fix.rs                      | 21.39%  | 91.12%   | 1d10faa |
| 2026-05-25 | src/rules/flutter/accessibility.rs | 6.57%   | 96.70%   | 57e5e94 |
| 2026-05-25 | src/analysis/platform_channels.rs  | 30.93%  | ~86%*    | b58b5e2 |
| 2026-05-25 | src/runtime/report/mod.rs          | 69.15%  | ~90%*    | 94927f5 |
| 2026-05-25 | src/ai/confidence.rs               | 54.08%  | 88.34%   | 9f1e363 |
| 2026-05-25 | src/review/codebase_intel.rs       | 67.95%  | 91.09%   | 3d00761 |
| 2026-05-25 | src/manage/architect.rs            | 52.34%  | 91.84%   | 28f8529 |
| 2026-05-25 | src/analysis/devtools_bridge.rs    | 49.25%  | 88.22%   | f9a4a0d |
| 2026-05-25 | src/mcp/tools.rs                   | 71.16%  | 97.21%   | 124d949 |
| 2026-05-25 | src/dashboard/server.rs            | 5.15%   | 92.00%   | 0d51fdd |
| 2026-05-25 | src/ci/webhook.rs                  | 24.56%  | ~85%*    | 9903c35 |
| 2026-05-25 | src/ai_score/regression_predict.rs | 50.00%  | ~85%*    | da04b62 |

\* `snapshot_times()` is fully covered. `pub fn watch()` body is wrapped in `LCOV_EXCL_START/STOP` markers (infinite poll loop, untestable as-is). However, `cargo llvm-cov --summary-only` does NOT honor LCOV_EXCL markers; the markers are accurate for downstream LCOV tools only. The 68.11% summary figure underrepresents the module's *testable* coverage.

## Deferred Modules

| Module | Reason |
|--------|--------|
| `pub fn watch` in `src/incremental/watcher.rs` | Infinite poll loop with no exit signal. Genuinely untestable without nightly Rust `#[coverage(off)]` attribute or a refactor that introduces a cancellation token. Both are out of the spec's refactor budget. Revisit if/when a CI gate is added that filters with `lcov`/`genhtml`. |
| `run_flutter_app` / `send_os_notification` / `send_webhook` in `src/flutter_run/mod.rs` | Each spawns an external subprocess (`flutter`, `osascript`/`notify-send`/`powershell`, `curl`). Wrapped in `LCOV_EXCL_START/STOP` markers. Full coverage would require injecting a process-runner trait (out of refactor budget) or running against a live Flutter SDK / OS notifier (not viable in CI). |
