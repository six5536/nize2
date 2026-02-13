# PLAN-007: Migrate from Native PostgreSQL to PGlite

| Field              | Value                                          |
|--------------------|------------------------------------------------|
| **Status**         | completed (phase 9 deferred)                   |
| **Workflow**       | top-down                                       |
| **Reference**      | PLAN-006 (cross-platform packaging)            |
| **Traceability**   | Supersedes PLAN-006 phases 2, 6, 8; simplifies 4, 5 |

## Goal

Replace the bundled native PostgreSQL distribution (~40–60 MB per platform, platform-specific binaries, pgvector build-from-source, `initdb`/`pg_ctl` process management, major-version migration code) with **PGlite** (3 MB WASM, uniform across platforms, pgvector built-in, instant startup, no process management).

## Decision

**PGlite via `pglite-socket`** — PGlite runs inside a Node.js sidecar process and exposes the standard PG wire protocol on `localhost:<port>`. `nize_api_server` connects via `sqlx::PgPool` unchanged. The Node.js runtime is bundled as a Tauri `externalBin` (required anyway — see resolved questions).

## Current State (post PLAN-006)

| Component | Current | After PGlite migration |
|-----------|---------|----------------------|
| PG runtime | Native binaries (EDB), 40–60 MB stripped, per-platform | PGlite WASM, ~3 MB, platform-agnostic |
| pgvector | Built from source per-platform in CI | Built into PGlite package (42.9 KB) |
| PG lifecycle | `initdb` → `pg_ctl start` → `pg_isready` poll → `pg_ctl stop` | `node pglite-server.mjs --db=<path>` → ready |
| PG process management | `nize_core::db::LocalDbManager` (PgConfig, bin_dir, set_pg_lib_env, etc.) | Simple `Command::new("node")` with port readiness check |
| PG migration (major ver) | `pg_migration.rs` (431 lines): dump/restore, version markers | **Deleted.** PGlite handles internal format |
| PG download script | `scripts/download-pg.sh` (137 lines) + `pg-versions.env` | **Deleted.** `npm install` fetches PGlite |
| Terminator cleanup | `pg_ctl_stop_command()` in manifest | `kill <pglite-pid>` (Node process dies cleanly) |
| `DYLD_LIBRARY_PATH` hacks | `set_pg_lib_env()` for bundled shared libs | **Deleted.** No native shared libs |
| macOS code signing | Must sign every bundled PG binary | No PG binaries to sign |
| CI pgvector build step | `git clone pgvector && make && make install` per platform | **Deleted.** |
| UpdateChecker pre-dump | `invoke("pre_update_dump")` → `pg_dumpall` | PGlite data is files on disk — just files, no dump needed |
| `nize_core::db::PgConfig` | `bin_dir`, `from_bundled()`, `lib_dir()` | Simplified: just `data_dir`, `port`, `database_name` |
| Node.js | Checked via `node --version`, not bundled | **Bundled** as Tauri `externalBin` |

## Architecture

### Process Model

```
Tauri (nize_desktop)
  ├── spawns: node pglite-server.mjs --db=<pgdata> --port=0
  │            ↳ prints {"port": N} to stdout
  │            ↳ PG wire protocol on localhost:N
  ├── spawns: nize_api_server --port=0 --database-url=postgresql://localhost:N/nize
  │            ↳ prints {"port": M} to stdout
  │            ↳ HTTP API on localhost:M
  └── spawns: nize_terminator --parent-pid=PID --manifest=<path>
               ↳ cleanup: kill pglite-server and nize_api_server
```

### Data Location

Same as before: `$APP_DATA/nize/pgdata/`. PGlite with Node FS persists to a directory. The directory layout is PGlite-internal (not standard PG layout).

### Bundle Layout (macOS .app example)

```
Nize Desktop.app/Contents/
├── MacOS/
│   ├── nize_desktop
│   ├── nize_api_server-aarch64-apple-darwin     # externalBin
│   ├── nize_terminator-aarch64-apple-darwin     # externalBin
│   └── node-aarch64-apple-darwin                # externalBin (NEW)
└── Resources/
    └── pglite/
        └── pglite-server.mjs                    # entry point
```

`node_modules/@electric-sql/pglite` is bundled into `pglite-server.mjs` (single-file ESM bundle via esbuild/rollup, <5 MB).

## Steps

### Phase 1 — PGlite Server Entry Point

Create a minimal Node.js script that starts PGlite with pglite-socket, matching the sidecar JSON stdout protocol.

- [x] **1.1** Add npm dependencies in `packages/nize-desktop`:
  - `@electric-sql/pglite`
  - `@electric-sql/pglite-socket`
- [x] **1.2** Create `packages/nize-desktop/scripts/pglite-server.mjs`:
  - Accept `--db=<path>` (data directory), `--port=<N>` (0 = ephemeral), `--database=<name>`
  - Create PGlite instance with `vector` extension enabled
  - Create `PGLiteSocketServer` on specified port (or ephemeral)
  - Print `{"port": N}` to stdout once listening (matches `nize_api_server` sidecar protocol)
  - Graceful shutdown on SIGTERM/SIGINT: `server.stop()`, `db.close()`
  - Create the application database: `CREATE DATABASE <name>` (if not template1)
  - Enable pgvector: `CREATE EXTENSION IF NOT EXISTS vector`
- [x] **1.3** Add esbuild script to bundle `pglite-server.mjs` into a single file:
  - esbuild to produce a self-contained ESM file
  - Output to `crates/app/nize_desktop/resources/pglite/pglite-server.mjs`
  - WASM files are loaded from the same directory at runtime
- [ ] **1.4** Verify standalone: `node pglite-server.mjs --db=./test-pgdata --port=5433` → connect with `psql` → `CREATE EXTENSION vector` → queries work

### Phase 2 — Bundle Node.js Runtime

Bundle a platform-specific Node.js binary as a Tauri `externalBin`.

- [x] **2.1** Create `scripts/download-node.sh`:
  - Accept platform argument: `macos-arm64`, `linux-x86_64`, `windows-x86_64`
  - Download official Node.js binary from `https://nodejs.org/dist/`
  - Extract just the `node` binary (not npm, npx, etc.)
  - Copy to `crates/app/nize_desktop/binaries/node-{triple}`
  - Pin version in `scripts/node-version.env`: `NODE_VERSION=24.x` (latest 24.x LTS)
- [x] **2.2** Update `tauri.conf.json`:
  - Add `"binaries/node"` to `externalBin` array
  - Replace `resources/pg/*` with `resources/pglite/*` in `resources`
- [x] **2.3** Update `scripts/setup-sidecar-binaries.sh`:
  - Add Node.js binary copy alongside `nize_api_server` and `nize_terminator`
- [x] **2.4** Add `.gitignore` entry for Node.js binary cache
- [x] **2.5** Verify: `ls binaries/` shows `node-aarch64-apple-darwin` (or platform equivalent)

### Phase 3 — Simplify `nize_core::db`

Strip out all native PG process management. Replace `LocalDbManager` with a slim `PgLiteManager` that spawns `node pglite-server.mjs`.

- [x] **3.1** Create `nize_core::db::PgLiteManager`:
  - Fields: `data_dir: PathBuf`, `port: u16`, `database_name: String`, `child: Option<Child>`, `started: bool`
  - `start(node_bin: &Path, server_script: &Path)`:
    - Find free port (reuse existing `find_free_port()`)
    - Spawn: `node_bin pglite-server.mjs --db=<data_dir> --port=<port> --database=<name>`
    - Read first stdout line for `{"port": N}` (reuse same `SidecarReady` pattern)
    - Wait for PG wire protocol ready (connect with sqlx, retry loop)
  - `stop()`: send SIGTERM to child, wait for exit
  - `connection_url()`: `postgresql://localhost:<port>/<database_name>`
  - `kill_command()`: returns `kill <pid>` for terminator manifest
- [x] **3.2** Preserve `DbProvisioner` — it still works (sqlx against any PG). But provisioning (CREATE DATABASE, CREATE EXTENSION vector) moves to `pglite-server.mjs` init, so `DbProvisioner` becomes optional / dev-only
- [x] **3.3** Preserve `PgConfig::from_env()` and `LocalDbManager::with_default_data_dir()` for dev-mode (developer has native PG on PATH)
- [x] **3.4** Preserve `LocalDbManager::ephemeral()` for integration tests (tests still use native PG)
- [x] **3.5** Remove from `PgConfig`: `from_bundled()`, `lib_dir()`
- [x] **3.6** Remove from `LocalDbManager`: `with_bundled_or_env()`, `set_pg_lib_env()`, `detect_pg_major_version()`
- [x] **3.7** Remove `shell_escape()` Windows variant (no longer generating `pg_ctl stop` commands for manifest)
  - Keep Unix `shell_escape()` — still used by `pg_ctl_stop_command()` for dev-mode

### Phase 4 — Delete `pg_migration.rs`

PGlite manages its own internal data format. Major-version migration is a PGlite concern, not ours.

- [x] **4.1** Delete `crates/lib/nize_core/src/pg_migration.rs`
- [x] **4.2** Remove `pub mod pg_migration;` from `crates/lib/nize_core/src/lib.rs`
- [x] **4.3** Remove `use nize_core::pg_migration;` from `nize_desktop/src/lib.rs`
- [x] **4.4** Remove `write_version_marker()` call from `LocalDbManager::setup()`
- [x] **4.5** Remove `pre_update_dump` Tauri command from `nize_desktop/src/lib.rs`
- [x] **4.6** Remove `pre_update_dump` from `invoke_handler` registration

### Phase 5 — Update `nize_desktop/src/lib.rs`

Rewire the app startup to use PGlite instead of native PG.

- [x] **5.1** Replace `LocalDbManager` usage with `PgLiteManager`:
  - Resolve `node` binary path via Tauri `externalBin` (same pattern as other sidecars)
  - Resolve `pglite-server.mjs` via Tauri `resource_dir`
  - Spawn PGlite server, get port
  - Spawn `nize_api_server` with `--database-url` pointing to PGlite
- [x] **5.2** Update terminator manifest:
  - Replace `pg_ctl_stop_command()` with `kill <pglite_pid>` (or platform equivalent)
  - PGlite's Node process exits cleanly on SIGTERM, no special shutdown needed
- [x] **5.3** Update shutdown handler:
  - Kill PGlite child process on `RunEvent::Exit` (instead of `db.stop()`)
- [x] **5.4** Remove bundled PG resource detection code (the `resource_dir` → `pg/bin/pg_ctl` check)
- [x] **5.5** Remove `AppServices::_db` field (no longer holds a `LocalDbManager`)
  - Replace with PGlite child process handle
- [x] **5.6** Keep dev-mode fallback: if `pglite-server.mjs` not found in resources, fall back to native PG via `LocalDbManager::with_default_data_dir()`

### Phase 6 — Simplify `UpdateChecker.tsx`

The pre-update database dump is no longer needed — PGlite data is just files on disk that survive the update.

- [x] **6.1** Remove `invoke("pre_update_dump")` call from `installUpdate()`
- [x] **6.2** Remove the `"dumping"` status state
- [x] **6.3** Simplify flow: check → download → relaunch (no dump step)

### Phase 7 — Delete Native PG Scripts

- [x] **7.1** Delete `scripts/download-pg.sh`
- [x] **7.2** Delete `scripts/pg-versions.env`
- [x] **7.3** Remove `resources/pg/` directory and `.gitkeep`
- [x] **7.4** Update `.gitignore`: remove `/crates/app/nize_desktop/resources/pg/` entry, add `/crates/app/nize_desktop/resources/pglite/` if needed

### Phase 8 — Update CI Workflows

Simplify the CI: no PG download, no pgvector build.

- [x] **8.1** Update `.github/workflows/desktop-build.yml`:
  - Remove `pg_platform` from matrix
  - Remove "Cache PG binaries" step
  - Remove "Download PostgreSQL binaries" step
  - Remove "Build pgvector" step
  - Add "Download Node.js binary" step: `bash scripts/download-node.sh <platform>`
  - Add "Bundle PGlite server" step: `npm run build:pglite-server`
  - Keep sidecar build step (add node binary setup)
- [x] **8.2** Update `.github/workflows/desktop-release.yml`:
  - Same changes as 8.1
- [ ] **8.3** Verify: CI builds produce working artifacts without native PG

### Phase 9 — Data Migration (existing native PG → PGlite)

One-time migration for users who have data in a native PG `pgdata` directory.

- [ ] **9.1** On first PGlite startup, detect if `$APP_DATA/nize/pgdata/PG_VERSION` exists (native PG marker)
- [ ] **9.2** If native PG data found:
  - Show UI notification: "Migrating database to new format..."
  - Use native `pg_ctl` / `pg_dumpall` if available on PATH (dev users)
  - If not available → show message: "Please export data manually or start fresh"
  - On successful dump → import into PGlite via `psql` equivalent (`db.exec(sql)`)
  - Rename old `pgdata/` → `pgdata.native-backup/`
- [ ] **9.3** If native PG data NOT found → start fresh (PGlite creates its own data dir)
- [ ] **9.4** This is low-priority: the app is pre-release, no real users have native PG data yet

## What Survives from PLAN-006 (unchanged)

| Component | Status |
|-----------|--------|
| `externalBin` for `nize_api_server`, `nize_terminator` | ✅ Keep |
| `scripts/setup-sidecar-binaries.sh` | ✅ Keep (extended for node) |
| `nize_terminator` (parent-death watch, Windows support) | ✅ Keep |
| `tauri-plugin-updater` + `tauri-plugin-process` | ✅ Keep |
| `UpdateChecker.tsx` (simplified) | ✅ Keep |
| `desktop-build.yml` / `desktop-release.yml` (simplified) | ✅ Keep |
| `DbProvisioner` (for dev-mode native PG) | ✅ Keep |
| `LocalDbManager` + `PgConfig::from_env()` (dev-mode) | ✅ Keep |

## What Gets Deleted

| File / Code | Lines | Reason |
|-------------|-------|--------|
| `pg_migration.rs` | ~431 | PGlite handles internal format |
| `PgConfig::from_bundled()` | ~20 | No bundled native PG |
| `PgConfig::lib_dir()` | ~10 | No shared libs to path |
| `LocalDbManager::with_bundled_or_env()` | ~15 | Replaced by PgLiteManager |
| `LocalDbManager::set_pg_lib_env()` | ~10 | No env vars needed |
| `LocalDbManager::detect_pg_major_version()` | ~20 | No version detection |
| `pre_update_dump` Tauri command | ~20 | No dump needed |
| `download-pg.sh` | ~137 | No native PG download |
| `pg-versions.env` | ~5 | No native PG version pins |
| `resources/pg/` | — | No native PG resources |
| pgvector CI build step | ~12 | Built into PGlite |

**Total deleted**: ~680 lines of Rust + ~142 lines of shell + CI steps

## What Gets Added

| File / Code | Est. Lines | Purpose |
|-------------|-----------|---------|
| `pglite-server.mjs` | ~80 | PGlite + pglite-socket entry point |
| `pglite build script` | ~10 | esbuild config to bundle |
| `PgLiteManager` in `db.rs` | ~100 | Spawn/stop node pglite-server |
| `download-node.sh` | ~60 | Download platform-specific Node binary |
| `node-version.env` | ~3 | Pin Node version |

**Total added**: ~100 lines of Rust + ~80 lines of JS + ~63 lines of shell

**Net**: ~600 lines deleted, significant complexity reduction.

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| PGlite single-connection limit | Medium | `nize_api_server` uses a single `sqlx::PgPool` with `max_connections(1)` — fine for desktop |
| PGlite WASM performance | Low | Benchmarks show sub-ms single-row CRUD. Desktop workload is light |
| PGlite data format changes between versions | Low | PGlite maintains backward compatibility. npm lockfile pins exact version |
| Node.js binary size (~40 MB) | Medium | Offsets removed PG binaries (~40–60 MB). Net neutral or smaller |
| pglite-socket maturity | Medium | 14.6k GitHub stars, active development. Socket server is simple TCP proxy |
| `sqlx` compatibility with PGlite wire protocol | Low | PGlite implements standard PG wire protocol. `sqlx` doesn't use exotic features |
| Node.js security (bundled binary) | Low | Pin to LTS, sign binary (same as we'd sign PG binaries) |
| PGlite missing PG features (stored procedures, etc.) | Low | We use basic SQL + pgvector. No advanced PG features needed |

## Resolved Questions

1. **Node.js runtime**: Bundled as `externalBin` (v24.x). Required anyway for future features.
2. **PGlite version**: Pin to latest stable via npm lockfile.
3. **PGlite persistence**: Node FS (`./path/to/datadir`) — persists to `$APP_DATA/nize/pgdata/`.
4. **Connection protocol**: Standard PG wire protocol via `pglite-socket`. `sqlx::PgPool` works unchanged.
5. **pgvector**: Built into PGlite's WASM bundle. `CREATE EXTENSION IF NOT EXISTS vector` works.
6. **Existing native PG users**: Pre-release app, no real users. Phase 9 covers migration if needed (low priority).

## Open Questions

~~1. **Node.js version**: 24.x — matches dev toolchain.~~ **Resolved.**
~~2. **Single-file bundle format**: esbuild to a single file.~~ **Resolved.**
~~3. **PGlite data directory name**: Reuse `pgdata/`.~~ **Resolved.**

## Completion Criteria

- [ ] `cargo tauri dev` starts PGlite via bundled Node.js, API sidecar connects, frontend works
- [ ] `cargo tauri build` on macOS produces a `.dmg` that starts without PG or Node on PATH
- [ ] CI builds all three platforms without native PG download or pgvector build
- [ ] `pg_migration.rs` deleted, `download-pg.sh` deleted
- [ ] No regression in existing tests (`nize_terminator`, `nize_core::db` dev-mode tests)
- [ ] pgvector works: `CREATE EXTENSION vector` + vector queries succeed through PGlite
