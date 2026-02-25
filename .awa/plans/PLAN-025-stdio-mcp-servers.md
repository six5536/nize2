# PLAN-025: Stdio MCP Server Support

**Status:** in-progress
**Workflow direction:** bottom-up
**Traceability:** PLAN-020 (MCP service registration); PLAN-024 (tool discovery & execution proxy)

## Goal

Enable Nize to spawn and communicate with **stdio-based MCP servers** (e.g. `@modelcontextprotocol/server-filesystem`) as first-class external MCP servers — not just HTTP servers. Stdio servers are registered in the DB like HTTP servers (transport = `stdio`), and the execution proxy spawns and manages their child processes.

## Context

### Current State

| Component | Status |
|-----------|--------|
| DB schema: `mcp_servers.transport` enum (`stdio`, `http`) | Done (PLAN-020) |
| Domain models: `StdioServerConfig`, `ServerConfig::Stdio` | Done |
| Test connection for stdio (raw JSON-RPC handshake) | Done (`test_connection_stdio` in `mcp_config.rs`) |
| Execution proxy (`nize_core::mcp::execution`) | Done — **HTTP only** |
| `ClientPool::get_or_connect` | Rejects stdio servers with error |
| rmcp `transport-child-process` feature | **Not enabled** in workspace `Cargo.toml` |
| Process lifecycle management for stdio child processes | Not implemented |

### Key Constraint

The execution proxy currently hardcodes HTTP-only:

```rust
// execution.rs:72-76
if server.transport != TransportType::Http {
    return Err(McpError::Validation(format!(
        "Server \"{}\" uses {:?} transport, only HTTP is supported",
        server.name, server.transport
    )));
}
```

### rmcp Support

rmcp provides `TokioChildProcess` (feature `transport-child-process`) which:
- Spawns a child process with piped stdin/stdout
- Implements `Transport<RoleClient>` — plugs into `().serve(transport)` identically to HTTP
- Returns a `RunningService<RoleClient, ()>` — same type as HTTP connections
- Handles graceful shutdown (close transport → wait → kill)
- Uses `process-wrap` for cross-platform process management

This means the `ClientPool` can store both HTTP and stdio connections under the same `RunningService<RoleClient, ()>` type with minimal changes.

## Decisions

### Process Lifecycle

**Each stdio server gets one long-lived child process**, managed by the `ClientPool`. The process is spawned on first tool call and kept alive for reuse. On error or stale connection, the process is killed and respawned on retry (same pattern as HTTP reconnect).

Rationale:
- Matches the existing HTTP connection pooling model
- Avoids per-call process spawn overhead (some MCP servers have slow startup)
- `TokioChildProcess`'s `Drop` impl auto-kills the child if the `RunningService` is removed from the pool

### Shutdown / Cleanup

When `ClientPool::remove()` is called (retry path), the `RunningService` is dropped, which drops the `TokioChildProcess`, which kills the child. On application shutdown, all `DashMap` entries are dropped, killing all child processes.

For crash recovery, stdio server PIDs are appended to the nize_terminator manifest when spawned, and removed when the process is killed. See Phase 5.

### Security

Stdio servers spawn arbitrary commands on the host. Access control:
- **Admin-only creation**: Only admins can register stdio servers (visibility `hidden` or `visible`, never `user`)
- **Execution for all**: Once registered and enabled, any authenticated user can call tools on stdio servers (same as HTTP)
- **No user-created stdio servers**: Users can only create HTTP servers (enforced by `McpConfigService`)

This matches the existing validation in `McpConfigService` (PLAN-020) — no new access control needed.

### Environment Variables

`StdioServerConfig` already supports an `env` field (`Option<HashMap<String, String>>`). When spawning the child process, these env vars are set on the `Command`. The child inherits the nize_desktop_server process's environment plus any overrides.

Important: The `PATH` env var must typically be set to ensure the command can find its dependencies (e.g. `node`, `npx`, `bun`).

## Implementation Plan

### Phase 1: Enable rmcp Stdio Transport

#### Step 1.1: Add `transport-child-process` Feature

Add `"transport-child-process"` to the rmcp features in workspace `Cargo.toml`:

```toml
rmcp = { version = "0.15", features = [
    "transport-streamable-http-server",
    "transport-streamable-http-client-reqwest",
    "transport-child-process",
    "client",
] }
```

Verify: `cargo check -p nize_core` compiles.

### Phase 2: Extend `ClientPool` for Stdio

#### Step 2.1: Use Atomic `DashMap::entry()` for Connection Insert

Replace the current `contains_key` + `insert` pattern with `DashMap::entry()` to prevent duplicate process spawns for the same server ID. This matters for stdio (each duplicate = wasted OS process) but also fixes a latent TOCTOU bug for HTTP.

Use a separate `Mutex<HashSet<Uuid>>` (or `DashSet`) as an "in-progress" guard: before spawning, insert the server ID into the guard set; after inserting into `connections`, remove from guard. Callers that see the ID in the guard set wait or return early.

Alternatively, use `DashMap::entry()` with `or_try_insert_with` (if available in dashmap 6), or simply hold the entry ref during async work.

#### Step 2.2: Refactor `get_or_connect` to Support Both Transports

In `crates/lib/nize_core/src/mcp/execution.rs`, replace the HTTP-only guard in `get_or_connect` with a match on transport type:

- For `TransportType::Http`: existing `StreamableHttpClientTransport` path (unchanged)
- For `TransportType::Stdio`: spawn via `TokioChildProcess`

Both produce `RunningService<RoleClient, ()>` so the pool type stays the same.

Pseudocode for stdio branch:

```rust
TransportType::Stdio => {
    let config: StdioServerConfig = serde_json::from_value(config_json)?;
    let mut cmd = tokio::process::Command::new(&config.command);
    if let Some(args) = &config.args {
        cmd.args(args);
    }
    if let Some(env) = &config.env {
        for (k, v) in env {
            cmd.env(k, v);
        }
    }
    let transport = TokioChildProcess::new(cmd)?;
    let service: RunningService<RoleClient, ()> = ().serve(transport).await?;
    self.connections.insert(server_id, service);
    // Register PID with terminator manifest (see Phase 5)
}
```

#### Step 2.3: Enforce Max Stdio Process Limit

Add a configurable `max_stdio_processes` limit (default: 50) via the DB-driven configuration system (`config_definitions` table, category `mcp`, key `mcp.max_stdio_processes`).

Before spawning a new stdio process in `get_or_connect`, count current stdio connections. If at limit, return `McpError::ResourceExhausted` with a clear message.

The `ClientPool` needs to track which connections are stdio vs HTTP. Either:
- Store a `HashMap<Uuid, TransportType>` alongside `connections`, or
- Store `(RunningService, TransportType)` tuples in the `DashMap`

The count check reads the DB config value from the `ConfigCache` (already available via `Arc<RwLock<ConfigCache>>`).

Seed migration: insert `mcp.max_stdio_processes` definition into `config_definitions` with default value `50`, type `integer`, scope `system`.

#### Step 2.4: Import `TokioChildProcess`

Add the import:

```rust
use rmcp::transport::TokioChildProcess;
```

### Phase 3: Validate End-to-End

#### Step 3.1: Register a Stdio Server via API

Use the existing `POST /mcp/servers` (admin) endpoint to register a stdio server:

```json
{
  "name": "filesystem",
  "description": "File system access",
  "domain": "filesystem",
  "transport": "stdio",
  "config": {
    "transport": "stdio",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users/rich/Desktop"],
    "env": {
      "PATH": "/Users/rich/.local/share/mise/installs/node/24.3.0/bin:/usr/local/bin:/usr/bin:/bin"
    }
  }
}
```

#### Step 3.2: Test Connection

Use `POST /mcp/servers/{id}/test` — the existing `test_connection_stdio` should work (it spawns its own short-lived process independently of the `ClientPool`).

#### Step 3.3: Refresh Tools

Use the existing tool refresh flow to populate `mcp_server_tools` for the stdio server. This likely requires the `test_connection` + `replace_server_tools` path — verify it stores tool manifests.

#### Step 3.4: Execute a Tool

Use the MCP `execute_tool` meta-tool to call a tool on the stdio server. Verify the `ClientPool` spawns the child process and proxies the call.

### Phase 4: Error Handling & Edge Cases

#### Step 4.1: Process Crash Recovery

If the stdio child process crashes (exits unexpectedly), the next tool call will fail. The existing retry logic in `execute_with_retry` should handle this:
1. First call fails (broken pipe / EOF)
2. `client_pool.remove(&server_id)` kills the stale process
3. `client_pool.get_or_connect()` spawns a new process
4. Retry succeeds

Verify this works with a test that kills the child process mid-session.

#### Step 4.2: Startup Timeout

Some stdio MCP servers take several seconds to initialize (e.g. `npx -y` downloads packages). The `().serve(transport).await` call waits for the MCP `initialize` handshake. If the server doesn't respond within a reasonable time, the call should time out.

Add a timeout wrapper around `().serve(transport).await` for stdio connections (e.g. 30 seconds).

#### Step 4.3: Command Not Found

If the command doesn't exist, `TokioChildProcess::new()` returns an `io::Error`. Map this to `McpError::ConnectionFailed` with a clear message.

#### Step 4.4: Stderr Logging

Stdio MCP servers often log to stderr. Use `TokioChildProcessBuilder` to let stderr **inherit** to the nize_desktop_server process's stderr, so stdio MCP server logs appear in the server's log output. No capture or background task needed.

```rust
let transport = TokioChildProcess::builder(cmd)
    .stderr(std::process::Stdio::inherit())
    .spawn()?;
```

### Phase 5: Terminator Integration

When stdio server processes are spawned, their PIDs must be registered with the nize_terminator manifest for crash recovery.

#### Step 5.1: Pass Manifest Path to `nize_desktop_server`

Add a `--terminator-manifest` CLI argument to `nize_desktop_server` (optional; only set when running as a Tauri sidecar). Store the path in `AppState` or a dedicated struct accessible by the `ClientPool`.

In `nize_desktop/src/lib.rs`, pass `--terminator-manifest <manifest_path>` when spawning the API sidecar.

#### Step 5.2: Write PID to Manifest on Stdio Process Spawn

After spawning a `TokioChildProcess` in `get_or_connect`, extract the PID via `transport.id()` (returns `Option<u32>`). If a manifest path is configured, append `kill <pid>` to the manifest file using the same atomic append + fsync pattern as `nize_desktop::append_cleanup`.

Extract the manifest append logic into `nize_core` so both `nize_desktop` and `nize_core::mcp::execution` can use it without a dependency on `nize_desktop`.

#### Step 5.3: Remove PID from Manifest on Process Removal

When `ClientPool::remove()` kills a stdio process, the PID is no longer valid. The terminator manifest is append-only by design — stale `kill` commands are harmless (killing a non-existent PID is a no-op). No manifest removal needed.

#### Step 5.4: Propagate Manifest Path Through `ClientPool`

Add an `Option<PathBuf>` field to `ClientPool` for the manifest path:

```rust
pub struct ClientPool {
    connections: Arc<DashMap<Uuid, (RunningService<RoleClient, ()>, TransportType)>>,
    manifest_path: Option<PathBuf>,
}
```

Set it from the `--terminator-manifest` CLI arg in `nize_desktop_server/main.rs` when constructing the `ClientPool`.

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Stdio process resource leak | Child processes survive server crash | Phase 5 (terminator manifest); `Drop` impl kills on pool entry removal |
| Slow startup (npx downloads) | Tool calls time out on first use | Phase 4.2 timeout; pre-warm via test_connection |
| Platform differences (Windows) | Command paths differ | `StdioServerConfig.env` allows per-platform PATH; process-wrap handles platform differences |
| Concurrent spawning race | Two calls to same server spawn two processes | Phase 2.1: atomic `DashMap::entry()` insert |
| Too many stdio processes | OS resource exhaustion | Phase 2.3: configurable limit (default 50) |

## Completion Criteria

- [ ] `transport-child-process` feature enabled in workspace `Cargo.toml`
- [ ] `ClientPool::get_or_connect` supports stdio transport via `TokioChildProcess`
- [ ] Atomic `DashMap::entry()` prevents duplicate process spawns
- [ ] `mcp.max_stdio_processes` config option (default 50) enforced
- [ ] Stdio server registered via API and tools listed
- [ ] Tool execution works end-to-end through MCP meta-tool → stdio child process
- [ ] Process crash + retry works (remove + reconnect)
- [ ] Connection timeout for slow-starting stdio servers
- [ ] Stderr inherits to server logs
- [ ] Stdio process PIDs registered in terminator manifest
- [ ] `--terminator-manifest` CLI arg passed from Tauri to `nize_desktop_server`

## Resolved Questions

1. **Concurrent spawn race**: Yes — use `DashMap::entry()` for atomic check-and-insert keyed by server ID. (Phase 2.1)
2. **Max stdio processes**: Yes — configurable via `mcp.max_stdio_processes` config option, default 50. (Phase 2.3)
3. **Stderr handling**: Inherit to server logs. (Phase 4.4)
4. **Terminator integration**: All running stdio processes registered in terminator manifest for cleanup on app crash/KILL. (Phase 5)
