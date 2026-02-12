#!/usr/bin/env bash
set -euo pipefail

# Build WASM bindings for Nize.
#
# Produces one target:
#   wasm/  – Node.js (CommonJS, synchronous instantiation)
#
# Optimization pipeline:
#   1. wasm-pack builds with --release (using profile.release settings)
#   2. wasm-opt -Oz shrinks the binary further
#
# wasm-pack drops a .gitignore containing "*" in each output directory,
# which causes npm pack to exclude the files even when they are listed in
# the package.json "files" array.  We remove those after every build.

CRATE=../../crates/wasm/nize_wasm
PKG_DIR=$(cd "$(dirname "$0")/.." && pwd)
WORKSPACE_ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
CRATE_ABS="$WORKSPACE_ROOT/crates/wasm/nize_wasm"

# Pinned wasm-opt version — change here to upgrade everywhere (dev + CI)
WASM_OPT_VERSION="0.116.1"
TOOLS_DIR="$WORKSPACE_ROOT/target/tools"
WASM_OPT="$TOOLS_DIR/bin/wasm-opt"

# ---------------------------------------------------------------------------
# Ensure wasm-opt is installed locally at the pinned version
# ---------------------------------------------------------------------------

ensure_wasm_opt() {
    cargo install wasm-opt \
        --version "$WASM_OPT_VERSION" \
        --root "$TOOLS_DIR" \
        --quiet
}

ensure_wasm_opt

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

echo "Building wasm (nodejs)…"
wasm-pack build "$CRATE_ABS" --release --target nodejs --out-dir "$PKG_DIR/wasm"
rm -f "$PKG_DIR/wasm/.gitignore"

# ---------------------------------------------------------------------------
# wasm-opt pass
# ---------------------------------------------------------------------------

echo "Running wasm-opt -Oz…"
for wasm_file in "$PKG_DIR/wasm/"*.wasm; do
    [ -f "$wasm_file" ] || continue
    "$WASM_OPT" -Oz --enable-bulk-memory "$wasm_file" -o "$wasm_file"
done

# ---------------------------------------------------------------------------
# Size report
# ---------------------------------------------------------------------------

echo ""
echo "═══════════════════════════════════════════"
echo " WASM Binary Size Report"
echo "═══════════════════════════════════════════"

report_size() {
    local label="$1" file="$2"
    if [ ! -f "$file" ]; then return; fi
    local raw_bytes gz_bytes
    raw_bytes=$(wc -c < "$file" | tr -d ' ')
    gz_bytes=$(gzip -c "$file" | wc -c | tr -d ' ')
    local raw_kb gz_kb
    raw_kb=$(awk "BEGIN { printf \"%.1f\", $raw_bytes / 1024 }")
    gz_kb=$(awk "BEGIN { printf \"%.1f\", $gz_bytes / 1024 }")
    printf "  %-10s %6s KB  (gzip %s KB)\n" "$label" "$raw_kb" "$gz_kb"
}

for wasm_file in "$PKG_DIR/wasm/"*.wasm; do
    [ -f "$wasm_file" ] || continue
    report_size "nodejs" "$wasm_file"
done

echo "═══════════════════════════════════════════"
