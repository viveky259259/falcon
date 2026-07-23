#!/usr/bin/env bash
# Coverage helper for Falcon.
# Wraps cargo-llvm-cov with the invocations used by the coverage-uplift initiative.
# See docs/superpowers/specs/2026-05-25-coverage-uplift-design.md.

set -euo pipefail

CMD="${1:-help}"
shift || true

case "$CMD" in
  summary)
    # Per-module table from cargo-llvm-cov. To get a sorted view, run:
    #   scripts/coverage.sh summary | sort -k2 -n   # adjust column index as needed
    cargo llvm-cov --workspace --summary-only "$@"
    ;;
  html)
    cargo llvm-cov --workspace --html --output-dir target/llvm-cov/html "$@"
    echo "Report: target/llvm-cov/html/index.html"
    ;;
  lcov)
    cargo llvm-cov --workspace --lcov --output-path target/llvm-cov/lcov.info "$@"
    echo "LCOV: target/llvm-cov/lcov.info"
    ;;
  ci)
    cargo llvm-cov --workspace --fail-under-lines 90 "$@"
    ;;
  module)
    if [ -z "${1:-}" ]; then
      echo "usage: $0 module <path/to/file.rs>" >&2
      exit 2
    fi
    cargo llvm-cov --workspace --summary-only "$@" | grep -E "^($1|Filename|---)"
    ;;
  help|*)
    cat <<'USAGE'
Usage: scripts/coverage.sh <command> [extra cargo llvm-cov args]

Commands:
  summary           Per-module table, sorted ascending by line coverage.
  html              Generate HTML report at target/llvm-cov/html/index.html.
  lcov              Emit LCOV file at target/llvm-cov/lcov.info.
  ci                Run with --fail-under-lines 90 (used in CI once stable).
  module <path.rs>  Show coverage for a single file.
  help              Show this message.
USAGE
    ;;
esac
