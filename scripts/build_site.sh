#!/usr/bin/env bash
# Assemble the GitHub Pages site in site/ before upload.
#
# Both Pages workflows (pages.yml on push, leaderboard.yml weekly) call this so
# they deploy the same site. It copies the Markdown docs next to the committed
# HTML docs in site/docs/ without deleting them, and never nests docs/ inside
# site/docs/. site/leaderboard/ is left untouched.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="${root}/docs"
dest="${root}/site/docs"

if [ ! -d "${src}" ]; then
  echo "build_site: ${src} not found" >&2
  exit 1
fi

mkdir -p "${dest}"
cp -R "${src}/." "${dest}/"

if [ ! -f "${dest}/index.html" ]; then
  echo "build_site: ${dest}/index.html is missing; the HTML docs were lost" >&2
  exit 1
fi
echo "build_site: copied docs/ into site/docs/ ($(find "${src}" -type f | wc -l | tr -d ' ') files)"
