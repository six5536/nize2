# PLAN-005: nize_terminator — Process Reaper for Unclean Shutdown

| Field              | Value                          |
|--------------------|--------------------------------|
| **Status**         | phase-3-testing                |
| **Workflow**       | bottom-up                      |
| **Reference**      | PLAN-002 (tauri bootstrap)     |
| **Traceability**   | —                              |

## Goal

Ensure child processes managed by Tauri (starting with PostgreSQL) are cleanly terminated when Tauri is killed via uncatchable signals (SIGKILL, OOM-kill, crash).

`nize_terminator` is a lightweight standalone binary spawned by Tauri. It watches Tauri's PID and executes registered cleanup commands when that PID disappears.

## Problem

- **Graceful shutdown** (SIGTERM, window close) → `RunEvent::Exit` fires → `DbManager::stop()` calls `pg_ctl stop` → ✅
- **Forceful kill** (SIGKILL, `kill -9`, OOM) → no handler runs → PG daemon orphaned → ❌

`DbManager::start()` already detects stale servers on next launch, but PG may sit orphaned indefinitely between launches.

## Design

### Architecture

```
Tauri (parent)
  │
  ├── 1. Spawn nize_terminator (reaper)
  │        • Watches parent PID
  │        • Watches manifest file for cleanup commands
  │        • On parent death → reads manifest → executes commands → exits
  │
  ├── 2. Start PG via DbManager
  │        • Write cleanup command to manifest
  │
  ├── 3. Start nize_api_server (sidecar)
  │        • (killed when parent dies — direct child)
  │
  └── 4. Start future services...
           • Write cleanup command to manifest after each
```

### nize_terminator CLI Interface

```
nize_terminator --parent-pid <PID> --manifest <PATH>
```

- `--parent-pid` — PID to watch (Tauri's PID).
- `--manifest` — Path to a file containing cleanup commands (one per line).
- Process is silent (no stdout), logs to stderr only on error.
- Exits 0 after cleanup completes, non-zero on error.

Example invocation from Tauri:

```
nize_terminator --parent-pid 12345 --manifest /tmp/nize-12345-cleanup.manifest
```

### Manifest File Format

Plain text, one shell command per line. Tauri appends lines synchronously after each subprocess start.

```
pg_ctl -D /Users/rich/Library/Application Support/nize/pgdata -m fast stop
```

After starting a second service in the future:

```
pg_ctl -D /Users/rich/Library/Application Support/nize/pgdata -m fast stop
kill <other_service_pid>
```

Terminator reads the file at the moment parent death is detected. Whatever is in the file at that instant is what gets executed. Tauri writes synchronously (atomic append + fsync) after each managed process start, so there is no race.

### Parent-Death Detection (Platform-Specific)

| Platform | Mechanism | Latency |
|----------|-----------|---------|
| macOS    | `kqueue` with `EVFILT_PROC` + `NOTE_EXIT` | Instant |
| Linux    | `pidfd_open` + `poll` (kernel ≥5.3) | Instant |
| Fallback | `kill(pid, 0)` polling every 1s | ≤1s |

### Lifecycle

1. Tauri creates empty manifest file at a known path (e.g., `$TMPDIR/nize-<pid>-cleanup.manifest`).
2. Tauri spawns `nize_terminator --parent-pid <self> --manifest <path>`.
3. Tauri starts PG via `DbManager::start()`.
4. Tauri appends PG cleanup command to manifest (synchronous write + fsync).
5. Tauri starts API sidecar, future services, etc. — appends cleanup for each.
6. **Graceful shutdown**: `RunEvent::Exit` stops PG, kills terminator, deletes manifest.
7. **SIGKILL at any point after step 2**: Terminator detects death, reads manifest, executes whatever commands are present, deletes manifest, exits.
8. **SIGKILL before step 4**: Manifest is empty → terminator has nothing to clean up → exits. PG wasn't started, so no orphan.
9. **SIGKILL between steps 4 and 5**: Manifest has PG cleanup only → terminator stops PG → exits.

### Idempotency

`pg_ctl -m fast stop` on a stopped server exits with non-zero but no harm. Terminator logs the error to stderr and continues to next cleanup command. This means graceful shutdown + terminator double-stop is safe.

### Extensibility

The manifest file is generic — any shell command, one per line. Future uses:
- Stop additional sidecar services
- Clean up temp files
- Send telemetry/crash reports

Tauri appends to the manifest after each managed process start. No terminator restart or CLI change needed.

## Steps

### Phase 1 — Create nize_terminator Crate

- [x] **1.1** Create `crates/app/nize_terminator/Cargo.toml`
  - Dependencies: `clap` (args), `libc` (POSIX APIs)
  - No async runtime — this is a simple synchronous process
  - Inherit `workspace.package` fields
- [x] **1.2** Add `"crates/app/nize_terminator"` to root `Cargo.toml` workspace `members` (NOT `default-members`)
- [x] **1.3** Implement `src/main.rs`:
  - Parse args: `--parent-pid <u32>`, `--manifest <PathBuf>`
  - Call platform-specific `wait_for_pid_exit(pid)`
  - On return, read manifest file (one command per line, skip blank lines)
  - Execute each command via `std::process::Command::new("sh").arg("-c").arg(&cmd)`
  - Delete manifest file after cleanup
  - Log errors to stderr, exit 0 on success
- [x] **1.4** Implement `src/pid_watch.rs` — platform-specific parent-death detection:
  - `#[cfg(target_os = "macos")]` — `kqueue` + `EVFILT_PROC` + `NOTE_EXIT`
  - `#[cfg(target_os = "linux")]` — `pidfd_open` + `poll`
  - Fallback — `kill(pid, 0)` polling at 1s interval
- [x] **1.5** Verify: `cargo build -p nize_terminator`

### Phase 2 — Integrate with Tauri

- [x] **2.1** Update `tauri.conf.json` — add `nize_terminator` to `beforeDevCommand` / `beforeBuildCommand` build chain
- [x] **2.2** Create helper: `fn manifest_path() -> PathBuf` — returns `$TMPDIR/nize-<pid>-cleanup.manifest`
- [x] **2.3** Create helper: `fn append_cleanup(manifest: &Path, cmd: &str)` — atomic append line + fsync
- [x] **2.4** Update `nize_desktop/src/lib.rs` startup sequence:
  1. Create empty manifest file
  2. Spawn `nize_terminator --parent-pid <self> --manifest <path>`
  3. Store `Child` handle in `AppServices`
  4. Start PG via `DbManager::start()`
  5. Append PG cleanup command to manifest
  6. Start API sidecar
- [x] **2.5** Update `RunEvent::Exit` handler:
  - Stop PG (existing)
  - Kill terminator process
  - Delete manifest file
- [ ] **2.6** Verify: `cargo tauri dev` starts terminator before PG

### Phase 3 — Testing

- [ ] **3.1** Manual test — graceful shutdown:
  - `cargo tauri dev` → close window → PG stopped by Tauri → terminator's cleanup is no-op → terminator exits
- [ ] **3.2** Manual test — SIGKILL after full startup:
  - `cargo tauri dev` → `kill -9 <tauri_pid>` → terminator reads manifest → runs `pg_ctl stop` → PG stopped
- [ ] **3.3** Manual test — SIGKILL before PG starts:
  - Start Tauri, SIGKILL immediately → terminator reads empty manifest → nothing to clean → exits
- [ ] **3.4** Manual test — double stop (graceful + terminator):
  - Close window (PG stops), terminator tries cleanup → `pg_ctl stop` returns non-zero → terminator logs warning, exits 0
- [x] **3.5** Unit test in `nize_terminator`:
  - Spawn a subprocess, get its PID, kill it, verify `wait_for_pid_exit` returns
- [x] **3.6** Unit test — manifest parsing:
  - Write multi-line manifest, verify commands are read correctly, blank lines skipped

### Phase 4 — Cleanup

- [ ] **4.1** On graceful exit in `RunEvent::Exit`: kill terminator, delete manifest (already in 2.5)
- [ ] **4.2** Verify no orphan terminator processes linger after clean shutdown
- [ ] **4.3** Verify no stale manifest files in `$TMPDIR` after clean shutdown

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| `kqueue`/`pidfd_open` not available on older kernels | Low | Fallback to `kill(pid, 0)` polling |
| Terminator itself is orphaned on graceful exit | Low | Kill it in `RunEvent::Exit`; even if missed, it detects parent death and exits immediately |
| Cleanup command fails (PG data dir moved, binary missing) | Low | Log error to stderr; exit non-zero; next app launch still has stale-server detection |
| Race: Tauri dies between terminator spawn and manifest write | Very Low | Manifest is empty → terminator exits cleanly → PG wasn't started yet, no orphan |
| Stale manifest from previous crash | Low | Terminator deletes manifest after cleanup; Tauri overwrites on startup |
| Windows support | Deferred | Windows uses `WaitForSingleObject` on process handle — implement when Windows target needed |

## Decisions

1. **Separate binary, not a thread** — Must survive SIGKILL of parent. Threads die with the process.
2. **Manifest file, not CLI args** — Cleanup commands registered dynamically after each process start. Eliminates race between process start and terminator spawn.
3. **Terminator starts first** — Spawned before any managed processes. If parent dies mid-startup, terminator cleans up whatever has been registered so far.
4. **Synchronous (no tokio)** — ~100 lines. Blocking `kqueue`/`poll`/sleep is fine for a watchdog.
5. **Hold `Child` handle** — In `AppServices` so it can be killed on graceful exit. If not killed, terminator detects parent death and runs (idempotent) cleanup anyway.
6. **Not in `default-members`** — Only built as part of Tauri build chain (like `nize_api_server`).
7. **Name: `nize_terminator`** — Generic reaper, not PG-specific.

## Completion Criteria

- `nize_terminator` binary builds: `cargo build -p nize_terminator`
- `cargo tauri dev` spawns terminator alongside PG
- SIGKILL of Tauri → PG is stopped within seconds (no orphan)
- Graceful shutdown still works as before (no regression)
- No orphan terminator processes after clean exit
