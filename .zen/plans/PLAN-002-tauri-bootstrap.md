# PLAN-002: Tauri Desktop App Bootstrap

| Field              | Value                          |
|--------------------|--------------------------------|
| **Status**         | in-progress                    |
| **Workflow**       | bottom-up                      |
| **Reference**      | PLAN-001 (project bootstrap)   |
| **Traceability**   | —                              |

## Goal

Evolve `nize-mcp` from CLI-only to a Tauri desktop app with:

- Sidecar PostgreSQL + pgvector (process-managed, not crate-embedded)
- Bundled Node.js 24 sidecar (for running MCP servers)
- Web UI via Tauri webview
- CLI preserved as separate crate

No VMs, no Docker — native processes only.

## Pre-Conditions

- PLAN-001 completed (workspace bootstrapped)
- Codebase is minimal (placeholder crates, no domain logic yet)

## Target Directory Layout

```
nize-mcp/
├── Cargo.toml
├── package.json
├── crates/
│   ├── app/
│   │   ├── nize-cli/              # ← renamed from nize/
│   │   │   ├── Cargo.toml         # binary name: nize-cli
│   │   │   └── src/
│   │   └── nize-desktop/          # NEW — Tauri app
│   │       ├── Cargo.toml         # binary name: nize-desktop
│   │       └── src/
│   ├── lib/
│   │   ├── nize_core/             # + sqlx, tokio (PG managed via process spawning)
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   └── nize_mcp/
│   │       ├── Cargo.toml
│   │       └── src/
│   └── wasm/
│       └── nize_wasm/
├── packages/
│   ├── nize-cli/                  # ← renamed from nize/
│   │   └── package.json           # npm package: @six5536/nize-cli
│   └── nize-desktop/              # NEW — Tauri frontend (React + Vite)
│       ├── package.json
│       ├── index.html
│       └── src/
└── src-tauri/ → symlink or config points to crates/app/nize-desktop
```

## Steps

### Phase 1 — Rename CLI Crate & Package

Disambiguate the CLI from the upcoming desktop app.

- [x] **1.1** Rename `crates/app/nize/` → `crates/app/nize-cli/`
  - Update `Cargo.toml`: `name = "nize-cli"`
  - Update `[[bin]]` if present, or let it default
  - Keep all source files unchanged
- [x] **1.2** Update root `Cargo.toml` workspace members
  - `"crates/app/nize"` → `"crates/app/nize-cli"`
  - Update both `members` and `default-members`
- [x] **1.3** Rename `packages/nize/` → `packages/nize-cli/`
  - Update `package.json`: `name` → `"@six5536/nize-cli"`
  - Update `bin` entry: `"nize-cli"` → `"./dist/cli.cli.js"`
  - Keep all source files unchanged
- [x] **1.4** Update root `package.json` if it references `packages/nize`
- [x] **1.5** Verify: `cargo build` succeeds, `cargo test` passes
- [ ] **1.6** Verify: `npm run build` in `packages/nize-cli/` succeeds

### Phase 2 — Create Tauri App Crate

- [x] **2.1** Install Tauri CLI: `cargo install tauri-cli`
- [x] **2.2** Create `packages/nize-desktop/` — Tauri frontend (React + Vite)
  - `package.json` with `react`, `react-dom`, `@vitejs/plugin-react`, Tauri deps
  - `vite.config.ts` with React plugin
  - Minimal `index.html` + `src/main.tsx` + `src/App.tsx` (placeholder)
- [x] **2.3** Create `crates/app/nize-desktop/` — Tauri Rust backend
  - `Cargo.toml` depending on `tauri`, `tauri-plugin-shell`, `nize-core`, `nize-mcp`
  - `src/main.rs` with Tauri builder setup
  - `tauri.conf.json` with `externalBin` configuration
  - `capabilities/default.json` with shell permissions for sidecars
- [x] **2.4** Add `"crates/app/nize-desktop"` to workspace `members` (NOT `default-members` — separate build)
- [x] **2.5** Verify: `cargo tauri dev` launches the app with React webview

### Phase 3 — PostgreSQL Sidecar Management in nize-core

Replace `postgresql-embedded` crate with direct process management.
PG binaries discovered via `pg_config` on PATH (dev: mise provides this; dist: bundled).

- [x] **3.1** Remove dependencies from `nize-core/Cargo.toml`:
  - Remove `postgresql_embedded`
  - Remove `postgresql_extensions`
- [x] **3.2** Remove `postgresql_embedded` and `postgresql_extensions` from root `Cargo.toml` workspace deps
- [x] **3.3** Rewrite `nize-core/src/db.rs`:
  - `DbManager` struct — holds `PgConfig { bin_dir, data_dir, port, database_name }`
  - `PgConfig::from_env()` — discover `bin_dir` via `pg_config --bindir` on PATH
  - `PgConfig::with_bin_dir(path)` — explicit path (for bundled sidecar)
  - `setup()` — run `initdb -D <data_dir>` if data dir doesn't exist
  - `start()` — run `pg_ctl -D <data_dir> -o "-p <port> -k <socket_dir>" -l <logfile> start`
  - Wait for ready via `pg_isready -p <port> -h localhost`
  - Create database + enable vector extension via sqlx
  - `stop()` — run `pg_ctl -D <data_dir> -m fast stop`
  - `connection_url()` — build URL from known host/port/db
  - Port: find free ephemeral port via `TcpListener::bind(":0")`
- [x] **3.4** Rewrite tests:
  - `lifecycle_setup_start_stop` — using mise PG from PATH
  - `ephemeral_data_dir_cleanup` — tempdir removed on drop
- [x] **3.5** Verify: `cargo test -p nize-core` passes (5/5, 3.6s)

### Phase 4 — pgvector Extension

Build pgvector from source against the target PG. Same extension as existing nize app.

- [ ] **4.1** Create `scripts/install-pgvector.sh`:
  - Clone `pgvector/pgvector` at pinned tag `v0.8.1` (first version with PG 18 support)
  - Build: `make PG_CONFIG=$(which pg_config)`
  - Install: `make install PG_CONFIG=$(which pg_config)`
  - Requires: C compiler (cc/gcc/clang), make, PG dev headers (included in mise PG)
  - Note: v0.8.0 fails on PG 18 (`vacuum_delay_point` API change); v0.8.1 fixes this
- [x] **4.2** Build and install pgvector v0.8.1 into mise PG 18 (manual; script TODO)
- [x] **4.3** Integration test exists:
  - `vector_extension_is_available` — `CREATE EXTENSION IF NOT EXISTS vector`, verify in `pg_extension`
- [x] **4.4** Verified: `cargo test -p nize-core` passes (5/5, 3.6s — all tests including vector)
- [ ] **4.5** Document: pgvector setup in README or CONTRIBUTING

### Phase 5 — Node.js 24 Sidecar

- [ ] **5.1** Download Node.js 24 binaries for each target platform:
  - `node-v24.x.x-darwin-arm64`
  - `node-v24.x.x-darwin-x64`
  - `node-v24.x.x-linux-x64`
  - `node-v24.x.x-win-x64.exe`
- [ ] **5.2** Create build script (`scripts/download-node-sidecar.sh`):
  - Downloads correct Node binary for current platform
  - Renames to `node-$TARGET_TRIPLE` in `crates/app/nize-desktop/binaries/`
- [ ] **5.3** Configure `tauri.conf.json`:
  ```json
  { "bundle": { "externalBin": ["binaries/node"] } }
  ```
- [ ] **5.4** Create `nize-core/src/sidecar.rs` (or in nize-desktop):
  - `SidecarManager` — spawn/manage Node.js child processes
  - Spawn via `app.shell().sidecar("node").args(["mcp-server.js"])`
  - Pipe stdin/stdout for MCP stdio transport
  - Handle lifecycle (start, health check, restart, stop)
- [ ] **5.5** Create integration test:
  - Spawn Node sidecar with a trivial script
  - Verify stdout communication works
- [ ] **5.6** Verify: `cargo tauri dev` can spawn and communicate with Node sidecar

### Phase 6 — Design Document

- [ ] **6.1** Create `DESIGN-BOOT-tauri-bootstrap.md`:
  - Component diagram: Tauri app → DbManager → PG process, SidecarManager → Node process
  - Data flow: webview ↔ Tauri commands ↔ nize-core ↔ PG
  - MCP flow: Tauri → SidecarManager → Node (stdio) → MCP server
  - Lifecycle: startup sequence, shutdown sequence, first-run setup
  - File locations: PG data dir, Node modules dir, config
- [ ] **6.2** Review design against existing `submodules/nize/` schema (Drizzle ORM, etc.)

### Phase 7 — Deferred (Installers)

> Not in scope for this plan. Will be a separate plan.

- macOS `.dmg` via Tauri bundler
- Windows `.msi` via Tauri bundler
- Linux `.deb` / `.AppImage` via Tauri bundler
- Code signing

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| PG binaries not on PATH during development | Low | mise provides `postgres@18`; `pg_config --bindir` discovery. Tests require mise-activated shell (`PATH="$PATH" cargo test`) |
| pgvector source build requires C toolchain | Low | All target platforms have cc/clang; one-time setup |
| Node.js 24 not yet LTS at time of implementation | Low | Pin to specific version; Node 24 enters LTS Oct 2026 |
| Bundling PG for distribution increases app size (~50 MB) | Medium | Acceptable for desktop app; compress in installer |
| Tauri webview inconsistencies across platforms | Low | React + Vite is well-tested with Tauri; keep UI minimal at bootstrap |
| Rename breaks existing CI/CD or developer workflows | Low | Codebase is early-stage; no external consumers |

## Decisions

1. **Sidecar PostgreSQL** — Manage PG via process spawning (`initdb`/`pg_ctl`/`pg_isready`), not `postgresql-embedded` crate. Eliminates OpenSSL linking issues and PG version constraints. Dev: mise PG from PATH. Dist: bundled binaries.
2. **pgvector v0.8.1 from source** — Build against target PG. v0.8.1 is first release with PG 18 support. Same extension as existing `submodules/nize/` app (ref uses `pgvector/pgvector:pg16` Docker image). No prebuilt binary constraints.
3. **React + Vite** — Lightweight React frontend via Vite. No Next.js, no SSR. Just bootstrap.
4. **nize-desktop** — Tauri app binary and package both named `nize-desktop`.
5. **Node.js 24** — Stick with Node.js 24. Bun compatibility not proven enough for MCP server ecosystem.

## Completion Criteria

- `nize-cli` crate builds and runs (`cargo run -p nize-cli -- version`)
- `@six5536/nize-cli` npm package builds (`npm run build` in `packages/nize-cli/`)
- Tauri app launches (`cargo tauri dev` in `packages/nize-desktop/`)
- PG starts on ephemeral port via `pg_ctl` with pgvector available
- Node sidecar spawns and communicates via stdio
- Design document exists at `.zen/specs/DESIGN-BOOT-tauri-bootstrap.md`
