#!/usr/bin/env bash
set -euo pipefail

base_ref="${1:-${BASE_REF:-}}"
head_ref="${2:-${HEAD_REF:-HEAD}}"
month="${FALCON_GUARDRAIL_MONTH:-$(date -u +%m)}"
labels=",${PR_LABELS:-},"

if [[ "${month}" != "06" ]]; then
  echo "Behavioral rule freeze inactive outside June (month=${month})."
  exit 0
fi

if [[ -z "${base_ref}" ]]; then
  echo "::error::BASE_REF or first argument is required for behavioral rule freeze check."
  exit 2
fi

if ! git rev-parse --verify "${base_ref}^{commit}" >/dev/null 2>&1; then
  echo "::error::Base ref '${base_ref}' is not available. Fetch the PR base before running this check."
  exit 2
fi

if ! git rev-parse --verify "${head_ref}^{commit}" >/dev/null 2>&1; then
  echo "::error::Head ref '${head_ref}' is not available."
  exit 2
fi

added_files="$(
  git diff --name-status --diff-filter=A "${base_ref}...${head_ref}" -- src/rules/behavioral |
    awk '$1 == "A" { print $2 }'
)"

if [[ -z "${added_files}" ]]; then
  echo "No new behavioral rule files detected."
  exit 0
fi

echo "::error::June behavioral rule freeze blocks newly added files under src/rules/behavioral/."
echo "${added_files}" | sed 's/^/  - /'

if [[ "${labels}" == *",defer-to-july,"* ]]; then
  echo "PR is labeled defer-to-july; keep this work deferred until the July milestone."
else
  echo "Add the defer-to-july label and move the new behavioral rule work to the July milestone."
fi

exit 1
