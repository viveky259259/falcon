# Native integration verification

`falcon x verify` runs a Flutter integration test on an explicitly selected device. It saves evidence and fails for incomplete, empty, skipped, or failed runs. It does not synthesize app assertions or infer camera success from screenshots.

```sh
cargo build --bin falcon
./target/debug/falcon x verify /path/to/flutter-app \
  --target integration_test/handshake_test.dart --device DEVICE_ID \
  --timeout 180 --output-dir .tmp/handshake-proof
```

The target must resolve inside the app's `integration_test/` directory and end in `_test.dart`. The output directory must not exist. Relative output paths resolve from the caller's working directory. `--flutter` selects an executable/wrapper when needed. A complete Flutter SDK and platform toolchain are prerequisites. Flutter owns platform build/install and native permission interaction; Falcon never switches devices silently.

## What the runner proves

The report names the device/platform, emulator status, environment, entrypoint hash, elapsed time, counts, failures, and process exit. The evidence directory also contains device discovery output, the raw Flutter machine event stream, and stderr. Logs are capped at 16 MiB for tests, 1 MiB for discovery, and 2 MiB per stderr stream. Exceeding a limit fails the run. Timeouts terminate owned subprocesses and retain captured output. All existing output directories are rejected to prevent stale evidence from becoming a new pass.

Tests define the actual proof. `physical_mobile` is device metadata; it does not certify that the test used physical input. `native_claim` remains `requires_review_of_test_assertions` even on a passing run. Inspect the test source and independently observable side effects before making native-integration claims. Invalid event streams can stop parsing before all counts are known; raw evidence remains available. Runner exit code is nonzero whenever verification does not pass or evidence cannot be written.

## Executed reference flow

Run `scripts/verify-native-probe.sh macos`. Its committed source lives in `examples/native-verification/`. The helper provisions a disposable macOS Flutter host, drives a visible increment action, and calls the real native `Clipboard.hasStrings` platform channel. No channel mocks are registered. It queries a capability bit without reading or changing clipboard content. The helper removes its scratch host and retains evidence at the printed `.tmp/native-proof.*` path. This is desktop channel evidence, not mobile-camera evidence.

## EventPulse camera acceptance contract

Create app-owned `integration_test/handshake_test.dart` with the Flutter SDK's `integration_test` dependency. Use an isolated nonproduction backend and two test attendees. The app's current widget test uses **Try a demo scan**; that path must not be used for physical acceptance.

1. Provision a signed, unexpired badge for attendee B and display it on another device or print it. Keep test badge values out of published logs. Sign attendee A into the test event.
2. Navigate to **My badge**, then **Scan**. Grant camera permission through platform automation or fixture setup. Confirm the real mobile_scanner controller starts; no mocked camera channel or synthetic scan callback may substitute for optical input.
3. Present B's QR to the camera. Wait for **Connected with B's name** and **Remember this conversation**. A bounded wait must fail if no barcode is decoded. Retain test/device logs and, where allowed, a recording of the stimulus and visible result.
4. Save a distinctive test note with **Save note**. Open **Connections** and verify the attendee, event context, and note. Independently read the test backend as both attendees and confirm exactly one mutual graph edge and the expected owner-private note.
5. Repeat with an expired badge, a malformed badge, denied permission, and rapid duplicate detections. Assert a visible recoverable error and no unauthorized/duplicate connection. Remove only accounts/data created by this fixture.

Camera optics and backend integration can also be tested separately, but label each narrower result. Simulated QR input verifies handshake logic; optical scanning with a demo repository verifies optics plus local behavior; a deployed physical flow and independent backend reads are required for the full claim. No physical camera result is implied by the shipped desktop reference test.

## Other integrations

Use the same runner with app-owned contracts. Location needs permission and valid/denied/unavailable outcomes. BLE needs a known peripheral and verified communication. Notifications need configured platform delivery and tap-navigation assertions. Sharing needs a receiving destination that independently confirms the file. Authentication needs native sign-in plus backend session evidence. Hardware fixtures and external services remain prerequisites; absent prerequisites are blocked acceptance.
