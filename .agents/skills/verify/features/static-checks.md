# Static checks

## Sub-features

Project configuration, JSON diagnostics, and fail-on severity exit gates.

## How to get to it (user POV)

Run `falcon check <project> --format json --fail-on info --no-graph`.

## Driving it with the CLI

Run `cargo test --test review_regressions`. This creates isolated Dart projects and drives the binary, checking diagnostics, output-file failures, and configuration. For the changed project, retain the actual check JSON and inspect `issues` as well as the exit code. Match reported locations to current source before fixing.

## Gotchas

Static diagnostics cannot prove permission dialogs, camera decoding, or backend side effects. Passing with selected rules proves that rule set only. An old global binary can differ from the checkout.
