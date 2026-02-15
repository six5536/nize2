# PLAN-020: MCP Service Registration via `/mcp/...` Routes

**Status:** completed
**Workflow direction:** lateral
**Traceability:** Reference project `submodules/nize/` — `packages/agent/src/mcp/`, `packages/db/src/schema/mcp.ts`, `apps/web/app/settings/tools/`, `apps/web/app/admin/tools/`

## Goal

Replace the demo/stub MCP config handlers (added in PLAN-017) with a fully working implementation: database-backed MCP server registration, user preferences, server tools, connection testing, and the corresponding web UI in nize-web. The design mirrors the reference project's `ConfigService`, `ServerRegistry`, and `UserPreferences` implementations.

## Scope

**In-scope:**
- Database migration for MCP tables (`mcp_servers`, `mcp_server_tools`, `user_mcp_preferences`, `mcp_server_secrets`, `mcp_oauth_tokens`, `mcp_config_audit`)
- `nize_core` domain models and DB queries for MCP entities
- `nize_api` service layer (`McpConfigService`) implementing business logic
- `nize_api` handler updates: real DB-backed responses replacing stubs
- `nize-web` UI: `/settings/tools` page (user server management)
- `nize-web` UI: `/admin/tools` page (admin server management)
- MCP server connection testing via rmcp client (basic HTTP transport)

**Out-of-scope:**
- Semantic tool discovery / text embeddings / pgvector search (PLAN-019)
- MCP tool execution proxy
- Stdio transport support (desktop-only, deferred)
- OAuth flow implementation (stub only — initiate/callback/revoke)
- Audit log UI (backend audit logging is in-scope, UI is not)
- Response size limits (RSL feature)

## Current State

| Layer | Status |
|-------|--------|
| TypeSpec contracts | Done — `API-NIZE-mcp-config.tsp` |
| OpenAPI + codegen | Done — route constants + models generated |
| Router wiring | Done — all `/mcp/...` routes wired to handlers |
| Handlers | Done — DB-backed handlers using McpConfigService |
| DB migration | Done — `0004_mcp_servers.sql` (6 tables) |
| nize_core models | Done — `models/mcp.rs` (enums, rows, views, configs) |
| nize_core queries | Done — `mcp/queries.rs` (full CRUD) |
| nize_core secrets | Done — `mcp/secrets.rs` (AES-256-GCM encryption) |
| nize_api service | Done — `services/mcp_config.rs` (McpConfigService) |
| nize-web UI | Done — `/settings/tools` + `/admin/tools` + admin layout |

## Implementation Plan

### Phase 1: Database Layer (nize_core)

#### Step 1.1: Migration — MCP Tables

Create `crates/lib/nize_core/migrations/0004_mcp_servers.sql`

Tables (ported from ref project `packages/db/src/schema/mcp.ts`):

- **`mcp_servers`** — Server registrations with visibility tiers, transport config, OAuth config
  - Columns: `id`, `name`, `description`, `domain`, `endpoint`, `visibility`, `transport`, `config` (JSONB), `oauth_config` (JSONB), `default_response_size_limit`, `owner_id`, `enabled`, `available`, `created_at`, `updated_at`
  - Enums: `visibility_tier` (hidden, visible, user), `transport_type` (stdio, http), `auth_type` (none, api-key, oauth)
  - Indexes: domain, enabled, visibility, owner_id

- **`mcp_server_tools`** — Tool manifests fetched from servers
  - Columns: `id`, `server_id`, `name`, `description`, `manifest` (JSONB), `response_size_limit`, `created_at`
  - Unique index: (server_id, name)

- **`user_mcp_preferences`** — Per-user server enablement toggles
  - Columns: `user_id`, `server_id`, `enabled`, `updated_at`
  - Primary key: (user_id, server_id)

- **`mcp_server_secrets`** — Encrypted API keys and OAuth client secrets
  - Columns: `id`, `server_id`, `api_key_encrypted`, `oauth_client_secret_encrypted`, `encryption_key_id`, `created_at`, `updated_at`

- **`mcp_oauth_tokens`** — Per-user OAuth tokens for MCP servers
  - Columns: `user_id`, `server_id`, `access_token_encrypted`, `refresh_token_encrypted`, `expires_at`, `scopes`, `created_at`, `updated_at`

- **`mcp_config_audit`** — Audit log for MCP config changes
  - Columns: `id`, `actor_id`, `server_id`, `server_name`, `action`, `details` (JSONB), `reason`, `created_at`

#### Step 1.2: Domain Models

Create `crates/lib/nize_core/src/models/mcp.rs`

Structs (aligned with ref project `packages/agent/src/mcp/types.ts`):

```rust
// Enums
pub enum VisibilityTier { Hidden, Visible, User }
pub enum TransportType { Stdio, Http }
pub enum AuthType { None, ApiKey, OAuth }
pub enum ServerStatus { Enabled, Disabled, Unavailable, Unauthorized }

// DB row structs
pub struct McpServerRow { ... }  // Direct DB mapping
pub struct McpServerToolRow { ... }
pub struct UserMcpPreferenceRow { ... }

// View structs (computed, returned to API)
pub struct UserServerView { id, name, domain, visibility, status, tool_count, is_owned }
pub struct AdminServerView { id, name, domain, visibility, status, tool_count, is_owned, transport, auth_type, owner_id, user_preference_count, enabled, available }

// Config types (stored in JSONB)
pub struct HttpServerConfig { transport, url, headers, auth_type, api_key_header }
pub struct StdioServerConfig { transport, command, args, env }

// Request types
pub struct CreateUserServerInput { name, description, domain, url, headers, auth_type, api_key, api_key_header }
pub struct UpdateUserServerInput { name, description, domain, url, headers, auth_type, api_key, api_key_header }
pub struct CreateBuiltInServerInput { name, description, domain, visibility, transport, ... }
pub struct UpdateBuiltInServerInput { name, description, domain, visibility, enabled, ... }

// Validation / connection testing
pub struct TestConnectionResult { success, server_name, server_version, tool_count, error }
pub struct McpToolSummary { name, description, input_schema }
```

#### Step 1.3: DB Queries

Create `crates/lib/nize_core/src/mcp/` module:

- `queries.rs` — Raw SQLx queries:
  - `list_servers_for_user(pool, user_id)` — SELECT with visibility/owner filter
  - `get_user_preferences(pool, user_id)` — SELECT from user_mcp_preferences
  - `get_server(pool, server_id)` — SELECT by ID
  - `insert_server(pool, ...)` — INSERT into mcp_servers
  - `update_server(pool, server_id, ...)` — UPDATE mcp_servers
  - `delete_server(pool, server_id)` — DELETE
  - `count_user_servers(pool, user_id)` — COUNT where visibility=user
  - `set_user_preference(pool, user_id, server_id, enabled)` — UPSERT
  - `list_server_tools(pool, server_id)` — SELECT from mcp_server_tools
  - `replace_server_tools(pool, server_id, tools)` — DELETE + INSERT
  - `get_tool_count(pool, server_id)` — COUNT
  - `list_all_servers(pool)` — Admin: SELECT all
  - `insert_audit_log(pool, ...)` — INSERT into mcp_config_audit

### Phase 2: Service Layer (nize_api)

#### Step 2.1: McpConfigService

Create `crates/lib/nize_api/src/services/mcp_config.rs`

Ported from ref project `packages/agent/src/mcp/config-service.ts`:

- **User operations:**
  - `get_servers_for_user(pool, user_id)` → `Vec<UserServerView>`
  - `create_user_server(pool, user_id, input)` → `UserServerView`
  - `update_user_server(pool, user_id, server_id, input)` → `UserServerView`
  - `delete_user_server(pool, user_id, server_id)` → `()`
  - `set_user_preference(pool, user_id, server_id, enabled)` → `()`
  - `get_server_tools(pool, server_id)` → `Vec<McpToolSummary>`

- **Admin operations:**
  - `get_all_servers(pool)` → `Vec<AdminServerView>`
  - `create_built_in_server(pool, admin_id, input)` → `AdminServerView`
  - `update_built_in_server(pool, admin_id, server_id, input)` → `AdminServerView`
  - `delete_built_in_server(pool, server_id)` → `DeleteResult`

- **Validation:**
  - `validate_http_config(config)` → `Result<()>`
  - `compute_server_status(server, user_id, preference)` → `ServerStatus`
  - Enforce 10-server limit per user
  - Enforce unique server name per user
  - HTTPS-only (except localhost)

- **Connection testing:**
  - `test_connection(config)` → `TestConnectionResult`
  - For MVP: basic HTTP probe to server URL, verify MCP protocol handshake
  - Stretch: use rmcp client to actually connect and list tools

#### Step 2.2: Update Handlers

Update `crates/lib/nize_api/src/handlers/mcp_config.rs`:

Replace every handler stub with real implementation:

- Inject `AppState` (contains `PgPool`) into each handler
- Extract authenticated user from middleware (`Extension<TokenClaims>`)
- Call `McpConfigService` methods
- Map domain errors to appropriate HTTP status codes
- Return typed JSON responses (use generated models where possible, or define response types)

### Phase 3: Connection Testing

#### Step 3.1: Basic MCP Client Probe

Implement `test_connection` in `crates/lib/nize_api/src/services/mcp_config.rs`:

- For HTTP transport: make an HTTP POST to the server URL with MCP `initialize` request
- Parse the MCP `ServerInfo` response (name, version, protocol version)
- Optionally follow up with `tools/list` to get tool count
- Timeout: 10 seconds
- For MVP, can use `reqwest` directly with MCP JSON-RPC format
- Future: integrate rmcp client SDK

### Phase 4: Web UI (nize-web)

#### Step 4.1: User Tools Page — `/settings/tools`

Port from ref project `apps/web/app/settings/tools/page.tsx`:

- Server list with status badges (enabled/disabled/unavailable/unauthorized)
- Toggle switch for enable/disable
- Expand to see tool list
- "Add Server" form:
  - Name, description (500 char max), domain, URL, auth type, API key
  - "Test Connection" button
  - Submit only after successful test
- Edit/delete for owned servers
- Error handling and loading states

#### Step 4.2: Admin Tools Page — `/admin/tools`

Port from ref project `apps/web/app/admin/tools/page.tsx`:

- Server list grouped by visibility tier
- Create built-in server form (supports hidden/visible, stdio/http)
- Edit/delete for built-in servers
- Toggle enabled/disabled
- View user preference counts
- Force-delete with affected user warning

#### Step 4.3: Navigation

- Add "Tools" link to settings sidebar/navigation
- Add "Tools" link to admin sidebar/navigation
- Ensure auth gating (redirect to login if unauthenticated)

### Phase 5: Integration Testing

#### Step 5.1: Rust Tests

- `nize_core` query tests (requires test DB or mock)
- `nize_api` handler integration tests:
  - Create server → list → update → delete lifecycle
  - Preference toggle
  - Admin CRUD
  - 10-server limit enforcement
  - Duplicate name rejection
  - Auth enforcement (401 on unauthenticated, 403 on non-admin)

#### Step 5.2: API Smoke Tests

- Update Bruno collection to use real payloads (already has `.bru` files)
- Verify all `/mcp/...` routes return proper responses

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Secret storage | AES-256-GCM encryption from day one | Security best practice; key rotation supported via `encryption_key_id` |
| OAuth | Stub handlers only | Full OAuth flow is complex; defer to separate plan |
| Stdio transport | Skip for now | Requires local process spawning; desktop-only concern |
| Connection test | Basic HTTP probe | Full rmcp client integration deferred; simple JSON-RPC handshake sufficient |
| Audit logging | DB-backed, no UI | Log all mutations; UI deferred to admin dashboard plan |
| Tool storage | Store on create/refresh | Tools fetched during connection test, stored in DB |

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| SQLx compile-time checking requires running DB | Build breaks in CI | Use `sqlx::query!` with `SQLX_OFFLINE=true` and prepare cached metadata |
| JSONB config column typing in Rust | Serialization bugs | Use `serde_json::Value` initially, typed structs later |
| MCP handshake format varies across servers | Connection test failures | Implement graceful error handling; accept any JSON-RPC response |
| nize-web and nize-api on different ports | CORS issues | Already handled by existing CORS configuration |

## Dependencies

- PLAN-017 (completed) — route stubs and TypeSpec contracts
- PLAN-008 (completed) — user auth system
- PLAN-013 (completed) — configuration system

## Completion Criteria

- [x] Migration `0004_mcp_servers.sql` creates all 6 tables
- [x] `nize_core::models::mcp` defines all domain types
- [x] `nize_core::mcp::queries` implements all DB queries
- [x] `McpConfigService` implements user + admin CRUD with validation
- [x] All `/mcp/...` handlers return real DB-backed responses
- [x] `/settings/tools` page in nize-web: list, add, delete, toggle
- [x] `/admin/tools` page in nize-web: list, create, delete built-in servers
- [x] `test_connection` performs basic MCP handshake probe
- [x] `cargo build && cargo clippy` pass
- [x] `cargo test` passes
- [ ] Bruno collection can exercise full CRUD lifecycle

## Resolved Questions

1. **Secret encryption:** Implement AES encryption for API keys / OAuth secrets from day one. Use AES-256-GCM with a server-side encryption key. Store encrypted values as base64 in the DB. Support key rotation via `encryption_key_id`.

2. **Tool auto-fetch:** Auto-fetch tools during server creation. When `test_connection` succeeds, store the tool list. On create, re-use tools from the test or fetch again.

3. **Admin page navigation:** Create admin layout/sidebar in nize-web. Add navigation links for Tools management under both settings and admin sections.
