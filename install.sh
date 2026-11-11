#!/bin/sh
set -eu

REPO="${GITHUB_REPO:-hexennacht/envault}"
VERSION="${ENVAULT_VERSION:-${VERSION:-latest}}"
CUSTOM_BIN_DIR=""

log() {
  printf '%s\n' "$*"
}

error() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

print_help() {
  cat << 'EOF'
Install the envault binary from a GitHub release.

Usage:
  install.sh [options]

Options:
  -b, --bin-dir <dir>   Set the target installation directory
  -v, --version <ver>   Set the release version to install
  -h, --help            Show this help message
EOF
}

detect_os() {
  case "$(uname -s)" in
    Darwin)
      printf 'apple-darwin'
      ;;
    Linux)
      printf 'unknown-linux-gnu'
      ;;
    *)
      error "Unsupported operating system: $(uname -s)"
      ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)
      printf 'x86_64'
      ;;
    arm64|aarch64)
      printf 'aarch64'
      ;;
    *)
      error "Unsupported architecture: $(uname -m)"
      ;;
  esac
}

select_bin_dir() {
  if [ -n "$CUSTOM_BIN_DIR" ]; then
    printf '%s' "$CUSTOM_BIN_DIR"
  elif [ -n "${INSTALL_DIR:-}" ]; then
    printf '%s' "$INSTALL_DIR"
  elif [ -n "${BIN_DIR:-}" ]; then
    printf '%s' "$BIN_DIR"
  elif [ -w "/usr/local/bin" ]; then
    printf '/usr/local/bin'
  else
    printf '%s' "${XDG_BIN_HOME:-$HOME/.local/bin}"
  fi
}

resolve_tag() {
  target_version="$1"
  if [ "$target_version" = "latest" ]; then
    url="https://github.com/${REPO}/releases/latest"
    effective_url=$(curl -fsSL -o /dev/null -w "%{url_effective}" "$url" 2>/dev/null || true)
    tag="${effective_url##*/}"
    if [ -z "$tag" ] || [ "$tag" = "latest" ]; then
      error "Failed to resolve latest release tag from GitHub"
    fi
    printf '%s' "$tag"
  else
    case "$target_version" in
      v*)
        printf '%s' "$target_version"
        ;;
      *)
        printf 'v%s' "$target_version"
        ;;
    esac
  fi
}

verify_checksum() {
  dir="$1"
  checksum_file="$2"

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$dir" && sha256sum -c "$checksum_file" >/dev/null)
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$dir" && shasum -a 256 -c "$checksum_file" >/dev/null)
  else
    log "Warning: sha256sum and shasum not found. Skipped checksum verification."
  fi
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      -b|--bin-dir)
        [ $# -ge 2 ] || error "Option $1 requires an argument"
        CUSTOM_BIN_DIR="$2"
        shift 2
        ;;
      -v|--version)
        [ $# -ge 2 ] || error "Option $1 requires an argument"
        VERSION="$2"
        shift 2
        ;;
      -h|--help)
        print_help
        exit 0
        ;;
      *)
        error "Unknown option: $1"
        ;;
    esac
  done

  if ! command -v curl >/dev/null 2>&1; then
    error "curl is required to install envault"
  fi

  if ! command -v tar >/dev/null 2>&1; then
    error "tar is required to install envault"
  fi

  OS=$(detect_os)
  ARCH=$(detect_arch)
  TARGET="${ARCH}-${OS}"
  TAG=$(resolve_tag "$VERSION")
  TARGET_BIN_DIR=$(select_bin_dir)

  ARCHIVE="envault-${TAG}-${TARGET}.tar.gz"
  CHECKSUM="envault-${TAG}-${TARGET}.sha256"
  ARCHIVE_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"
  CHECKSUM_URL="https://github.com/${REPO}/releases/download/${TAG}/${CHECKSUM}"

  log "Installing envault ${TAG} (${TARGET})..."

  TMP_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t 'envault')
  trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

  log "Downloading ${ARCHIVE}..."
  curl -fsSL "$ARCHIVE_URL" -o "${TMP_DIR}/${ARCHIVE}"
  curl -fsSL "$CHECKSUM_URL" -o "${TMP_DIR}/${CHECKSUM}"

  log "Verifying checksum..."
  verify_checksum "$TMP_DIR" "$CHECKSUM"

  log "Extracting binary..."
  tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "$TMP_DIR"

  if [ ! -f "${TMP_DIR}/envault" ]; then
    error "Archive did not contain the envault binary"
  fi

  mkdir -p "$TARGET_BIN_DIR"
  cp "${TMP_DIR}/envault" "${TARGET_BIN_DIR}/envault"
  chmod 755 "${TARGET_BIN_DIR}/envault"

  log "Installed envault to ${TARGET_BIN_DIR}/envault"

  case ":$PATH:" in
    *":$TARGET_BIN_DIR:"*) ;;
    *)
      log ""
      log "Notice: ${TARGET_BIN_DIR} is not in your PATH."
      log "Add it to your shell configuration:"
      log "  export PATH=\"${TARGET_BIN_DIR}:\$PATH\""
      ;;
  esac
}

main "$@"
