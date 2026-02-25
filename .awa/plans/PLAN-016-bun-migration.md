# PLAN-016: Replace Node.js with Bun

- **Status**: in-progress
- **Workflow direction**: lateral
- **Traceability**: PLAN-007 (PGlite sidecar), PLAN-011 (MCP client config), PLAN-012 (nize-web sidecar)

## Motivation

Node.js is used as a runtime for three bundled sidecars (PGlite, nize-web, mcp-remote) and as the dev-time package manager / build runner. Bun is a drop-in replacement that offers:

- **Smaller binary** (~55 MB vs ~110 MB Node.js) — reduces app bundle size
- **Faster startup** — improves sidecar cold-start latency
- **Built-in bundler** — can replace esbuild for PGlite/mcp-remote bundling
- **Native TypeScript** — no tsc step for scripts
- **Built-in package manager** — replaces npm, faster installs
- **Built-in test runner** — can replace vitest/jest if needed later

## Current Node.js Usage Inventory

### A. Bundled Sidecar Runtime (shipped in app)

| Use | Binary | Script | Source |
|-----|--------|--------|--------|
| PGlite server | `binaries/node-{triple}` | `pglite-server.mjs` | `nize_core::db::PgLiteManager::start()` |
| nize-web server | `binaries/node-{triple}` | `nize-web-server.mjs` | `nize_desktop::start_nize_web_sidecar()` |
| mcp-remote bridge | `binaries/node-{triple}` | `mcp-remote.mjs` | `nize_desktop::mcp_clients::configure_claude_desktop()` |

Rust code references: `nize_desktop/src/lib.rs`, `nize_desktop/src/mcp_clients.rs`, `nize_core/src/db.rs`, `nize_core/src/node_sidecar.rs`

### B. Build-Time / Dev-Time Runtime

| Use | Tool | Config |
|-----|------|--------|
| Package management | `npm` | `package.json`, `package-lock.json` |
| Vite dev server | `npm run dev` | `packages/nize-desktop/vite.config.ts` |
| TypeSpec compilation | `npm run generate:api` | `scripts/generate-api.sh` |
| PGlite bundle build | `node scripts/build-pglite-server.mjs` | esbuild |
| mcp-remote bundle build | `node scripts/build-mcp-remote.mjs` | esbuild |
| nize-web production build | `node scripts/build-nize-web.mjs` | Next.js |
| nize-cli WASM build | `npm run build:wasm && npm run build:ts` | tsup |
| OpenAPI codegen | `node scripts/generate-openapi-json.js` | js-yaml |
| Tauri dev command | `npm run --prefix packages/nize-desktop dev` | `tauri.conf.json` |
| Tauri build command | `npm run --prefix packages/nize-desktop build` | `tauri.conf.json` |

### C. Download & Packaging Infrastructure

| File | Purpose |
|------|---------|
| `scripts/download-node.sh` | Downloads platform-specific Node.js binary |
| `scripts/node-version.env` | Pins Node.js version (24.3.0) |
| `scripts/setup-sidecar-binaries.sh` | Symlinks system node in dev; copies bundled binary |
| `tauri.conf.json` `externalBin` | Declares `binaries/node` as bundled binary |

### D. Rust Code References to "node"

| File | Reference |
|------|-----------|
| `nize_core/src/node_sidecar.rs` | `Command::new("node")`, `NodeInfo`, `check_node_available()` |
| `nize_core/src/db.rs` | `PgLiteManager::start(node_bin, ...)` |
| `nize_desktop/src/lib.rs` | `exe_dir.join("node")`, passed to PGlite & nize-web spawners |
| `nize_desktop/src/mcp_clients.rs` | `sidecar_node_path()`, `"command": "node"` in Claude config |

## Compatibility Assessment

### PGlite (`@electric-sql/pglite` + `pglite-socket`)

- PGlite uses native Node.js addons (N-API) for its socket layer
- **Risk**: Bun's N-API compatibility is mature but must be tested
- **Mitigation**: PGlite v0.3.x is pure JS/WASM; `pglite-socket` uses `net` module — both should work

### Next.js (nize-web)

- Next.js 16 standalone server uses `node:` built-ins heavily
- **Risk**: Bun's Next.js support is functional but may have edge cases
- **Mitigation**: nize-web is a simple hello-world app; test early. If issues arise, keep Node.js for nize-web only

### mcp-remote

- Pure JS/TS package, uses `node:net`, `node:http`
- **Risk**: Low — these are well-supported in Bun
- **Mitigation**: Test the bundled mcp-remote.mjs with Bun runtime

### esbuild (build-time)

- Used for PGlite and mcp-remote bundling
- **Risk**: None — Bun has a built-in bundler, or can run esbuild as-is
- Can migrate to `Bun.build()` later; not required for initial migration

### TypeSpec / Vite / tsup (build-time)

- All are npm packages run via CLI
- **Risk**: Low — Bun runs npm packages; `bun run` replaces `npm run`
- Vite specifically supports Bun

## Migration Plan

### Phase 1: Build-Time Migration (dev workflow)

Replace `npm` with `bun` for package management and script running. No changes to bundled runtime yet.

#### 1.1 — Install Bun in dev environment

- Add `bun` to mise/tool-versions configuration
- Pin version: `bun@1.3.9`

#### 1.2 — Replace npm with bun for package management

- Run `bun install` to generate `bun.lockb` (replaces `package-lock.json`)
- Keep `package-lock.json` temporarily until validated
- Update root `package.json` scripts if needed

#### 1.3 — Update Tauri commands to use bun

- `tauri.conf.json` `beforeDevCommand`: replace `npm run` with `bun run`
- `tauri.conf.json` `beforeBuildCommand`: replace `npm run` with `bun run`

#### 1.4 — Update build scripts

- `scripts/generate-api.sh`: replace `npx` with `bunx`
- Build scripts (`build-pglite-server.mjs`, etc.): test with `bun` runner

#### 1.5 — Validate build-time migration

- `cargo tauri dev` works with Bun
- `bun run generate:api` produces correct output
- All packages build successfully

### Phase 2: Sidecar Runtime Migration (bundled binary)

Replace the bundled Node.js binary with Bun binary for running sidecars.

#### 2.1 — Create `scripts/download-bun.sh`

- Download platform-specific Bun binary (replaces `download-node.sh`)
- Platforms: `macos-arm64`, `linux-x86_64`, `windows-x86_64`
- Output: `crates/app/nize_desktop/binaries/bun-{triple}`
- Bun download URLs (all `.zip`):
  - macOS arm64: `https://github.com/oven-sh/bun/releases/latest/download/bun-darwin-aarch64.zip`
  - Linux x64: `https://github.com/oven-sh/bun/releases/latest/download/bun-linux-x64.zip`
  - Windows x64: `https://github.com/oven-sh/bun/releases/latest/download/bun-windows-x64.zip`
- For pinned version: replace `latest` with `bun-v{VERSION}` in URL path

#### 2.2 — Create `scripts/bun-version.env`

- Pin Bun version: `BUN_VERSION=1.3.9`
- Replace `scripts/node-version.env`

#### 2.3 — Update `scripts/setup-sidecar-binaries.sh`

- Change `node` → `bun` in the dev-mode symlink section
- Symlink system `bun` instead of system `node`

#### 2.4 — Update `tauri.conf.json`

- `externalBin`: change `"binaries/node"` → `"binaries/bun"`

#### 2.5 — Update Rust code — binary name

- `nize_desktop/src/lib.rs`: `exe_dir.join("node")` → `exe_dir.join("bun")`
- `nize_desktop/src/mcp_clients.rs`: `sidecar_node_path()` → `sidecar_bun_path()`, `exe_dir.join("node")` → `exe_dir.join("bun")`
- `nize_desktop/src/mcp_clients.rs`: Claude Desktop config `"command": "node"` → `"command": "bun"`

#### 2.6 — Update Rust code — node_sidecar module

- Rename `nize_core/src/node_sidecar.rs` → `nize_core/src/bun_sidecar.rs`
- `Command::new("node")` → `Command::new("bun")`
- `NodeInfo` → `BunInfo`
- `check_node_available()` → `check_bun_available()`
- Update `nize_core/src/lib.rs` module declaration

#### 2.7 — Update PGlite manager

- `nize_core/src/db.rs`: doc comments referencing "Node.js" → "Bun"
- `PgLiteManager::start()` takes `bun_bin` parameter (rename from `node_bin`)

#### 2.8 — Validate sidecar runtime migration

- PGlite server starts correctly under Bun
- nize-web server starts correctly under Bun
- mcp-remote works correctly under Bun
- Claude Desktop MCP bridge connects successfully

### Phase 3: Cleanup

#### 3.1 — Remove Node.js artifacts

- Delete `scripts/download-node.sh`
- Delete `scripts/node-version.env`
- Remove `package-lock.json` (Bun uses `bun.lockb`)
- Remove `node_modules` references from `.gitignore` if Bun uses different cache

#### 3.2 — Update documentation

- Update `ARCHITECTURE.md`: technology stack, sidecar descriptions, mermaid diagrams
- Update README if it references npm/node
- Update `.github/instructions/tauri.instructions.md`

#### 3.3 — Update CI/CD workflows

- Replace `npm ci` / `npm install` with `bun install`
- Replace `npm run` with `bun run` in CI scripts
- Install Bun in CI runners (e.g., `setup-bun` GitHub Action)
- Remove Node.js setup steps if no longer needed (keep if Next.js needs it)

#### 3.4 — Optional: migrate esbuild → Bun.build()

- Replace `build-pglite-server.mjs` esbuild calls with `Bun.build()`
- Replace `build-mcp-remote.mjs` esbuild calls with `Bun.build()`
- Remove `esbuild` dev dependency

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| PGlite N-API incompatibility | Low | High | Test PGlite socket under Bun early; fallback to Node.js for PGlite only |
| Next.js edge cases under Bun | Medium | Medium | nize-web is minimal; test standalone server; fallback option available |
| Bun binary size larger than expected | Low | Low | Current Bun ~55 MB vs Node.js ~110 MB; verify per platform |
| Windows Bun stability | Low | Medium | Bun v1.3.9 ships official Windows x64 binaries with active bug fixes; test on CI |
| `pglite-socket` `node:net` usage | Low | High | Bun supports `node:net`; test wire protocol handshake |

## Completion Criteria

- [ ] `bun install` succeeds for all workspaces
- [ ] `cargo tauri dev` works with Bun as both build tool and sidecar runtime
- [ ] PGlite starts and accepts SQL queries via Bun runtime
- [ ] nize-web serves pages via Bun runtime
- [ ] mcp-remote bridge works via Bun runtime for Claude Desktop
- [ ] App bundle size is reduced (or at least not increased)
- [ ] All existing tests pass
- [ ] No Node.js binary bundled in final app

## Resolved Questions

1. **Next.js under Bun**: Yes — test nize-web under Bun as part of Phase 2. nize-web is minimal, so risk is low. If issues arise, can fall back to keeping Node.js for nize-web only.
2. **Bun version pinning**: Pin to **v1.3.9** (latest stable, released 2026-02-08).
3. **CI/CD**: Yes — add Phase 3.3 to update CI workflows (`bun install`, `bun run`, `setup-bun` action).
4. **Windows**: Confirmed stable — Bun v1.3.9 ships official Windows x64 binaries (standard + baseline), with active Windows-specific bug fixes in recent releases.
