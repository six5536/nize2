# PLAN-018: Bruno API Test Collections

**Status:** completed
**Workflow direction:** lateral
**Traceability:** PLAN-017 (MCP API surface), generated routes in `crates/lib/nize_api/src/generated/routes.rs`

## Goal

Create a Bruno project at `/bruno` to manually test all nize-mcp REST API endpoints. One collection with folders per API group, following the conventions established in `submodules/bitmark-configurator-api/bruno/`.

## Scope

**In-scope:**
- Bruno project config (`bruno.json`)
- Environment file (Local — `http://localhost:3100`)
- `.bru` request files for all 42 endpoints (public + protected + admin)
- Post-response script on login to capture `accessToken` env var
- Bearer auth header on protected/admin requests

**Out-of-scope:**
- Automated test assertions (Bruno tests/assertions section)
- CI integration
- Multiple environments beyond Local

## Structure

```
bruno/
├── bruno.json
├── environments/
│   └── Local.bru
├── Public/
│   ├── hello.bru
│   ├── auth-status.bru
│   ├── auth-login.bru
│   ├── auth-register.bru
│   ├── auth-refresh.bru
│   ├── auth-logout.bru
│   ├── oauth-callback.bru
│   └── shared-access.bru
├── Auth/
│   ├── logout-all.bru
│   ├── create-mcp-token.bru
│   ├── list-mcp-tokens.bru
│   └── revoke-mcp-token.bru
├── Chat/
│   └── send-message.bru
├── Conversations/
│   ├── list-conversations.bru
│   ├── create-conversation.bru
│   ├── get-conversation.bru
│   ├── update-conversation.bru
│   └── delete-conversation.bru
├── Ingest/
│   ├── list-documents.bru
│   ├── upload-document.bru
│   ├── get-document.bru
│   └── delete-document.bru
├── Permissions/
│   ├── create-grant.bru
│   ├── list-grants.bru
│   ├── revoke-grant.bru
│   ├── create-link.bru
│   ├── list-links.bru
│   └── revoke-link.bru
├── Config/
│   ├── list-user-config.bru
│   ├── update-user-config.bru
│   └── reset-user-config.bru
├── MCP-Servers/
│   ├── list-servers.bru
│   ├── add-server.bru
│   ├── update-server.bru
│   ├── delete-server.bru
│   ├── update-preference.bru
│   ├── list-server-tools.bru
│   ├── oauth-status.bru
│   ├── oauth-initiate.bru
│   ├── oauth-revoke.bru
│   └── test-connection.bru
└── Admin/
    ├── list-admin-config.bru
    ├── update-admin-config.bru
    ├── list-all-grants.bru
    ├── admin-revoke-grant.bru
    ├── list-all-groups.bru
    ├── list-all-links.bru
    ├── admin-revoke-link.bru
    ├── set-admin-role.bru
    ├── list-admin-servers.bru
    ├── create-admin-server.bru
    ├── update-admin-server.bru
    ├── delete-admin-server.bru
    └── chat-trace.bru
```

## Steps

### 1 — Project scaffold
Create `bruno/bruno.json` and `bruno/environments/Local.bru`.

### 2 — Public folder
Create `.bru` files for unauthenticated endpoints. Login includes a `script:post-response` to store `accessToken` and `refreshToken` in env vars.

### 3 — Auth folder
Protected auth management requests (logout-all, MCP tokens). All use `Authorization: Bearer {{accessToken}}`.

### 4 — Chat folder
Single `POST /chat` request with sample JSON body.

### 5 — Conversations folder
CRUD requests with sample bodies for create/update, path vars for get/update/delete.

### 6 — Ingest folder
Upload (POST), list (GET), get by ID, delete by ID.

### 7 — Permissions folder
Grant and link CRUD requests with path params `{{resourceType}}` and `{{resourceId}}`.

### 8 — Config folder
User config list/update/reset.

### 9 — MCP-Servers folder
User server CRUD, preference, tools, OAuth, test-connection.

### 10 — Admin folder
Admin config, admin permissions (grants/groups/links/role), admin MCP servers, dev trace.

## Open Questions

None.
