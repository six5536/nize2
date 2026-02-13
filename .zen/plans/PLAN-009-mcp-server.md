# PLAN-009: MCP Server + DB Layer Refactor

| Field              | Value                                                    |
|--------------------|----------------------------------------------------------|
| **Status**         | in-progress                                              |
| **Workflow**       | bottom-up                                                |
| **Reference**      | PLAN-008 (user-auth), PLAN-003 (api-bootstrap)           |
| **Traceability**   | —                                                        |

## Goal

1. **Refactor DB ownership**: Move migrations, domain models, auth data-access, and JWT logic from `nize_api` into `nize_core` so any crate can access the shared database layer without depending on the HTTP API.
2. **Add MCP server**: Implement an MCP server (Streamable HTTP transport) in `nize_mcp`, exposed on a separate port by `nize_api_server`, with bearer token auth and a single hello-world tool.

## Decisions

### SDK: `rmcp` (official Rust MCP SDK)

| Crate | Version | Downloads | Transport | Notes |
|-------|---------|-----------|-----------|-------|
| **`rmcp`** | 0.15.0 | 3.5M+ | stdio, **Streamable HTTP server/client** | Official SDK (modelcontextprotocol org). Tower service, Axum `nest_service`. `#[tool]` macro. Auth middleware examples. Active dev. |
| `mcp-core` | 0.1.50 | 78K | stdio, SSE only | Community. No Streamable HTTP. Merging into official. |
| `mcp-server` | 0.1.0 | 46K | stdio only | Old official. Superseded by `rmcp`. |

**Choice: `rmcp`** with features `["server", "transport-streamable-http-server"]`.

Rationale:
- Official SDK, actively maintained, 3.5M+ downloads
- Native Streamable HTTP server as Tower service — plugs into Axum via `nest_service("/mcp", service)`
- `#[tool]` / `#[tool_router]` / `#[tool_handler]` macros for ergonomic tool definition
- Auth middleware examples (`simple_auth_streamhttp.rs`) match our bearer token requirement
- Session management built-in (`LocalSessionManager`)

### Transport: Streamable HTTP (MCP 2025-03-26 spec)

Single endpoint (`POST` + `GET` + `DELETE`) at `/mcp`. Stateful sessions via `Mcp-Session-Id` header. SSE streaming for server→client messages. Replaces deprecated HTTP+SSE transport.

### Architecture: Library + embedded in `nize_api_server`

```
nize_api_server (binary)
├── port A: Axum REST API (nize_api::router)
└── port B: MCP Streamable HTTP (nize_mcp, via rmcp)
     └── shares same PgPool
```

Same process, separate ports. PGlite supports only one connection so sharing the pool is required. `nize_mcp` is a library crate; `nize_api_server` wires it up. Minimal glue code in the binary — a separate MCP-only binary is possible later (out of scope).

### Auth: Long-lived MCP bearer tokens

** see below.**

## Decision: MCP Auth — Long-lived API Tokens (Option A)

- New table `mcp_tokens` (id, user_id, token_hash, name, created_at, expires_at, revoked_at)
- REST endpoint `POST /auth/mcp-tokens` (authenticated) → generates token, returns plaintext once
- MCP auth middleware: extract `Authorization: Bearer <token>`, SHA-256 hash, look up in `mcp_tokens`, resolve user
- Claude Desktop config: `"headers": {"Authorization": "Bearer <token>"}`
- Session sharing: token is linked to user_id, so MCP operations are user-scoped
- Upgrade path to OAuth later without breaking changes

## Current State

| Layer | Crate | Owns |
|-------|-------|------|
| DB lifecycle | `nize_core::db` | `LocalDbManager`, `PgLiteManager`, `DbProvisioner` |
| DB migrations | `nize_api` | `migrations/0001_auth.sql`, `migrate()` |
| Domain models | `nize_api::generated::models` | `AuthUser`, `TokenResponse`, etc. (codegen'd) |
| Auth data-access | `nize_api::services::auth` | User CRUD, token CRUD, password hashing, JWT |
| Auth middleware | `nize_api::middleware::auth` | `require_auth`, `AuthenticatedUser` |
| HTTP API | `nize_api` | Axum router, handlers, config |
| MCP protocol | `nize_mcp` | **Empty** (just `version()`) |
| Server binary | `nize_api_server` | Wires pool → migrations → router → serve |

## Plan

### Phase 1 — Move Migrations to `nize_core`

Move SQL migrations and the `migrate()` function so any crate can run them.

- [x] **1.1** Move `crates/lib/nize_api/migrations/` → `crates/lib/nize_core/migrations/`
- [x] **1.2** Add `pub async fn migrate(pool: &PgPool)` to `nize_core` (new module `nize_core::migrate`)
  - `sqlx::migrate!("./migrations").run(pool).await`
- [x] **1.3** Update `nize_api::migrate()` → re-export or delegate to `nize_core::migrate()`
  - Or remove `nize_api::migrate()` entirely and update `nize_api_server/src/main.rs` to call `nize_core::migrate()`
- [x] **1.4** Verify: `cargo build -p nize_core` compiles with migrations embedded

### Phase 2 — Move Domain Models to `nize_core`

Create domain-level models in `nize_core` that both `nize_api` and `nize_mcp` can use. These are distinct from the API-specific generated models (which have `#[serde(rename)]` for camelCase etc.).

- [x] **2.1** Create `crates/lib/nize_core/src/models/` module:
  - `mod.rs` — re-exports
  - `auth.rs` — domain structs
- [x] **2.2** Define domain models in `nize_core::models::auth`:
  - `User { id: String, email: String, name: Option<String> }`
  - `UserWithPassword { user: User, password_hash: Option<String> }`
  - `UserRole { user_id: String, role: String }`
  - `RefreshTokenRecord { id: String, user_id: String, expires_at: DateTime<Utc> }`
  - `TokenClaims { sub, email, roles, exp, iat }` (moved from `nize_api::services::auth`)
- [x] **2.3** Update `nize_core/Cargo.toml`: add `chrono` dependency (for `DateTime<Utc>`)
- [x] **2.4** Update `nize_api::services::auth` to import `TokenClaims` from `nize_core::models::auth`

### Phase 3 — Move Auth Logic to `nize_core`

Move password hashing, JWT, and DB query functions to `nize_core` so `nize_mcp` can reuse them.

- [x] **3.1** Create `crates/lib/nize_core/src/auth/` module:
  - `mod.rs` — re-exports
  - `password.rs` — `hash_password()`, `verify_password()`
  - `jwt.rs` — `generate_access_token()`, `verify_access_token()`, `resolve_jwt_secret()`
  - `queries.rs` — `find_user_by_email()`, `create_user()`, `get_user_roles()`, `store_refresh_token()`, `find_and_revoke_refresh_token()`, `admin_exists()`, etc.
- [x] **3.2** Move `nize_core/Cargo.toml` deps: add `bcrypt`, `jsonwebtoken`, `sha2`, `rand`, `chrono`, `dirs`, `tracing`
  - These move from `nize_api` deps (keep in `nize_api` only if still needed there directly)
- [x] **3.3** Define error types in `nize_core::auth`:
  - `AuthError` enum (CredentialError, TokenError, ValidationError, DbError)
  - `nize_api::error::AppError` gets `From<AuthError>` impl
- [x] **3.4** Refactor `nize_api::services::auth`:
  - `login()` → calls `nize_core::auth::queries::find_user_by_email()`, `nize_core::auth::password::verify_password()`, `nize_core::auth::jwt::generate_access_token()`
  - `register()` → calls `nize_core::auth::queries::create_user()`, etc.
  - `refresh()` → calls `nize_core::auth::queries::find_and_revoke_refresh_token()`, etc.
  - `logout()` → calls `nize_core::auth::queries::revoke_refresh_token()`
  - Retain API-specific response building (mapping domain models → generated API models)
- [x] **3.5** Update `nize_api::middleware::auth` to import JWT verification from `nize_core::auth::jwt`
- [x] **3.6** Verify: `cargo test -p nize_api` still passes
- [x] **3.7** Verify: `cargo build -p nize_core` compiles cleanly

### Phase 4 — MCP Token Table (if Option A auth confirmed)

- [x] **4.1** Create migration `crates/lib/nize_core/migrations/0002_mcp_tokens.sql`:
  ```sql
  CREATE TABLE IF NOT EXISTS mcp_tokens (
      id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
      token_hash VARCHAR(64) NOT NULL UNIQUE,
      name VARCHAR(255) NOT NULL,
      created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
      expires_at TIMESTAMPTZ,
      revoked_at TIMESTAMPTZ
  );
  ```
- [x] **4.2** Add `nize_core::auth::mcp_tokens` module:
  - `create_mcp_token(pool, user_id, name) → (token_plaintext, McpTokenRecord)`
  - `validate_mcp_token(pool, token) → Option<User>` (SHA-256 hash lookup)
  - `revoke_mcp_token(pool, token_id)`
  - `list_mcp_tokens(pool, user_id) → Vec<McpTokenRecord>`
- [x] **4.3** Add REST endpoint in `nize_api`:
  - `POST /auth/mcp-tokens` (requires auth) → creates token, returns plaintext
  - `GET /auth/mcp-tokens` (requires auth) → lists tokens (no plaintext)
  - `DELETE /auth/mcp-tokens/{id}` (requires auth) → revokes token
- [x] **4.4** Update TypeSpec: add MCP token endpoints to `API-NIZE-auth.tsp`
- [x] **4.5** Regenerate codegen

### Phase 5 — MCP Server Library (`nize_mcp`)

Implement the MCP server as a library using `rmcp`.

- [x] **5.1** Update `crates/lib/nize_mcp/Cargo.toml`:
  ```toml
  [dependencies]
  nize_core.workspace = true
  rmcp = { version = "0.15", features = ["server", "transport-streamable-http-server"] }
  sqlx = { workspace = true }
  tracing = { workspace = true }
  serde = { workspace = true, features = ["derive"] }
  serde_json = { workspace = true }
  schemars = { workspace = true }
  axum = { workspace = true }
  tokio-util = { workspace = true }
  ```
- [x] **5.2** Add `rmcp`, `schemars`, `tokio-util` to workspace `Cargo.toml` dependencies
- [x] **5.3** Create `crates/lib/nize_mcp/src/tools/` module:
  - `mod.rs` — re-exports
  - `hello.rs` — hello world tool parameter types
- [x] **5.4** Implement hello world tool in `nize_mcp::server` via `#[tool_router]`:
  - `#[tool(description = "Say hello from Nize MCP server")]`
  - Uses `Parameters<HelloRequest>` from `tools::hello`
- [x] **5.5** Create `crates/lib/nize_mcp/src/server.rs`:
  - `NizeMcpServer` struct (holds `PgPool`, `ToolRouter<Self>`)
  - `#[tool_router]` impl with hello tool
  - `#[tool_handler]` `ServerHandler` impl with `get_info()`, `ServerCapabilities::builder().enable_tools()`
- [x] **5.6** Create `crates/lib/nize_mcp/src/auth.rs`:
  - `mcp_auth_middleware` — Axum middleware for MCP bearer token validation
  - Extracts `Authorization: Bearer <token>` header
  - Calls `nize_core::auth::mcp_tokens::validate_mcp_token()`
  - Returns 401 on invalid/missing/expired token, 500 on DB error
- [x] **5.7** Create `crates/lib/nize_mcp/src/lib.rs`:
  - `pub fn mcp_router(pool: PgPool, ct: CancellationToken) → axum::Router`
  - Creates `StreamableHttpService::new(...)` with `LocalSessionManager`, stateful mode
  - Wraps in `mcp_auth_middleware` layer via `from_fn_with_state`
  - Returns `Router::new().nest_service("/mcp", service).layer(auth)`
- [x] **5.8** Verify: `cargo build -p nize_mcp` compiles

### Phase 6 — Wire MCP into `nize_api_server`

Expose the MCP server on a separate port from the same binary.

- [x] **6.1** Add CLI arg to `nize_api_server`:
  - `--mcp-port <PORT>` (default: 0 = ephemeral, like `--port`)
- [x] **6.2** In `main()`, after pool + migrations:
  - Build REST API router: `nize_api::router(state)` → bind to `--port`
  - Build MCP router: `nize_mcp::mcp_router(mcp_pool, mcp_ct)` → bind to `--mcp-port`
  - MCP server spawned via `tokio::spawn`, REST API on main task
  - `CancellationToken` for graceful shutdown coordination
- [x] **6.3** Report both ports in JSON stdout:
  - `{"port": N, "mcpPort": M}`
- [x] **6.4** Update `nize_api_server/Cargo.toml`:
  - Add `nize_mcp = { workspace = true }`, `tokio-util = { workspace = true }`
- [x] **6.5** `nize_mcp` already in workspace `Cargo.toml` dependencies (was there before)
- [ ] **6.6** Verify: `cargo run -p nize_desktop_server` starts both servers (requires running PG)
- [ ] **6.7** Verify: MCP Inspector (`npx @modelcontextprotocol/inspector`) can connect and call hello tool (requires running PG)

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| `sqlx::migrate!` macro is path-relative to crate root | Migration move may break compile | Test 1.4 early; use `SQLX_OFFLINE=true` if needed |
| `rmcp` 0.15 is pre-1.0, API may change | Breaking changes on upgrade | Pin exact version in workspace Cargo.toml |
| Bearer token UX for Claude Desktop | User must copy-paste token | Document in README; Tauri UI for token generation later |
| Separate port adds complexity for desktop app | Tauri must track two ports | JSON stdout already has port; add `mcpPort` field |
| Moving auth logic to `nize_core` is a large refactor | Risk of regressions | Run existing tests after each phase; keep `nize_api` tests passing |

## Completion Criteria

- [x] `cargo build --workspace` succeeds
- [x] `cargo test -p nize_core` passes (including migration embedding)
- [x] `cargo test -p nize_api` passes (auth still works via refactored core)
- [ ] `nize_desktop_server` starts, reports `{"port": N, "mcpPort": M}` (requires running PG)
- [ ] MCP Inspector connects to `http://localhost:M/mcp` with bearer token (requires running PG)
- [ ] MCP Inspector lists `hello` tool and calls it successfully (requires running PG)
- [x] Existing REST API endpoints unaffected
