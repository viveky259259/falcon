# Runtime Observability & App-Understanding Roadmap

Research + implementation plan for a set of developer-experience features that
help engineers **see, record, and understand** a running Flutter app — screenshots,
user-journey capture, architecture maps, and more.

## Why

Falcon already ships deep *static* analysis (rules, smells, metrics, AI score) and
a solid *runtime* layer (`falcon devtools …` over the Dart VM Service: memory,
network, performance, profiler, debugger, logging, rebuilds, inspector, hot
reload/restart; plus `run`, `runtime-check`, `live`, `dashboard serve`).

What's missing is the **visual / experiential** dimension — the things a developer
or an AI agent needs to *understand what the app actually does at runtime*:

- a picture of the current screen (screenshot),
- a recording of a flow through the app (user journey),
- a map of how the codebase is organized (architecture map).

These all build on the existing `VmServiceClient` (`src/runtime/connection.rs`) and
the `collect_* / print_*` tool pattern in `src/runtime/tools.rs`. No new heavy
dependencies are required — `base64` (already present) covers PNG decode.

## Architecture fit (the repeatable pattern)

Each runtime feature is four small, well-isolated edits:

1. **RPC** — add a method to `VmServiceClient` in `src/runtime/connection.rs`.
2. **Tool** — add `collect_<feature>` + report struct + `print_<feature>` in `src/runtime/tools.rs`.
3. **CLI** — add a `DevtoolsAction` (or top-level `Commands`) variant in `src/cli_args.rs`.
4. **Dispatch** — add the arm in `handle_devtools` (or a new handler) in `src/main.rs`.

Tests live alongside the code in `#[cfg(test)] mod tests` and exercise the pure
summarize/parse helpers (the VM Service I/O is not unit-tested, matching the
existing convention).

## Feature backlog (implemented one-by-one via `/loop`)

### 1. `falcon devtools screenshot`  — STATUS: done
Capture a PNG of the running app via the engine RPC `_flutter.screenshot`
(returns base64 PNG). Save to `--out` (default `falcon-screenshot.png`).
`--json` prints metadata (uri, isolate, bytes, path). Graceful message if the
engine doesn't support the RPC (e.g. headless test mode).
- Foundation for the journey recorder.

### 2. `falcon journey`  — STATUS: done
Record a **user journey**: poll at an interval while the developer drives the app,
capturing on each tick a screenshot + the current top-of-tree route/screen name
(from `get_root_widget_tree`) + memory/frame stats. Detect *screen changes* by
diffing the route widget name (and screenshot byte length as a cheap fallback),
emit a timeline of distinct screens, and write an HTML report with embedded
thumbnails + a per-screen metrics strip. Output dir holds the PNGs + `journey.html`.

### 3. `falcon arch-map`  — STATUS: done
Generate a **visual architecture map** (static): combine the existing dependency
graph, layer classification, and feature-folder structure into a single Mermaid
diagram + HTML page — layers as subgraphs, feature modules as clusters, edges as
cross-module imports, with hotspot/god-file annotations. Builds on
`analysis/layer_enforcement.rs`, `dep-graph`, and `codebase-intel`.

### 4. `falcon devtools route-log`  — STATUS: planned
Stream `Extension` events filtered to navigation (`ext.flutter.navigation` /
`Flutter.Navigation`) and print a chronological route push/pop log — the
text-only sibling of `journey`, useful in CI.

### 5. `falcon trace`  — STATUS: planned
Correlate a tap/interaction window with the frames it produced: record timeline +
rebuilds for N seconds and attribute jank to the widgets that rebuilt, producing
an "interaction → frames → hot widgets" report.

### 6. `falcon devtools tree-diff`  — STATUS: planned
Snapshot the widget tree twice (before/after a hot reload or an interaction) and
print the structural diff — added/removed/moved subtrees.

> Items 4–6 are "and many more" — researched and queued; scope/ordering may
> adjust as the earlier features land and inform the design.

## Conventions to honor
- `anyhow::Result<T>` at boundaries; no panics in library code.
- Files < 500 lines, functions < 80 lines where practical.
- `serde_json` for `--json`, `colored` for console, HTML reports under `src/runtime/report/`.
- `cargo fmt` + `cargo clippy` clean; add unit tests for pure helpers.
