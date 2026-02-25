# PLAN-028: AI Provider API Key Management

**Status:** in-progress (manual validation remaining: 5.3, 5.4, 5.5)
**Workflow direction:** lateral
**Traceability:** ARCHITECTURE.md → nize_core (config, secrets), nize_api, nize-web (settings UI), nize-chat (model-registry); PLAN-027 (chat backend)

## Goal

Allow users to manage AI provider API keys (Anthropic, OpenAI, Google) through the settings UI, stored encrypted in the database. Decrypted keys never leave the Rust API process — nize-chat routes AI SDK requests through a Rust proxy that injects auth headers server-side.

**In scope:** Encrypted secret storage in config system, masked display in API/UI, Rust AI proxy for key injection, custom provider base URLs, env var fallback, admin-managed shared keys (system scope).
**Out of scope:** Key rotation UX, per-conversation model/key override, key usage quotas.

## Architecture Decisions

### Storage: Extend Config System

| Considered | Decision | Rationale |
|---|---|---|
| (A) Separate `user_secrets` table | Rejected | New CRUD; duplicates config system features (category, user-override, UI rendering) |
| (B) Store keys in environment variables only | Rejected | Single-user only; no UI management; desktop users can't easily set them |
| (C) Extend config system with `"secret"` display type | **Selected** | Minimal new code; leverages existing config definitions, validation, settings UI, user-override scope; consistent with `embedding.openaiApiKey` precedent |

### Key Delivery: Rust AI Proxy

| Considered | Decision | Rationale |
|---|---|---|
| (D) Secrets HTTP endpoint + nize-chat injects keys | Rejected | Decrypted keys exposed over HTTP; env var sharing insecure in desktop mode; XSS can exfiltrate |
| (E) Embed keys in JWT claims | Rejected | Visible in browser DevTools; inflates cookie size; key changes require re-login |
| (F) Internal service token header | Rejected | Shared secret exposed as env var in desktop mode (all processes on same machine) |
| (P1) Rust proxy with per-provider `baseURL` routes | Rejected | Requires per-provider route setup; less extensible for future providers |
| (P2) Rust proxy with custom `fetch` — single endpoint | **Selected** | Single proxy endpoint; provider-agnostic; keys never leave Rust; future providers need zero proxy changes |

## Current State

1. AI SDK providers read from env vars: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_GENERATIVE_AI_API_KEY`
2. `getChatModel(spec)` in nize-chat calls bare `anthropic(modelName)` — uses implicit env var lookup
3. Config system has `config_definitions` + `config_values` tables with `display_type` column (number, text, longText, selector)
4. `embedding.openaiApiKey` already exists as a config definition — stored as *plaintext* in `config_values` (security gap; also inconsistent naming)
5. Rust has AES-256-GCM encryption in `nize_core::mcp::secrets` (`encrypt`/`decrypt`) using `MCP_ENCRYPTION_KEY`
6. Settings UI renders config items dynamically by `display_type`

## Design

### Secret Config Values

Add a new `display_type: "secret"` to the config system. Config values with this display type:

- **Stored encrypted** in `config_values.value` via `nize_core::mcp::secrets::encrypt()`
- **Masked in API responses** — never return plaintext; return `"••••••4567"` (last 4 chars) or `""` if unset
- **Decrypted only server-side** — no HTTP endpoint ever returns decrypted keys; decryption happens only inside the Rust AI proxy handler
- **Settings UI** renders a password input with show/hide toggle, clear button, and "configured" indicator

### AI Proxy

Single reverse-proxy endpoint in the Rust API: `POST /ai-proxy`. nize-chat injects a custom `fetch` implementation into AI SDK provider constructors that routes all outbound requests through this proxy.

The proxy:
1. Authenticates the user (JWT cookie — same as all other endpoints)
2. Reads the target URL and provider type from query params `?target={url}&provider=anthropic`
3. Validates the provider type against a known set (`anthropic`, `openai`, `google`)
4. Decrypts the user's API key for that provider from config (user-override → system → env var fallback)
5. Injects the provider-specific auth header based on the provider type
6. Proxies the request to the target URL and streams the response back

The provider type is explicit (not inferred from domain) because users may configure custom base URLs (e.g., Azure OpenAI, self-hosted endpoints, corporate proxies) where the domain doesn't match the standard provider domain.

**Provider type → auth header mapping:**

| Provider type | Config key (API key) | Config key (base URL) | Auth header | Env var fallback |
|---|---|---|---|---|
| `anthropic` | `agent.apiKey.anthropic` | `agent.baseUrl.anthropic` | `x-api-key: {key}` | `ANTHROPIC_API_KEY` |
| `openai` | `agent.apiKey.openai` | `agent.baseUrl.openai` | `Authorization: Bearer {key}` | `OPENAI_API_KEY` |
| `google` | `agent.apiKey.google` | `agent.baseUrl.google` | `x-goog-api-key: {key}` | `GOOGLE_GENERATIVE_AI_API_KEY` |

Unrecognized provider types are rejected (403). Adding a future provider requires only a new row in this mapping (config key + auth header format) — no proxy code changes.

### Config Keys

**API keys** (secret — encrypted at rest, masked in API responses):

| Key | Category | Display Type | Default | Label |
|---|---|---|---|---|
| `agent.apiKey.anthropic` | agent | secret | `""` | Anthropic API Key |
| `agent.apiKey.openai` | agent | secret | `""` | OpenAI API Key |
| `agent.apiKey.google` | agent | secret | `""` | Google API Key |
| `embedding.apiKey.openai` | embedding | secret | `""` | OpenAI API Key (Embeddings) |

**Base URLs** (text — user-configurable, empty = use SDK default):

| Key | Category | Display Type | Default | Label |
|---|---|---|---|---|
| `agent.baseUrl.anthropic` | agent | text | `""` | Anthropic Base URL |
| `agent.baseUrl.openai` | agent | text | `""` | OpenAI Base URL |
| `agent.baseUrl.google` | agent | text | `""` | Google AI Base URL |

All keys support both scopes:
- **user-override**: each user provides their own keys/URLs via Settings → General
- **system**: admin sets shared org keys/URLs via Settings → Admin; users inherit unless they override

### Data Flow

```
Settings UI
  └─ PATCH /config/user/{key} { value: "sk-ant-..." }
       └─ Rust API detects display_type="secret"
            └─ encrypt(value, MCP_ENCRYPTION_KEY) → store encrypted in config_values

Chat Request
  └─ nize-chat processChat()
       ├─ fetchChatConfig() → GET /config/user → model name, temperature, base URLs, etc.
       ├─ getChatModel(spec, proxyFetch) → provider with custom fetch + baseURL
       │    └─ e.g. anthropic(modelName, { baseURL: config.baseUrl.anthropic, fetch: proxyFetch })
       └─ streamText({ model }) → AI SDK calls provider via custom fetch
            └─ custom fetch rewrites: POST https://my-proxy.corp.com/v1/messages
                 → POST http://127.0.0.1:3001/api/ai-proxy?target=https://my-proxy.corp.com/v1/messages&provider=anthropic
                      └─ Rust proxy: auth user → decrypt key → inject header → proxy to target
```

### Env Var Fallback

When no DB key is set for a provider, fall back to the environment variable. This supports:
- Development without DB setup
- Cloud deployments with env-injected keys
- Backward compatibility

Priority: `user-override config → system config → env var → error at call time`

### Fixing `embedding.openaiApiKey`

The existing `embedding.openaiApiKey` stores keys in plaintext and uses inconsistent naming. This plan:
1. Renames `embedding.openaiApiKey` → `embedding.apiKey.openai` (consistent `{category}.apiKey.{provider}` format)
2. Changes its `display_type` from `"text"` to `"secret"` in the migration
3. Migrates any existing plaintext value: decrypt-if-needed → re-encrypt under new key
4. Drops the old `embedding.openaiApiKey` definition

## Steps

### Phase 1: Config System — Secret Display Type

- [x] 1.1 — Migration `0007_secret_config.sql`:
  - Add `agent.apiKey.anthropic`, `agent.apiKey.openai`, `agent.apiKey.google` config definitions with `display_type = 'secret'`
  - Add `agent.baseUrl.anthropic`, `agent.baseUrl.openai`, `agent.baseUrl.google` config definitions with `display_type = 'text'` and standard defaults
  - Add `embedding.apiKey.openai` config definition with `display_type = 'secret'`
  - Migrate existing `embedding.openaiApiKey` values to `embedding.apiKey.openai` (re-encrypt)
  - Drop old `embedding.openaiApiKey` definition
- [x] 1.2 — Rust config service (`nize_core::config`): detect `display_type = "secret"` on write → encrypt value before storing in `config_values`
- [x] 1.3 — Rust config service: on read, mask secret values in `ResolvedConfigItem.value` (last 4 chars or empty string)
- [x] 1.4 — New handler `POST /ai-proxy` in `nize_api`: authenticated, proxies AI SDK requests to providers with injected auth headers. Reads `?target={url}&provider={type}` from query params. Validates provider type against known set. Decrypts user's API key from config, injects provider-specific auth header, streams response. Rejects unknown provider types (403).

### Phase 2: Settings UI — Secret Input

- [x] 2.1 — Add `"secret"` case to `renderInput()` in `packages/nize-web/app/settings/page.tsx`:
  - Password input with show/hide eye toggle
  - "Configured" badge when masked value is non-empty
  - Clear button to remove the key (DELETE endpoint)
  - On save, send plaintext via PATCH (HTTPS in transit; Rust encrypts at rest)
- [x] 2.2 — Admin settings page (`/settings/admin`): render secret inputs for system-scope keys (admin sets shared org keys that users inherit)

### Phase 3: nize-chat — Proxy Integration

- [x] 3.1 — Create `proxyFetch(apiBaseUrl, cookie, providerType)` factory in nize-chat: returns a custom `fetch` that rewrites target URLs through `POST /ai-proxy?target={originalUrl}&provider={providerType}`, forwarding the user's cookie for auth
- [x] 3.2 — Extend `ChatConfig` type: add optional `baseUrls: { anthropic?: string; openai?: string; google?: string }` read from config
- [x] 3.3 — Extend `fetchChatConfig()`: read `agent.baseUrl.*` config values into `ChatConfig.baseUrls`
- [x] 3.4 — Update `getChatModel(spec, proxyFetch, baseUrls)`: pass custom `fetch` and `baseURL` to provider constructors:
  ```ts
  case "anthropic": return anthropic(modelName, { baseURL: baseUrls?.anthropic, fetch: proxyFetch });
  case "openai":    return openai(modelName, { baseURL: baseUrls?.openai, fetch: proxyFetch });
  case "google":    return google(modelName, { baseURL: baseUrls?.google, fetch: proxyFetch });
  ```
  Providers use the custom base URL + custom fetch → requests go to configured endpoint through Rust proxy.
- [x] 3.5 — Update `processChat()` and `generateTitle()`: create `proxyFetch` with provider type extracted from model spec, pass with `baseUrls` through to `getChatModel`

### Phase 4: Embedding Config Fix

- [x] 4.1 — Update `EmbeddingConfig::resolve()` in `nize_core::embedding::config`: read from `embedding.apiKey.openai` (new key name) via secret decryption path instead of plaintext `embedding.openaiApiKey`
- [x] 4.2 — Update any other references to `embedding.openaiApiKey` in Rust code to use `embedding.apiKey.openai`

### Phase 5: Validation

- [x] 5.1 — Rust unit tests: encrypt-on-write, mask-on-read, proxy auth header injection, unknown provider type rejection
- [x] 5.2 — nize-chat tests: `proxyFetch` URL rewriting with provider type, `getChatModel` with custom fetch + baseURL, `fetchChatConfig` base URL resolution
- [ ] 5.3 — Manual test: open Settings → agent category → enter Anthropic key → verify masked display → send chat message → verify streaming works
- [ ] 5.4 — Verify `embedding.openaiApiKey` → `embedding.apiKey.openai` migration: old values migrated, new writes encrypted
- [ ] 5.5 — Verify admin shared keys: admin sets system-scope key → user without override inherits it → chat works
- [x] 5.6 — `bun run build` (nize-web), `cargo check` (nize_api), `cargo test` (nize_core)

## Dependencies

| Dependency | Status | Blocking? |
|---|---|---|
| PLAN-027 (chat backend) | in-progress | Partially — Phase 3 requires nize-chat to exist |
| Config system (nize_core) | done | No |
| Encryption module (nize_core::mcp::secrets) | done | No |
| Settings UI | done | No |

## Risks

| Risk | Mitigation |
|---|---|
| Proxy used as open relay | Provider type allowlist (unknown types rejected 403) + JWT auth (only authenticated users; each request tied to user identity for abuse detection/rate limiting) |
| MCP_ENCRYPTION_KEY not set in production | Already defaulted for dev; document production requirement |
| Plaintext embedding key migration loses data | SQL migration copies + encrypts existing values before dropping old key |
| AI SDK custom fetch signature changes | Pin provider SDK versions; custom fetch uses standard Web Fetch API |
| Latency: extra hop through Rust proxy | ~0.1ms on loopback; negligible vs. provider RTT (100ms+) |
| Proxy must handle SSE streaming | Rust reqwest + hyper stream response bodies natively; no buffering needed |

## Resolved Questions

1. ~~Should admin-managed shared keys be in scope?~~ **Yes.** Admin sets system-scope keys via admin settings; users inherit unless they override with their own.
2. ~~Should the secrets endpoint require additional auth?~~ **Resolved by elimination.** No secrets endpoint exists — decrypted keys never leave the Rust process. The AI proxy uses standard JWT auth (same as all endpoints). XSS cannot exfiltrate keys because no HTTP response ever contains them.
3. ~~Should `embedding.openaiApiKey` and `agent.apiKey.openai` be unified?~~ **No — keep separate** (different purposes, potentially different keys). **Rename** `embedding.openaiApiKey` → `embedding.apiKey.openai` for consistent `{category}.apiKey.{provider}` naming.

## Completion Criteria

- Users can enter/update/clear AI provider API keys via Settings → Agent
- Keys are encrypted at rest in `config_values` (AES-256-GCM)
- API responses mask secret values (never return plaintext in config list)
- Decrypted keys never leave the Rust API process (no HTTP endpoint returns them)
- AI SDK requests route through Rust AI proxy which injects auth headers server-side
- Chat streaming works with DB-stored key (no env var set)
- Users can configure custom base URLs per provider (e.g., Azure OpenAI, corporate proxy)
- `embedding.openaiApiKey` renamed to `embedding.apiKey.openai` and migrated to encrypted storage
- Admin can set system-scope shared keys and base URLs; users inherit or override
