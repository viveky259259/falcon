---
name: verify
description: Drive Falcon's CLI and native Flutter verification runner, reproduce failures, and audit saved evidence. Use after changing Falcon checks, fixes, or device-test execution.
---

# Verify Falcon

## Plan the proof

For a native integration, first identify the production plugin/platform channel, input stimulus, permissions, external fixture, visible result, and independent side effect. Inspect existing app-owned integration tests before adding another harness. Use the acceptance contracts in [native verification](../../../docs/native-verification.md) when the app has no contract. Record missing prerequisites explicitly; select simulated, emulator, or physical evidence based on the actual inputs and assertions, not just the device label.

## Launch

From the Falcon repository root, run `cargo build --bin falcon`. Use `./target/debug/falcon` for this checkout; the globally installed `falcon` may be older. Each CLI run is short-lived and needs no server. Put evidence under `.tmp/` and give every device run a fresh output directory. Do not run two drivers against the same native app/device at once.

For the real macOS native reference flow, run `scripts/verify-native-probe.sh macos`. It creates a disposable Flutter host, builds and drives it, removes the host on exit, and retains its logs and result under the printed `.tmp/native-proof.*` directory. It selects installed Xcode through DEVELOPER_DIR without changing system settings. Other platforms require their own app target and device setup.

## Doctor

Run `./target/debug/falcon x verify --help`. Confirm `--target`, `--device`, `--timeout`, `--output-dir`, and `--flutter` exist before using the runner. Then inspect `flutter devices --machine` and choose an exact supported ID. A passing Rust test run does not prove that an old executable in PATH has the new command. If development artifacts are demonstrably stale, rebuild this package with `cargo clean -p falcon-flutter` followed by `cargo build --bin falcon`; do not change global toolchains.

## Drive

Use the [feature map](features/README.md) to select the actual user path. The runner command is:

```sh
./target/debug/falcon x verify /path/to/app --target integration_test/native_test.dart --device macos --timeout 180 --output-dir .tmp/native-proof
```

Choose an existing app-owned target inside `integration_test/` and a new output directory. `--flutter` accepts a toolchain executable/wrapper path when the project does not use PATH's Flutter. All arguments are passed as separate process arguments. The deadline includes device discovery; logs are bounded. Tests with skips, no visible successes, malformed/incomplete events, failures, or nonzero exit do not pass.

For fixes, reproduce the failure first, change the relevant source, rerun the same failing target, then use `cargo test`, `cargo fmt --all -- --check`, and `cargo clippy --all-targets -- -D warnings`. Do not weaken an assertion or swap a physical test for a simulation to obtain a pass.

## Evidence

Inspect the exit code and `summary.json` together with `test-events.jsonl`, `test-stderr.log`, `devices.json`, and the target source identified by its SHA-256. Failure counts and named tests remain in the summary when parsing reaches them; raw logs retain earlier details if the stream is incomplete. Keep the failed run and the passing rerun in separate directories.

`environment` describes the device: desktop, emulator, browser, or physical_mobile. It does not describe whether the test injected a fake input. Review assertions and native mocks to classify the actual evidence. `native_claim` explicitly requires assertion review. The reference flow proves a visible counter action and a native clipboard-capability method returning a boolean. It never reads/writes clipboard content and does not prove camera scanning.

For hardware claims, require a physical stimulus through the real integration, the visible app result, and independent side-effect evidence. Camera/QR acceptance is specified in [native verification](../../../docs/native-verification.md). A QR string injected into business logic is simulation evidence. Missing devices/services or fixture setup are blocked acceptance, not passing tests. Do not publish raw logs containing credentials or personal data.

## Cleanup

The reference helper traps exit and removes only its own disposable Flutter host. Proof artifacts survive cleanup. The runner terminates its owned subprocess tree on timeout/output overflow and reaps the direct child. Never kill by process name or delete evidence to retry. For application-owned fixtures, use that application's teardown to remove only test accounts/data it created.

## QR fixture and physical camera checks

Run `falcon x qr-check --video VIDEO --manifest MANIFEST --output-dir NEW_DIRECTORY` on macOS before presenting a video fixture to the phone. Inspect summary.json and sample PNGs. This command owns frame extraction, Vision decoding, exact payload comparison and retained evidence. It proves only the encoded stimulus; the manifest contract and dependencies are documented in [native verification](../../../docs/native-verification.md).

Run app-owned optical assertions with `x verify --physical-camera` and an exact physical iOS/Android device. This guard rejects desktop/emulator targets. Retain fixture proof and device proof separately. Do not infer the second from a passing first, and do not replace the real scanner with a synthetic scan callback.
