# Flutter flow verification proposal

## Problem and outcome

EventPulse's static checks and widget tests cannot prove that its camera reads a badge and creates a mutual connection. Falcon's journey recorder captures a manually driven session. Add a Falcon command that runs application-owned Flutter integration tests on an explicitly selected device and saves inspectable evidence. Application tests define business assertions; Falcon owns execution, prerequisites, timeout, and reporting.

## Proposed command and implementation

`falcon x verify <project> --target integration_test/handshake_test.dart --device <id> --timeout 180 --output-dir <dir>`

- Add CLI arguments, runtime dispatch, and a focused `src/runtime/verify.rs` module. Use existing Rust/Serde/Tokio dependencies and Flutter tooling. Keep the current journey command intact.
- Validate project and target before launching. Read `flutter devices --machine` and require an exact supported device match; do not silently choose another device. Run `flutter test <target> -d <id> --machine` with argument arrays, not a shell string.
- Save the Flutter test event stream and a JSON summary with target, device platform, emulator status, elapsed time, test counts, failures, timeout, and evidence paths. Parse test events rather than treating process exit alone as proof. A run with zero tests, skipped required tests, malformed/incomplete events, or a nonzero exit must not pass.
- Bound execution and captured output. Terminate the launched test process on timeout and preserve diagnostics. Fail on report-write errors. Reports describe what the tests assert; they never infer camera success from screenshots or a passing generic test.

## Camera acceptance and EventPulse example

Provide documented EventPulse integration-test acceptance: open scanner, handle camera permission, decode a valid event badge through the actual scanner callback, assert the connected attendee and note prompt, save a note, and independently confirm mutual backend connection records. Also cover invalid/expired badges and duplicate callbacks. Setup must use nonproduction accounts and fixtures.

Distinguish simulated QR-input tests from physical-camera tests in the example and reports. A physical run requires an actual supported iOS/Android device, camera permission, and a badge presented to the camera. Human badge placement can be a setup step; the test drives and asserts the flow. A simulated input is useful regression evidence but cannot satisfy physical-camera acceptance. Do not claim hardware proof from app-authored labels alone; retain test source, device metadata, and observable scan/backend evidence for review.

## Verification and current constraints

Write failing Rust tests first for command arguments, exact-device selection, event parsing, failures, skipped/empty runs, timeouts, output bounds, and report-write failures. Use hermetic subprocess fixtures, then run full Rust tests, formatting, and Clippy. Verify orchestration against a real Flutter integration test. Verify physical scanning only when an actual mobile device and test backend are available.

Current device discovery returns only macOS, with no iOS/Android target. EventPulse currently has widget tests and no integration_test dependency. The global Falcon installation is unchanged. This proposal requires changes across CLI, runtime, tests, documentation, and application acceptance setup; implementation awaits scope approval.
