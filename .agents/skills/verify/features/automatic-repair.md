# Automatic repair

## Sub-features

Preview edits, apply syntax-bounded fixes, preserve named/optional parameter delimiters, and avoid repeated changes.

## How to get to it (user POV)

Run `falcon fix <project> --preview`, then `falcon fix <project>` within the authorized workspace.

## Driving it with the CLI

Run `cargo test --test review_regressions trailing_comma`. Check both missing commas and existing commas, named and optional parameters, comments, and the idempotent second fix. For app code, inspect the actual diff, then run the app's Dart analyzer and behavioral tests. Preserve the failing source/diagnostic and fixed result as evidence.

## Gotchas

Preview is a read-only source inspection; applying fixes changes source. A parseable edit still requires the project's analyzer and behavior tests. Never apply bulk fixes to unrelated user work.
