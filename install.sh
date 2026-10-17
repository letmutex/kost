#!/bin/sh
set -eu

# Kost installer for Unix-like systems (Linux, macOS)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/letmutex/kost/main/install.sh | sh
#   or with options:
#   curl -fsSL https://raw.githubusercontent.com/letmutex/kost/main/install.sh | sh -s -- --dir /custom/path

REPO="${KOST_REPO:-letmutex/kost}"
BIN_NAME="kost"
TAG="${KOST_VERSION:-}"
FORCE="${KOST_FORCE:-0}"

# 1. Detect OS and architecture first to establish platform defaults
OS="$(uname -s)"
case "$OS" in
  Linux*)
    OS_NAME="linux"
    EXT="tar.gz"
    DEFAULT_INSTALL_DIR="${KOST_INSTALL_DIR:-$HOME/.kost/bin}"
    ;;
  Darwin*)
    OS_NAME="macos"
    EXT="tar.gz"
    DEFAULT_INSTALL_DIR="${KOST_INSTALL_DIR:-$HOME/.kost/bin}"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    OS_NAME="windows"
    EXT="zip"
    BIN_NAME="kost.exe"
    if [ -n "${KOST_INSTALL_DIR:-}" ]; then
      DEFAULT_INSTALL_DIR="$KOST_INSTALL_DIR"
    elif [ -n "${LOCALAPPDATA:-}" ]; then
      if command -v cygpath >/dev/null 2>&1; then
        DEFAULT_INSTALL_DIR="$(cygpath -u "$LOCALAPPDATA")/kost/bin"
      else
        DEFAULT_INSTALL_DIR="$HOME/.kost/bin"
      fi
    else
      DEFAULT_INSTALL_DIR="$HOME/.kost/bin"
    fi
    ;;
  *)
    echo "Error: Unsupported operating system: $OS" >&2
    exit 1
    ;;
esac

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)
    ARCH_NAME="x64"
    ;;
  aarch64|arm64)
    ARCH_NAME="arm64"
    ;;
  *)
    echo "Error: Unsupported system architecture: $ARCH" >&2
    exit 1
    ;;
esac

INSTALL_DIR="$DEFAULT_INSTALL_DIR"

# 2. Parse command-line arguments (explicit arguments naturally override defaults)
while [ $# -gt 0 ]; do
  case "$1" in
    -v|--version)
      [ $# -ge 2 ] || { echo "Error: $1 requires an argument." >&2; exit 1; }
      TAG="$2"
      shift 2
      ;;
    --version=*)
      TAG="${1#*=}"
      shift
      ;;
    -d|--dir)
      [ $# -ge 2 ] || { echo "Error: $1 requires an argument." >&2; exit 1; }
      INSTALL_DIR="$2"
      shift 2
      ;;
    --dir=*)
      INSTALL_DIR="${1#*=}"
      shift
      ;;
    -f|--force)
      FORCE=1
      shift
      ;;
    -h|--help)
      echo "Kost Installer"
      echo ""
      echo "Usage: install.sh [options]"
      echo ""
      echo "Options:"
      echo "  -v, --version <tag>  Install specific version (e.g. v0.1.0)"
      echo "  -d, --dir <path>     Installation directory (default: ~/.kost/bin)"
      echo "  -f, --force          Force install even if the latest version is already installed"
      echo "  -h, --help           Display this help message"
      exit 0
      ;;
    *)
      echo "Error: Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if command -v cygpath >/dev/null 2>&1; then
  INSTALL_DIR="$(cygpath -u "$INSTALL_DIR")"
fi
INSTALL_DIR="${INSTALL_DIR%/}"

# 3. HTTP helper functions (no authentication headers)
http_get() {
  url="$1"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$url"
  else
    echo "Error: Neither curl nor wget was found. Please install curl or wget." >&2
    exit 1
  fi
}

download_file() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url"
  else
    echo "Error: Neither curl nor wget was found. Please install curl or wget." >&2
    exit 1
  fi
}

# 4. Fetch latest release version if not specified
if [ -z "$TAG" ]; then
  echo "Fetching latest release information for ${REPO}..."
  LATEST_JSON="$(http_get "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)"
  if [ -n "$LATEST_JSON" ]; then
    TAG="$(echo "$LATEST_JSON" | sed -n -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/p' | head -n 1)"
  fi

  # Fallback to redirect URL if GitHub API is rate-limited or fails
  if [ -z "$TAG" ]; then
    if command -v curl >/dev/null 2>&1; then
      REDIRECT_URL="$(curl -sIL -o /dev/null -w "%{url_effective}" "https://github.com/${REPO}/releases/latest" 2>/dev/null || true)"
      TAG="${REDIRECT_URL##*/}"
    elif command -v wget >/dev/null 2>&1; then
      TAG="$(wget --max-redirect=0 -S "https://github.com/${REPO}/releases/latest" 2>&1 | awk -F'/' '/^[[:space:]]*[Ll]ocation:/{print $NF}' | tr -d '\r\n')"
    fi
  fi

  if [ -z "$TAG" ] || [ "$TAG" = "latest" ]; then
    echo "Error: Unable to determine latest release tag for ${REPO}." >&2
    exit 1
  fi
fi

ASSET_SUFFIX="${OS_NAME}-${ARCH_NAME}.${EXT}"
ARCHIVE_NAME="kost-${TAG}-${ASSET_SUFFIX}"
HASH_NAME="${ARCHIVE_NAME}.sha256"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE_NAME}"
HASH_URL="https://github.com/${REPO}/releases/download/${TAG}/${HASH_NAME}"

# 5. Check local and remote version, skip if latest version is installed
EXISTING_KOST=""
if [ -x "${INSTALL_DIR}/${BIN_NAME}" ]; then
  EXISTING_KOST="${INSTALL_DIR}/${BIN_NAME}"
elif command -v "$BIN_NAME" >/dev/null 2>&1; then
  EXISTING_KOST="$(command -v "$BIN_NAME")"
fi

IS_UPDATE=0
LOCAL_VER=""

if [ -n "$EXISTING_KOST" ]; then
  LOCAL_VER_RAW="$("$EXISTING_KOST" --version 2>/dev/null || true)"
  LOCAL_VER="$(echo "$LOCAL_VER_RAW" | awk '{print $NF}' | sed 's/^v//')"
  if [ -n "$LOCAL_VER" ]; then
    IS_UPDATE=1
  fi
fi

CLEAN_LOCAL="$(echo "$LOCAL_VER" | sed 's/^v//')"
CLEAN_REMOTE="$(echo "$TAG" | sed 's/^v//')"

if [ -n "$CLEAN_LOCAL" ] && [ "$CLEAN_LOCAL" = "$CLEAN_REMOTE" ] && [ "$FORCE" != "1" ]; then
  echo "kost is already up to date (${TAG}). Skipping installation."
  exit 0
fi

# 6. Setup temporary directory with guaranteed cleanup trap
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'kost-install')"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

# 7. Download archive and verify hash
echo "Downloading ${ARCHIVE_NAME}..."
download_file "$DOWNLOAD_URL" "$TMP_DIR/$ARCHIVE_NAME"

echo "Downloading ${HASH_NAME}..."
download_file "$HASH_URL" "$TMP_DIR/$HASH_NAME"

# Read expected hash (first field of .sha256 file)
EXPECTED_HASH="$(awk '{print $1}' "$TMP_DIR/$HASH_NAME" | tr -d '\r\n' | tr '[:upper:]' '[:lower:]')"

compute_sha256() {
  target_file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$target_file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$target_file" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$target_file" | awk '{print $NF}'
  else
    echo "Error: Neither sha256sum, shasum, nor openssl found to compute SHA256 checksum." >&2
    exit 1
  fi
}

echo "Verifying SHA256 checksum..."
ACTUAL_HASH="$(compute_sha256 "$TMP_DIR/$ARCHIVE_NAME" | tr -d '\r\n' | tr '[:upper:]' '[:lower:]')"

if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
  echo "Error: SHA256 checksum verification failed!" >&2
  echo "Expected: $EXPECTED_HASH" >&2
  echo "Actual:   $ACTUAL_HASH" >&2
  exit 1
fi
echo "Checksum verified successfully."

# 8. Extract to install
echo "Extracting archive..."
case "$ARCHIVE_NAME" in
  *.tar.gz)
    tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"
    ;;
  *.zip)
    if command -v unzip >/dev/null 2>&1; then
      unzip -q -o "$TMP_DIR/$ARCHIVE_NAME" -d "$TMP_DIR"
    elif command -v 7z >/dev/null 2>&1; then
      7z x -y "$TMP_DIR/$ARCHIVE_NAME" -o"$TMP_DIR" >/dev/null
    else
      tar -xf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"
    fi
    ;;
esac

EXTRACTED_BIN="$TMP_DIR/$BIN_NAME"
if [ ! -f "$EXTRACTED_BIN" ]; then
  FOUND_BIN="$(find "$TMP_DIR" -name "$BIN_NAME" -type f | head -n 1)"
  if [ -n "$FOUND_BIN" ]; then
    EXTRACTED_BIN="$FOUND_BIN"
  else
    echo "Error: Binary '$BIN_NAME' not found in extracted archive." >&2
    exit 1
  fi
fi

mkdir -p "$INSTALL_DIR"

# Atomic install to prevent ETXTBUSY if kost is currently running
TMP_INSTALL_BIN="$INSTALL_DIR/${BIN_NAME}.tmp.$$"
cp -f "$EXTRACTED_BIN" "$TMP_INSTALL_BIN"
chmod +x "$TMP_INSTALL_BIN"
mv -f "$TMP_INSTALL_BIN" "$INSTALL_DIR/$BIN_NAME"

# 9. Check PATH and print add install path to path env or update success message
INSTALL_DIR_CLEAN="${INSTALL_DIR%/}"

in_path() {
  case ":$PATH:" in
    *":$1:"*) return 0 ;;
    *) return 1 ;;
  esac
}

if in_path "$INSTALL_DIR_CLEAN"; then
  if [ "$IS_UPDATE" = "1" ]; then
    echo "Successfully updated kost to ${TAG}!"
  else
    echo "Successfully installed kost ${TAG} to ${INSTALL_DIR_CLEAN}/${BIN_NAME}!"
  fi
  echo "Run 'kost --help' to get started."
else
  if [ "$IS_UPDATE" = "1" ]; then
    echo "Successfully updated kost to ${TAG}!"
  else
    echo "Successfully installed kost ${TAG} to ${INSTALL_DIR_CLEAN}/${BIN_NAME}!"
  fi
  echo ""
  echo "NOTICE: '${INSTALL_DIR_CLEAN}' is not in your PATH environment variable."
  echo "To run kost from anywhere, add it to your PATH:"
  echo ""
  case "${SHELL:-}" in
    */zsh)
      echo "  echo 'export PATH=\"${INSTALL_DIR_CLEAN}:\$PATH\"' >> ~/.zshrc"
      echo "  source ~/.zshrc"
      ;;
    */fish)
      echo "  fish_add_path \"${INSTALL_DIR_CLEAN}\""
      ;;
    *)
      echo "  echo 'export PATH=\"${INSTALL_DIR_CLEAN}:\$PATH\"' >> ~/.bashrc"
      echo "  source ~/.bashrc"
      ;;
  esac
  echo ""
  echo "Or for the current terminal session only:"
  echo "  export PATH=\"${INSTALL_DIR_CLEAN}:\$PATH\""
  echo ""
fi
