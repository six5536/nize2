# PLAN-030: ClientPool Idle Timeout & LRU Eviction

**Status:** in-progress
**Workflow direction:** lateral
**Traceability:** PLAN-025 (stdio server support); ARCHITECTURE.md → nize_core, nize_mcp

## Goal

Prevent resource exhaustion when many stdio MCP servers are registered by adding idle timeout eviction and LRU-based capacity management to `ClientPool`. Currently all spawned stdio processes live until application shutdown or error. With 50–100+ stdio servers registered, this means up to 4–8GB of memory consumed by idle Node/Bun runtimes.

**In scope:** Idle timeout on pool entries, LRU tracking with last-accessed timestamps, eviction when approaching `max_stdio_processes`, transparent respawn on next call, configurable timeout.
**Out of scope:** Worker-thread multiplexer (future optimization), HTTP connection eviction (cheap — keep as-is), changes to MCP protocol or meta-tools.

## Current State

| Component | Status |
|-----------|--------|
| `ClientPool` with `DashMap<Uuid, PoolEntry>` | Done — PLAN-025 |
| `max_stdio_processes` hard cap (default 50) | Done — returns `ResourceExhausted` error |
| Lazy connect on first tool call | Done — `get_or_connect()` |
| Retry on error (remove + reconnect) | Done — `execute_with_retry()` |
| Terminator manifest PID tracking | Done — PLAN-025 Phase 5 |
| Idle timeout / eviction | **Not done** |
| LRU tracking | **Not done** |
| Background reaper task | **Not done** |

## Design

### Data Model Changes

Add LRU metadata to `PoolEntry`:

```rust
use std::time::Instant;
use std::sync::atomic::{AtomicU64, Ordering};

struct PoolEntry {
    service: RunningService<RoleClient, ()>,
    transport: TransportType,
    last_accessed: AtomicU64,  // epoch millis, atomic for lock-free reads
    created_at: Instant,
}
```

`last_accessed` uses `AtomicU64` storing milliseconds since a fixed reference point (`Instant` at pool creation). This allows `call_tool()` to update the timestamp without holding a write lock on the `DashMap` entry — just an `AtomicU64::store(Relaxed)` on the existing `&PoolEntry` ref from `DashMap::get()`.

### Touch on Access

In `call_tool()`, after obtaining the `DashMap::get()` ref and before calling `peer.call_tool()`, update `last_accessed`:

```rust
let conn = client_pool.connections.get(&server_id).ok_or_else(/* ... */)?;
conn.touch(&client_pool.epoch);  // atomic store, no lock
let peer = conn.service.peer().clone();
drop(conn);
```

Where `touch()` is:

```rust
impl PoolEntry {
    fn touch(&self, epoch: &Instant) {
        let ms = epoch.elapsed().as_millis() as u64;
        self.last_accessed.store(ms, Ordering::Relaxed);
    }

    fn idle_duration(&self, epoch: &Instant) -> Duration {
        let now_ms = epoch.elapsed().as_millis() as u64;
        let last = self.last_accessed.load(Ordering::Relaxed);
        Duration::from_millis(now_ms.saturating_sub(last))
    }
}
```

Also touch in `get_or_connect()` on the fast path (entry already exists) so that connection checks count as activity.

### Idle Timeout Reaper

A background `tokio::spawn` task that runs periodically (e.g. every 30s) and evicts idle **stdio** entries:

```rust
impl ClientPool {
    /// Spawn background reaper. Returns JoinHandle for shutdown.
    pub fn spawn_reaper(self: &Arc<Self>, idle_timeout: Duration) -> tokio::task::JoinHandle<()> {
        let pool = Arc::clone(self);
        tokio::spawn(async move {
            let interval = idle_timeout / 4;  // check 4× per timeout period
            loop {
                tokio::time::sleep(interval).await;
                pool.evict_idle(idle_timeout);
            }
        })
    }

    fn evict_idle(&self, timeout: Duration) {
        let mut evicted = Vec::new();
        self.connections.retain(|id, entry| {
            if entry.transport == TransportType::Stdio
                && entry.idle_duration(&self.epoch) > timeout
            {
                evicted.push(*id);
                false  // remove from map
            } else {
                true
            }
        });
        for id in &evicted {
            info!(server_id = %id, "Evicted idle stdio connection");
        }
    }
}
```

Note: `DashMap::retain` drops the removed `PoolEntry`, which drops the `RunningService`, which cancels the `CancellationToken` and drops the `TokioChildProcess`, killing the child. This is the same cleanup path as `remove()`.

### LRU Eviction on Capacity Pressure

When `connect_stdio()` is called and `stdio_count() >= max_stdio_processes`, instead of returning `ResourceExhausted` immediately, first try to evict the least-recently-used stdio entry:

```rust
async fn connect_stdio(&self, server: &McpServerRow, server_id: Uuid) -> Result<(), McpError> {
    if self.stdio_count() >= self.max_stdio_processes {
        if !self.evict_lru_stdio() {
            return Err(McpError::ResourceExhausted(/* ... */));
        }
    }
    // ... proceed with spawn
}

/// Evict the single least-recently-used stdio connection.
/// Returns true if an entry was evicted.
fn evict_lru_stdio(&self) -> bool {
    let oldest = self.connections.iter()
        .filter(|e| e.value().transport == TransportType::Stdio)
        .min_by_key(|e| e.value().last_accessed.load(Ordering::Relaxed))
        .map(|e| *e.key());

    if let Some(id) = oldest {
        self.remove(&id);
        info!(server_id = %id, "LRU-evicted stdio connection to make room");
        true
    } else {
        false
    }
}
```

### Configuration

Add `idle_timeout_secs` to `ClientPool`:

```rust
pub struct ClientPool {
    connections: Arc<DashMap<Uuid, PoolEntry>>,
    connecting: Arc<Mutex<HashSet<Uuid>>>,
    manifest_path: Option<PathBuf>,
    max_stdio_processes: usize,
    idle_timeout: Duration,       // new
    epoch: Instant,               // new — reference point for timestamps
}
```

- Default idle timeout: **5 minutes** (`300s`)
- Configurable via `set_idle_timeout(&mut self, timeout: Duration)`
- Future: expose as `mcp.idle_timeout_secs` config option in the configuration system

### Transparent Respawn

No changes needed — `execute_with_retry()` already handles this:
1. `get_or_connect()` → entry missing (evicted) → `connect()` → spawns new process
2. First call fails (stale connection) → `remove()` → `get_or_connect()` again → reconnects

The eviction path (idle timeout or LRU) removes the entry from the `DashMap`. The next `get_or_connect()` sees no entry and spawns a new process. The cold-start cost (~1-5s for npm-based servers) is the trade-off; this is acceptable because the server was idle.

### Integration Point

In `nize_mcp::mcp_router_with_manifest()`, after creating the `ClientPool`, spawn the reaper:

```rust
let client_pool = Arc::new(match manifest_path {
    Some(path) => ClientPool::with_manifest(path),
    None => ClientPool::new(),
});

// Spawn idle timeout reaper
let _reaper = client_pool.spawn_reaper(client_pool.idle_timeout);
```

The reaper task runs until the `tokio` runtime shuts down (application exit). No explicit cleanup needed — all `PoolEntry` values are dropped on pool drop.

## Implementation Plan

### Phase 1: Add LRU Metadata to PoolEntry

**File:** `crates/lib/nize_core/src/mcp/execution.rs`

#### Step 1.1: Add `epoch` and `idle_timeout` to `ClientPool`

- Add `epoch: Instant` field (initialized to `Instant::now()` in `new()`)
- Add `idle_timeout: Duration` field (default `DEFAULT_IDLE_TIMEOUT = 300s`)
- Add `set_idle_timeout(&mut self, timeout: Duration)` method
- Update `with_manifest()` to also set epoch

#### Step 1.2: Add `last_accessed` and `created_at` to `PoolEntry`

- Add `last_accessed: AtomicU64` (millis since epoch)
- Add `created_at: Instant`
- Add `touch(&self, epoch: &Instant)` and `idle_duration(&self, epoch: &Instant)` methods
- Update `connect_http()` and `connect_stdio()` to initialize these fields

#### Step 1.3: Touch on access

- In `get_or_connect()`: touch entry on fast path (already connected)
- In `call_tool()`: touch entry before cloning peer

### Phase 2: Idle Timeout Reaper

**File:** `crates/lib/nize_core/src/mcp/execution.rs`

#### Step 2.1: Implement `evict_idle()`

- Iterate `connections` with `retain()`, remove stdio entries exceeding idle timeout
- Log evicted server IDs
- Cancel service via `CancellationToken` on drop (existing behavior)

#### Step 2.2: Implement `spawn_reaper()`

- Takes `&Arc<Self>`, spawns `tokio::spawn` loop
- Check interval: `idle_timeout / 4` (e.g. 75s for 5min timeout)
- Returns `JoinHandle<()>` (caller can abort on shutdown if desired, but not required)

#### Step 2.3: Wire up reaper in nize_mcp

**File:** `crates/lib/nize_mcp/src/lib.rs`

- After creating `Arc<ClientPool>`, call `client_pool.spawn_reaper(client_pool.idle_timeout())`
- Store the `JoinHandle` only if explicit abort is needed (not required — runtime drop handles it)

### Phase 3: LRU Eviction on Capacity

**File:** `crates/lib/nize_core/src/mcp/execution.rs`

#### Step 3.1: Implement `evict_lru_stdio()`

- Find stdio entry with smallest `last_accessed`
- Call `remove()` on it
- Return `bool` indicating success

#### Step 3.2: Update `connect_stdio()` to try LRU eviction before failing

- When `stdio_count() >= max_stdio_processes`, call `evict_lru_stdio()`
- Only return `ResourceExhausted` if eviction fails (no stdio entries to evict — shouldn't happen)

### Phase 4: Tests

**File:** `crates/lib/nize_core/src/mcp/execution.rs`

#### Step 4.1: Unit tests for PoolEntry touch/idle

- `pool_entry_touch_updates_last_accessed`
- `pool_entry_idle_duration_increases_over_time`

#### Step 4.2: Unit tests for eviction

- `evict_idle_removes_timed_out_stdio_entries` (requires injecting entries — may need test helper)
- `evict_lru_stdio_removes_oldest_entry`
- `evict_lru_stdio_returns_false_on_empty_pool`

#### Step 4.3: Integration-level assertions

- `connect_stdio_evicts_lru_when_at_capacity` (requires mock server or test helper)
- Existing tests continue to pass unchanged

## Edge Cases

1. **Entry evicted between `get_or_connect` and `call_tool`**: Already handled — `call_tool()` returns error, `execute_with_retry()` reconnects.
2. **Reaper runs during `connect_stdio`**: The connecting guard prevents the entry from being evicted before it's fully initialized (entry isn't in `connections` yet during spawn).
3. **HTTP entries**: Never evicted — HTTP connections are cheap (no OS process). Only stdio entries have LRU/idle eviction.
4. **All stdio entries are active**: LRU eviction still evicts the least-recently-used one. The evicted server gets a cold start next time. This is better than refusing the new connection entirely.
5. **Concurrent eviction + access**: `DashMap` is safe for concurrent reads/writes. `retain()` locks shards one at a time. A `call_tool()` racing with eviction either sees the entry (and touches it) or doesn't (and gets a retry). The `AtomicU64` touch is lock-free.

## Completion Criteria

- [x] `PoolEntry` tracks `last_accessed` (atomic) and `created_at`
- [x] `call_tool()` and `get_or_connect()` touch entries on access
- [x] Background reaper evicts idle stdio entries (default 5min timeout)
- [x] `connect_stdio()` LRU-evicts when at `max_stdio_processes` capacity
- [x] Transparent respawn works (evicted server reconnects on next call)
- [x] Idle timeout is configurable via `set_idle_timeout()`
- [x] HTTP connections are not evicted
- [x] All existing tests pass
- [x] New unit tests for touch, idle duration, eviction logic
