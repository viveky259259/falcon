# Phase 2: PLAN — Engineering Review Output

## Architecture Decision
Extend existing snapshot infrastructure. Three changes:

1. **Auto-save**: After every `falcon analyze`, auto-capture and save a snapshot
2. **CLI `compare-reports`**: New subcommand that loads two snapshots and prints a delta  
3. **HTML comparison**: Generate an HTML report showing side-by-side comparison with deltas

## Implementation Plan

### Files to Change (in order)
1. `src/main.rs` — Add auto-save after analyze, add `CompareReports` subcommand
2. `src/dashboard/mod.rs` — Add `pub mod compare_reports;`
3. `src/dashboard/compare_reports.rs` — NEW: comparison logic + HTML generation

### Data Flow
```
falcon analyze → report → auto-save snapshot → .falcon-data/history.json
falcon compare-reports → load history → pick run1 + run2 → compute deltas → print + HTML
```

### Risk Assessment
- Low risk: builds on existing proven snapshot infrastructure
- No breaking changes to existing commands
