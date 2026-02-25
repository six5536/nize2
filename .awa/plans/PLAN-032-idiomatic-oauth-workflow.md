# PLAN-032: Idiomatic OAuth Workflow & Form Refactor

**Status:** in-progress
**Workflow direction:** lateral (UI refactor + backend behavior changes)
**Traceability:** `PLAN-031-google-oauth.md` (original OAuth impl)

## Problem

The current OAuth workflow for MCP servers is disjointed and non-idiomatic:

1. **Re-authorize** does not revoke first — user must manually Disconnect then Re-authorize
2. **Test Connection** for OAuth servers doesn't initiate auth if not connected — it just fails
3. **Save Changes** doesn't validate connection or trigger auth — it blindly saves and hopes
4. **Create flow** ties OAuth auth to the "Test Connection" button, which is conceptually wrong
5. **"OAuth settings changed" warning** is confusing — implies destructive action separate from save
6. **Massive form duplication** across 4 form components (~1000 lines of copy-pasted UI)

### Current Button Semantics (OAuth, Edit Form)

| Button | Current behavior | Problem |
|--------|-----------------|---------|
| Test Connection | Sends test request using stored token | Fails silently if no token / expired |
| Re-authorize | Initiates OAuth without revoking old token | Stale tokens remain; Google may not re-prompt |
| Disconnect | Revokes token (with confirmation) | Separate manual step before re-auth |
| Save Changes | Saves config; if OAuth settings changed, revokes after save | Saves even if connection is broken |

### Current Button Semantics (OAuth, Create Form)

| Button | Current behavior | Problem |
|--------|-----------------|---------|
| Connect with OAuth | Creates server + initiates OAuth + polls for success | Conflates server creation with auth testing |
| Create Server / Done | If OAuth succeeded, just closes form | Server already created during "test" |

## Proposed Idiomatic Workflow

### Principle: actions should do what their label says, cascading where necessary

### Edit Form — OAuth Servers

| Button | New behavior |
|--------|-------------|
| **Test Connection** | 1. Check if valid token exists → test connection<br/>2. If no token or expired → auto-initiate OAuth flow → on success → test connection<br/>3. If token refresh possible → refresh → test<br/>Reports result: connected + tool count, or specific error |
| **Save Changes** | 1. Validate form fields<br/>2. If OAuth settings changed (client_id, scopes, etc.) → revoke old token<br/>3. Save config to backend<br/>4. If OAuth server → ensure valid connection (auto-auth if needed)<br/>5. If connection fails → roll back save and report error<br/>Result: saved + connected, or error with reason |
| **Re-authorize** | 1. Revoke existing token automatically (no separate Disconnect needed)<br/>2. Initiate fresh OAuth flow<br/>3. On success → test connection + update status display |
| **Disconnect** | Remove — users can Re-authorize which handles revoke+re-auth. To truly disconnect, delete the server or change auth type to "none" |

### Create Form — OAuth Servers

| Button | New behavior |
|--------|-------------|
| **Test Connection** | Same as edit: initiates OAuth if no token → tests connection<br/>But for create: server must exist first, so auto-create server → auth → test.<br/>If auth/test fails → clean up server |
| **Create Server** | 1. Validate form<br/>2. Create server<br/>3. If OAuth → initiate auth → test connection<br/>4. If fails → delete server + report error<br/>Result: server created + connected, or error |

### Create Form — Non-OAuth Servers

No change. Test Connection → Create Server works well for stdio/http-none/api-key.

## Form Refactor

### Current State (4 forms, ~1000 lines of duplication)

| Component | File | Lines | Supports |
|-----------|------|-------|----------|
| `CreateServerForm` | `admin/tools/page.tsx` | ~390 | stdio + http + oauth, admin |
| `EditServerForm` | `admin/tools/page.tsx` | ~420 | stdio + http + oauth, admin |
| `AddServerForm` | `tools/page.tsx` | ~100 | http only, user |
| `EditUserServerForm` | `tools/page.tsx` | ~100 | http only, user |

### Target State: shared components, extracted to own files

```
packages/nize-web/components/mcp-server/
├── ServerForm.tsx          # Unified create/edit form shell
├── HttpConfigFields.tsx    # URL, auth type, API key fields
├── StdioConfigFields.tsx   # Command, args, env fields
├── OAuthConfigFields.tsx   # Client ID, secret, scopes, advanced
├── OAuthStatusBanner.tsx   # Connected/disconnected status + Re-authorize button
├── useOAuthFlow.ts         # Hook: popup/poll logic for OAuth authorization
├── useServerForm.ts        # Hook: form state, buildConfig, validation
└── types.ts                # Shared types (ServerConfig, AuthType, etc.)
```

### Shared Components

#### `ServerForm.tsx`
- Single form component for both create and edit
- Props: `mode: "create" | "edit"`, `initialValues?`, callbacks
- Renders transport-specific field sections via `HttpConfigFields` / `StdioConfigFields`
- Renders `OAuthStatusBanner` when authType === "oauth" and mode === "edit"
- Footer: Cancel, Test Connection, Save/Create
- Admin form passes `showVisibility: true`, `showTransport: true`
- User form passes `showVisibility: false`, `showTransport: false` (HTTP only)

#### `useOAuthFlow.ts`
- Consolidates popup open + message listener + status polling + timeout
- `startOAuthFlow(authUrl, serverId, authFetch) → Promise<{ success, error? }>`
- Used by both Test Connection and Re-authorize actions

#### `useServerForm.ts`
- Manages all form field state
- `buildConfig()` → server config object
- `isValid` computed property
- `hasOAuthConfigChanged(original, current)` comparison

### Migration Strategy

1. Extract shared types to `types.ts`
2. Extract `useOAuthFlow` hook (dedup popup/poll logic from both forms)
3. Extract field components (`HttpConfigFields`, `StdioConfigFields`, `OAuthConfigFields`)
4. Extract `OAuthStatusBanner` (status display + Re-authorize + status fetch)
5. Build `useServerForm` hook
6. Build unified `ServerForm` component
7. Replace `CreateServerForm` + `EditServerForm` in admin page
8. Replace `AddServerForm` + `EditUserServerForm` in user page
9. Delete old form components

## Backend Changes

### `POST /mcp/test-connection` (existing endpoint)

Current: takes config + optional serverId, returns test result.

Change: when `authType === "oauth"` and no valid token exists, return a structured
response indicating auth is required rather than just failing:

```json
{
  "success": false,
  "error": "OAuth authorization required",
  "authRequired": true
}
```

This lets the frontend distinguish "server is down" from "need to auth first" and
trigger the OAuth flow automatically.

### `POST /mcp/servers/{serverId}/oauth/initiate` (existing endpoint)

No change needed. Already returns `{ authUrl }`.

### Re-authorize = Revoke + Initiate

Option A: Frontend calls revoke then initiate (2 requests, simple).
Option B: New endpoint `POST /mcp/servers/{serverId}/oauth/reauthorize` that does both.

**Decision: Option A.** Keep endpoints orthogonal; frontend orchestrates. The `useOAuthFlow`
hook handles the sequence: revoke → initiate → popup → poll → done.

### Save Changes + OAuth validation

The `PATCH /mcp/admin/servers/{serverId}` handler currently saves and lets the frontend
handle OAuth revocation separately. Two options:

Option A: Frontend orchestrates: save → if OAuth changed, revoke → re-auth → test.
Option B: Backend does it: PATCH detects OAuth changes, revokes, returns `{ saved, authRequired }`.

**Decision: Option A.** Keep the backend PATCH idempotent and simple. The frontend
`useServerForm` hook handles the orchestration:

```
handleSave():
  1. call PATCH to save config
  2. if oauthConfigChanged → call revoke
  3. if authType === "oauth" → call initiate → popup → poll
  4. call test-connection to verify
  5. if test fails → show error (config is saved but connection broken)
```

Note: we don't roll back the PATCH on test failure — the config is correct, just
needs auth. The UI shows "Saved. OAuth authorization required." prompting the user.

## Implementation Steps

### Step 1: Backend — test-connection auth-required response

File: `crates/lib/nize_api/src/handlers/mcp_config.rs`

In `test_connection_handler`, when `authType === "oauth"` and no token found (or
decryption fails), return `{ success: false, authRequired: true, error: "..." }`
instead of attempting a connection without auth.

### Step 2: Extract `types.ts`

Shared TS types used across all form components.

### Step 3: Extract `useOAuthFlow` hook

Pull popup/message-listener/poll logic from `CreateServerForm.handleTest` and
`EditServerForm.handleOAuthReauthorize` into a reusable hook.

### Step 4: Extract field components

`HttpConfigFields`, `StdioConfigFields`, `OAuthConfigFields` — pure presentation
components that take values + onChange handlers.

### Step 5: Extract `OAuthStatusBanner`

Status display with Re-authorize button. Re-authorize calls:
revoke → initiate → `useOAuthFlow` → refresh status.

### Step 6: Build `useServerForm` hook

Form state management, validation, `buildConfig()`.

### Step 7: Build `ServerForm` component

Unified form shell using extracted components + hooks.

### Step 8: Wire up idiomatic button behaviors

- Test Connection: check auth → auto-auth if needed → test
- Save Changes: save → revoke if changed → auth if needed → test
- Re-authorize: revoke → auth → test

### Step 9: Replace admin page forms

Swap `CreateServerForm` + `EditServerForm` with `ServerForm` in
`app/settings/admin/tools/page.tsx`.

### Step 10: Replace user page forms

Swap `AddServerForm` + `EditUserServerForm` with `ServerForm` in
`app/settings/tools/page.tsx`.

### Step 11: Clean up

Remove old form components, dead code, unused imports.

## Risks & Open Questions

| # | Risk / Question | Mitigation |
|---|-----------------|------------|
| 1 | Save-then-auth means config is saved in a "not yet connected" state | Acceptable — UI shows clear status. Config is correct, auth is separate concern |
| 2 | Re-authorize removing Disconnect button — users may want to fully disconnect without re-auth | Users can change auth type to "none" to disable OAuth, or delete the server. Edge case; address if users request it |
| 3 | Auto-auth on Test Connection opens a popup unexpectedly | Show a brief "Authorization required — opening login..." message before popup |
| 4 | User page forms (simpler) don't support OAuth/stdio yet | Unified form still renders only HTTP fields when `showTransport: false`. OAuth can be added to user form later trivially |
| 5 | Large refactor risk — 4 forms + 2 pages touched | Incremental: extract hooks first (no UI changes), then swap forms one at a time |

## Completion Criteria

1. Re-authorize automatically revokes before re-initiating OAuth
2. Test Connection auto-initiates OAuth if authorization required
3. Save Changes saves config, then triggers auth + test for OAuth servers
4. Disconnect button removed; Re-authorize handles full cycle
5. All 4 form components replaced with unified `ServerForm`
6. OAuth popup/poll logic exists in one place (`useOAuthFlow`)
7. No functional regression for stdio or api-key server management
8. Backend `test-connection` returns `authRequired: true` for un-authed OAuth servers

## Change Log

- 0.1.0 (2026-02-19): Initial plan
