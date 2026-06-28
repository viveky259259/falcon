#!/bin/sh
# Falcon pre-commit hook
# Install: cp ci-templates/pre-commit-hook.sh .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
# Or use with pre-commit framework (see .pre-commit-hooks.yaml)
# Pre-commit stays syntactic-only by default; use falcon review/check --semantic in CI.

set -e

if ! command -v falcon &> /dev/null; then
    echo "falcon: not found. Install with: cargo install falcon"
    exit 0
fi

STAGED_DART=$(git diff --cached --name-only --diff-filter=ACM | grep '\.dart$' || true)

if [ -z "$STAGED_DART" ]; then
    exit 0
fi

echo "falcon: analyzing $(echo "$STAGED_DART" | wc -l | tr -d ' ') staged Dart file(s)..."

falcon analyze . --since HEAD --fail-on error 2>&1

EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    echo ""
    echo "falcon: analysis found errors. Fix them before committing."
    echo "  Bypass with: git commit --no-verify"
    exit 1
fi

echo "falcon: all checks passed."
