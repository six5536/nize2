# PLAN-024: Live Tool Discovery & Hook/Middleware Pipeline

**Status:** in-progress
**Workflow direction:** top-down
**Traceability:** PLAN-019 (MCP semantic tool discovery stubs); PLAN-020 (MCP service registration); PLAN-022 (text embedding generation); PLAN-023 (admin embeddings UI); Reference project `submodules/nize/` — `packages/agent/src/mcp/discovery.ts`, `execution-proxy.ts`, `meta-tools.ts`, `tool-index.ts`

## Goal

Two deliverables:

1. **Wire the 5 MCP meta-tools to real data** — replace the dummy/stub handlers in `nize_mcp::server` with calls to the embedding search and registered MCP server tools (pgvector similarity, tool index, execution proxy).

2. **Implement a hook/middleware pipeline** — configurable per-server and per-tool hooks that run before/after every MCP tool call (both our meta-tools and proxied calls to external MCP servers). Hooks apply at global and per-user scope.

## Context

### Current State

| Component | Status |
|-----------|--------|
| 5 meta-tool MCP stubs (`nize_mcp::server`) | Done — return dummy data (PLAN-019) |
| DB tables: `mcp_servers`, `mcp_server_tools`, `user_mcp_preferences` | Done (PLAN-020) |
| Embedding infra: providers, indexer, pgvector tables | Done (PLAN-022) |
| Admin search endpoint (`POST /admin/embeddings/search`) | Done (PLAN-023) |
| `nize_core::embedding::indexer::embed_server_tools` | Done |
| `nize_core::embedding::embed_single` | Done |
| User preferences service (`McpConfigService`) | Done (PLAN-020) |
| Actual MCP execution proxy (calling external servers via rmcp client) | **Not done** |
| Hook/middleware pipeline | **Not done** |

### Ref Project Approach

In `submodules/nize/`, tool discovery is implemented as internal Vercel AI SDK tools within the TypeScript agent. The `MetaToolHandler` delegates to `DiscoveryService` (pgvector search), `ExecutionProxy` (proxied calls + audit), and `UserPreferences` (access control). There is no explicit hook/middleware system — audit, access control, and response limiting are hard-coded inside the execution proxy.

### Key Differences from Ref Project

| Aspect | Ref Project (nize) | nize-mcp |
|--------|-------------------|----------|
| Transport | Internal Vercel AI SDK | MCP Streamable HTTP (rmcp) |
| Tool surface | SDK tool defs | First-class MCP `#[tool]` |
| Execution | Direct SDK calls | rmcp client proxy |
| Hooks | None (hardcoded audit/RSL) | Configurable pipeline (this plan) |
| Scope | Per-user only | Global + per-user hooks |

## Part 1: Wire Meta-Tools to Real Data

### 1.1 Discovery Service (`nize_core::mcp::discovery`)

Create a discovery service module in `nize_core` that:
- Accepts a query string and optional domain filter
- Resolves the active embedding model via `embedding::config::EmbeddingConfig`
- Embeds the query via `embedding::embed_single`
- Runs cosine similarity search against the active tool embedding table
- Filters by user-enabled servers (via `user_mcp_preferences`)
- Returns ranked `DiscoveredTool` results with server metadata

```rust
pub struct DiscoveryQuery {
    pub query: String,
    pub domain: Option<String>,
    pub user_id: String,
    pub top_k: Option<i64>,
    pub min_similarity: Option<f64>,
}

pub struct DiscoveredToolRow {
    pub tool_id: Uuid,
    pub tool_name: String,
    pub tool_description: String,
    pub domain: String,
    pub server_id: Uuid,
    pub server_name: String,
    pub server_description: String,
    pub similarity: f64,
}

pub async fn discover_tools(
    pool: &PgPool,
    config_cache: &Arc<RwLock<ConfigCache>>,
    query: &DiscoveryQuery,
) -> Result<Vec<DiscoveredToolRow>, DiscoveryError>;
```

Reuse the similarity SQL pattern from the admin search handler (`PLAN-023`), but add the user preference filter.

### 1.2 Tool Manifest Lookup

Add `get_tool_manifest` to `nize_core::mcp::queries`:
- Fetch a single tool from `mcp_server_tools` by ID
- Return the stored JSONB manifest
- Verify user has access (server enabled for user)

### 1.3 Domain Listing

Add `list_tool_domains` and `browse_tool_domain` to `nize_core::mcp::queries`:
- `list_tool_domains`: `SELECT DISTINCT domain, COUNT(*) as tool_count FROM mcp_server_tools GROUP BY domain` — filtered by servers enabled for user
- `browse_tool_domain`: list all tools in a domain, filtered by user-enabled servers — no embedding needed, just a DB query

### 1.4 Execution Proxy (`nize_core::mcp::execution`)

Create an execution proxy that:
- Validates the tool ID exists and user has access
- Validates parameters against the tool's input schema
- Connects to the external MCP server via rmcp client (HTTP transport)
- Calls the tool with the provided parameters
- Records audit log entry
- Returns the result

```rust
pub struct ExecutionRequest {
    pub tool_id: Uuid,
    pub tool_name: String,
    pub params: serde_json::Value,
    pub user_id: String,
}

pub struct ExecutionResult {
    pub success: bool,
    pub tool_name: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<ExecutionError>,
}

pub async fn execute_tool(
    pool: &PgPool,
    request: &ExecutionRequest,
) -> Result<ExecutionResult, ExecutionError>;
```

For MVP, connect via rmcp's HTTP client transport. Stdio transport is deferred (desktop-only use case).

### 1.5 Update `NizeMcpServer` Handlers

Replace each dummy call in `crates/lib/nize_mcp/src/server.rs`:
- `discover_tools` → call `nize_core::mcp::discovery::discover_tools`
- `get_tool_schema` → call `nize_core::mcp::queries::get_tool_manifest`
- `execute_tool` → call `nize_core::mcp::execution::execute_tool`
- `list_tool_domains` → call `nize_core::mcp::queries::list_tool_domains`
- `browse_tool_domain` → call `nize_core::mcp::queries::browse_tool_domain`

The `NizeMcpServer` needs additional state:
- `Arc<RwLock<ConfigCache>>` for embedding config resolution
- User ID from the MCP auth context (currently not extracted — see 1.6)

### 1.6 Pass User Context to MCP Tools

Currently, `mcp_auth_middleware` validates the token but doesn't pass user info downstream to the `NizeMcpServer` instance. Options:

**Solution (confirmed via rmcp source analysis):** The Axum auth middleware inserts a `McpUser` struct into `request.extensions()` after token validation. rmcp's `StreamableHttpService` injects `http::request::Parts` (including extensions) into each tool handler's context. Tool handlers extract user info via `Extension<http::request::Parts>` → `parts.extensions.get::<McpUser>()`.

This works because:
1. Auth middleware runs as an Axum layer BEFORE rmcp handles the request
2. rmcp calls `req.request.extensions_mut().insert(part)` for every POST
3. Tool handlers can use rmcp's `Extension<T>` extractor to get `Parts`
4. Each HTTP request to `/mcp` runs through the auth middleware, so user context is fresh per-call

## Part 2: Hook/Middleware Pipeline

### 2.1 Concept

A hook is a unit of logic that runs before and/or after an MCP tool call. Hooks form an ordered pipeline. Each hook can:
- **Inspect** — read the request/response (logging, telemetry)
- **Transform** — modify the request params or response result
- **Reject** — block a call by returning an error (access control, rate limiting)

Hooks apply independently to:
- **Our meta-tools** (`discover_tools`, `execute_tool`, etc.)
- **Proxied external tool calls** (the actual call made by `execute_tool` to an external MCP server)

### 2.2 Hook Scopes

| Scope | Applied to | Configurable by |
|-------|-----------|-----------------|
| Global | All tool calls across all servers | Admin |
| Per-server | All tool calls to a specific MCP server | Admin |
| Per-user | All tool calls by a specific user | User or Admin |
| Per-user-per-server | Tool calls by a user to a specific server | User |

Resolution order (pipeline order): global → per-server → per-user → per-user-per-server

### 2.3 Hook Interface

```rust
/// Context passed to hooks
pub struct HookContext {
    pub user_id: String,
    pub server_id: Option<Uuid>,        // None for meta-tools
    pub tool_name: String,
    pub tool_id: Option<Uuid>,          // None for meta-tools
    pub scope: HookScope,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub enum HookScope {
    Global,
    Server(Uuid),
    User(String),
    UserServer(String, Uuid),
}

/// Hook trait — implement for custom hook logic
#[async_trait]
pub trait ToolHook: Send + Sync {
    /// Called before tool execution. Return Err to reject.
    async fn before_call(
        &self,
        ctx: &HookContext,
        params: &mut serde_json::Value,
    ) -> Result<(), HookError>;

    /// Called after tool execution.
    async fn after_call(
        &self,
        ctx: &HookContext,
        result: &mut ToolCallOutcome,
    ) -> Result<(), HookError>;

    /// Hook identifier for debugging/logging
    fn name(&self) -> &str;
}

pub enum ToolCallOutcome {
    Success(serde_json::Value),
    Error(String),
}
```

### 2.4 Built-in Hooks

| Hook | Scope | Purpose |
|------|-------|---------|
| `AuditHook` | Global | Log all tool calls to `mcp_config_audit` — always first in pipeline |
| `AccessControlHook` | Global | Verify user has access to the server/tool — reject if not enabled |
| `RateLimitHook` | Global / per-user | Rate limit tool calls (future) |
| `ParamSanitizerHook` | Per-server | Sanitize/validate params before forwarding (future) |
| `ResponseTruncationHook` | Global | Apply response size limits |

For MVP, implement `AuditHook` and `AccessControlHook`. Others are future.

### 2.5 Hook Registry & Pipeline

```rust
pub struct HookPipeline {
    hooks: Vec<(HookScope, Arc<dyn ToolHook>)>,
}

impl HookPipeline {
    /// Run all before_call hooks in order. Short-circuit on error.
    pub async fn run_before(
        &self,
        ctx: &HookContext,
        params: &mut serde_json::Value,
    ) -> Result<(), HookError>;

    /// Run all after_call hooks in reverse order.
    pub async fn run_after(
        &self,
        ctx: &HookContext,
        result: &mut ToolCallOutcome,
    ) -> Result<(), HookError>;
}
```

The pipeline is constructed at server startup from:
1. Hard-coded built-in hooks (audit, access control)
2. DB-configured hooks (admin-defined per-server/per-user hooks — future)

### 2.6 Database Schema for Custom Hooks (Future)

For MVP, hooks are Rust structs registered in code. Future iterations could support DB-driven hook configuration, e.g.:

```sql
CREATE TABLE mcp_hooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    hook_type VARCHAR(50) NOT NULL,    -- 'audit', 'access_control', 'rate_limit', 'custom'
    scope VARCHAR(50) NOT NULL,         -- 'global', 'server', 'user', 'user_server'
    scope_id VARCHAR(100),              -- server_id or user_id depending on scope
    config JSONB,                       -- hook-specific configuration
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

This table design is documented here for future reference but is **not** implemented in this plan.

### 2.7 Integration Points

The hook pipeline wraps two call sites:

1. **Meta-tool calls** — each `#[tool]` method in `NizeMcpServer` runs the pipeline around its logic
2. **External tool proxy** — `execute_tool` runs the pipeline around the rmcp client call

```
LLM → NizeMcpServer::execute_tool
        ├── meta-tool pipeline: before_call → execute_tool logic → after_call
        │       └── within execute_tool:
        │           └── proxy pipeline: before_call → rmcp_client.call_tool → after_call
```

This gives two interception points: one at the MCP interface level, one at the proxy level.

## Implementation Steps

### Phase 1: Discovery Service (nize_core)

#### Step 1.1: Create `nize_core::mcp::discovery` module
- `DiscoveryQuery`, `DiscoveredToolRow` types
- `discover_tools()` function: embed query → pgvector search → filter by user preferences
- Reuse `embedding::embed_single` and active model resolution

#### Step 1.2: Add domain queries to `nize_core::mcp::queries`
- `list_tool_domains(pool, user_id)` — distinct domains with tool counts, filtered by enabled servers
- `browse_tool_domain(pool, user_id, domain_id)` — all tools in domain, filtered
- `get_tool_manifest(pool, user_id, tool_id)` — full manifest with access check

#### Step 1.3: Unit tests for discovery
- Test `discover_tools` with local embedding provider
- Test domain listing
- Test access filtering (user without server access gets empty results)

### Phase 2: Execution Proxy (nize_core)

#### Step 2.1: Create `nize_core::mcp::execution` module
- `ExecutionRequest`, `ExecutionResult`, `ExecutionError` types
- `execute_tool()` function: validate → connect via rmcp → call tool → audit log

#### Step 2.2: rmcp client integration
- Add `transport-streamable-http-client-reqwest` and `client` features to `rmcp` dependency in `Cargo.toml`
- Use `StreamableHttpClientTransport::from_uri(url)` to create transport, `().serve(transport).await` to establish session
- Use `service.call_tool(CallToolRequestParams { name, arguments, .. })` for tool execution
- Connection pooling: `Arc<DashMap<Uuid, RunningService<RoleClient, ()>>>` keyed by server ID
- Reconnect-on-error: on `ServiceError`, remove from pool and retry once
- Timeouts: 30 seconds per tool call by default (tokio::time::timeout)

#### Step 2.3: Unit tests for execution
- Test with mock MCP server (rmcp provides test utilities)
- Test access denied when server not enabled for user
- Test audit log recording

### Phase 3: Hook Pipeline (nize_mcp)

#### Step 3.1: Define hook trait and types
- Create `crates/lib/nize_mcp/src/hooks/mod.rs`
- `ToolHook` trait, `HookContext`, `HookScope`, `ToolCallOutcome`
- `HookPipeline` with ordered hook execution

#### Step 3.2: Implement `AuditHook`
- `after_call`: insert into `mcp_config_audit` (fire-and-forget)
- Always enabled, global scope, first in pipeline

#### Step 3.3: Implement `AccessControlHook`
- `before_call`: check `user_mcp_preferences` for server access
- Reject with clear error if user doesn't have access
- Global scope

#### Step 3.4: Tests for hook pipeline
- Verify pipeline execution order (before → handler → after)
- Verify short-circuit on `before_call` error
- Verify `after_call` runs in reverse order
- Verify audit hook records entries

### Phase 4: Wire Everything Together (nize_mcp)

#### Step 4.1: Update `NizeMcpServer` state and auth middleware
- Add `Arc<RwLock<ConfigCache>>` for embedding config
- Add `HookPipeline` (wrapped in `Arc`) for hook execution
- Add `McpUser` struct to `nize_mcp::auth` module
- Update `mcp_auth_middleware` to insert `McpUser` into `request.extensions_mut()` after successful token validation
- Tool handlers extract user via `Extension<http::request::Parts>` → `parts.extensions.get::<McpUser>()`

#### Step 4.2: Replace dummy handlers
- `discover_tools` → `nize_core::mcp::discovery::discover_tools` wrapped in hook pipeline
- `get_tool_schema` → `nize_core::mcp::queries::get_tool_manifest` wrapped in hooks
- `execute_tool` → `nize_core::mcp::execution::execute_tool` with nested hook pipelines
- `list_tool_domains` → `nize_core::mcp::queries::list_tool_domains` wrapped in hooks
- `browse_tool_domain` → `nize_core::mcp::queries::browse_tool_domain` wrapped in hooks

#### Step 4.3: Update `mcp_router` factory
- Pass `ConfigCache` and construct `HookPipeline` with built-in hooks
- Pass user context from auth middleware to `NizeMcpServer` factory

#### Step 4.4: Integration tests
- End-to-end: register server → index tools → discover via MCP tool → execute
- Verify audit trail entries
- Verify access control enforcement

### Phase 5: Build & Verify

1. `cargo build` — compilation succeeds
2. `cargo clippy` — no warnings
3. `cargo test` — all tests pass (existing + new)
4. Manual test: connect Claude Desktop → call `discover_tools` → get real results

## Dependencies

| Dependency | Status |
|-----------|--------|
| PLAN-019 meta-tool stubs | Completed |
| PLAN-020 MCP service registration | Completed |
| PLAN-022 embedding infrastructure | Completed |
| PLAN-023 admin embeddings UI | Completed |
| rmcp HTTP client transport | Available (rmcp crate — requires adding `transport-streamable-http-client-reqwest` and `client` features) |
| pgvector similarity search | Available (migration 0005) |
| `dashmap` crate | Needs adding to workspace (for client pool) |

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| rmcp client API may differ from server API patterns | Low (confirmed) | Client uses `().serve(transport).await` → `service.call_tool()`. Pattern is straightforward. |
| User context propagation through rmcp sessions | Low (resolved) | Auth middleware inserts `McpUser` into request extensions; tool handlers extract via `Extension<Parts>`. Confirmed in rmcp source. |
| Client pool stale sessions | Medium | Reconnect-on-error pattern; remove dead entries from `DashMap` on connection failure |
| Tool execution timeouts and error handling | Low | Use reqwest/tokio timeouts; follow ref project's error code pattern |
| Hook pipeline performance overhead | Low | Hooks are in-process async calls; audit is fire-and-forget |
| pgvector similarity threshold tuning | Low | Start with 0.5 min similarity (matches ref project); tune via admin config |
| Feature flag addition for rmcp client | Low | Need `transport-streamable-http-client-reqwest` + `client` features on rmcp dependency |

## Research Findings

### Q1: rmcp Client Reuse / Connection Pooling

**Finding:** rmcp does NOT provide built-in connection pooling. Each `().serve(transport).await` call creates a dedicated `RunningService` that:
- Establishes a new MCP session (initialize handshake)
- Maintains a persistent SSE connection for server notifications
- Returns a `Peer<RoleClient>` with methods like `call_tool()`, `list_tools()`
- Must be explicitly closed via `service.cancel().await`

The `reqwest::Client` used underneath does pool HTTP connections (via hyper), but the MCP session layer on top is stateful and per-connection.

**Tradeoffs:**

| Approach | Pros | Cons |
|----------|------|------|
| **A: Client-per-call** (create → call → close) | Simple, no state mgmt | High latency (MCP handshake per call), wasteful |
| **B: Long-lived client pool** (`Arc<DashMap<Uuid, RunningService>>`) | Fast calls, amortized handshake | Must handle disconnects, server restarts, stale sessions |
| **C: Lazy pool with TTL** (pool + idle timeout) | Balanced; auto-cleanup | More complex; need background reaper task |

**Decision: Option B with reconnect-on-error.** Store `RunningService<RoleClient, ()>` (the `()` client handler) per server ID in an `Arc<DashMap<Uuid, RunningService>>`. On connection error during `call_tool()`, remove the entry and reconnect. This matches the ref project's pooled client pattern (`getPooledClientFromConfig()`).

The transport is created via:
```rust
let transport = StreamableHttpClientTransport::from_uri(server_url);
let service = ().serve(transport).await?;
// service.call_tool(params).await
```

For servers requiring auth, use `StreamableHttpClientTransportConfig::with_uri(url).auth_header(token)`.

### Q2: User Context Propagation in rmcp Sessions

**Finding:** rmcp injects `http::request::Parts` into the tool handler's `Extensions` for every request. This means:

1. The Axum auth middleware runs BEFORE rmcp receives the request (it's a layer on the outer router)
2. The middleware can **insert data into request extensions** before passing to `next.run()`
3. rmcp's `StreamableHttpService` picks up the `Parts` (including extensions) and injects them into the tool handler context
4. Tool handlers can extract them via `Extension<T>` extractor

**Solution:** Modify `mcp_auth_middleware` to insert a `McpUser` struct into request extensions after successful validation:

```rust
#[derive(Clone, Debug)]
pub struct McpUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

pub async fn mcp_auth_middleware(...) -> Result<Response, StatusCode> {
    // ... validate token ...
    match nize_core::auth::mcp_tokens::validate_mcp_token(&pool, &token).await {
        Ok(Some(user)) => {
            let mut request = request;
            request.extensions_mut().insert(McpUser {
                id: user.id,
                email: user.email,
                name: user.name,
            });
            Ok(next.run(request).await)
        }
        // ...
    }
}
```

Then in tool handlers, extract via rmcp's `Extension<Parts>` → read `Parts.extensions.get::<McpUser>()`:

```rust
#[tool(description = "Search for tools")]
fn discover_tools(
    &self,
    Extension(parts): Extension<http::request::Parts>,
    Parameters(req): Parameters<DiscoverToolsRequest>,
) -> Result<CallToolResult, ErrorData> {
    let user = parts.extensions.get::<McpUser>()
        .ok_or_else(|| ErrorData::internal_error("missing user context", None))?;
    // user.id is now available
}
```

**Caveat:** In stateful mode, `Parts` are only injected for the first request (session creation/initialize) and subsequent requests within the same session. Each POST to `/mcp` with a session ID injects fresh `Parts` from that HTTP request, so the auth middleware runs on every call — this is correct behavior.

### Q3: Hook Persistence

**Decision:** Code-only registration for now. DB-stored custom hooks are out-of-scope for this plan but documented in Section 2.6 for future work.

## Completion Criteria

- [ ] `discover_tools` returns real pgvector similarity results filtered by user preferences
- [ ] `list_tool_domains` and `browse_tool_domain` return real data from `mcp_server_tools`
- [ ] `get_tool_schema` returns stored manifest from DB
- [ ] `execute_tool` proxies to external MCP servers via rmcp client
- [ ] All tool calls pass through the hook pipeline
- [ ] `AuditHook` records entries in `mcp_config_audit`
- [ ] `AccessControlHook` blocks calls to servers the user hasn't enabled
- [ ] Existing tests continue to pass
- [ ] New unit tests cover discovery, execution, and hook pipeline
