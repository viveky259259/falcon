#!/bin/sh
# Falcon installer.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/viveky259259/falcon/master/scripts/install.sh | sh
#
# Downloads the falcon / falcon-lsp / falcon-mcp binaries for your platform
# from a GitHub Release, verifies them against SHA256SUMS, and installs them
# to $FALCON_INSTALL_DIR (default: $HOME/.local/bin). No Rust toolchain, npm,
# or Homebrew required.
#
# Options (flags or matching env vars):
#   --version vX.Y.Z     FALCON_VERSION        Install a specific release (default: latest)
#   --install-dir DIR    FALCON_INSTALL_DIR    Where to put the binaries (default: $HOME/.local/bin)
#   --yes / -y            FALCON_INSTALL_YES=1  Don't prompt (accept defaults, e.g. the `ff` shortcut)
#   --no-shortcut                               Skip the `ff` shortcut prompt entirely
#   -h / --help
set -eu

REPO="${FALCON_RELEASE_REPO:-viveky259259/falcon}"
VERSION="${FALCON_VERSION:-}"
INSTALL_DIR="${FALCON_INSTALL_DIR:-"$HOME/.local/bin"}"
ASSUME_YES="${FALCON_INSTALL_YES:-}"
SKIP_SHORTCUT=""
BINARIES="falcon falcon-lsp falcon-mcp"

usage() {
  sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --install-dir)
      INSTALL_DIR="${2:-}"
      shift 2
      ;;
    -y|--yes)
      ASSUME_YES="1"
      shift
      ;;
    --no-shortcut)
      SKIP_SHORTCUT="1"
      shift
      ;;
    -h|--help)
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

info()  { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
warn()  { printf '\033[1;33mwarning:\033[0m %s\n' "$1" >&2; }
fail()  { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

detect_platform() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) plat_os="macos" ;;
    Linux) plat_os="linux" ;;
    *)
      fail "unsupported OS '$os'. Falcon's installer supports macOS and Linux; on Windows use WSL, or run 'cargo install falcon-flutter'."
      ;;
  esac

  case "$arch" in
    arm64|aarch64)
      if [ "$plat_os" = "linux" ]; then
        fail "linux-arm64 has no prebuilt release yet. Run 'cargo install falcon-flutter' (or 'cargo binstall falcon-flutter') instead."
      fi
      plat_arch="arm64"
      ;;
    x86_64|amd64) plat_arch="x64" ;;
    *)
      fail "unsupported architecture '$arch'. Run 'cargo install falcon-flutter' instead."
      ;;
  esac

  LABEL="${plat_os}-${plat_arch}"
}

resolve_version() {
  if [ -n "$VERSION" ]; then
    VERSION="${VERSION#v}"
    return
  fi

  info "Looking up the latest release..."
  api_response="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")" ||
    fail "could not reach GitHub to resolve the latest version. Pass --version vX.Y.Z to skip this lookup."

  VERSION="$(printf '%s\n' "$api_response" | grep '"tag_name"' | head -n1 | sed -E 's/.*"tag_name": *"v?([^"]+)".*/\1/')"
  [ -n "$VERSION" ] || fail "could not determine the latest version from the GitHub API response."
}

checksum_cmd() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1"
  else
    fail "need either sha256sum or shasum to verify the download."
  fi
}

download() {
  url="$1"
  dest="$2"
  info "Downloading $(basename "$dest")..."
  curl -fsSL "$url" -o "$dest" || fail "download failed: $url"
}

main() {
  detect_platform
  resolve_version

  tag="v${VERSION}"
  archive="falcon-${VERSION}-${LABEL}.tar.gz"
  base_url="https://github.com/${REPO}/releases/download/${tag}"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT

  download "${base_url}/${archive}" "${tmp_dir}/${archive}"
  download "${base_url}/SHA256SUMS" "${tmp_dir}/SHA256SUMS"

  info "Verifying checksum..."
  expected="$(grep " ${archive}\$" "${tmp_dir}/SHA256SUMS" | awk '{print $1}')"
  [ -n "$expected" ] || fail "SHA256SUMS in release ${tag} has no entry for ${archive}."
  actual="$(checksum_cmd "${tmp_dir}/${archive}" | awk '{print $1}')"
  [ "$expected" = "$actual" ] || fail "checksum mismatch for ${archive} (expected ${expected}, got ${actual})."

  info "Installing to ${INSTALL_DIR}..."
  mkdir -p "$INSTALL_DIR"
  tar -xzf "${tmp_dir}/${archive}" -C "$tmp_dir"
  staging="${tmp_dir}/falcon-${VERSION}-${LABEL}"

  for bin in $BINARIES; do
    [ -f "${staging}/${bin}" ] || fail "release archive did not contain ${bin}."
    cp "${staging}/${bin}" "${INSTALL_DIR}/${bin}"
    chmod +x "${INSTALL_DIR}/${bin}"
  done

  info "Installed falcon ${VERSION} to ${INSTALL_DIR}"
  offer_shortcut
  check_path
  "${INSTALL_DIR}/falcon" --version 2>/dev/null || true
}

# Prompts (when possible) to symlink `falcon` as the shorter `ff`, so
# `falcon run` becomes `ff run`. Requires an interactive terminal since stdin
# is normally consumed by the `curl | sh` pipe; falls back to /dev/tty.
offer_shortcut() {
  [ -z "$SKIP_SHORTCUT" ] || return 0

  if command -v ff >/dev/null 2>&1 && [ ! -L "${INSTALL_DIR}/ff" ]; then
    warn "'ff' is already on your PATH pointing elsewhere; skipping the shortcut."
    return 0
  fi

  if [ -n "$ASSUME_YES" ]; then
    reply="y"
  elif [ -r /dev/tty ]; then
    printf 'Add a short "ff" alias for falcon, so "falcon run" becomes "ff run"? [Y/n] '
    reply="n"
    { read -r reply < /dev/tty; } 2>/dev/null || reply="n"
  else
    return 0
  fi

  case "$reply" in
    ""|y|Y|yes|YES)
      ln -sf "${INSTALL_DIR}/falcon" "${INSTALL_DIR}/ff"
      info "Created shortcut: ff -> falcon (try 'ff run' or 'ff score .')"
      ;;
    *)
      ;;
  esac
}

check_path() {
  case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      warn "${INSTALL_DIR} is not on your PATH."
      echo "  Add it, e.g.: echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc"
      ;;
  esac
}

main
