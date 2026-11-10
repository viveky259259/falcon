# Native flow execution

## Sub-features

Exact device selection, deadline and process-tree cleanup, bounded logs, strict result parsing, and fresh evidence directories.

## How to get to it (user POV)

Run `falcon x verify` with the project, integration test target, and exact Flutter device ID. Check `falcon x verify --help` for options.

## Driving it with the CLI

Run `scripts/verify-native-probe.sh macos`. Confirm `result/summary.json` has status passed, one visible test passed, environment desktop, and target SHA-256. Review the retained event stream: loading and teardown are hidden; the native platform-channel/UI test is visible. Confirm the temporary host is removed and retained logs remain. Run `cargo test --test verify_cli` for unavailable devices, failed/skipped/empty runs, output overflow, timeout cleanup, and write failures.

## Gotchas

The helper needs complete Xcode and macOS. A physical_mobile device label does not prove native input: the app test can still use mocks. Existing evidence directories are rejected. The native probe queries only whether the clipboard has strings; it never accesses content.
