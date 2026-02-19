# Google OAuth for gogmcp Integration

## Overview

This document describes the authentication architecture for nize-mcp connecting to gogmcp instances, enabling end users to interact with their Google Calendar (and future Google APIs) through AI agents.

The key insight: Google OAuth already produces a signed identity token (ID token). By having gogmcp validate Google ID tokens directly using Google's public keys, we eliminate the need for a shared JWT secret between nize-mcp and gogmcp.

## End-to-End Flow

```
User clicks "Connect Google" in UI
  → Rust backend starts OAuth with PKCE (scopes include openid + email)
  → System browser opens Google consent screen
  → Google redirects to localhost callback
  → Rust backend exchanges code for: id_token + access_token + refresh_token
  → Stores refresh_token + access_token encrypted in DB
  → Connects to gogmcp SSE with:
      Authorization: Bearer <google_id_token>
      X-Google-Access-Token: <google_access_token>
  → Proxies tool calls to gogmcp, handles token refresh + reconnect
```

Reference: `submodules/gogmcp/doc/SECURITY.md`

## System Architecture

```
┌───────────────────────────────────────────────┐
│                  Tauri App                     │
│  ┌──────────┐        ┌─────────────────────┐  │
│  │ Webview  │◄──IPC──│    Rust Backend      │  │
│  │ (UI)     │        │                      │  │
│  └──────────┘        │  • OAuth flow mgmt   │
│                      │  • Token store        │
│                      │  • MCP SSE proxy      │
│                      └──────────┬────────────┘  │
└─────────────────────────────────┼───────────────┘
                                  │ TLS + Google ID token + access token
                                  ▼
                          ┌──────────────┐
                          │   gogmcp     │
                          │  (SSE/TLS)   │
                          │              │
                          │  Validates   │
                          │  ID token    │
                          │  via Google  │
                          │  public keys │
                          └──────┬───────┘
                                 │ OAuth access_token
                                 ▼
                          ┌─────────────┐
                          │ Google APIs  │
                          └─────────────┘
```

**Core principle:** The Rust backend is the sole trust boundary. The webview never sees tokens or the gogmcp connection directly. All sensitive operations live in Rust.

**Auth principle:** No shared secrets. Identity is verified by Google's asymmetric signatures. Trust is anchored on OAuth client IDs.

## Authentication Design: Google ID Tokens

### Why Not Custom JWTs

The original SECURITY.md design minted custom HS256 JWTs signed with a shared `GOGMCP_JWT_SECRET`. This required:
- Manual secret synchronization between nize-mcp and gogmcp
- Encrypted secret storage per-server in nize-mcp's DB
- A JWT minting module in nize-mcp
- Risk: anyone with the secret can forge any identity

### How Google ID Tokens Work

When OAuth scopes include `openid` and `email`, Google returns an **ID token** alongside the access token. This ID token is:
- A JWT signed by Google using RS256 (asymmetric)
- Verifiable using Google's public JWKS keys at `https://www.googleapis.com/oauth2/v3/certs`
- Contains `email`, `email_verified`, `aud` (= your OAuth client ID), `iss`, `exp`
- Valid for ~1 hour (same lifecycle as the access token)

### Wire Format

```
GET /sse
Authorization: Bearer <google_id_token>
X-Google-Access-Token: <google_access_token>
```

The Bearer token is a Google-signed JWT. gogmcp validates it using Google's public keys (fetched and cached). No shared secret needed.

### Google ID Token Claims

```json
{
  "iss": "https://accounts.google.com",
  "aud": "123456789.apps.googleusercontent.com",
  "sub": "1234567890",
  "email": "user@example.com",
  "email_verified": true,
  "iat": 1708300000,
  "exp": 1708303600
}
```

### Security Comparison

| Aspect | Shared JWT Secret (old) | Google ID Token (new) |
|--------|------------------------|-----------------------|
| **Local** | HMAC, secret in both processes | RS256, no shared secret |
| **Remote** | Must securely distribute secret | Just configure client ID allowlist |
| **Compromised gogmcp** | Attacker can forge any identity | Attacker can't forge — needs Google's private key |
| **Key rotation** | Manual, coordinated | Automatic (Google rotates keys) |
| **Trust model** | "who has the secret" | "Google vouches for this user" |
| **Eliminated** | — | JWT minting, secret storage, secret distribution |

## What Exists Already

### In nize-mcp

| Artifact | Location | Status |
|----------|----------|--------|
| DB tables: `mcp_oauth_tokens`, `mcp_server_secrets`, `mcp_servers.oauth_config` | `crates/lib/nize_core/migrations/0004_mcp_servers.sql` | Created |
| Models: `OAuthConfig`, `McpOauthTokenRow`, `AuthType::OAuth` | `crates/lib/nize_core/src/models/mcp.rs` | Defined |
| Route: `POST /mcp/servers/{serverId}/oauth/initiate` | `crates/lib/nize_api/src/handlers/mcp_config.rs` | **Stub** |
| Route: `GET /mcp/servers/{serverId}/oauth/status` | `crates/lib/nize_api/src/handlers/mcp_config.rs` | **Stub** |
| Route: `POST /mcp/servers/{serverId}/oauth/revoke` | `crates/lib/nize_api/src/handlers/mcp_config.rs` | **Stub** |
| Route: `GET /auth/oauth/mcp/callback` | `crates/lib/nize_api/src/handlers/oauth.rs` | **Stub** |
| Query: `has_valid_oauth_token` | `crates/lib/nize_core/src/mcp/queries.rs` | Implemented |

### In gogmcp (current, to be changed)

| Artifact | Location | Details |
|----------|----------|---------|
| JWT validation (HMAC-SHA256) | `internal/auth/jwt.go` | **Replace with Google OIDC validation** |
| JWT claims | `internal/auth/types.go` | **Replace with Google ID token claims** |
| Auth context injection | `internal/auth/context.go` | **Update to use new claims struct** |
| Google token context | `internal/google/auth.go` | No change needed |
| SSE server | `internal/mcp/server.go` | No change needed (uses auth.ContextFunc) |
| Tool registration | `internal/mcp/tools.go` | No change needed |
| Config | `internal/config/config.go` | **Replace `JWTSecret` with `AllowedClientIDs`** |

### gogmcp Changes Required

See `doc/gog-plan.md` for detailed implementation plan.

## Key Decisions

### 1. Google OAuth Client Credentials Source

**Decision:** Per-server `oauth_config` in DB. The `OAuthConfig` model has `client_id`, `authorization_url`, `token_url`, `scopes`. Client secret stored in `mcp_server_secrets.oauth_client_secret_encrypted`.

### 2. Identity Verification (replaces JWT shared secret)

**Decision:** Google ID tokens validated with Google's public JWKS keys. No shared secret. Trust anchored on OAuth `client_id` allowlist in gogmcp config.

### 3. OAuth Redirect URI

| Mode | Redirect URI | Token Storage |
|------|-------------|---------------|
| Desktop | `http://localhost:{api_port}/auth/oauth/mcp/callback` | Encrypted DB |
| Server | `https://yourapp.com/auth/oauth/mcp/callback` | Encrypted DB |

### 4. Google Token Delivery

| Token | Delivered Via | Purpose |
|-------|-------------|---------|
| ID token | `Authorization: Bearer <id_token>` | Identity verification |
| Access token | `X-Google-Access-Token: <access_token>` | Google API calls |

Both tokens refresh together (single refresh_token call returns new id_token + access_token).

## Implementation Phases (nize-mcp side)

### Phase 1: Google OAuth Token Acquisition

1. **Add `oauth2` crate** to `nize_core` dependencies
2. **Extend `CreateUserServerRequest` / `CreateAdminServerRequest`** to accept `oauthConfig` and `clientSecret`
3. **Implement `oauth_initiate_handler`**
   - Load `OAuthConfig` from server's `oauth_config` JSONB
   - Decrypt `oauth_client_secret_encrypted` from `mcp_server_secrets`
   - Generate PKCE `code_verifier` + `code_challenge`
   - Build Google authorization URL with `state` parameter (encodes `serverId` + CSRF token)
   - Scopes **must include** `openid` and `email` (for ID token)
   - Store PKCE verifier in ephemeral state (in-memory map with TTL)
   - Return `{ authUrl }` to frontend
4. **Implement `oauth_callback_handler`**
   - Receive `code` + `state` from Google redirect
   - Exchange code for tokens (response includes `id_token`, `access_token`, `refresh_token`)
   - Store `id_token` in `mcp_oauth_tokens` (new column or alongside access_token)
   - Encrypt and store `access_token` and `refresh_token`
   - Mark server as available for this user
   - Redirect to desktop UI with success indicator
5. **Implement `oauth_status_handler`** and **`oauth_revoke_handler`**

### Phase 2: gogmcp Connection

6. **Connect to gogmcp SSE** with:
   - `Authorization: Bearer <id_token>` (from stored tokens)
   - `X-Google-Access-Token: <access_token>` (decrypted from DB)
7. **Token refresh**: When access_token expires (~1 hour):
   - Call Google's token endpoint with refresh_token
   - Get new `id_token` + `access_token` in one call
   - Reconnect SSE with fresh tokens
8. **Preemptive reconnection** at ~80% of token lifetime

### Phase 3: gogmcp Auth Rewrite

See `doc/gog-plan.md` — replace HMAC JWT validation with Google OIDC validation.

## Server Registration

### User Registration

```json
POST /mcp/servers
{
  "name": "My Google Calendar",
  "url": "https://localhost:8443",
  "authType": "oauth",
  "domain": "google",
  "oauthConfig": {
    "clientId": "123456.apps.googleusercontent.com",
    "authorizationUrl": "https://accounts.google.com/o/oauth2/v2/auth",
    "tokenUrl": "https://oauth2.googleapis.com/token",
    "scopes": ["openid", "email", "https://www.googleapis.com/auth/calendar.readonly"]
  },
  "clientSecret": "GOCSPX-..."
}
```

### Admin Registration (pre-configured for all users)

```json
POST /mcp/admin/servers
{
  "name": "Google Calendar",
  "domain": "google",
  "visibility": "visible",
  "config": {
    "transport": "http",
    "url": "https://gogmcp.internal:8443",
    "authType": "oauth"
  },
  "oauthConfig": {
    "clientId": "123456.apps.googleusercontent.com",
    "authorizationUrl": "https://accounts.google.com/o/oauth2/v2/auth",
    "tokenUrl": "https://oauth2.googleapis.com/token",
    "scopes": ["openid", "email", "https://www.googleapis.com/auth/calendar.readonly"]
  },
  "clientSecret": "GOCSPX-..."
}
```

Users see the server as `Unauthorized` until they complete Google OAuth consent.

## gogmcp Configuration (new)

```bash
GOGMCP_PORT=8443
GOGMCP_TLS_CERT=/path/to/cert.pem
GOGMCP_TLS_KEY=/path/to/key.pem
GOGMCP_AUTH_MODE=google                          # "google" or "jwt" (legacy)
GOGMCP_ALLOWED_CLIENT_IDS=123456.apps.googleusercontent.com
# Legacy mode (backward compatible):
# GOGMCP_AUTH_MODE=jwt
# GOGMCP_JWT_SECRET=<shared-secret>
```

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| XSS in webview | Webview never holds tokens; all auth material is in Rust process |
| Forged identity | ID tokens signed by Google (RS256); unforgeable without Google's private key |
| Stolen refresh token | Encrypted at rest. Revocable via Google account settings |
| Leaked ID token | ~1 hour expiry. Does not contain Google API access token |
| Man-in-the-middle | TLS 1.2+ enforced between nize-mcp and gogmcp |
| Token replay | TLS prevents interception. Tokens expire in ~1 hour |
| Compromised gogmcp | Cannot forge identity (no signing key). Google tokens are per-request |
| Rogue client ID | gogmcp validates `aud` against `GOGMCP_ALLOWED_CLIENT_IDS` allowlist |

## Anti-Patterns to Avoid

- OAuth in the webview (token interception via XSS/DOM access)
- Sending refresh_token to gogmcp (only access_token + id_token needed)
- Webview connecting to gogmcp directly (bypasses Rust trust boundary)
- Logging tokens
- Skipping `aud` validation in gogmcp (allows tokens from other apps)

## Google Cloud Console Setup

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create OAuth 2.0 credentials (type: Web application)
3. Add authorized redirect URI: `http://localhost:{port}/auth/oauth/mcp/callback`
4. Note the `client_id` and `client_secret`
5. Enable the Google Calendar API
6. Required scopes: `openid`, `email`, `https://www.googleapis.com/auth/calendar.readonly`

The `client_id` is configured in **both** nize-mcp (for OAuth flow) and gogmcp (for `aud` validation in `GOGMCP_ALLOWED_CLIENT_IDS`). The `client_secret` is only in nize-mcp.
