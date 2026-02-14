#!/usr/bin/env bash
# @zen-impl: PLAN-016-2.1
# Download a platform-specific Bun binary for bundling inside the Tauri app.
#
# Usage:
#   scripts/download-bun.sh <platform>
#
# Platforms:
#   macos-arm64      — macOS Apple Silicon
#   linux-x86_64     — Linux x86_64
#   windows-x86_64   — Windows x86_64
#
# Output: crates/app/nize_desktop/binaries/bun-{triple}

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=bun-version.env
source "$SCRIPT_DIR/bun-version.env"

PLATFORM="${1:?Usage: download-bun.sh <macos-arm64|linux-x86_64|windows-x86_64>}"
BINARIES_DIR="$REPO_ROOT/crates/app/nize_desktop/binaries"
CACHE_DIR="${BUN_CACHE_DIR:-$REPO_ROOT/.cache/bun}"
TEMP_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Map platform to Bun download filename and target triple.
case "$PLATFORM" in
    macos-arm64)
        BUN_ARCHIVE="bun-darwin-aarch64.zip"
        BUN_DIR="bun-darwin-aarch64"
        TRIPLE="aarch64-apple-darwin"
        BUN_BIN_NAME="bun"
        ;;
    linux-x86_64)
        BUN_ARCHIVE="bun-linux-x64.zip"
        BUN_DIR="bun-linux-x64"
        TRIPLE="x86_64-unknown-linux-gnu"
        BUN_BIN_NAME="bun"
        ;;
    windows-x86_64)
        BUN_ARCHIVE="bun-windows-x64.zip"
        BUN_DIR="bun-windows-x64"
        TRIPLE="x86_64-pc-windows-msvc"
        BUN_BIN_NAME="bun.exe"
        ;;
    *)
        echo "ERROR: Unknown platform '$PLATFORM'. Use: macos-arm64, linux-x86_64, windows-x86_64" >&2
        exit 1
        ;;
esac

DOWNLOAD_URL="https://github.com/oven-sh/bun/releases/download/bun-v${BUN_VERSION}/${BUN_ARCHIVE}"

# ── Download (with caching) ─────────────────────────────────────────
mkdir -p "$CACHE_DIR"
CACHED_FILE="$CACHE_DIR/$BUN_ARCHIVE"

if [ -f "$CACHED_FILE" ]; then
    echo "download-bun: Using cached $CACHED_FILE"
else
    echo "download-bun: Downloading $DOWNLOAD_URL ..."
    curl -fSL -o "$CACHED_FILE" "$DOWNLOAD_URL"
fi

# ── Extract just the bun binary ──────────────────────────────────────
echo "download-bun: Extracting bun binary..."
unzip -q "$CACHED_FILE" "${BUN_DIR}/${BUN_BIN_NAME}" -d "$TEMP_DIR"
mv "$TEMP_DIR/${BUN_DIR}/${BUN_BIN_NAME}" "$TEMP_DIR/${BUN_BIN_NAME}"

# ── Copy to binaries directory ───────────────────────────────────────
mkdir -p "$BINARIES_DIR"

if [ "$PLATFORM" = "windows-x86_64" ]; then
    DEST="$BINARIES_DIR/bun-${TRIPLE}.exe"
else
    DEST="$BINARIES_DIR/bun-${TRIPLE}"
fi

cp "$TEMP_DIR/$BUN_BIN_NAME" "$DEST"
chmod +x "$DEST"

# ── Size report ──────────────────────────────────────────────────────
SIZE=$(du -sh "$DEST" | cut -f1)
echo "download-bun: Done. $DEST ($SIZE)"
