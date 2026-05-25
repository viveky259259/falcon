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

Next module to work on (lowest-covered, not excluded, smallest): ~~`src/parser/visitor.rs`~~ done — next: **`src/resolver/scope.rs`** (36 lines, 0.00%).

## Completed Modules

| Date       | Module                | Before  | After   | Commit  |
|------------|-----------------------|---------|---------|---------|
| 2026-05-25 | src/parser/visitor.rs | 0.00%   | 91.18%  | b39470b |

## Deferred Modules

_Module → reason → blocking issue._
