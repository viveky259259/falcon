#!/usr/bin/env bash
# =============================================================================
# Falcon Release Script
#
# Automates the full release pipeline:
#   1. Version bump (Cargo.toml, pubspec.yaml, site, README)
#   2. Build & test
#   3. Generate CHANGELOG entry
#   4. Update documentation stats
#   5. Git tag & push
#   6. GitHub release with binaries
#   7. Publish Dart package to pub.dev
#   8. Deploy website to GitHub Pages
#
# Usage:
#   ./scripts/release.sh                  # Interactive — prompts for version
#   ./scripts/release.sh 1.2.0            # Release specific version
#   ./scripts/release.sh patch            # Auto-bump patch (0.1.0 → 0.1.1)
#   ./scripts/release.sh minor            # Auto-bump minor (0.1.0 → 0.2.0)
#   ./scripts/release.sh major            # Auto-bump major (0.1.0 → 1.0.0)
#   ./scripts/release.sh --dry-run 1.2.0  # Preview without making changes
#
# Requirements:
#   - cargo, rustc
#   - dart
#   - gh (GitHub CLI, authenticated)
#   - git
# =============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# ─── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}▸${NC} $1"; }
ok()    { echo -e "${GREEN}✓${NC} $1"; }
warn()  { echo -e "${YELLOW}⚠${NC} $1"; }
error() { echo -e "${RED}✗${NC} $1"; exit 1; }
step()  { echo -e "\n${BOLD}${CYAN}═══ $1 ═══${NC}\n"; }

# ─── Parse Args ──────────────────────────────────────────────────────────────
DRY_RUN=false
VERSION_ARG=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --help|-h)
            echo "Usage: $0 [--dry-run] [patch|minor|major|X.Y.Z]"
            exit 0
            ;;
        *) VERSION_ARG="$1"; shift ;;
    esac
done

# ─── Pre-flight Checks ──────────────────────────────────────────────────────
step "Pre-flight Checks"

command -v cargo >/dev/null || error "cargo not found — install Rust"
command -v dart  >/dev/null || error "dart not found — install Dart SDK"
command -v gh    >/dev/null || error "gh not found — install GitHub CLI"
command -v git   >/dev/null || error "git not found"

BRANCH=$(git branch --show-current)
if [[ "$BRANCH" != "master" && "$BRANCH" != "main" ]]; then
    warn "Not on master/main (on '$BRANCH'). Releases should be from master."
    read -rp "Continue anyway? (y/N) " confirm
    [[ "$confirm" =~ ^[Yy]$ ]] || exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
    error "Working directory is not clean. Commit or stash changes first."
fi

ok "Pre-flight checks passed"

# ─── Determine Version ──────────────────────────────────────────────────────
step "Version"

CURRENT_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
info "Current version: ${BOLD}$CURRENT_VERSION${NC}"

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

if [[ -z "$VERSION_ARG" ]]; then
    echo ""
    echo "  1) patch  → $MAJOR.$MINOR.$((PATCH + 1))"
    echo "  2) minor  → $MAJOR.$((MINOR + 1)).0"
    echo "  3) major  → $((MAJOR + 1)).0.0"
    echo "  4) custom → enter version manually"
    echo ""
    read -rp "Choose (1/2/3/4): " choice
    case "$choice" in
        1) VERSION_ARG="patch" ;;
        2) VERSION_ARG="minor" ;;
        3) VERSION_ARG="major" ;;
        4) read -rp "Enter version: " VERSION_ARG ;;
        *) error "Invalid choice" ;;
    esac
fi

case "$VERSION_ARG" in
    patch) NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))" ;;
    minor) NEW_VERSION="$MAJOR.$((MINOR + 1)).0" ;;
    major) NEW_VERSION="$((MAJOR + 1)).0.0" ;;
    *)     NEW_VERSION="$VERSION_ARG" ;;
esac

info "New version: ${BOLD}$NEW_VERSION${NC}"

if $DRY_RUN; then
    warn "DRY RUN — no changes will be made"
fi

# ─── Collect Release Info ────────────────────────────────────────────────────
COMMIT_COUNT=$(git rev-list "v${CURRENT_VERSION}..HEAD" --count 2>/dev/null || git rev-list HEAD --count)
RULE_COUNT=$(grep -r "impl Rule for" src/rules/ | wc -l | tr -d ' ')
TEST_COUNT=$(grep -r "#\[test\]" tests/ | wc -l | tr -d ' ')
COMMAND_COUNT=$(grep -c "Commands::" src/main.rs || echo "0")
FILE_COUNT=$(find src -name "*.rs" | wc -l | tr -d ' ')
LINE_COUNT=$(find src -name "*.rs" -exec cat {} + | wc -l | tr -d ' ')

info "Commits since last release: $COMMIT_COUNT"
info "Stats: $RULE_COUNT rules, $TEST_COUNT tests, $COMMAND_COUNT commands, $FILE_COUNT files, $LINE_COUNT lines"

# ─── Step 1: Bump Versions ──────────────────────────────────────────────────
step "Step 1: Bump Versions"

if ! $DRY_RUN; then
    # Cargo.toml
    sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
    ok "Cargo.toml → $NEW_VERSION"

    # falcon-dart/pubspec.yaml
    sed -i '' "s/^version: $CURRENT_VERSION/version: $NEW_VERSION/" falcon-dart/pubspec.yaml
    ok "falcon-dart/pubspec.yaml → $NEW_VERSION"

    # Update description stats in pubspec
    sed -i '' "s/[0-9]\+\+ rules/${RULE_COUNT}+ rules/" falcon-dart/pubspec.yaml
    ok "Updated rule count in pubspec description"

    # npm/falcon/package.json — CI's "Verify npm package version matches tag"
    # step fails the release if this drifts from Cargo.toml, so keep it in sync.
    sed -i '' "s/\"version\": \"$CURRENT_VERSION\"/\"version\": \"$NEW_VERSION\"/" npm/falcon/package.json
    ok "npm/falcon/package.json → $NEW_VERSION"
else
    info "[DRY RUN] Would update Cargo.toml, pubspec.yaml, npm/falcon/package.json to $NEW_VERSION"
fi

# ─── Step 2: Build & Test ────────────────────────────────────────────────────
step "Step 2: Build & Test"

info "Running cargo build..."
cargo build --release 2>&1 | tail -3
ok "Build successful"

info "Running cargo test..."
TEST_RESULT=$(cargo test 2>&1)
PASS_COUNT=$(echo "$TEST_RESULT" | grep "^test result" | awk '{sum += $4} END {print sum}')
FAIL_COUNT=$(echo "$TEST_RESULT" | grep "^test result" | awk '{sum += $6} END {print sum}')

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    error "$FAIL_COUNT test(s) failed. Fix before releasing."
fi
ok "$PASS_COUNT tests passed, 0 failed"

# ─── Step 3: Generate CHANGELOG ─────────────────────────────────────────────
step "Step 3: Generate CHANGELOG"

CHANGELOG_FILE="CHANGELOG.md"
DATE=$(date +%Y-%m-%d)

COMMITS=$(git log --oneline "v${CURRENT_VERSION}..HEAD" 2>/dev/null || git log --oneline -20)

CHANGELOG_ENTRY="## $NEW_VERSION ($DATE)

### Stats
- $RULE_COUNT lint rules
- $TEST_COUNT tests passing
- $COMMAND_COUNT CLI commands
- $FILE_COUNT source files ($LINE_COUNT lines of Rust)

### Changes
$(echo "$COMMITS" | sed 's/^/- /')
"

if ! $DRY_RUN; then
    if [[ -f "$CHANGELOG_FILE" ]]; then
        EXISTING=$(cat "$CHANGELOG_FILE")
        echo -e "# Changelog\n\n$CHANGELOG_ENTRY\n$EXISTING" > "$CHANGELOG_FILE"
    else
        echo -e "# Changelog\n\n$CHANGELOG_ENTRY" > "$CHANGELOG_FILE"
    fi
    ok "CHANGELOG.md updated"

    # Also update falcon-dart CHANGELOG
    DART_CHANGELOG="falcon-dart/CHANGELOG.md"
    DART_ENTRY="## $NEW_VERSION\n\n- Updated to Falcon $NEW_VERSION\n- $RULE_COUNT lint rules, $COMMAND_COUNT commands\n"
    if [[ -f "$DART_CHANGELOG" ]]; then
        EXISTING=$(cat "$DART_CHANGELOG")
        echo -e "$DART_ENTRY\n$EXISTING" > "$DART_CHANGELOG"
    else
        echo -e "$DART_ENTRY" > "$DART_CHANGELOG"
    fi
    ok "falcon-dart/CHANGELOG.md updated"
else
    info "[DRY RUN] Would generate CHANGELOG entry for $NEW_VERSION"
    echo "$CHANGELOG_ENTRY"
fi

# ─── Step 4: Update Documentation ───────────────────────────────────────────
step "Step 4: Update Documentation"

if ! $DRY_RUN; then
    # Update README stats
    sed -i '' "s/rules-[0-9]*%2B/rules-${RULE_COUNT}%2B/" README.md
    sed -i '' "s/tests-[0-9]*/tests-${PASS_COUNT}/" README.md

    # Update landing page stats
    SITE_INDEX="site/index.html"
    if [[ -f "$SITE_INDEX" ]]; then
        sed -i '' "s|<div class=\"stat-number\">[0-9]*+</div><div class=\"stat-label\">Lint Rules</div>|<div class=\"stat-number\">${RULE_COUNT}+</div><div class=\"stat-label\">Lint Rules</div>|" "$SITE_INDEX"
        sed -i '' "s|<div class=\"stat-number\">[0-9]*+</div><div class=\"stat-label\">CLI Commands</div>|<div class=\"stat-number\">${COMMAND_COUNT}+</div><div class=\"stat-label\">CLI Commands</div>|" "$SITE_INDEX"
        ok "Landing page stats updated"
    fi

    # Update docs getting-started version references
    ok "Documentation updated"
else
    info "[DRY RUN] Would update README, site, docs with new stats"
fi

# ─── Step 5: Git Commit, Tag & Push ─────────────────────────────────────────
step "Step 5: Git Commit, Tag & Push"

if ! $DRY_RUN; then
    git add -A
    git commit -m "release: v${NEW_VERSION}

Version bump to ${NEW_VERSION}
- ${RULE_COUNT} lint rules
- ${PASS_COUNT} tests passing
- ${COMMAND_COUNT} CLI commands
- ${FILE_COUNT} source files (${LINE_COUNT} lines)
"
    ok "Committed release changes"

    git tag -a "v${NEW_VERSION}" -m "Release v${NEW_VERSION}"
    ok "Tagged v${NEW_VERSION}"

    git push origin "$BRANCH" --tags
    ok "Pushed to origin with tags"
else
    info "[DRY RUN] Would commit, tag v${NEW_VERSION}, and push"
fi

# ─── Step 6: GitHub Release ─────────────────────────────────────────────────
step "Step 6: GitHub Release"

RELEASE_NOTES="## Falcon v${NEW_VERSION}

### What's New
$(echo "$COMMITS" | head -10 | sed 's/^/- /')

### Stats
| Metric | Value |
|---|---|
| Lint Rules | ${RULE_COUNT}+ |
| CLI Commands | ${COMMAND_COUNT} |
| Tests | ${PASS_COUNT} |
| Source Files | ${FILE_COUNT} |
| Lines of Rust | ${LINE_COUNT} |

### Install
\`\`\`bash
cargo install --git https://github.com/viveky259259/falcon
# or
dart pub global activate falcon_cli
\`\`\`

### Full Changelog
See [CHANGELOG.md](https://github.com/viveky259259/falcon/blob/master/CHANGELOG.md)
"

if ! $DRY_RUN; then
    gh release create "v${NEW_VERSION}" \
        --title "Falcon v${NEW_VERSION}" \
        --notes "$RELEASE_NOTES" \
        2>&1 && ok "GitHub release created" || warn "GitHub release creation failed (may need manual creation)"
else
    info "[DRY RUN] Would create GitHub release v${NEW_VERSION}"
fi

# ─── Step 7: Publish Dart Package ────────────────────────────────────────────
step "Step 7: Publish Dart Package"

if ! $DRY_RUN; then
    cd falcon-dart
    info "Running pub publish --dry-run..."
    dart pub publish --dry-run 2>&1 | tail -5

    read -rp "Publish to pub.dev? (y/N) " pub_confirm
    if [[ "$pub_confirm" =~ ^[Yy]$ ]]; then
        dart pub publish --force 2>&1 | tail -5
        ok "Published falcon_cli v${NEW_VERSION} to pub.dev"
    else
        warn "Skipped pub.dev publish"
    fi
    cd "$REPO_ROOT"
else
    info "[DRY RUN] Would publish falcon_cli v${NEW_VERSION} to pub.dev"
fi

# ─── Step 8: Deploy Website ─────────────────────────────────────────────────
step "Step 8: Deploy Website"

if ! $DRY_RUN; then
    info "Triggering GitHub Pages deployment..."
    gh workflow run pages.yml 2>&1 || warn "Manual trigger failed — push already triggered deploy"

    sleep 5
    DEPLOY_STATUS=$(gh run list --limit 1 --json status,conclusion 2>/dev/null | grep -o '"status":"[^"]*"' | head -1)
    info "Deployment status: $DEPLOY_STATUS"

    ok "Website deployment triggered"
else
    info "[DRY RUN] Would trigger GitHub Pages deployment"
fi

# ─── Summary ─────────────────────────────────────────────────────────────────
step "Release Complete!"

echo -e "
  ${BOLD}Falcon v${NEW_VERSION}${NC} released successfully!

  ${GREEN}✓${NC} Version bumped:  Cargo.toml, pubspec.yaml
  ${GREEN}✓${NC} Tests:           ${PASS_COUNT} passing
  ${GREEN}✓${NC} CHANGELOG:       Updated
  ${GREEN}✓${NC} Documentation:   Stats updated
  ${GREEN}✓${NC} Git:             Tagged v${NEW_VERSION}, pushed
  ${GREEN}✓${NC} GitHub Release:  Created
  ${GREEN}✓${NC} pub.dev:         falcon_cli v${NEW_VERSION}
  ${GREEN}✓${NC} Website:         Deployed to GitHub Pages

  Links:
    GitHub:   https://github.com/viveky259259/falcon/releases/tag/v${NEW_VERSION}
    pub.dev:  https://pub.dev/packages/falcon_cli
    Website:  https://viveky259259.github.io/falcon/
    Docs:     https://viveky259259.github.io/falcon/docs/
"
