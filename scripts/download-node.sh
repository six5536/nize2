#!/usr/bin/env bash
# @zen-impl: PLAN-007-2.1
# Download a platform-specific Node.js binary for bundling inside the Tauri app.
#
# Usage:
#   scripts/download-node.sh <platform>
#
# Platforms:
#   macos-arm64      — macOS Apple Silicon
#   linux-x86_64     — Linux x86_64
#   windows-x86_64   — Windows x86_64
#
# Output: crates/app/nize_desktop/binaries/node-{triple}

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=node-version.env
source "$SCRIPT_DIR/node-version.env"

PLATFORM="${1:?Usage: download-node.sh <macos-arm64|linux-x86_64|windows-x86_64>}"
BINARIES_DIR="$REPO_ROOT/crates/app/nize_desktop/binaries"
CACHE_DIR="${NODE_CACHE_DIR:-$REPO_ROOT/.cache/node}"
TEMP_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Map platform to Node.js download filename and target triple.
case "$PLATFORM" in
    macos-arm64)
        NODE_ARCH="darwin-arm64"
        NODE_FILE="node-v${NODE_VERSION}-darwin-arm64.tar.gz"
        TRIPLE="aarch64-apple-darwin"
        NODE_BIN_NAME="node"
        ;;
    linux-x86_64)
        NODE_ARCH="linux-x64"
        NODE_FILE="node-v${NODE_VERSION}-linux-x64.tar.xz"
        TRIPLE="x86_64-unknown-linux-gnu"
        NODE_BIN_NAME="node"
        ;;
    windows-x86_64)
        NODE_ARCH="win-x64"
        NODE_FILE="node-v${NODE_VERSION}-win-x64.zip"
        TRIPLE="x86_64-pc-windows-msvc"
        NODE_BIN_NAME="node.exe"
        ;;
    *)
        echo "ERROR: Unknown platform '$PLATFORM'. Use: macos-arm64, linux-x86_64, windows-x86_64" >&2
        exit 1
        ;;
esac

DOWNLOAD_URL="https://nodejs.org/dist/v${NODE_VERSION}/${NODE_FILE}"

# ── Download (with caching) ─────────────────────────────────────────
mkdir -p "$CACHE_DIR"
CACHED_FILE="$CACHE_DIR/$NODE_FILE"

if [ -f "$CACHED_FILE" ]; then
    echo "download-node: Using cached $CACHED_FILE"
else
    echo "download-node: Downloading $DOWNLOAD_URL ..."
    curl -fSL -o "$CACHED_FILE" "$DOWNLOAD_URL"
fi

# ── Extract just the node binary ─────────────────────────────────────
echo "download-node: Extracting node binary..."
case "$NODE_FILE" in
    *.tar.gz)
        tar -xzf "$CACHED_FILE" -C "$TEMP_DIR" --strip-components=2 "node-v${NODE_VERSION}-${NODE_ARCH}/bin/node"
        ;;
    *.tar.xz)
        tar -xJf "$CACHED_FILE" -C "$TEMP_DIR" --strip-components=2 "node-v${NODE_VERSION}-${NODE_ARCH}/bin/node"
        ;;
    *.zip)
        unzip -q "$CACHED_FILE" "node-v${NODE_VERSION}-${NODE_ARCH}/${NODE_BIN_NAME}" -d "$TEMP_DIR"
        mv "$TEMP_DIR/node-v${NODE_VERSION}-${NODE_ARCH}/${NODE_BIN_NAME}" "$TEMP_DIR/${NODE_BIN_NAME}"
        ;;
esac

# ── Copy to binaries directory ───────────────────────────────────────
mkdir -p "$BINARIES_DIR"

if [ "$PLATFORM" = "windows-x86_64" ]; then
    DEST="$BINARIES_DIR/node-${TRIPLE}.exe"
else
    DEST="$BINARIES_DIR/node-${TRIPLE}"
fi

cp "$TEMP_DIR/$NODE_BIN_NAME" "$DEST"
chmod +x "$DEST"

# ── Size report ──────────────────────────────────────────────────────
SIZE=$(du -sh "$DEST" | cut -f1)
echo "download-node: Done. $DEST ($SIZE)"
