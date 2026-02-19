# PLAN-033: Extended MCP Transport Modes (SSE + Managed HTTP)

**Status:** in-progress
**Workflow direction:** lateral
**Traceability:** PLAN-020 (MCP service registration); PLAN-025 (stdio servers); PLAN-030 (ClientPool eviction)

## Goal

Extend the MCP server registration system from 2 transport modes to 5, adding SSE protocol support and managed HTTP lifecycle:

| Mode | Lifecycle | Protocol | Status |
|------|-----------|----------|--------|
| **Stdio** | Managed (spawned by nize) | stdio | Existing — no change |
| **Managed HTTP (SSE)** | Managed (spawned by nize) | Legacy SSE | **New** |
| **Managed HTTP (Streamable HTTP)** | Managed (spawned by nize) | Streamable HTTP | **New** |
| **HTTP (SSE)** | External (connect to running server) | Legacy SSE | **New** |
| **HTTP (Streamable HTTP)** | External (connect to running server) | Streamable HTTP | Existing — no change |

## Context

### Current State

| Component | Status |
|-----------|--------|
| DB enum `transport_type`: `stdio`, `http` | Done |
| Rust enum `TransportType`: `Stdio`, `Http` | Done |
| `ClientPool` with stdio + HTTP connect | Done (PLAN-025) |
| LRU eviction & idle timeout for stdio | Done (PLAN-030) |
| `connect_http()` — StreamableHttpClientTransport | Done |
| `connect_stdio()` — TokioChildProcess | Done |
| `test_http_connection()` — StreamableHttp only | Done |
| Managed HTTP (spawned subprocess + HTTP connect) | Not done |
| SSE client transport | Not done |
| TypeSpec API enum `ServerTransport`: `stdio`, `http` | Done |

### Two Orthogonal Dimensions

The current model conflates **lifecycle** and **protocol**:
- `Stdio` = managed lifecycle + stdio protocol
- `Http` = external lifecycle + streamable-HTTP protocol

The new model separates them conceptually but encodes them as a single enum for simplicity (finite, well-known combinations).

### MCP SSE Protocol (Legacy)

The legacy MCP SSE protocol differs from Streamable HTTP:

1. Client sends `GET /sse` → server responds with SSE event stream
2. Server sends an SSE event with type `endpoint` containing a URL (e.g. `/message?sessionId=xxx`)
3. Client POSTs JSON-RPC messages to that endpoint URL
4. Server sends JSON-RPC responses & notifications via the SSE stream

This is distinct from Streamable HTTP where:
- Client POSTs to a single endpoint (`/mcp`)
- Server may respond with JSON or SSE stream
- Server may support GET for standalone SSE stream

### rmcp SSE Support

rmcp 0.16 does **not** include a legacy SSE client transport. It has:
- `StreamableHttpClientTransport` — implements the Streamable HTTP protocol
- `TokioChildProcess` — implements stdio transport
- `Transport<R>` trait — `send()`, `receive()`, `close()` — can be implemented by custom types
- `client-side-sse` feature — SSE stream parsing utilities (`sse-stream` crate) used internally
- `SinkStreamTransport` — adaptor wrapping `(Sink, Stream)` into `Transport`

We need to implement a custom `SseClientTransport` that:
- Uses `reqwest` for HTTP requests (GET for SSE stream, POST for messages)
- Uses `sse-stream` (already a transitive dep via rmcp) for SSE event parsing
- Implements rmcp's `Transport<RoleClient>` trait
- Produces `RunningService<RoleClient, ()>` — same type as HTTP and stdio connections

### Managed HTTP Concept

"Managed" means the MCP server process is spawned by nize backend, similar to stdio:
- Process spawned on first tool call (or explicitly)
- Child process with piped stdin for lifecycle coupling
- PID registered in terminator manifest
- Subject to `max_managed_processes` limit and LRU eviction
- Process killed on pool entry removal

The difference from stdio is the **communication protocol**: instead of piping JSON-RPC over stdin/stdout, we spawn the server process, wait for it to bind a port, then connect to it via SSE or Streamable HTTP over localhost.

## Design Decisions

### D1: Single Transport Enum (not two separate fields)

Use a single `transport_type` enum with 5 values rather than separate `lifecycle` + `protocol` fields.

Rationale:
- Finite, well-known combinations (5 total)
- Simpler DB queries — single column filter
- Avoids invalid combinations (e.g. `managed` + `stdio-protocol` is just `stdio`)
- Matches how users think about it ("I want a managed SSE server")

### D2: Enum Values

DB enum: `stdio`, `sse`, `http`, `managed-sse`, `managed-http`

- `stdio` — no change
- `http` — no change (external streamable HTTP, backward compatible)
- `sse` — new: external legacy SSE
- `managed-sse` — new: spawned process + SSE connection
- `managed-http` — new: spawned process + streamable HTTP connection

### D3: SSE Client Transport — Custom Implementation in `nize_core`

Implement `SseClientTransport` in `crates/lib/nize_core/src/mcp/sse_transport.rs`:
- Uses `reqwest` (already a dependency) for HTTP
- Uses `sse-stream` (already a transitive dependency via rmcp `client-side-sse` feature) for SSE parsing
- Implements rmcp `Transport<RoleClient>` trait directly
- Returns `RunningService<RoleClient, ()>` via `().serve(transport).await`

Alternative considered: `reqwest-eventsource` crate. Rejected because `sse-stream` is already available as a transitive dep, and we need tight integration with rmcp's `Transport` trait. A thin custom wrapper is simpler than adapting a higher-level crate.

### D4: Managed Process Port Configuration

Managed HTTP servers need a known port to connect to after spawning. The port is specified as a **config parameter** on the managed server registration. The admin sets the port when registering the server, and is responsible for ensuring the port is also passed to the managed process via `args` or `env`.

nize connects to `http://localhost:{port}` (with the appropriate path based on protocol) after spawning the process and waiting for it to become ready.

Rationale: The port location in CLI args/env varies per server implementation — some use `--port`, some use `PORT` env var, some embed it in a URL arg. Extracting it automatically would be fragile. A simple `port` config field is explicit and reliable.

### D5: Managed Process Config Model

The `ManagedHttpServerConfig` stores:
- `command` — executable to run
- `args` — command arguments (admin includes port in args/env as needed by the server)
- `env` — environment variables
- `port` — the port the managed server will listen on; nize connects to `localhost:{port}`
- `path` — optional URL path suffix (default: `/sse` for managed-sse, `/mcp` for managed-http)
- `ready_timeout_secs` — seconds to wait for the server to become ready (default: 30)

This is similar to `StdioServerConfig` but with additional network-related fields. After spawning, nize retries connecting to `http://localhost:{port}{path}` until success or timeout.

### D6: Eviction Applies to All Managed Transports

The existing LRU eviction and idle timeout from PLAN-030 applies to ALL managed transports (stdio, managed-sse, managed-http), not just stdio. The check changes from `transport == TransportType::Stdio` to `is_managed(transport)`.

### D7: Security — Same as Stdio

Managed SSE/HTTP servers spawn arbitrary commands. Same restrictions as stdio:
- Admin-only creation
- Users cannot create managed servers
- Once registered and enabled, any authenticated user can call tools

External SSE servers (`sse` transport) follow the same rules as external HTTP servers (users can create their own).

## Implementation Plan

### Phase 1: SSE Client Transport

**Goal:** Implement a Rust SSE client transport compatible with rmcp's `Transport<RoleClient>` trait.

#### Step 1.1: Add `sse-stream` as Direct Dependency

Currently `sse-stream` is only a transitive dep via rmcp. Add it as a direct workspace dependency:

```toml
# Cargo.toml (workspace)
sse-stream = "0.2"
```

And in `nize_core/Cargo.toml`:

```toml
sse-stream = { workspace = true }
```

#### Step 1.2: Implement `SseClientTransport`

Create `crates/lib/nize_core/src/mcp/sse_transport.rs`.

Protocol flow:
1. `send()` first message (initialize request) triggers connection:
   - `GET {base_url}` → SSE stream
   - Wait for SSE event with type `endpoint` → extract message endpoint URL
   - `POST {endpoint_url}` with the initialize JSON-RPC message
   - Parse subsequent SSE events as JSON-RPC responses
2. `receive()` reads next SSE event from the stream, parses as JSON-RPC
3. `send()` subsequent messages: `POST {endpoint_url}` with JSON-RPC body
4. `close()` drops the SSE stream + cancels background tasks

Key types:

```rust
pub struct SseClientTransport {
    base_url: String,
    client: reqwest::Client,
    // Internal state managed via channels
    tx: mpsc::Sender<ClientJsonRpcMessage>,
    rx: mpsc::Receiver<ServerJsonRpcMessage>,
    cancel: CancellationToken,
}
```

Implementation approach — use a background task (similar to rmcp's `Worker` pattern):
- Background task connects to SSE endpoint, discovers message URL
- Routes outgoing messages to POST endpoint
- Routes incoming SSE events to receive channel
- The `Transport` impl just sends/receives via channels

#### Step 1.3: Unit Tests for SSE Transport

- Test SSE event parsing (message event → JSON-RPC message)
- Test endpoint discovery from SSE `endpoint` event
- Test error handling (connection refused, timeout, invalid endpoint event)

### Phase 2: Extend Transport Types

**Goal:** Add new enum variants and DB migration.

#### Step 2.1: DB Migration — Extend `transport_type` Enum

Create migration `NNNN_extend_transport_type.sql`:

```sql
ALTER TYPE transport_type ADD VALUE IF NOT EXISTS 'sse';
ALTER TYPE transport_type ADD VALUE IF NOT EXISTS 'managed-sse';
ALTER TYPE transport_type ADD VALUE IF NOT EXISTS 'managed-http';
```

Note: PostgreSQL `ADD VALUE` for enums cannot be inside a transaction block. Use separate statements.

#### Step 2.2: Update Rust `TransportType` Enum

In `crates/lib/nize_core/src/models/mcp.rs`:

```rust
pub enum TransportType {
    Stdio,
    Http,
    Sse,
    #[sqlx(rename = "managed-sse")]
    #[serde(rename = "managed-sse")]
    ManagedSse,
    #[sqlx(rename = "managed-http")]
    #[serde(rename = "managed-http")]
    ManagedHttp,
}
```

Add helper:

```rust
impl TransportType {
    /// Whether this transport type spawns a managed child process.
    pub fn is_managed(&self) -> bool {
        matches!(self, Self::Stdio | Self::ManagedSse | Self::ManagedHttp)
    }

    /// The protocol used for communication.
    pub fn protocol(&self) -> TransportProtocol {
        match self {
            Self::Stdio => TransportProtocol::Stdio,
            Self::Http | Self::ManagedHttp => TransportProtocol::StreamableHttp,
            Self::Sse | Self::ManagedSse => TransportProtocol::Sse,
        }
    }
}
```

#### Step 2.3: Update `ServerConfig` Enum

Add new config variants:

```rust
pub enum ServerConfig {
    #[serde(rename = "stdio")]
    Stdio(StdioServerConfig),
    #[serde(rename = "http")]
    Http(HttpServerConfig),
    #[serde(rename = "sse")]
    Sse(SseServerConfig),
    #[serde(rename = "managed-sse")]
    ManagedSse(ManagedHttpServerConfig),
    #[serde(rename = "managed-http")]
    ManagedHttp(ManagedHttpServerConfig),
}
```

New config types:

```rust
/// External SSE MCP server configuration.
pub struct SseServerConfig {
    pub url: String,                              // SSE endpoint URL (e.g. http://host:port/sse)
    pub headers: Option<serde_json::Value>,        // custom headers
    pub auth_type: String,                         // "none", "api-key", "oauth"
    pub api_key_header: Option<String>,
}

/// Managed HTTP/SSE MCP server configuration.
/// Server is spawned as a child process, then connected via HTTP or SSE.
pub struct ManagedHttpServerConfig {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub port: u16,                                 // port the managed server listens on
    pub path: Option<String>,                      // URL path (default: "/sse" for managed-sse, "/mcp" for managed-http)
    pub ready_timeout_secs: Option<u32>,            // seconds to wait for the server to accept connections (default: 30)
}
```

#### Step 2.4: Update TypeSpec API Contract

In `API-NIZE-mcp-config.tsp`:

```typespec
enum ServerTransport {
  stdio,
  http,
  sse,
  `managed-sse`,
  `managed-http`,
}
```

Regenerate OpenAPI + codegen.

### Phase 3: Integrate SSE into ClientPool

**Goal:** Wire SSE connections into the existing pool alongside HTTP and stdio.

#### Step 3.1: Add `connect_sse()` Method

In `execution.rs`, add:

```rust
async fn connect_sse(
    &self,
    server: &McpServerRow,
    oauth_headers: Option<&OAuthHeaders>,
) -> Result<(), McpError> {
    let config: SseServerConfig = /* parse from server.config */;
    let transport = SseClientTransport::new(&config.url, /* headers, auth */)?;
    let service: RunningService<RoleClient, ()> = ().serve(transport).await?;
    self.connections.insert(server.id, PoolEntry { service, transport: TransportType::Sse, .. });
    Ok(())
}
```

#### Step 3.2: Add `connect_managed()` Method

Handles both `ManagedSse` and `ManagedHttp`:

```rust
async fn connect_managed(
    &self,
    server: &McpServerRow,
    server_id: Uuid,
    transport_type: TransportType,
) -> Result<(), McpError> {
    // 1. Enforce max managed processes limit (same as stdio)
    // 2. Parse ManagedHttpServerConfig
    // 3. Spawn child process (inherit stderr, pipe stdin for lifecycle coupling)
    // 4. Construct URL: http://localhost:{config.port}{config.path}
    // 5. Retry-connect via SSE or StreamableHttp until ready or timeout
    // 6. Register PID in terminator manifest
    // 7. Insert into pool with appropriate TransportType + child process handle
}
```

This method combines the process spawning logic from `connect_stdio()` with the network connection logic from `connect_http()` / `connect_sse()`.

**Key difference from stdio:** The child process communicates over the network, not stdin/stdout. The port is known from config; nize retries connecting until the server is ready (up to `ready_timeout_secs`).

#### Step 3.3: Update `get_or_connect()` Match

```rust
match transport_type {
    TransportType::Http => self.connect_http(&server, oauth_headers).await?,
    TransportType::Stdio => self.connect_stdio(&server, server_id).await?,
    TransportType::Sse => self.connect_sse(&server, oauth_headers).await?,
    TransportType::ManagedSse | TransportType::ManagedHttp => {
        self.connect_managed(&server, server_id, transport_type).await?
    }
}
```

#### Step 3.4: Update Eviction to Use `is_managed()`

Replace all `transport == TransportType::Stdio` checks with `transport.is_managed()`:
- `stdio_count()` → `managed_count()`
- `evict_idle()` — evict managed entries (not just stdio)
- `evict_lru_stdio()` → `evict_lru_managed()`
- `managed_count()` checks all managed types (stdio, managed-sse, managed-http)
- `max_stdio_processes` → `max_managed_processes`

### Phase 4: Connection Testing for SSE

#### Step 4.1: Add `test_sse_connection()`

Similar to `test_http_connection()` but using `SseClientTransport`:
- Connect to SSE endpoint
- Perform MCP `initialize` handshake
- List tools
- Return `TestConnectionResult`

#### Step 4.2: Update `test_connection` in McpConfigService

Route to appropriate test function based on transport type:
- `Stdio` → existing stdio test
- `Http` / `ManagedHttp` → existing `test_http_connection()`
- `Sse` / `ManagedSse` → new `test_sse_connection()`

For managed types, a test-connection spawns a temporary process (same as stdio test_connection).

### Phase 5: Managed Process Lifecycle

#### Step 5.1: Spawn + Connect Helper

Create helper that spawns a child process and waits for the server to accept connections:

```rust
async fn spawn_managed_process(
    config: &ManagedHttpServerConfig,
) -> Result<tokio::process::Child, McpError> {
    // 1. Build Command from config (command, args, env)
    // 2. Pipe stdin (lifecycle coupling), inherit stderr
    // 3. Spawn
    // 4. Return child handle
}

async fn wait_for_ready(
    url: &str,
    timeout: Duration,
) -> Result<(), McpError> {
    // Retry HTTP GET to url every 500ms until success or timeout
    // Connection refused → retry; other errors → retry; timeout → McpError
}
```

The port is known from `ManagedHttpServerConfig.port`. After spawning, nize calls `wait_for_ready()` then connects via SSE or StreamableHttp transport.

#### Step 5.2: Managed Process Tracking in PoolEntry

Extend `PoolEntry` to optionally hold a `tokio::process::Child` handle:

```rust
struct PoolEntry {
    service: RunningService<RoleClient, ()>,
    transport: TransportType,
    last_accessed: AtomicU64,
    created_at: Instant,
    child_process: Option<tokio::process::Child>,  // for managed transports
}
```

When the entry is removed/evicted, the child process is killed alongside the service cancellation.

#### Step 5.3: Terminator Manifest for Managed Processes

Same as stdio (PLAN-025 Phase 5): write `kill <pid>` to manifest on spawn.

### Phase 6: McpConfigService Validation Updates

#### Step 6.1: Validate New Transport Configs

- `Sse` servers: validate URL, enforce HTTPS (except localhost)
- `ManagedSse` / `ManagedHttp` servers: validate command exists, admin-only creation
- Same security model as stdio for managed types

#### Step 6.2: User Server Creation Rules

- Users can create: `Http`, `Sse` (external servers only)
- Admins can create: all transport types
- Enforce via `McpConfigService` validation (extend existing admin-only check)

### Phase 7: UI Updates (nize-web)

#### Step 7.1: Transport Selection in Server Creation Form

- Admin form: show all 5 transport types
- User form: show only `Http` and `Sse`
- Dynamic form fields based on selected transport

#### Step 7.2: Transport Badge in Server List

Display transport type badge on server cards (e.g. "SSE", "Managed HTTP").

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Legacy SSE protocol implementation bugs | Tool calls fail for SSE servers | Thorough testing against reference MCP SSE servers (e.g. gogmcp) |
| Managed process port discovery race | Port not ready when we connect | Ready timeout + retry on connection refused |
| DB migration enum extension | Breaking change for existing data | `ADD VALUE IF NOT EXISTS` is safe; existing `stdio`/`http` values unchanged |
| Managed process resource leak | Orphaned HTTP server processes | Terminator manifest + child process handle drop kills process |
| `sse-stream` API compatibility | Transitive dep version may conflict | Pin as direct dependency at compatible version |
| Port conflicts | Configured port may be in use | Admin responsibility; connection failure reported clearly |

## Resolved Questions

1. **Port discovery:** Resolved — port is a config parameter on `ManagedHttpServerConfig`. Admin specifies the port explicitly and ensures it's passed to the managed process via args/env. No stdout JSON parsing needed.

2. **Ready detection:** Resolved — nize retries connecting to `http://localhost:{port}{path}` until the server accepts connections or `ready_timeout_secs` elapses. No `ready_pattern` regex needed.

3. **Config key rename:** Resolved — rename `mcp.max_stdio_processes` to `mcp.max_managed_processes` with a DB migration updating the `config_definitions` key. Rust code renames accordingly.

## Completion Criteria

- [ ] `SseClientTransport` implements rmcp `Transport<RoleClient>` trait
- [ ] SSE transport tested against a real legacy SSE MCP server
- [ ] DB `transport_type` enum extended with `sse`, `managed-sse`, `managed-http`
- [ ] Rust `TransportType` enum has 5 variants with `is_managed()` helper
- [ ] `ServerConfig` has 5 variants with appropriate config types
- [ ] `ClientPool.get_or_connect()` routes all 5 transport types
- [ ] `connect_sse()` works for external SSE servers
- [ ] `connect_managed()` spawns process, discovers port, connects via SSE or HTTP
- [ ] Managed process PID tracked in terminator manifest
- [ ] LRU eviction applies to all managed transports (not just stdio)
- [ ] `test_connection` works for all 5 transport types
- [ ] TypeSpec API contract updated with new transport values
- [ ] McpConfigService validates new transport configs
- [ ] Admin-only restriction for managed transports
- [ ] nize-web UI allows creating/viewing servers with new transport types
