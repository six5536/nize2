# PLAN-017: Implement MCP API Surface (Demo Responses)

**Status:** completed
**Workflow direction:** lateral
**Traceability:** Reference project `submodules/nize/` TypeSpec contracts

## Goal

Port the remaining REST API surface from the nize reference project into the nize-mcp Rust codebase (nize_api crate). All new handlers return demo/stub responses — no real business logic, no database queries, no external service calls.

## Scope

**In-scope:**
- TypeSpec contract definitions for all missing API groups
- Generated route constants and models via codegen pipeline
- Axum handler stubs returning hardcoded demo JSON
- Router wiring (public, protected, admin layers)

**Out-of-scope:**
- Real implementation (DB queries, AI calls, file processing, OAuth flows)
- Database migrations or schema changes
- Frontend integration
- MCP tool implementations (already separate in nize_mcp crate)

## Current State

Already implemented in nize-mcp:

| Group | Routes | Status |
|-------|--------|--------|
| Hello | `GET /api/hello` | Done |
| Auth | `POST /auth/login`, `register`, `refresh`, `logout` | Done |
| Auth | `GET /auth/status` | Done |
| MCP Tokens | `POST /auth/mcp-tokens`, `GET`, `DELETE /{id}` | Done |
| User Config | `GET /config/user`, `PATCH /{key}`, `DELETE /{key}` | Done |
| Admin Config | `GET /admin/config`, `PATCH /{scope}/{key}` | Done |

## Missing API Groups

### 1. Chat (`/chat`)

| Method | Path | Summary |
|--------|------|---------|
| POST | `/chat` | Send chat message (streaming) |

Demo: return a static SSE stream or a simple JSON response.

### 2. Conversations (`/conversations`)

| Method | Path | Summary |
|--------|------|---------|
| GET | `/conversations` | List conversations |
| POST | `/conversations` | Create conversation |
| GET | `/conversations/{id}` | Get conversation with messages |
| PATCH | `/conversations/{id}` | Update conversation |
| DELETE | `/conversations/{id}` | Delete conversation |

Demo: return hardcoded conversation list/details.

### 3. Ingestion (`/ingest`)

| Method | Path | Summary |
|--------|------|---------|
| POST | `/ingest` | Upload and ingest file |
| GET | `/ingest` | List documents |
| GET | `/ingest/{id}` | Get document by ID |
| DELETE | `/ingest/{id}` | Delete document |

Demo: return hardcoded document metadata.

### 4. Permissions (`/permissions`)

| Method | Path | Summary |
|--------|------|---------|
| POST | `/permissions/{resourceType}/{resourceId}/grants` | Create grant |
| GET | `/permissions/{resourceType}/{resourceId}/grants` | List grants |
| DELETE | `/permissions/grants/{grantId}` | Revoke grant |
| POST | `/permissions/{resourceType}/{resourceId}/links` | Create share link |
| GET | `/permissions/{resourceType}/{resourceId}/links` | List share links |
| DELETE | `/permissions/links/{linkId}` | Revoke share link |
| GET | `/permissions/shared/{token}` | Access shared resource |

Demo: return hardcoded grant/link responses.

### 5. Admin Permissions (`/admin/permissions`)

| Method | Path | Summary |
|--------|------|---------|
| GET | `/admin/permissions/grants` | List all grants |
| DELETE | `/admin/permissions/grants/{grantId}` | Admin revoke grant |
| GET | `/admin/permissions/groups` | List all groups |
| GET | `/admin/permissions/links` | List all links |
| DELETE | `/admin/permissions/links/{linkId}` | Admin revoke link |
| PATCH | `/admin/permissions/users/{userId}/admin` | Set admin role |

Demo: return empty lists and success responses.

### 6. MCP Configuration (`/mcp`)

| Method | Path | Summary |
|--------|------|---------|
| GET | `/mcp/servers` | List servers for current user |
| POST | `/mcp/servers` | Add user server |
| PATCH | `/mcp/servers/{serverId}` | Update user server |
| DELETE | `/mcp/servers/{serverId}` | Delete user server |
| PATCH | `/mcp/servers/{serverId}/preference` | Toggle server preference |
| GET | `/mcp/servers/{serverId}/tools` | Get server tools |
| GET | `/mcp/servers/{serverId}/oauth/status` | Get OAuth status |
| POST | `/mcp/servers/{serverId}/oauth/initiate` | Initiate OAuth flow |
| POST | `/mcp/servers/{serverId}/oauth/revoke` | Revoke OAuth |
| POST | `/mcp/test-connection` | Test MCP server connection |
| GET | `/mcp/admin/servers` | List all servers (admin) |
| POST | `/mcp/admin/servers` | Create built-in server |
| PATCH | `/mcp/admin/servers/{serverId}` | Update built-in server |
| DELETE | `/mcp/admin/servers/{serverId}` | Delete built-in server |

Demo: return hardcoded server lists and tool inventories.

### 7. OAuth Callback (`/auth/oauth/mcp/callback`)

| Method | Path | Summary |
|--------|------|---------|
| GET | `/auth/oauth/mcp/callback` | OAuth callback handler |

Demo: return success status.

### 8. Development/Trace (`/dev`)

| Method | Path | Summary |
|--------|------|---------|
| GET | `/dev/chat_trace` | Get chat trace events |

Demo: return hardcoded trace events.

## Implementation Steps

### Step 1: TypeSpec Contracts

Add TypeSpec files for each missing API group under `.zen/specs/`:

- `API-NIZE-chat.tsp` — Chat models and routes
- `API-NIZE-conversations.tsp` — Conversation CRUD models and routes
- `API-NIZE-ingest.tsp` — Ingestion models and routes
- `API-NIZE-permissions.tsp` — Permission grant/link models and routes
- `API-NIZE-mcp-config.tsp` — MCP server configuration models and routes
- `API-NIZE-trace.tsp` — Dev trace models and routes

Update `API-NIZE-index.tsp` to import all new modules. Adapt models from the ref project's TypeSpec, aligning namespace to `NizeApi` and error models to `NizeApi.*Error`.

Update `API-NIZE-common.tsp` to add missing common types (`NotFoundError`, `ForbiddenError`, `UUID`, `DateTime`, `PaginationParams`, `PaginatedResponse`).

Update `API-NIZE-auth.tsp` to add the OAuth callback route and `logout/all` route.

### Step 2: Generate OpenAPI + Rust Code

1. Run `bun run generate:api` to compile TypeSpec → OpenAPI YAML
2. Run `cargo run -p nize-codegen` to generate Rust route constants and models
3. Verify generated constants appear in `crates/lib/nize_api/src/generated/routes.rs`

### Step 3: Handler Stubs — Chat

Create `crates/lib/nize_api/src/handlers/chat.rs`:
- `chat_handler` — returns a demo JSON response (not actual SSE streaming)

### Step 4: Handler Stubs — Conversations

Create `crates/lib/nize_api/src/handlers/conversations.rs`:
- `list_conversations_handler` — returns hardcoded list
- `create_conversation_handler` — returns hardcoded conversation
- `get_conversation_handler` — returns hardcoded conversation with messages
- `update_conversation_handler` — returns hardcoded updated conversation
- `delete_conversation_handler` — returns 204

### Step 5: Handler Stubs — Ingestion

Create `crates/lib/nize_api/src/handlers/ingest.rs`:
- `upload_handler` — returns hardcoded ingest response
- `list_documents_handler` — returns hardcoded document list
- `get_document_handler` — returns hardcoded document
- `delete_document_handler` — returns 204

### Step 6: Handler Stubs — Permissions

Create `crates/lib/nize_api/src/handlers/permissions.rs`:
- Grant CRUD handlers (create, list, revoke)
- Share link CRUD handlers (create, list, revoke)
- `access_shared_handler`

### Step 7: Handler Stubs — Admin Permissions

Create `crates/lib/nize_api/src/handlers/admin_permissions.rs`:
- List all grants/links/groups
- Admin revoke grant/link
- Set admin role

### Step 8: Handler Stubs — MCP Config

Create `crates/lib/nize_api/src/handlers/mcp_config.rs`:
- User server CRUD (list, add, update, delete)
- Server preference toggle
- Server tools listing
- OAuth status/initiate/revoke
- Test connection
- Admin server CRUD (list, create, update, delete)

### Step 9: Handler Stubs — OAuth Callback + Trace

Add to existing or new handler files:
- `crates/lib/nize_api/src/handlers/oauth.rs` — OAuth callback
- `crates/lib/nize_api/src/handlers/trace.rs` — Chat trace

### Step 10: Router Wiring

Update `crates/lib/nize_api/src/lib.rs`:
- Add new handler module imports
- Wire all new routes into `public`, `protected`, and `admin` Router groups
- Chat, Conversations, Ingestion, Permissions → `protected` (require auth)
- Admin Permissions, MCP Admin → `admin` (require admin)
- OAuth callback → `public`
- MCP user routes → `protected`
- Dev trace → `admin`

### Step 11: Handler Module Registration

Update `crates/lib/nize_api/src/handlers/mod.rs`:
- Register all new handler modules

### Step 12: Build & Verify

1. `cargo build` — ensure compilation succeeds
2. `cargo clippy` — no warnings
3. `cargo test` — existing tests pass
4. Manual smoke test with `curl` against running server

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| TypeSpec compilation fails with new modules | Validate incrementally; run `tsp compile` after each new file |
| Codegen doesn't generate constants for new routes | Check OpenAPI YAML output before running Rust codegen |
| Route conflicts with existing paths | The nize-mcp project uses different route patterns; verify no clashes |
| Auth middleware compatibility | Reuse existing `require_auth` / `require_admin` middleware layers |

## Completion Criteria

- All routes from the ref project are defined in TypeSpec contracts
- OpenAPI YAML is regenerated with all routes
- Rust route constants and models are regenerated
- Every endpoint has a handler stub returning demo data
- `cargo build` and `cargo clippy` pass cleanly
- `cargo test` passes (existing tests unbroken)

## Resolved Questions

1. **Chat endpoint**: Simple JSON response (no SSE streaming for demo).
2. **Auth `logout/all`**: Yes, add for API parity with ref project.
3. **Pagination**: Accept parameters but ignore them; return hardcoded data.
