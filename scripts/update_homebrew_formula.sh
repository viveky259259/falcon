#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: update_homebrew_formula.sh \
  --version VERSION \
  --tag TAG \
  --source-repo OWNER/REPO \
  --checksums SHA256SUMS \
  --tap-dir PATH
USAGE
}

VERSION=""
TAG=""
SOURCE_REPO=""
CHECKSUMS=""
TAP_DIR=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --tag)
      TAG="${2:-}"
      shift 2
      ;;
    --source-repo)
      SOURCE_REPO="${2:-}"
      shift 2
      ;;
    --checksums)
      CHECKSUMS="${2:-}"
      shift 2
      ;;
    --tap-dir)
      TAP_DIR="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ -z "${VERSION}" ] ||
   [ -z "${TAG}" ] ||
   [ -z "${SOURCE_REPO}" ] ||
   [ -z "${CHECKSUMS}" ] ||
   [ -z "${TAP_DIR}" ]; then
  usage >&2
  exit 2
fi

if [ ! -f "${CHECKSUMS}" ]; then
  echo "checksums file not found: ${CHECKSUMS}" >&2
  exit 1
fi

arm_archive="falcon-${VERSION}-macos-arm64.tar.gz"
x64_archive="falcon-${VERSION}-macos-x64.tar.gz"

checksum_for() {
  archive="$1"
  awk -v archive="${archive}" '$2 == archive { print $1 }' "${CHECKSUMS}"
}

arm_sha="$(checksum_for "${arm_archive}")"
x64_sha="$(checksum_for "${x64_archive}")"

if [ -z "${arm_sha}" ]; then
  echo "missing SHA256SUMS entry for ${arm_archive}" >&2
  exit 1
fi

if [ -z "${x64_sha}" ]; then
  echo "missing SHA256SUMS entry for ${x64_archive}" >&2
  exit 1
fi

formula_dir="${TAP_DIR}/Formula"
formula="${formula_dir}/falcon.rb"
mkdir -p "${formula_dir}"

cat > "${formula}" <<FORMULA
class Falcon < Formula
  desc "Rust-powered static analysis for Flutter and Dart"
  homepage "https://github.com/${SOURCE_REPO}"
  version "${VERSION}"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/${SOURCE_REPO}/releases/download/${TAG}/${arm_archive}"
      sha256 "${arm_sha}"
    end

    on_intel do
      url "https://github.com/${SOURCE_REPO}/releases/download/${TAG}/${x64_archive}"
      sha256 "${x64_sha}"
    end
  end

  def install
    package_dir = Dir["falcon-#{version}-macos-*"].first
    bin.install "#{package_dir}/falcon"
    bin.install "#{package_dir}/falcon-lsp"
    bin.install "#{package_dir}/falcon-mcp"
    prefix.install "#{package_dir}/README.md" if File.exist?("#{package_dir}/README.md")
    prefix.install "#{package_dir}/LICENSE" if File.exist?("#{package_dir}/LICENSE")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/falcon --version")
    assert_match version.to_s, shell_output("#{bin}/falcon-lsp --version")
    assert_match version.to_s, shell_output("#{bin}/falcon-mcp --version")
  end
end
FORMULA

echo "wrote ${formula}"
