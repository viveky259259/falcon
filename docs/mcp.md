# Falcon MCP server

`falcon-mcp` is the Model Context Protocol server for Falcon. It speaks
JSON-RPC over stdio so any MCP-aware agent (Claude Code, Cursor, etc.) can
call Falcon as a tool.

The surface is **locked at 20 tools**: nine that analyze code and eleven that
run, watch and drive the app. Older names (`falcon_*`) are still accepted for
one release as aliases. They go to the same handler but do **not** appear in
`tools/list`, and each call logs a `log::warn!` line.

## The 20 canonical tools

### Code

| Name             | What it does                                                                                 | Required args  |
|------------------|----------------------------------------------------------------------------------------------|----------------|
| `lint_file`      | Analyze a single Dart file. Fast, for "after you generated or edited a file" loops.          | `file_path`    |
| `lint_diff`      | Analyze only Dart files changed vs. `base_ref` (default `origin/main`).                      | `path`         |
| `review`         | Full-project analysis: issues with rule, severity, file, line, message.                      | `path`         |
| `explain`        | Explain a rule: rationale, good and bad examples, exceptions.                                 | `rule`         |
| `fix_safe`       | Auto-fix suggestions. Preview by default; `preview:false` writes changes.                    | `path`         |
| `doctor`         | Diagnose the Flutter toolchain; with `execute:true` and `decisions`, install what's missing. | `path`         |
| `score`          | 0-100 AI Code Quality Score across six dimensions, with findings.                            | `path`         |
| `conventions`    | The project's own conventions (state management, naming, structure) to match.                | `path`         |
| `generate_tests` | Golden tests per widget (`kind:golden`) or unit test stubs (`kind:unit`). Preview by default. | `path`, `kind` |

### Running app

| Name          | What it does                                                                                       | Required args      |
|---------------|----------------------------------------------------------------------------------------------------|--------------------|
| `run_app`     | Start the app with `flutter run` and keep it running for the tools below.                          | none               |
| `stop_app`    | Stop that app.                                                                                     | none               |
| `reload`      | Hot reload (or `full_restart`) after an edit; reports compile errors.                              | none               |
| `screenshot`  | PNG of the screen, returned as an image.                                                           | none               |
| `widget_tree` | The screen as text: on-screen text, buttons, fields and controls with positions (or the inspector tree). | none         |
| `interact`    | Tap, long press, type, scroll, go back, or wait for a widget.                                      | `action`           |
| `logs`        | What the app printed, including whole Flutter error blocks. Cursor-based.                          | none               |
| `tree_diff`   | Widgets that appeared or disappeared over a short window.                                          | none               |
| `app_metrics` | `memory`, `network`, `performance`, `profiler`, `rebuilds`, `trace` or `routes`.                   | `kind`             |
| `journey`     | Screenshot every few seconds and report each distinct screen; returns new-screen images.           | none               |
| `verify`      | Run one `integration_test` file on an exact device and keep the evidence.                          | `target`, `device` |

All schemas are JSON-Schema draft-07 and surfaced via the standard MCP
`inputSchema` field on `tools/list`. The canonical argument structs and schema
builders live in `src/mcp/schema.rs`; `src/mcp/tools.rs` only advertises and
dispatches them.

### `lint_file`

`lint_file` accepts `file_path` and an optional `source` string for generated or
edited code that has not been written to disk yet. Pass `project_root` when the
caller wants resolver-backed cross-file rules, such as lifecycle checks that
need inheritance facts from the rest of the project. Without `project_root`,
`lint_file` keeps the original fast single-file behavior.

Disk-backed calls are cached under `${TMPDIR}/falcon-mcp/` and version-scoped by
the Falcon crate version. Cache entries validate the file mtime and SHA-256 of
the current source before reuse, so rewritten files with the same path are
recomputed.

Request:

```json
{
  "file_path": "lib/main.dart",
  "source": "class badName { void run() { print('debug'); } }",
  "project_root": "."
}
```

Response:

```json
{
  "file": "lib/main.dart",
  "issue_count": 1,
  "issues": [
    {
      "rule": "avoid-print",
      "message": "Avoid print statements in production code",
      "severity": "Warning",
      "line": 1,
      "column": 30
    }
  ]
}
```

### `lint_diff`

`lint_diff` shells out to `git diff --name-only --diff-filter=ACMR
<base_ref>...HEAD`, keeps existing `.dart` files, and analyzes only that
scoped file list.

Request:

```json
{
  "path": ".",
  "base_ref": "origin/main"
}
```

Response:

```json
{
  "path": ".",
  "base_ref": "origin/main",
  "changed_file_count": 1,
  "file_count": 1,
  "issue_count": 0,
  "changed_files": ["lib/main.dart"],
  "issues": []
}
```

If no Dart files changed, the tool returns zero counts and an empty issue
list. If the base ref is missing, the git error is returned to the caller.

### `review`

`review` runs full-project Falcon analysis. `preset` is optional and may be one
of `recommended`, `strict`, `flutter`, `riverpod`, `bloc`, `performance`, or
`ai-generated`.

Request:

```json
{
  "path": ".",
  "preset": "ai-generated"
}
```

Response:

```json
{
  "file_count": 12,
  "issue_count": 1,
  "issues": [
    {
      "rule": "avoid-print",
      "message": "Avoid print statements in production code",
      "severity": "Warning",
      "file": "lib/main.dart",
      "line": 8,
      "column": 5
    }
  ]
}
```

### `explain`

`explain` returns Falcon's built-in rule explanation, including examples and
exceptions.

Request:

```json
{
  "rule": "avoid-empty-catch"
}
```

Response:

```json
{
  "rule": "avoid-empty-catch",
  "category": "error-handling",
  "severity": "warning",
  "summary": "Empty catch blocks hide failures.",
  "why": "Silent failures make generated code harder to debug.",
  "bad_example": "try { risky(); } catch (e) {}",
  "good_example": "try { risky(); } catch (e, st) { log(e, st); }",
  "exceptions": "Only suppress intentionally with a comment."
}
```

### `fix_safe`

`fix_safe` generates safe fixes. It previews by default; pass `preview:false`
only when the caller explicitly wants Falcon to apply auto-fixable changes.

Request:

```json
{
  "path": ".",
  "preview": true
}
```

Response:

```json
{
  "preview": true,
  "fix_count": 1,
  "fixes": [
    {
      "rule": "avoid-print",
      "file": "lib/main.dart",
      "line": 8,
      "original": "print('debug');",
      "replacement": "debugPrint('debug');",
      "description": "Use debugPrint for debug logging."
    }
  ]
}
```

## Running-app tools

These tools let an agent prove a change works: start the app, edit, hot reload,
read the screen, use the app and catch runtime errors without anyone helping.

### Which app they target

`run_app` starts `flutter run --machine` and keeps it alive across calls (one
app per server). Every other running-app tool uses that app by default. To use
an app you started yourself, pass `attach` with the VM Service URI that
`flutter run` prints; `path` names the project when it can't be inferred.

```json
{ "path": ".", "device": "emulator-5554" }
```

`run_app` returns once the app is up, or fails with the end of the build
output. Calling it again for the same project returns the running app;
another project is refused until `stop_app`. Set `device` to `flutter-tester`
for a headless run that needs no simulator or emulator. When the MCP server
exits, it stops the app.

Through `run_app`, `reload` is a real hot reload: the flutter tool recompiles
the changed sources first, so the result carries `success`, the tool's
message and `errors_since_reload` (compiler errors included). With `attach`
it falls back to the VM Service, which can't recompile, and says so in `note`.

`run_app` also switches off Flutter's structured errors, which `flutter run
--machine` turns on for IDEs and which would otherwise keep framework
exceptions out of the app's output. `logs` returns that output since `since`;
pass back `next_cursor` to read only new lines. With `errors_only`, each
`══╡ EXCEPTION CAUGHT … ╞══` block comes back whole, stack included.

### Reading and using the screen

`widget_tree` (default `mode: visible`) lists what is on screen: text, buttons,
fields, toggles, list tiles and icons, each with a label, its `ValueKey` if any,
and `rect` as `[left, top, width, height]` in logical pixels. Widgets covered by
another route or a dialog are left out and counted in `covered_count`.

`interact` acts on a widget chosen by `find`, whose fields must all match:
`text`, `text_contains`, `key` (a `ValueKey` value), `type`, `tooltip` or
`label` (Semantics). Actions:

| `action`     | Needs                                    | Does                                                         |
|--------------|------------------------------------------|--------------------------------------------------------------|
| `tap`        | `find`, or `x` and `y`                   | Pointer down and up at the widget's centre.                  |
| `long_press` | `find`                                   | Holds for 700 ms.                                            |
| `type`       | `find`, `text` (optional `submit`)       | Focuses the nearest text field and replaces its text; fires `onChanged`. |
| `scroll`     | `find`, or `dx`/`dy` (optional `within`) | Brings the widget into view, paging through lazy lists to build it; or scrolls the first list that can scroll. |
| `back`       | nothing                                  | Pops the current route, like the system back button.         |
| `wait`       | `find` (optional `gone`, `timeout_ms`)   | Waits for the widget to appear, or disappear.                |

A `find` that matches more than one widget is refused with the candidates
listed; pass `index` to pick one. Nothing is tapped on a guess, and widgets
covered by another screen are never tapped. After each action the result says
what it hit, which labels `appeared` and `disappeared` once the screen settled,
and what is on screen now:

```json
{
  "target": { "widget": { "type": "Tooltip", "label": "Increment", "rect": [728, 528, 56, 56] }, "position": [756, 556] },
  "appeared": ["Text: 1"],
  "disappeared": ["Text: 0"],
  "screen_after": ["…"]
}
```

`interact` and `widget_tree` work by evaluating small Dart expressions inside
the app over the VM Service, so they need a debug build and nothing added to
the app.

### `screenshot`, `journey`, metrics and tests

`screenshot` tries the Flutter rasterizer first and falls back to
`flutter screenshot`; Flutter web isn't supported. The PNG comes back as an
`image` content block (unless `include_image:false`) and is written to `out` or
a new temp file. `journey` returns up to `max_images` screenshots of new
screens the same way.

`app_metrics`, `tree_diff`, `journey` and VM-based `logs` record for a window
of at most 120 seconds. `verify` runs `flutter test` on one `integration_test`
file on the exact `device` given and writes its evidence to a new directory.

Every call has a deadline. Over the HTTP bridge (`falcon x api`) the
running-app tools are not listed, and calls to them are refused.
`generate_tests` previews there but can't write.

## Prompts and resources

`prompts/list` offers two workflows:

- `dev_loop` (argument `goal`, optional `path` and `device`): run the app,
  lint, reload, check logs, look at the screen and use the app until the goal
  is shown to work.
- `review_ai_code` (optional `path`, `base_ref`): lint the diff, explain and
  fix findings, and report the score.

`resources/list` offers `falcon://rules` (every rule with category, severity
and summary) and `falcon://session` (the app started by `run_app`). The
template `falcon://rules/{name}` returns one rule in full.

## Protocol

The server answers `initialize` with the client's protocol version when it
supports it (`2025-06-18`, `2025-03-26` or `2024-11-05`), otherwise the
newest. When a `tools/call` request carries `_meta.progressToken`, long tools
(`run_app`, `journey`, `verify`) send `notifications/progress`.

## Deprecation table

These names continue to work for one release. They are **not advertised** in
`tools/list` and will be removed in a future version.

| Old name               | New canonical name | Status                                                       |
|------------------------|--------------------|--------------------------------------------------------------|
| `falcon_check_file`    | `lint_file`        | Alias. Logs deprecation warning.                             |
| `falcon_analyze`       | `review`           | Alias. Logs deprecation warning.                             |
| `falcon_explain_rule`  | `explain`          | Alias. Logs deprecation warning.                             |
| `falcon_fix`           | `fix_safe`         | Alias. Logs deprecation warning.                             |
| `falcon_ai_score`      | `score`            | Alias. Logs deprecation warning.                             |
| `falcon_conventions`   | `conventions`      | Alias. Logs deprecation warning.                             |
| `falcon_provenance`    | _none_             | Reachable by old name only; no canonical replacement.        |

## Client setup

### Claude Code (`.mcp.json`)

Drop this at the root of your repo (or merge into an existing `.mcp.json`):

```json
{
  "mcpServers": {
    "falcon": {
      "command": "falcon-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

Then in Claude Code, `falcon` will appear as an MCP server and the 5 tools
above will be available as `mcp__falcon__lint_file`, `mcp__falcon__review`,
etc.

### Cursor (`.mcp.json`)

Cursor uses the same `.mcp.json` shape. Place it at the project root:

```json
{
  "mcpServers": {
    "falcon": {
      "command": "falcon-mcp",
      "args": []
    }
  }
}
```

If `falcon-mcp` is not on PATH, point `command` at the absolute binary path
(e.g. `/usr/local/bin/falcon-mcp` or `target/release/falcon-mcp`).

### Agent-specific install docs

- [Cursor](install/cursor.md)
- [Claude Code](install/claude-code.md)
- [Cline](install/cline.md)

## Notes

- Transport is **stdio only**. There is no HTTP/WebSocket variant.
- The server name reported during `initialize` is `falcon`; the version
  matches the crate version.
- `cargo test --test mcp_tool_surface` exercises the locked 20-tool surface
  and deprecated aliases.
- `cargo test --test mcp_cache` exercises disk-backed `lint_file` cache reuse
  and invalidation.
- `cargo build --release --bin falcon-mcp && cargo bench --bench mcp_warm_cache`
  verifies the warm-cache MCP roundtrip budget on a 200-file fixture.
- Local MCP telemetry is opt-in and documented in [Telemetry](telemetry.md).
