#!/usr/bin/env bash
set -euo pipefail

args=(check "${INPUT_PATH:-.}" --format "${INPUT_FORMAT:-console}" --fail-on "${INPUT_FAIL_ON:-error}")
if [[ -n "${INPUT_SINCE:-}" ]]; then
  args+=(--since "$INPUT_SINCE")
fi
if [[ -n "${INPUT_BASELINE:-}" ]]; then
  args+=(--baseline "$INPUT_BASELINE")
fi
if [[ -n "${INPUT_CONFIG:-}" ]]; then
  args+=(--config "$INPUT_CONFIG")
fi
falcon "${args[@]}"
