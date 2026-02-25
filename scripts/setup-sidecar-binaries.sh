#!/usr/bin/env bash
# @awa-impl: PLAN-006-1.2
# Copy sidecar binaries to the Tauri binaries/ directory with the correct
# platform triple suffix. Tauri externalBin expects this naming convention.
#
# Usage:
#   scripts/setup-sidecar-binaries.sh           # debug (default)
#   scripts/setup-sidecar-binaries.sh release    # release

set -euo pipefail

PROFILE="${1:-debug}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARIES_DIR="$REPO_ROOT/crates/app/nize_desktop/binaries"

# Determine the current target triple.
TRIPLE="$(rustc -vV | grep '^host:' | cut -d' ' -f2)"

# Source directory for built binaries.
if [ "$PROFILE" = "release" ]; then
  SRC_DIR="$REPO_ROOT/target/release"
else
  SRC_DIR="$REPO_ROOT/target/debug"
fi

mkdir -p "$BINARIES_DIR"

# Sidecar binaries to copy.
SIDECARS="nize_desktop_server nize_terminator"

for BIN in $SIDECARS; do
  SRC="$SRC_DIR/$BIN"
  # On Windows, binaries have .exe suffix.
  if [ -f "$SRC.exe" ]; then
    SRC="$SRC.exe"
    DEST="$BINARIES_DIR/${BIN}-${TRIPLE}.exe"
  else
    DEST="$BINARIES_DIR/${BIN}-${TRIPLE}"
  fi

  if [ ! -f "$SRC" ]; then
    echo "ERROR: $SRC not found. Build $BIN first." >&2
    exit 1
  fi

  cp "$SRC" "$DEST"
  echo "setup-sidecar-binaries: $SRC → $DEST"
done

# @awa-impl: PLAN-016-2.3
# Copy bundled Bun binary if it exists (downloaded by scripts/download-bun.sh).
# In dev mode, create a symlink to the system Bun binary.
if [ -f "$BINARIES_DIR/bun-${TRIPLE}" ] || [ -f "$BINARIES_DIR/bun-${TRIPLE}.exe" ]; then
  echo "setup-sidecar-binaries: bun-${TRIPLE} already in place"
else
  # Dev mode: symlink system bun so Tauri externalBin validation passes.
  BUN_PATH="$(which bun 2>/dev/null || true)"
  if [ -n "$BUN_PATH" ]; then
    ln -sf "$BUN_PATH" "$BINARIES_DIR/bun-${TRIPLE}"
    echo "setup-sidecar-binaries: symlinked bun-${TRIPLE} → $BUN_PATH (dev mode)"
  else
    echo "WARNING: bun not found on PATH and no bundled binary available" >&2
  fi
fi
