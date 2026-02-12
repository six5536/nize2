#!/usr/bin/env bash
# Full API code generation pipeline:
#   1. TypeSpec compile → OpenAPI YAML
#   2. YAML → JSON (for swagger docs / client codegen)
#   3. nize-codegen → Rust models + route constants
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

log() { echo "[generate-api] $*"; }

# --- Step 1: TypeSpec Compile ---
log "Compiling TypeSpec..."
npx tsp compile "$ROOT_DIR/.zen/specs/API-NIZE-index.tsp"
log "TypeSpec compilation complete."

# --- Step 2: YAML → JSON ---
log "Converting OpenAPI YAML to JSON..."
node "$SCRIPT_DIR/generate-openapi-json.js"
log "JSON conversion complete."

# --- Step 3: nize-codegen → Rust ---
log "Generating Rust models via nize-codegen..."
cargo run -p nize_codegen
log "Pipeline complete."
