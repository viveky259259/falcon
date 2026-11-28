# Falcon bugs found in end-to-end validation

Found on 2026-11-21 by running Falcon against a real Flutter app on macOS.

**Setup:** Flutter 3.47.6, macOS desktop target, debug mode, Impeller (Metal). The test app was made with `flutter create` and has planted bugs: hardcoded secrets, undisposed controllers, an empty catch, an unawaited future, `setState` after `await`, `print`, `http://` URLs and a RenderFlex overflow. An `AUTOPILOT` flag opens the profile screen automatically so the runtime commands have activity to see. I tested about 90 static command forms, 18 runtime commands (idle and active), `x run`, `verify`, all 7 MCP tools, the HTTP API, `watch` and the webhook.

## Fix status (2026-11-21)

All 14 bugs are fixed in the working tree (uncommitted). Each was re-checked on the iOS simulator and, where it applies, on macOS. `cargo test` (2806 tests) and `cargo clippy --all-targets` pass.

| # | Fix | Verified |
|---|---|---|
| 1 | Unwrap `result.result` (already fixed in the working tree); the selected widget is now unwrapped too | iOS |
| 2 | Frame stats come from `Flutter.Frame` events (new `src/runtime/vm_events.rs`); DDS's replayed history is dropped on subscribe | iOS: `trace` 360 frames/6s, `runtime-check` 312 frames/6s; macOS: 241 frames/4s |
| 3 | Reload/restart call the `sN.reloadSources` / `sN.hotRestart` services that `flutter run` registers (found on the Service stream), with a timeout; restart without them says why | iOS and macOS: `success: true, via: flutter run` |
| 4 | AppleScript coerces each value to text; the parser also tolerates the old list output | macOS: 800×632 window PNG |
| 5 | New `inspector` path (`ext.flutter.inspector.screenshot`) between the engine and `flutter screenshot` | macOS and iOS: `method: inspector` |
| 6 | Measure from the signature to the end of its `function_body` | repro and unit tests |
| 7 | Only `build` methods, and only `setState` outside closures and local functions | test app and unit tests |
| 8 | Strip strings/comments; bare names match whole identifiers; TextTheme names only as `.member` | test app and unit tests |
| 9 | `explain` falls back to the rule registry for every registered rule | unit test over every rule |
| 10 | "Another exception was thrown" lines become their own error files; a new header closes an open block | iOS: 3 error files instead of 1 |
| 11 | Rebuilds turn on `trackRebuildDirtyWidgets` and count `Flutter.RebuiltWidgets` events, named via `widgetLocationIdMap`; `x devtools rebuilds` gains `--duration` | iOS and macOS: real counts, `stats_enabled: true` |
| 12 | `getSelectedWidget` gets `objectGroup`; the group is disposed afterwards | no framework assertion in the app log |
| 13 | Expressions return `BUSY` mid-frame and are retried; taps, typing, scrolling and back run from `Timer.run`; hit tests try 5 points and tap one that reaches the widget | iOS: `type` 7/7 across runs, no assertions |
| 14 | MCP `reload` returns an error when the reload fails | iOS |

Also fixed: requests that end without a status (connection refused) count as failed, and `verify --target app_test.dart` resolves to `integration_test/`.

Correction: bug 10's example said "two `SocketException`s". That was the macOS run; on iOS the same lines were HTTP 404 errors for the avatar image.

Fixed on 2026-11-24 (branch `fix/loop-readiness`):
- The `inspector` screenshot is cut to the screen; overflowing content no longer appears below the visible area. Checked on the iOS simulator with a column overflowing by 2312 px.
- MCP `interact` with `tap` (or `long_press`) and `index` alone acts on that `widget_tree` element. Checked on macOS and the iOS simulator.
- Found while running the agent loop: an exception thrown by the app after an `interact` tap was logged as a `stack_frame.dart` assertion instead of the real exception (the tap ran from an evaluator closure). This is the cause of the `'match != null'` failures in bug 13.
- Found in the same run: `logs` with `errors_only` returned only the header of an error block on macOS and iOS.

Still open:
- `verify`'s `summary.json` still says "See exception logs above".
- The smaller issues listed above (duplicate findings, `fix` only adding trailing commas, the vuln-scan false positive, placeholder marketplace data, `cupertino_icons`).

## Confirmed bugs

### 1. Widget tree is never unwrapped
- **Where:** `src/runtime/connection.rs`, `get_root_widget_tree` (about line 316)
- **What:** `ext.flutter.inspector.getRootWidgetTree` returns `{type: "_extensionType", method, result: {...tree...}}`. Falcon uses the outer object as the tree.
- **Effect:** `x devtools inspector`, `x devtools tree-diff` and the `journey` screen labels (`src/runtime/journey.rs:94`) always show a single node named `_extensionType`.
- **Fix:** read `result.result` and keep the outer object only as a fallback.
- **Status:** already fixed in uncommitted working-tree changes (`inspector_root` in `src/runtime/tools.rs`, used by `journey.rs`). On iOS the rebuilt binary showed a 226-node tree and `MyHomePage` journey labels.

### 2. Frame metrics come from an extension that doesn't exist
- **Where:** `src/runtime/connection.rs`, `get_rendering_stats` (about line 285)
- **What:** it calls `ext.flutter.inspector.getRenderStats`, which isn't in Flutter. The error is replaced with `{}`.
- **Effect:** `trace`, `x runtime-check` and `journey` report 0 frames and 0 jank while the app renders at about 60fps, and the Rendering dimension scores 100/100.
- **Fix:** subscribe to the `Extension` stream and collect `Flutter.Frame` events (`build`, `raster` and `elapsed` in µs). Report an error instead of zeros when there is no data.

### 3. `devtools reload` and `restart` always fail
- **Where:** the reload and restart code in `src/runtime/connection.rs`
- **What:** they call the VM's own `reloadSources`, which fails with "Error while starting Kernel isolate task".
- **Effect:** hot reload and restart never work from Falcon.
- **Fix:** `flutter run` registers `sN.reloadSources` and `sN.hotRestart`; find their names from `ServiceRegistered` events on the `Service` stream and call those. Calling `s1.reloadSources` by hand worked.

### 4. `screenshot --window` can never succeed
- **Where:** `src/runtime/screenshot.rs`, the AppleScript in `window_bounds`, which `parse_window_bounds` (line 136) then parses
- **What:** `(x as integer) & "," & ...` builds an AppleScript list, so the output is `293, ,, 212, ,, 800, ,, 632` and parsing fails.
- **Fix:** coerce each value to text, for example `((x as integer) as text) & "," & ...`.

### 5. The engine screenshot fails under Impeller, and macOS has no fallback
- **Where:** `src/runtime/tools.rs`, `collect_screenshot` (about line 260), and the `_flutter.screenshot` call in `connection.rs` (about line 383)
- **What:** the engine refuses `_flutter.screenshot` ("Compressed screenshots not supported for Impeller"). `flutter screenshot` fails the same way.
- **Effect:** `falcon screenshot` and the new MCP `screenshot` tool fail on every path on macOS. On iOS the engine call fails the same way, but `flutter screenshot` works, so both succeed there through the `device` path (about 3.5s).
- **Fix:** fall back to `ext.flutter.inspector.screenshot` (args: `id` of the root render object, `width`, `height`, `margin`, `maxPixelRatio`, `debugPaint`). Calling it directly returned a correct PNG.

### 6. `avoid-long-functions` measures top-level functions as the whole file
- **What:** a one-line top-level function was reported as 65 lines because the measured span runs to the end of the file. Methods inside classes are measured correctly.
- **Repro:** a file with one short top-level function followed by about 60 lines of other code.

### 7. `x check-widgets` reports false errors for `setState` in `build`
- **What:** "setState inside build" (severity error) is reported when `setState` is inside a closure such as `onTap: () => setState(...)`, which is the normal pattern.
- **Effect:** the unmodified `flutter create` template fails (`lib/main.dart:88`).
- **Fix:** skip calls inside function expressions and closures within `build`.

### 8. `x upgrade-check` matches text inside string literals
- **What:** the template's string `'You have pushed the button this many times:'` is reported as the removed `TextTheme.button` API.
- **Fix:** match only identifier and member-access nodes, not string contents.

### 9. `x explain` doesn't know most rules
- **What:** `explain` has docs for 24 rules, and the generated rule docs list 58. Rules that `check` reports, such as `avoid-hardcoded-credentials` and `prefer-trailing-comma`, return "unknown rule".
- **Fix:** generate `explain` text from the rule registry (name, description, severity) so every registered rule is covered.

### 10. `x run` misses short-form Flutter exceptions
- **What:** after the first error, Flutter prints later errors as one line, `Another exception was thrown: ...`. `x run` only captures the full `EXCEPTION CAUGHT BY` block.
- **Effect:** two `SocketException`s in the test app were never written to `errs/`.

### 11. `x devtools rebuilds` always reports 0
- **What:** it never enables `ext.flutter.inspector.trackRebuildDirtyWidgets`, so no rebuild events are produced.

### 12. The inspector triggers an assertion inside the app
- **Where:** `src/runtime/connection.rs:342`
- **What:** `ext.flutter.inspector.getSelectedWidget` is called without `objectGroup`, which fails `parameters.containsKey('objectGroup')` in `widget_inspector.dart`.
- **Effect:** each `x devtools inspector` run prints a framework exception in the user's app log, and `selected` is always `<unknown>`. Seen on macOS and iOS.
- **Fix:** pass an `objectGroup` (and dispose it afterwards), as the other inspector calls do.

### 13. MCP `interact` with `type` is unreliable on screens that throw layout errors
- **What:** on the profile screen (which overflows), `type` into the `TextField` failed with `'sizeAccessAllowed'` in `RenderFractionalTranslation` during the route transition. After the screen settled it worked once ("hello falcon" was entered), then the identical call failed with `'match != null'` in `stack_frame.dart` (`Eval.<anonymous closure>`).
- **Second iOS run:** the first `type` was refused with "Every widget matching {"type":"TextField"} is covered by another screen or dialog; nothing was tapped" while the profile screen was on top and the field was visible (cause not found). After reopening the screen, 2 of 3 `type` calls failed with the `stack_frame.dart` assertion and the third worked.
- **Cause (likely):** the finder runs `localToGlobal` in an `evaluate` while a layout or error report is in progress, and Flutter can't parse the eval stack frames when reporting the error.
- **Fix:** do the lookups after a frame (`SchedulerBinding.endOfFrame`), skip render objects that aren't laid out (`hasSize`/`attached`), and catch errors per element.

### 14. MCP `reload` reports failure as success
- **What:** `reload` (and `full_restart: true`) returned `success: false, reason: "Error while starting Kernel isolate task"` with `isError: false`. This is bug 3 seen through MCP, but the agent isn't told the call failed.
- **Fix:** set `isError` when `success` is false; use the `sN.reloadSources`/`sN.hotRestart` services when attached to a `flutter run` session.

## Minor issues

- `x devtools network`: requests that fail with no HTTP status are counted as `failed_requests: 0`.
- `fix` only adds trailing commas, which `dart format` undoes. None of the real problems have autofixes.
- The same empty catch is reported three times under different rule IDs, and undisposed controllers are reported twice.
- `x vuln-scan` reports an open redirect for a constant URL.
- `x check-dependencies` reports `cupertino_icons` as unused (the template includes it for icon fonts).
- `x marketplace` and `x community contributed` show placeholder data with `github.com/example` links.
- `verify` writes "See exception logs above" in `summary.json` instead of naming the failing test and error.
- `check` doesn't flag `http://` URLs; only `x vuln-scan` does (CWE-319).
- `docs/mcp.md` said the tool list was "locked at 5 tools".
- `verify --help` says `--target` is a file "inside integration_test/", but `--target app_test.dart` fails with "Integration test target does not exist"; only `integration_test/app_test.dart` works.
- MCP `interact` with `tap` and `index` fails with "`tap` needs `find`", although `widget_tree` returns indexes. (Fixed 2026-11-24.)

## Not tested

- `x qr-check` (needs a video file), `x ai triage` (needs `--features ai-local`), `x fvm` (FVM not installed).
- Android (no `adb` or emulator).
- A physical iOS device.

## Works as expected

`check` (all formats, `--since`, presets, baselines), `review`, `score`, `doctor`, `fix`, `x vuln-scan`, `x manage *`, `x refactor-sim`, `x agents init`, `x dashboard serve`, `x api` (token enforced), `x webhook`, `x watch`, the static MCP tools, and the runtime commands `x live`, `x run`, `x devtools memory|profiler|performance|debugger|logging|route-log|network`, and `verify`.

## iOS simulator run

iPhone 17 simulator, iOS 27.2, Impeller (Metal), debug mode, run on 2026-11-21 with the binary rebuilt from the working tree, including the uncommitted runtime and MCP changes.

| Area | Result on iOS |
|---|---|
| `screenshot` (CLI and MCP) | Works through the `device` path; the engine path still fails (bug 5) |
| `screenshot --window` | Fails, as expected; it only applies to macOS windows (bug 4) |
| `x devtools inspector`, `tree-diff`, `journey` labels | Correct (bug 1 is fixed in the working tree) |
| `trace`, `x runtime-check`, MCP `app_metrics trace` | Still 0 frames (bug 2) |
| `x devtools reload`/`restart`, MCP `reload` | Still fail (bugs 3 and 14) |
| `x devtools rebuilds`, MCP `app_metrics rebuilds` | Still 0, `stats_enabled: false` (bug 11) |
| `x live` | Detects the RenderFlex overflow |
| `x devtools route-log` | Caught `/profile` |
| `x devtools memory`, `profiler`, `performance`, `logging` | Real data |
| `x run` | Wrote `error_1.md`; missed both "Another exception" lines (bug 10) |
| `verify` | Fails the test correctly and keeps the evidence; summary is still vague |
| MCP static tools | All work |
| MCP `widget_tree` | Works, with rects and keys |
| MCP `interact` | `tap` by key or tooltip, `back` and `wait` work (counter went 0→1, profile opened and closed); `type` is unreliable (bug 13); `scroll` correctly reports nothing scrollable |
| MCP `logs`, `tree_diff`, `app_metrics memory/network/performance/profiler/routes` | Work |

Evidence is in the session scratchpad under `ios/` (runtime_results.json, mcp_results.json, mcp_runtime_1.json, interact/, errs/, verify_out/).

### Second iOS run

Rerun on 2026-11-21 after `xcode-select` was switched to full Xcode, with the live simulator panel open. Falcon was rebuilt from the working tree first (the `falcon` binary had changed). Every CLI runtime command, every MCP tool, `x run` and `verify` gave the same results as the first iOS run. The one difference is MCP `interact type`, which got less reliable (see bug 13). Bugs 2, 3, 4, 5, 10, 11, 12 and 14 are unchanged, and bug 1 is still fixed in the working tree. Evidence is in the scratchpad under `ios2/`.
