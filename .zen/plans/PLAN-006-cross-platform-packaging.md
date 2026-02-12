# PLAN-006: Cross-Platform Packaging (macOS → Linux → Windows)

| Field              | Value                                          |
|--------------------|------------------------------------------------|
| **Status**         | in-progress                                    |
| **Workflow**       | top-down                                       |
| **Reference**      | PLAN-002 (tauri bootstrap), PLAN-005 (terminator) |
| **Traceability**   | —                                              |

## Goal

Ship `nize-desktop` as a self-contained, installable desktop app on macOS, Linux, and Windows — including all sidecar binaries and a bundled PostgreSQL runtime — with CI/CD via GitHub Actions.

## Current State

| Component | Status | Cross-platform? |
|-----------|--------|-----------------|
| `nize_desktop` (Tauri) | ✅ Working | Tauri handles this |
| `nize_api_server` | ✅ Working | Pure Rust, compiles everywhere |
| `nize_terminator` | ✅ macOS + Linux | ❌ No Windows (`pid_watch.rs`, `sh -c`) |
| PostgreSQL runtime | ❌ Discovered via `pg_config` on PATH | Not bundled — end-users won't have it |
| Node.js sidecar | ⚠️ Checked via `node --version` | Not bundled — same problem |
| GitHub Actions CI | ✅ Rust/WASM tests on Ubuntu | ❌ No Tauri build, no multi-platform |
| Tauri `externalBin` | ❌ Not configured | Sidecars located via `exe.parent()` |

## Decisions

1. **Bundle PostgreSQL binaries** — Ship a minimal PG distribution (postgres, initdb, pg_ctl, pg_isready, + pgvector extension) inside the app bundle per-platform. Adds ~40–60 MB compressed per platform. No alternative gives us pgvector without PG.
2. **Use Tauri `externalBin`** — Register `nize_api_server` and `nize_terminator` as Tauri external binaries. Tauri appends the platform triple automatically (`-aarch64-apple-darwin`, `-x86_64-unknown-linux-gnu`, etc.) and places them in the correct location at bundle time.
3. **Bundle PG as Tauri `resources`** — PG binaries + pgvector `.so`/`.dylib` go into `resources/pg/{platform}/` in the bundle. The app resolves them at runtime via `tauri::api::path::resource_dir()`.
4. **Platform priority**: macOS (arm64) → Linux (x86_64) → Windows (x86_64).
5. **Node.js** — Deferred. Not needed for MVP packaging. Currently only checked, not required at startup.
6. **PG source**: EDB binary distributions for all platforms. Pin to PG 18.0, pgvector 0.8.0.
7. **Auto-update**: Ship from day one via `tauri-plugin-updater`. GitHub Releases hosts the static `latest.json`. Signing key stored as GH secret.

## Architecture

### Bundle Layout (macOS .app)

```
Nize Desktop.app/Contents/
├── MacOS/
│   ├── nize_desktop                           # main binary
│   ├── nize_api_server-aarch64-apple-darwin   # externalBin (Tauri-managed)
│   └── nize_terminator-aarch64-apple-darwin   # externalBin (Tauri-managed)
├── Resources/
│   └── pg/
│       ├── bin/
│       │   ├── postgres
│       │   ├── initdb
│       │   ├── pg_ctl
│       │   └── pg_isready
│       └── lib/
│           ├── postgresql/
│           │   └── vector.so             # pgvector extension
│           └── libpq.5.dylib             # (if needed by PG binaries)
└── Info.plist
```

### Bundle Layout (Linux AppImage / .deb)

```
usr/
├── bin/
│   ├── nize-desktop
│   ├── nize_api_server-x86_64-unknown-linux-gnu
│   └── nize_terminator-x86_64-unknown-linux-gnu
└── share/nize-desktop/
    └── pg/
        ├── bin/
        │   ├── postgres, initdb, pg_ctl, pg_isready
        └── lib/
            └── postgresql/
                └── vector.so
```

### Bundle Layout (Windows .msi / .nsis)

```
Nize Desktop/
├── nize-desktop.exe
├── nize_api_server-x86_64-pc-windows-msvc.exe
├── nize_terminator-x86_64-pc-windows-msvc.exe
└── pg/
    ├── bin/
    │   ├── postgres.exe, initdb.exe, pg_ctl.exe, pg_isready.exe
    └── lib/
        └── vector.dll
```

## Steps

### Phase 1 — Tauri `externalBin` Configuration

Migrate from `exe.parent().join("nize_api_server")` to Tauri's `externalBin` system. This ensures sidecars are bundled with the correct platform triple suffix.

- [ ] **1.1** Update `tauri.conf.json` — add `externalBin` entries:
  ```json
  "bundle": {
    "externalBin": [
      "binaries/nize_api_server",
      "binaries/nize_terminator"
    ]
  }
  ```
- [ ] **1.2** Create `crates/app/nize_desktop/binaries/` directory structure for dev-time:
  - Symlink or copy script that places `target/debug/nize_api_server` → `binaries/nize_api_server-{triple}`
  - Document the triple-suffix convention (e.g., `nize_api_server-aarch64-apple-darwin`)
- [ ] **1.3** Update `nize_desktop/src/lib.rs` — use `tauri::api::shell::sidecar()` or resolve via `app.path().resource_dir()` instead of `exe.parent().join(...)`:
  - `start_api_sidecar()` → resolve `nize_api_server` via Tauri's sidecar mechanism
  - `create_manifest_and_spawn_terminator()` → resolve `nize_terminator` via Tauri's sidecar mechanism
- [ ] **1.4** Update `tauri.conf.json` `beforeDevCommand` and `beforeBuildCommand` to copy binaries with the correct triple suffix after building
- [ ] **1.5** Verify: `cargo tauri dev` still works on macOS with `externalBin`

### Phase 2 — Bundle PostgreSQL Binaries

Bundle a minimal PG distribution as Tauri resources so end-users don't need PG installed.

- [ ] **2.1** Create script `scripts/bundle-postgres.sh` that:
  - Accepts a platform argument (macos-arm64, linux-x86_64, windows-x86_64)
  - Copies minimal PG binaries from a known installation (or downloads from official archives)
  - Strips debug symbols to reduce size
  - Copies pgvector extension `.so`/`.dylib`/`.dll`
  - Outputs to `crates/app/nize_desktop/resources/pg/`
- [ ] **2.2** Update `tauri.conf.json` — add `resources` configuration:
  ```json
  "bundle": {
    "resources": {
      "resources/pg": "pg"
    }
  }
  ```
- [ ] **2.3** Update `nize_core::db::PgConfig` — add `from_bundled(resource_dir: PathBuf)`:
  - Resolves `bin_dir` to `resource_dir/pg/bin/`
  - Sets `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH` for the PG processes to find pgvector
- [ ] **2.4** Update `nize_desktop/src/lib.rs` startup — detect bundled PG vs PATH PG:
  - If `resource_dir/pg/bin/pg_ctl` exists → use bundled PG
  - Else → fallback to `pg_config --bindir` (developer mode)
- [ ] **2.5** Update `nize_terminator` manifest commands — ensure cleanup commands reference bundled PG paths (already handled: `db.pg_ctl_stop_command()` uses absolute paths)
- [ ] **2.6** macOS: code-sign the bundled PG binaries (required for notarization)
- [ ] **2.7** Verify: `cargo tauri build` produces a `.dmg` that starts PG from bundled binaries

### Phase 3 — Windows Support for `nize_terminator`

Add Windows platform support to `pid_watch.rs` and command execution.

- [ ] **3.1** Add `#[cfg(target_os = "windows")]` path in `pid_watch.rs`:
  - Use `OpenProcess(SYNCHRONIZE, ...)` + `WaitForSingleObject(handle, INFINITE)`
  - Use `windows-sys` crate (zero-cost FFI, no C++ runtime)
  - Fallback: `GetExitCodeProcess` polling (for permission-denied edge cases)
- [ ] **3.2** Update `Cargo.toml` for `nize_terminator`:
  - Add `[target.'cfg(windows)'.dependencies]` with `windows-sys`
  - Keep `libc` dependency as `[target.'cfg(unix)'.dependencies]`
- [ ] **3.3** Update `run_cleanup()` in `main.rs`:
  - `#[cfg(unix)]`: `Command::new("sh").arg("-c").arg(cmd)` (existing)
  - `#[cfg(windows)]`: `Command::new("cmd").arg("/C").arg(cmd)`
- [ ] **3.4** Update `shell_escape()` in `nize_core::db` — handle Windows path escaping (backslashes, spaces) for manifest commands
- [ ] **3.5** Add Windows unit tests for `pid_watch` (spawn a process, kill it, verify detection)
- [ ] **3.6** Verify: `cargo build -p nize_terminator --target x86_64-pc-windows-msvc` compiles (cross-compile check or CI)

### Phase 4 — GitHub Actions CI: Multi-Platform Build

Create a Tauri-specific build workflow that produces installable artifacts for all platforms.

- [ ] **4.1** Create `.github/workflows/desktop-build.yml`:
  - Trigger: `push` to `main`, `pull_request` to `main`, `workflow_dispatch`
  - Matrix strategy:
    ```yaml
    strategy:
      matrix:
        include:
          - platform: macos-latest
            target: aarch64-apple-darwin
            pg_platform: macos-arm64
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            pg_platform: linux-x86_64
          - platform: windows-latest
            target: x86_64-pc-windows-msvc
            pg_platform: windows-x86_64
    ```
- [ ] **4.2** CI steps per platform:
  1. Checkout
  2. Install Rust toolchain (1.93.0)
  3. Install Node.js 24.x
  4. Install platform dependencies:
     - Ubuntu: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev`
     - macOS: Xcode CLI tools (pre-installed)
     - Windows: MSVC (pre-installed)
  5. Cache Cargo registry + target
  6. Install `npm` dependencies (root + `packages/nize-desktop`)
  7. Run `scripts/bundle-postgres.sh ${{ matrix.pg_platform }}` — download/copy PG binaries
  8. Build sidecar binaries with correct triple suffix
  9. `npx tauri build`
  10. Upload artifacts (`.dmg`, `.deb`, `.AppImage`, `.msi`)
- [ ] **4.3** Add Cargo caching optimized for multi-platform (separate cache keys per OS)
- [ ] **4.4** Verify: CI builds and produces artifacts for all three platforms

### Phase 5 — GitHub Actions: Release Workflow

Extend the existing release workflow (or create a new one) that publishes desktop installers.

- [ ] **5.1** Create `.github/workflows/desktop-release.yml` (or add job to existing `release.yml`):
  - Trigger: `workflow_dispatch` or tag push (`v*`)
  - Run the same build matrix as Phase 4
  - Upload artifacts to a GitHub Release
- [ ] **5.2** macOS code signing and notarization:
  - Secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`
  - Configure in `tauri.conf.json` or env vars
- [ ] **5.3** Windows code signing (deferred — can use unsigned installers initially)
- [ ] **5.4** Attach installers to GitHub Release alongside existing NPM/WASM artifacts
- [ ] **5.5** Verify: full release flow produces signed macOS `.dmg`, Linux `.deb`+`.AppImage`, Windows `.msi`

### Phase 6 — PG Binary Acquisition (EDB)

All platforms use EDB binary distributions. Single source simplifies versioning and CI.

- [ ] **6.1** macOS (arm64): Download from [EDB PG 18 binaries](https://www.enterprisedb.com/download-postgresql-binaries) (macOS arm64 zip)
- [ ] **6.2** Linux (x86_64): Download from EDB (Linux x86_64 zip — no installer, just binaries)
- [ ] **6.3** Windows (x86_64): Download from EDB (Windows x86_64 zip — no installer, just binaries)
- [ ] **6.4** pgvector: Build from source against the bundled PG 18 headers for each platform in CI
  - `git clone --branch v0.8.0 https://github.com/pgvector/pgvector.git && make && make install`
  - Requires PG dev headers (included in EDB distribution)
- [ ] **6.5** Create `scripts/download-pg.sh` that:
  - Downloads EDB zip for the target platform
  - Extracts minimal set: `bin/{postgres,initdb,pg_ctl,pg_isready}`, `lib/`, `share/`
  - Strips debug symbols (`strip` on Unix, no-op on Windows)
  - Copies pgvector `.so`/`.dylib`/`.dll` into `lib/postgresql/`
  - Outputs to `crates/app/nize_desktop/resources/pg/`
- [ ] **6.6** Pin versions in `scripts/pg-versions.env`:
  ```
  PG_VERSION=18.0
  PGVECTOR_VERSION=0.8.0
  EDB_BASE_URL=https://get.enterprisedb.com/postgresql
  ```
- [ ] **6.7** Cache PG binaries in CI (`actions/cache` keyed on `pg-${{ matrix.pg_platform }}-$PG_VERSION`)

### Phase 7 — Auto-Update via `tauri-plugin-updater`

Ship in-app auto-updates backed by GitHub Releases as static JSON.

- [ ] **7.1** Install `tauri-plugin-updater`:
  - `cargo add tauri-plugin-updater` in `nize_desktop/Cargo.toml`
  - `npm add @tauri-apps/plugin-updater @tauri-apps/plugin-process` in `packages/nize-desktop`
- [ ] **7.2** Generate signing keypair:
  - `npx tauri signer generate -w ~/.tauri/nize-desktop.key`
  - Store private key as GH secret `TAURI_SIGNING_PRIVATE_KEY`
  - Store public key in `tauri.conf.json` under `plugins.updater.pubkey`
- [ ] **7.3** Update `tauri.conf.json`:
  ```json
  {
    "bundle": {
      "createUpdaterArtifacts": true
    },
    "plugins": {
      "updater": {
        "pubkey": "<PUBLIC_KEY>",
        "endpoints": [
          "https://github.com/six5536/nize2/releases/latest/download/latest.json"
        ]
      }
    }
  }
  ```
- [ ] **7.4** Add `updater:default` to capabilities:
  - Update `crates/app/nize_desktop/capabilities/default.json`
- [ ] **7.5** Register plugin in Rust:
  - Add `.plugin(tauri_plugin_updater::Builder::new().build())` to `tauri::Builder`
  - Add `.plugin(tauri_plugin_process::init())` for relaunch support
- [ ] **7.6** Add update check UI in frontend:
  - On app start (or menu action): call `check()` from `@tauri-apps/plugin-updater`
  - Show notification if update available, allow download + install + relaunch
- [ ] **7.7** Update release workflow (Phase 5):
  - Set `TAURI_SIGNING_PRIVATE_KEY` env var during `tauri build`
  - Use `tauri-action` which auto-generates `latest.json` and `.sig` files
  - Attach `latest.json` to each GitHub Release
- [ ] **7.8** Verify: install v0.1.0 → publish v0.2.0 → app detects and installs update

### Phase 8 — PG Data Migration on Version Upgrade

Handle PostgreSQL major version upgrades without data loss. After an auto-update, only the NEW PG binaries exist — new PG cannot read an old-format data directory. Strategy: **dump before update, restore after**.

Key insight: we control the update flow via `tauri-plugin-updater` (Phase 7.6). The frontend calls `check()` → user clicks "Update" → we dump → then `downloadAndInstall()`.

- [ ] **8.1** Write bundled PG major version to a marker file on first `initdb`:
  - `{pgdata}/NIZE_PG_VERSION` containing e.g. `18` (the major version we shipped)
  - Written by `LocalDbManager::setup()` after `initdb` succeeds
- [ ] **8.2** On startup, detect version mismatch:
  - Read `{pgdata}/NIZE_PG_VERSION`
  - Compare to bundled PG major version (compiled into binary or read from `resources/pg/PG_VERSION`)
  - If same major version → start normally (minor upgrades are binary-compatible)
  - If mismatch → enter migration flow (8.5)
- [ ] **8.3** Bundle `pg_dumpall` and `pg_restore` in `resources/pg/bin/` (add to Phase 6.5 extraction list)
- [ ] **8.4** Pre-update dump in the frontend update flow:
  - Before calling `update.downloadAndInstall()`, invoke a Tauri command `pre_update_dump`
  - Rust side: `pg_dumpall -f {app_data}/nize_backup.sql` using the CURRENT (still-running) PG
  - Verify dump file exists and is non-empty
  - If dump fails or produces empty file → return error, **abort upgrade** (do NOT call `downloadAndInstall()`)
  - Frontend shows error: "Database backup failed. Update cancelled. Please try again."
  - Only on success: write `{app_data}/PENDING_MIGRATION` marker with `{old_major}→{new_major}`, then proceed
- [ ] **8.5** Post-update restore on startup (migration flow):
  - Detect `PENDING_MIGRATION` marker OR `NIZE_PG_VERSION` mismatch with no running PG
  - Stop PG if running
  - Rename `{pgdata}` → `{pgdata}.old`
  - Run `initdb` with new PG binaries → creates fresh `{pgdata}`
  - Start new PG
  - Restore: `psql -f {app_data}/nize_backup.sql`
  - Re-create pgvector extension
  - Update `NIZE_PG_VERSION` to new major
  - Delete `PENDING_MIGRATION` marker
  - Delete `nize_backup.sql`
  - Keep `{pgdata}.old` for 1 launch as safety net, delete on next clean startup
- [ ] **8.6** Guard against unclean state (should never happen due to 8.4 abort):
  - If `NIZE_PG_VERSION` mismatches AND no `nize_backup.sql` exists → refuse to start
  - Show error: "Database version mismatch detected without backup. The old data directory is preserved at {pgdata}.old."
  - Offer only: "Quit" — force user to investigate (prevents silent data loss)
  - Log `{pgdata}.old` path and instructions for manual `pg_dumpall` with the old PG binaries if available
- [ ] **8.7** Add `pre_update_dump` Tauri command to `nize_desktop`:
  - Exposed to frontend via `tauri::command`
  - Returns `Result<(), String>` — frontend shows error if dump fails, blocks update
- [ ] **8.8** Verify: install with PG 18 → create data → simulate upgrade to PG 19 → data preserved

## Resolved Questions

1. **PG version**: **18.x** — matches dev setup (`mise +postgres@18.0`). Pin to 18.0 initially.
2. **macOS arch**: **arm64-only** — add x86_64 later if needed.
3. **Auto-update**: **Include now** — Phase 7 covers `tauri-plugin-updater` with GitHub Releases as static JSON backend.
4. **PG binary source**: **EDB** — download from [EDB PostgreSQL binaries](https://www.enterprisedb.com/download-postgresql-binaries) for all platforms.
5. **PG data migration**: **Dump before update, restore after**. Pre-update dump via Tauri command before `downloadAndInstall()`. Post-update restore detects major version mismatch and re-initializes. Phase 8 covers the full flow.

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| PG binary size bloats the installer | Medium | Strip symbols, exclude unused locale/timezone data, compress with UPX (PG binaries ~25 MB stripped) |
| pgvector not available as prebuilt for target | Medium | Build from source in CI (simple: `make && make install`, needs PG dev headers) |
| macOS notarization rejects bundled PG binaries | High | Sign every binary with `codesign` before bundling; use `--options runtime` for hardened runtime |
| Windows PG requires MSVC redistributable | Low | EDB distributes PG with bundled CRT, or ship VC++ redistributable |
| CI build time too long (3 platforms × PG download) | Medium | Cache PG binaries, use `sccache` for Rust compilation |
| `nize_terminator` `sh -c` on Windows | High | Phase 3.3 addresses this — use `cmd /C` on Windows |
| Unix socket paths in PG config on Windows | Medium | PG on Windows uses TCP only (no unix sockets) — `pg_ctl` options already use `-h localhost`, so this works |
| PG `DYLD_LIBRARY_PATH` blocked by macOS SIP | Medium | Set `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH` on the `Command` env, or use `@rpath` in PG dylibs |
| Lost updater signing key | Critical | Back up `~/.tauri/nize-desktop.key` securely; losing it means existing users can never auto-update |
| Update replaces PG binaries while PG is running | Medium | Graceful shutdown of PG before applying update (Tauri auto-exits app on Windows; macOS/Linux update applies on next launch) |
| PG major upgrade without pre-dump (dump fails) | High | Abort upgrade: if `pg_dumpall` fails, block `downloadAndInstall()` and show error to user. Never proceed without a verified dump. |
| `pg_dumpall` slow on large local DB | Low | Desktop app DBs are small (< 100 MB typically); dump takes seconds. Show progress indicator in UI |

## Completion Criteria

- [ ] `cargo tauri build` on macOS produces a `.dmg` that starts without PG on PATH
- [ ] `cargo tauri build` on Linux produces a `.deb`/`.AppImage` that starts without PG on PATH
- [ ] `cargo tauri build` on Windows produces an `.msi` that starts without PG on PATH
- [ ] GitHub Actions CI builds all three platforms on every push to main
- [ ] GitHub Actions release workflow produces signed macOS installer + Linux + Windows installers
- [ ] Auto-update: installed app detects new GitHub Release and updates in-place
- [ ] PG migration: major version upgrade preserves user data via dump/restore
- [ ] No regression: `cargo tauri dev` still works for developers with PG on PATH
