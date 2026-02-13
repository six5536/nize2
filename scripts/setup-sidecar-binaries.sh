#!/usr/bin/env bash
# @zen-impl: PLAN-006-1.2
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
SIDECARS="nize_api_server nize_terminator"

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

# @zen-impl: PLAN-007-2.3
# Copy bundled Node.js binary if it exists (downloaded by scripts/download-node.sh).
# In dev mode, create a symlink to the system Node.js binary.
if [ -f "$BINARIES_DIR/node-${TRIPLE}" ] || [ -f "$BINARIES_DIR/node-${TRIPLE}.exe" ]; then
  echo "setup-sidecar-binaries: node-${TRIPLE} already in place"
else
  # Dev mode: symlink system node so Tauri externalBin validation passes.
  NODE_PATH="$(which node 2>/dev/null || true)"
  if [ -n "$NODE_PATH" ]; then
    ln -sf "$NODE_PATH" "$BINARIES_DIR/node-${TRIPLE}"
    echo "setup-sidecar-binaries: symlinked node-${TRIPLE} → $NODE_PATH (dev mode)"
  else
    echo "WARNING: node not found on PATH and no bundled binary available" >&2
  fi
fi
