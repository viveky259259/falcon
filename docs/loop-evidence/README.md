# Evidence: one run of the agent loop through falcon-mcp

On 2026-11-24 an AI agent (Claude, in a Claude Code session) drove
`falcon-mcp` through the loop the README describes: run the app, use it, read
the failure, fix the code, hot reload, check again, run the test. It did this
on macOS desktop and on the iOS simulator. No person typed anything between
the first call and the last.

This page says exactly what was run, what came back, and what the run does
not show.

## Setup

| | |
|---|---|
| Falcon | built from this branch at commit `a5ec0c7` (`cargo build`, debug profile) |
| Flutter | 3.47.6 stable, debug mode |
| macOS run | device `macos`, macOS 27.0.1, arm64, 800x600 window |
| iOS run | iPhone 17 simulator, iOS 27.2, 402x874 at 3x |
| App | made with `flutter create --platforms=macos,ios`, then `lib/main.dart` replaced by [app/main_with_bug.dart.txt](app/main_with_bug.dart.txt) and [app/remove_test.dart.txt](app/remove_test.dart.txt) added as `integration_test/remove_test.dart` |

The app is a shopping list with two buttons. The planted bug is in "Remove
last": it calls `_items.removeAt(_items.length)`, which always throws a
`RangeError`.

The agent talked to `falcon-mcp` over stdio, the same JSON-RPC a coding tool
uses. [driver/mcp_daemon.py](driver/mcp_daemon.py) keeps one `falcon-mcp`
process alive and records every call; [driver/mcp_call.py](driver/mcp_call.py)
sends one `tools/call`:

```
python3 mcp_daemon.py mcp.sock /path/to/falcon-mcp macos.jsonl &
python3 mcp_call.py mcp.sock run_app '{"path": "<demo>", "device": "macos"}'
```

## What happened

The same steps ran on both platforms. Times are from the recorded calls.

| Step | Tool call | macOS | iOS simulator |
|---|---|---|---|
| Test before the fix | `verify` on `integration_test/remove_test.dart` | not run | `status: failed`, 0 passed, 1 failed (38.6 s) |
| Start the app | `run_app` | running after 8.2 s | running after 13.7 s |
| Read the screen | `widget_tree` | 4 elements: "0 items", "Add item", "Remove last", "Shopping list" | same |
| Add two items | `interact` `{"action":"tap","index":1}` twice | "2 items", "Item 1", "Item 2" appeared | same |
| Tap "Remove last" | `interact` `{"action":"tap","find":{"key":"remove"}}` | nothing appeared or disappeared | same |
| Read the failure | `logs` `{"errors_only":true}` | `RangeError (length): Invalid value: Not in inclusive range 0..1: 2` at `_ShoppingListState._removeLast (package:loop_demo/main.dart:29:5)` | same |
| Fix | agent edited `lib/main.dart`: [app/fix.diff](app/fix.diff) | | |
| Hot reload | `reload` | `success: true`, "Reloaded 1 of 754 libraries", 156 ms | `success: true`, 160 ms |
| Tap "Remove last" again | `interact` | "1 items" appeared; "2 items" and "Item 2" disappeared | same |
| Check for new errors | `logs` with `since` and `errors_only` | no lines | no lines |
| Look at the screen | `screenshot` | [macos-after-fix.png](macos-after-fix.png), 800x600 | [ios-after-fix.png](ios-after-fix.png), 1206x2622 |
| Test after the fix | `verify` | `status: passed`, 1 passed (19.1 s), app still running | `status: passed`, 1 passed (20.9 s), after `stop_app` |

Every call with its arguments and reply: [macos.md](macos.md) and
[ios.md](ios.md). The raw records are [macos.jsonl](macos.jsonl) and
[ios.jsonl](ios.jsonl). Local paths are replaced by `<demo>` and `<work>`, and
image data is left out; nothing else is edited.

The fix:

```diff
   void _removeLast() {
-    setState(() => _items.removeAt(_items.length));
+    if (_items.isEmpty) return;
+    setState(() => _items.removeLast());
   }
```

## The loop did not work on the first two tries

The recorded runs are the third attempt. The first two failed because of bugs
in Falcon, both fixed in this branch:

1. After the tap on "Remove last", `logs` showed
   `'package:flutter/src/foundation/stack_frame.dart': Failed assertion: line 210 pos 12: 'match != null'`
   and not the `RangeError`. `interact` sent the tap from a closure inside the
   expression it evaluates in the app, and Flutter cannot print an error whose
   stack has such a frame. The agent could not have found the bug from that
   log. This is how the build on `master` before this branch behaved for any
   handler that throws.
2. With that fixed, `logs` with `errors_only` returned only the
   `══╡ EXCEPTION CAUGHT BY GESTURE ╞══` header, without the exception or the
   stack. On macOS and iOS every app line starts with `flutter: `, which the
   block detection did not expect.

## Screenshot of a screen that overflows

Separate from the loop: the app's list was replaced by a 3000 px tall box so
the column overflows by 2312 px, on the iOS simulator. `falcon screenshot
--attach <uri>` now returns the screen only, with the overflow stripes at the
bottom edge: [ios-overflow-screenshot.png](ios-overflow-screenshot.png). It is
506x1101, not 1206x2622: when the painted area is larger than 4096 px, the
capture is scaled down. Before the fix the image continued below the screen.

## What this run does not show

- **The agent knew the bug.** The same agent planted it and fixed it. The run
  shows that the tools carry the information needed (the exception, the file
  and the line reached the agent through `logs`), not that an agent finds an
  unknown bug without help.
- **One app, one bug, one kind of failure.** A thrown exception in a tap
  handler. Layout bugs, wrong data, slow frames, navigation, platform
  channels, network errors and state management were not tried.
- **Not run from a coding tool's MCP client.** The calls went over stdio
  through the driver above, not through Cursor, Claude Code or Windsurf with
  `falcon-mcp` configured.
- **Falcon did not choose the fix or apply it.** The agent edited the file
  with its own tools. Falcon ran the app, sent the tap, returned the log,
  reloaded and ran the test.
- **Android, a physical iPhone, Flutter web, Windows and Linux were not run.**
- **`verify` does not say why a test failed.** Before the fix it returned
  "Test failed. See exception logs above." and `isError: false` with
  `status: failed`; the agent has to read `status`.
- **One run on each platform.** No repeat count, so nothing here measures how
  often a step fails.
