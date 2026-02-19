# PLAN-031: Google OAuth for gogmcp Integration

**Status:** in-progress
**Workflow direction:** bottom-up (DB → Core → API → Connection → UI)
**Traceability:** `doc/GOOGLE_AUTH.md` (architecture), `doc/gog-plan.md` (gogmcp side — implemented)

## Goal

Implement Google OAuth flow in nize-mcp so users can authorize their Google account,
acquire tokens (id_token + access_token + refresh_token), and connect to gogmcp instances
using Google ID tokens for identity verification.

## Prerequisites

- gogmcp auth rewrite complete (`doc/gog-plan.md` — done)
- Google Cloud OAuth 2.0 credentials created (client_id + client_secret)
- gogmcp configured with `GOGMCP_AUTH_MODE=google` and `GOGMCP_ALLOWED_CLIENT_IDS`

## What Exists

| Artifact | Status |
|----------|--------|
| DB: `mcp_oauth_tokens` table | Created (missing `id_token_encrypted` column) |
| DB: `mcp_server_secrets` table | Created (has `oauth_client_secret_encrypted`) |
| DB: `mcp_servers.oauth_config` JSONB column | Created |
| Model: `OAuthConfig` | Defined (`client_id`, `authorization_url`, `token_url`, `scopes`) |
| Model: `McpOauthTokenRow` | Defined (missing `id_token_encrypted` field) |
| Model: `McpServerSecretRow` | Defined (has `oauth_client_secret_encrypted`) |
| Query: `has_valid_oauth_token` | Implemented |
| Query: `store_api_key`, `get_api_key_encrypted` | Implemented |
| Encryption: `mcp::secrets::{encrypt, decrypt}` | Implemented (AES-256-GCM) |
| Handler stubs: `oauth_initiate`, `oauth_status`, `oauth_revoke` | Stubs returning 501/empty |
| Handler stub: `oauth_callback` | Stub returning demo JSON |
| TypeSpec: OAuth endpoints defined | Yes (`API-NIZE-mcp-config.tsp`, `API-NIZE-auth.tsp`) |
| Routes: generated constants for OAuth paths | Yes |
| `CreateUserServerRequest` / `CreateAdminServerRequest` | Missing `oauthConfig` and `clientSecret` fields |
| `ClientPool::connect_http` | No auth header support |
| `AppState` | No PKCE state storage |

## Implementation Phases

### Phase 1: DB & Model Updates

Add `id_token_encrypted` to `mcp_oauth_tokens` — needed because Google ID tokens
are the Bearer credential sent to gogmcp.

**NOTE:** Per `ARCHITECTURE.md` release status: "This project is NOT in production;
the DB can be cleared and migrations are not necessary." So we modify the existing
migration inline rather than adding a new migration file.

#### Step 1.1: Add `id_token_encrypted` column to migration

File: `crates/lib/nize_core/migrations/0004_mcp_servers.sql`

Add `id_token_encrypted TEXT` column to `mcp_oauth_tokens` table (after `access_token_encrypted`).

#### Step 1.2: Update `McpOauthTokenRow` model

File: `crates/lib/nize_core/src/models/mcp.rs`

Add `pub id_token_encrypted: Option<String>` to `McpOauthTokenRow`.

### Phase 2: OAuth Queries in nize_core

New queries for storing and retrieving OAuth tokens.

#### Step 2.1: Store OAuth token query

File: `crates/lib/nize_core/src/mcp/queries.rs`

`store_oauth_token(pool, user_id, server_id, id_token_encrypted, access_token_encrypted,
refresh_token_encrypted, expires_at, scopes)` — UPSERT into `mcp_oauth_tokens`.

#### Step 2.2: Get OAuth token query

`get_oauth_token(pool, user_id, server_id) -> Option<McpOauthTokenRow>` — returns row
if exists (regardless of expiry; caller handles refresh).

#### Step 2.3: Delete OAuth token query

`delete_oauth_token(pool, user_id, server_id)` — for revocation.

#### Step 2.4: Store OAuth client secret query

`store_oauth_client_secret(pool, server_id, encrypted_secret, key_id)` — UPSERT into
`mcp_server_secrets.oauth_client_secret_encrypted`.

#### Step 2.5: Get OAuth client secret query

`get_oauth_client_secret_encrypted(pool, server_id) -> Option<String>` — fetch encrypted
client secret.

### Phase 3: PKCE State Management

OAuth PKCE requires in-memory ephemeral state between initiate and callback.

#### Step 3.1: Create `OAuthStateStore`

File: `crates/lib/nize_core/src/mcp/oauth.rs` (new file)

```rust
pub struct OAuthPendingState {
    pub server_id: String,
    pub user_id: String,
    pub pkce_verifier: String,
    pub created_at: Instant,
}

pub struct OAuthStateStore {
    states: DashMap<String, OAuthPendingState>,  // key = state param (CSRF token)
}
```

- `insert(state_key, pending)` — store with TTL
- `take(state_key) -> Option<OAuthPendingState>` — remove and return
- `cleanup()` — evict expired entries (>10 min)
- Auto-cleanup via `spawn_cleanup_task()` periodic task

#### Step 3.2: Add `OAuthStateStore` to `AppState`

File: `crates/lib/nize_api/src/lib.rs`

Add `pub oauth_state: Arc<OAuthStateStore>` to `AppState`.

### Phase 4: Server Registration Updates

Accept `oauthConfig` and `clientSecret` when creating servers with `authType: "oauth"`.

#### Step 4.1: Update `CreateUserServerRequest`

File: `crates/lib/nize_api/src/handlers/mcp_config.rs`

Add:
- `oauth_config: Option<OAuthConfig>` — Google OAuth client config
- `client_secret: Option<String>` — OAuth client secret (stored encrypted, not in config JSONB)

#### Step 4.2: Update `CreateAdminServerRequest`

Same fields as Step 4.1.

#### Step 4.3: Update `create_user_server` service

File: `crates/lib/nize_api/src/services/mcp_config.rs`

When `auth_type == "oauth"`:
- Validate `oauth_config` is present
- Validate `client_secret` is present
- Store `oauth_config` as JSONB in `mcp_servers.oauth_config`
- Encrypt and store `client_secret` via `store_oauth_client_secret` query
- Scopes must include `openid` and `email` (validate or inject)

#### Step 4.4: Update `create_built_in_server` service

Same as Step 4.3 for admin server creation.

#### Step 4.5: Update TypeSpec contracts

Files: `.zen/specs/API-NIZE-mcp-config.tsp`

Add `oauthConfig` and `clientSecret` to `CreateUserServerRequest` and
`CreateAdminServerRequest` models in TypeSpec. Re-generate.

### Phase 5: OAuth Flow Handlers

Implement the three OAuth endpoints + callback.

#### Step 5.1: Implement `oauth_initiate_handler`

File: `crates/lib/nize_api/src/handlers/mcp_config.rs`

1. Load `OAuthConfig` from `mcp_servers.oauth_config` JSONB
2. Load and decrypt `oauth_client_secret` from `mcp_server_secrets`
3. Generate PKCE `code_verifier` + `code_challenge` (S256)
4. Generate cryptographic `state` parameter (random UUID or similar)
5. Store `OAuthPendingState` in `OAuthStateStore` (server_id, user_id, pkce_verifier)
6. Build Google authorization URL:
   - `authorization_url` from OAuthConfig
   - `client_id` from OAuthConfig
   - `redirect_uri` = `http://localhost:{port}/auth/oauth/mcp/callback`
   - `response_type=code`
   - `scope` from OAuthConfig (must include `openid email`)
   - `access_type=offline` (for refresh token)
   - `prompt=consent` (force consent to get refresh token)
   - `state` = generated state
   - `code_challenge` + `code_challenge_method=S256`
7. Return `{ authUrl }` to frontend

Dependencies: Add `oauth2` crate to `nize_core` (for PKCE generation utilities).
Alternative: implement PKCE manually with `sha2` + `base64` (minimal code, no new dep).

#### Step 5.2: Implement `oauth_callback_handler`

File: `crates/lib/nize_api/src/handlers/oauth.rs`

1. Extract `code` and `state` from query parameters
2. Look up `OAuthPendingState` from `OAuthStateStore` using `state`
3. If not found → error (expired or invalid CSRF)
4. Exchange `code` for tokens via Google's token endpoint:
   - POST to `token_url` with `code`, `client_id`, `client_secret`, `redirect_uri`,
     `grant_type=authorization_code`, `code_verifier`
   - Response contains: `access_token`, `refresh_token`, `id_token`, `expires_in`
5. Encrypt `id_token`, `access_token`, `refresh_token` using `mcp::secrets::encrypt`
6. Store via `store_oauth_token` query
7. Redirect to desktop UI success page (e.g. `http://localhost:1420/settings/tools?oauth=success&serverId=...`)
   OR return HTML page that posts a message to opener window and closes

#### Step 5.3: Implement `oauth_status_handler`

File: `crates/lib/nize_api/src/handlers/mcp_config.rs`

1. Call `has_valid_oauth_token(pool, user_id, server_id)`
2. Optionally check if refresh_token exists (can reconnect even if expired)
3. Return `{ connected: bool, expiresAt?: string }`

#### Step 5.4: Implement `oauth_revoke_handler`

File: `crates/lib/nize_api/src/handlers/mcp_config.rs`

1. Load token from DB
2. Optionally call Google's revoke endpoint (`https://oauth2.googleapis.com/revoke?token=...`)
3. Delete from `mcp_oauth_tokens` via `delete_oauth_token` query
4. Return `204 No Content`

### Phase 6: Token Refresh Service

Google access_tokens and id_tokens expire in ~1 hour. Refresh proactively.

#### Step 6.1: Create `refresh_google_tokens` function

File: `crates/lib/nize_core/src/mcp/oauth.rs`

```rust
pub async fn refresh_google_tokens(
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<GoogleTokenResponse, McpError>
```

- POST to `token_url` with `grant_type=refresh_token`, `client_id`, `client_secret`, `refresh_token`
- Response includes new `access_token`, `id_token`, `expires_in`
- (No new `refresh_token` — Google keeps the original unless revoked)

#### Step 6.2: Token refresh integration point

When `ClientPool::connect_http` connects to an OAuth server, check token expiry.
If expired or near-expiry (>80% of lifetime), refresh first, update DB, then connect.

This is consumed during tool execution (Phase 7).

### Phase 7: Authenticated Connection to gogmcp

The `ClientPool` currently doesn't support custom headers. For OAuth-authenticated
servers (`authType: "oauth"`), we need to pass:
- `Authorization: Bearer <id_token>`
- `X-Google-Access-Token: <access_token>`

#### Step 7.1: Extend `ClientPool::connect_http` for OAuth

File: `crates/lib/nize_core/src/mcp/execution.rs`

The `StreamableHttpClientTransportConfig` needs custom headers. Check if `rmcp`
supports custom headers on the transport config. If not, we need to either:

a) Use a custom `reqwest::Client` with default headers and pass it to the transport
b) Add header injection to the transport layer
c) Store the auth tokens in the transport configuration

Investigation needed: Check `rmcp` 0.15 API for header customization in
`StreamableHttpClientTransportConfig`.

#### Step 7.2: User-scoped connection context

Current `ClientPool` connects per-server (server_id key). For OAuth servers, different
users have different tokens, so the connection must be per-(user, server) pair, or the
tokens must be injected per-request.

Options:
a) **Per-request header injection** — if rmcp transport supports per-request headers
b) **Per-(user, server) pool key** — compound key `(user_id, server_id)` in pool
c) **Reconnect on user change** — simpler but wasteful

Decision needed. For desktop (single user), option (a) or (c) works.
For cloud/multi-user, option (b) is necessary.

Given current architecture (desktop-first), start with approach that supports
both: pass user-scoped OAuth credentials at connection time.

#### Step 7.3: Token lifecycle during tool execution

In `execute_tool` (or `execute_with_retry`):
1. Check if server has `auth_type == "oauth"`
2. Load user's OAuth tokens from DB
3. If expired, refresh tokens (Step 6.1), update DB
4. Connect with auth headers
5. Execute tool
6. On auth error (401), refresh and retry once

### Phase 8: Frontend Integration

#### Step 8.1: "Connect Google" button in server settings

When a server has `authType: "oauth"` and user has no valid token,
show "Connect Google" button that:
1. Calls `POST /mcp/servers/{serverId}/oauth/initiate`
2. Opens returned `authUrl` in system browser
3. Polls `GET /mcp/servers/{serverId}/oauth/status` until connected

#### Step 8.2: OAuth status display

Show connection status:
- "Connected" (green) — valid token exists
- "Expired — Reconnect" — token expired, has refresh_token
- "Not connected" — no token

#### Step 8.3: Disconnect button

Calls `POST /mcp/servers/{serverId}/oauth/revoke`.

### Phase 9: Testing

#### Step 9.1: Unit tests for PKCE state store

Test insert, take, expiry, cleanup.

#### Step 9.2: Unit tests for OAuth queries

Test store/get/delete OAuth tokens and client secrets.

#### Step 9.3: Integration test for OAuth flow

Mock Google's token endpoint. Test full initiate → callback → store cycle.

#### Step 9.4: Integration test for token refresh

Mock Google's token endpoint. Test refresh updates DB correctly.

#### Step 9.5: Connection test with auth headers

Test that `ClientPool::connect_http` sends correct headers for OAuth servers.

## Risks & Open Questions

| # | Risk / Question | Mitigation / Answer |
|---|-----------------|---------------------|
| 1 | `rmcp` StreamableHTTP transport may not support custom headers | Investigate API. Fallback: fork or add header middleware via `reqwest::Client` |
| 2 | Multi-user token scoping in `ClientPool` | Start desktop-scoped (single user), extend to compound key later |
| 3 | Redirect URI port varies per launch (desktop mode) | Use `AppState` to resolve current API port dynamically |
| 4 | Google refresh tokens may be revoked externally | Handle revocation error in refresh, clear DB token, prompt re-auth |
| 5 | PKCE state memory lost on server restart | Acceptable — user retries OAuth flow. Short window (10 min TTL) |
| 6 | `id_token` not returned on refresh in some Google configs | Verify Google always returns `id_token` on refresh when `openid` scope used |

## Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| `reqwest` | HTTP calls to Google token endpoint | Already in workspace |
| `sha2` | PKCE S256 code challenge | Already in workspace (`aes-gcm` pulls in `sha2`) |
| `base64` | PKCE + token encoding | Already in workspace |
| `dashmap` | PKCE state store | Already used in `ClientPool` |
| `oauth2` crate | Optional — PKCE helpers | Evaluate if needed vs manual impl |

## Completion Criteria

1. Admin can register a gogmcp server with `authType: "oauth"` and Google OAuth config
2. User can initiate Google OAuth flow from UI → consent → callback → tokens stored
3. `oauth/status` returns correct connected state
4. User can revoke OAuth authorization
5. Tool execution against gogmcp sends `Authorization: Bearer <id_token>` + `X-Google-Access-Token`
6. Expired tokens are automatically refreshed before tool execution
7. All new code has unit tests
8. E2E test: register gogmcp → authorize → execute a calendar tool

## Implementation Order

Execute phases in order, running `cargo test` and `cargo clippy` after each step:

1. Phase 1 (DB + models) — non-breaking, additive
2. Phase 2 (queries) — new functions, no existing code changed
3. Phase 3 (PKCE state store) — new module + AppState change
4. Phase 4 (server registration) — extend request DTOs + service logic
5. Phase 5 (OAuth handlers) — replace stubs with real implementations
6. Phase 6 (token refresh) — new function in oauth module
7. Phase 7 (authenticated connection) — extend ClientPool (**hardest phase**, depends on rmcp API)
8. Phase 8 (frontend) — UI changes
9. Phase 9 (testing) — tests written alongside each phase, integration tests last
