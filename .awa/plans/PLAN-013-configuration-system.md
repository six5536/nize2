# PLAN-013: Configuration System

| Field              | Value                                                    |
|--------------------|----------------------------------------------------------|
| **Status**         | in-progress                                              |
| **Workflow**       | top-down                                                 |
| **Reference**      | Ref: `submodules/nize` (CFG, AUTH specs + implementation) |
| **Traceability**   | REQ-CFG-configuration, DESIGN-CFG-configuration, PLAN-008 (user-auth) |

## Goal

Implement a hierarchical configuration system in nize-mcp, ported from the reference project (`submodules/nize`). The backend lives in `nize_core` (Rust) with REST API in `nize_api` (Axum), and the UI in `nize-web` (Next.js). nize-web also needs its own login UI (since it cannot obtain JWTs from nize-desktop like the embedded iframe model).

## Scope

1. **Database schema** — two tables (`config_definitions`, `config_values`) with SQL migration
2. **Seed data** — config definitions inserted at startup (idempotent)
3. **Config service** — resolver (user-override → system → defaultValue), validation, caching
4. **REST API** — user config endpoints (`GET/PATCH/DELETE /config/user[/:key]`), admin endpoints (`GET/PATCH /admin/config`)
5. **TypeSpec contract** — config endpoints added to the API contract, codegen updated
6. **nize-web auth** — login/register UI, auth context with JWT management, protected routes
7. **nize-web settings UI** — user settings page consuming config API

## Current State

| Component | State |
|-----------|-------|
| `nize_core` | auth module (password, jwt, queries), db/migration infra, models |
| `nize_api` | Axum router, auth handlers (login/register/refresh/logout), MCP token handlers, auth middleware |
| Migration | `0001_auth.sql` — users, refresh_tokens, user_roles tables |
| TypeSpec | Auth + hello endpoints, codegen pipeline (OpenAPI → Rust models/routes) |
| `nize-desktop` | AuthProvider with JWT (memory + localStorage), login/register UI |
| `nize-web` | Hello world page only, no auth, no settings, Next.js 16 standalone |
| `nize-api-client` | TS client generated from OpenAPI for nize-desktop use |

## Architecture Decisions

### D1 — Rust Config Service (not TypeScript)

The ref project implements config in TS/Hono. nize-mcp's API is Rust/Axum. Port the service logic to Rust with sqlx queries instead of Drizzle ORM.

### D2 — SQL Migration for Schema + Seed

Use a single SQL migration file (`0002_config.sql`) for both table creation and seed data insertion. Seed uses `INSERT ... ON CONFLICT DO UPDATE` for idempotency. No separate seed binary needed.

### D3 — In-Memory Cache in Rust

Use `tokio::sync::RwLock<HashMap<...>>` for the config cache, stored in `AppState`. TTLs loaded from system config after bootstrap. Simpler than the ref project's class-based cache.

### D4 — nize-web Auth via API (not Tauri)

nize-web calls the Rust API directly for auth (login/register/refresh/logout). Tokens stored in httpOnly cookies (set by the API), same pattern as the ref project's `auth-context.tsx`. Only non-sensitive user info is stored in localStorage. The web client uses `credentials: "include"` on all API calls — no manual token management. The API URL comes from a runtime environment variable or a build-time config.

### D5 — Dual Auth: Cookies + Bearer

The API supports both authentication methods, matching the ref project's middleware pattern:
- **httpOnly cookies** (`nize_access`, `nize_refresh`) — for nize-web (browser). Auth handlers set cookies on login/register/refresh and clear them on logout.
- **Authorization: Bearer** header — for nize-desktop, API clients, MCP tokens.

The Rust auth middleware checks cookies first, then falls back to Bearer header. This requires:
1. A cookie service module in `nize_api` (set/get/clear cookies via `axum_extra::extract::cookie`)
2. Auth handlers updated to set httpOnly cookies alongside the JSON response
3. CORS configuration updated to allow `credentials: include` from nize-web origin
4. `require_auth` middleware updated to check cookie → Bearer fallback

### D6 — API URL Discovery for nize-web

nize-web runs as a sidecar (PLAN-012). The API port is dynamic. Options:
- **Option A**: Tauri injects the API port into the nize-web server via env var at spawn time, Next.js exposes it as `NEXT_PUBLIC_API_PORT`.
- **Option B**: nize-web fetches the port from a Tauri command via postMessage from the parent iframe.

**Choice: Option A** — env var injection at spawn time. Simpler, works without iframe messaging. The nize-web server.mjs wrapper already receives args from Tauri; adding `--api-port` is trivial. For cloud deployment, a fixed API URL can be configured.

## Risks

| Risk | Mitigation |
|------|------------|
| Config cache race conditions | Use `RwLock`, invalidate on write before returning |
| Migration ordering (seed depends on tables) | Single migration file, tables first then inserts |
| nize-web auth complexity | Copy cookie-based patterns from ref project's auth-context.tsx, simpler than manual token management |
| TypeSpec/codegen complexity | Add config endpoints incrementally, verify codegen output |

## Steps

### Phase 1: Database Schema & Migration

Create the config tables and seed data.

- [ ] **1.1** Create `crates/lib/nize_api/migrations/0002_config.sql`:
  - `config_scope` enum type (`system`, `user-override`)
  - `config_definitions` table (key PK, category, type, display_type, possible_values jsonb, validators jsonb, default_value, label, description)
  - `config_values` table (id UUID PK, key FK→config_definitions, scope enum, user_id FK→users nullable, value text, updated_at timestamp)
  - Unique index on (key, scope, user_id) in config_values
  - Category index on config_definitions
  - Scope and user_id indexes on config_values
  - Seed INSERT statements for initial config definitions (port from ref project's seed.ts):
    - `system.cache.ttlSystem` (default: 300000)
    - `system.cache.ttlUserOverride` (default: 30000)
    - `agent.model.temperature` (default: 0.7)
    - `agent.model.maxTokens` (default: 4096)
    - `agent.model.id` (default: "gpt-4o")
    - `agent.model.provider` (default: "openai")
    - `agent.instruction.setContent` (default: system prompt)
    - `ui.theme` (default: "auto", possibleValues: ["light","dark","auto"])

- [ ] **1.2** Verify migration runs cleanly on fresh and existing databases (PGlite).

### Phase 2: Rust Config Models

Define Rust types for config in `nize_core`.

- [ ] **2.1** Create `crates/lib/nize_core/src/models/config.rs`:
  - `ConfigScope` enum (System, UserOverride) with sqlx FromRow/serde
  - `ConfigDefinition` struct matching config_definitions table
  - `ConfigValue` struct matching config_values table
  - `ConfigValidator` struct (type, value, message)
  - `ResolvedConfigItem` struct (definition metadata + resolved value + isOverridden flag)

- [ ] **2.2** Create `crates/lib/nize_core/src/config/mod.rs`:
  - `ConfigCache` struct using `HashMap` + TTL entries
  - Cache get/set/invalidate/invalidate_all_for_key/clear methods
  - Default TTL constants (5 min system, 30s user-override)

- [ ] **2.3** Create `crates/lib/nize_core/src/config/resolver.rs`:
  - `get_definition(pool, key)` — fetch config definition with cache
  - `get_effective_value(pool, cache, key, user_id)` — user-override → system → defaultValue
  - `get_all_effective_values(pool, cache, user_id)` — all user-applicable settings
  - `get_system_value(pool, cache, key)` — system-only resolution

- [ ] **2.4** Create `crates/lib/nize_core/src/config/validation.rs`:
  - `validate_value(value, validators)` — run all validators, return errors
  - Support: required, min, max, regex

### Phase 3: Config Service (nize_api)

Wire config logic into the API layer.

- [ ] **3.1** Create `crates/lib/nize_api/src/services/config.rs`:
  - `get_user_config(pool, cache, user_id)` → `Vec<ResolvedConfigItem>`
  - `update_user_config(pool, cache, user_id, key, value)` → Result
  - `reset_user_config(pool, cache, user_id, key)` → Result
  - `get_admin_config(pool, cache, filters)` → Vec with scope info
  - `update_admin_config(pool, cache, scope, key, value, user_id?)` → Result
  - All write operations: validate, persist, invalidate cache

- [ ] **3.2** Add `ConfigCache` to `AppState`:
  - Wrap in `Arc<RwLock<ConfigCache>>`
  - Initialize in server startup, reload TTLs after migration

### Phase 4: TypeSpec Contract & Codegen

Define config API endpoints.

- [ ] **4.1** Add config endpoints to TypeSpec (`API-NIZE-*.tsp` or new `API-NIZE-config.tsp`):
  - `GET /config/user` → `{ items: ResolvedConfigItem[] }`
  - `PATCH /config/user/{key}` → `ResolvedConfigItem` (body: `{ value }`)
  - `DELETE /config/user/{key}` → `204`
  - `GET /admin/config?scope=&userId=&category=&search=` → `{ items: AdminConfigItem[] }`
  - `PATCH /admin/config/{scope}/{key}?userId=` → `AdminConfigItem` (body: `{ value }`)

- [ ] **4.2** Run codegen pipeline to generate:
  - Rust route constants in `generated/routes.rs`
  - Rust request/response models in `generated/models.rs`
  - TypeScript types for `nize-api-types`
  - TypeScript client methods for `nize-api-client`

- [ ] **4.3** Verify generated code compiles and matches expected shapes.

### Phase 5: Config Handlers + Cookie Auth (nize_api)

Create Axum handlers for config endpoints and add cookie-based auth support.

- [ ] **5.1** Add cookie auth support to `nize_api`:
  - Add `axum_extra` dependency with `cookie` feature to `Cargo.toml`
  - Add `tower-cookies` dependency for cookie middleware layer
  - Create `crates/lib/nize_api/src/services/cookies.rs`:
    - Cookie names: `nize_access`, `nize_refresh`
    - `set_auth_cookies(jar, access_token, refresh_token, access_expires_in)` — sets httpOnly, Secure (in prod), SameSite=Lax cookies
    - `clear_auth_cookies(jar)` — deletes both cookies
    - `get_access_token_from_cookie(jar)` → `Option<String>`
    - `get_refresh_token_from_cookie(jar)` → `Option<String>`
  - Update `require_auth` middleware: check cookie first, fall back to `Authorization: Bearer` header (matching ref project's `middleware.ts`)
  - Update auth handlers (login/register/refresh/logout) to set/clear cookies alongside JSON response
  - Add CORS layer allowing `credentials: true` for nize-web origin

- [ ] **5.2** Create `crates/lib/nize_api/src/handlers/config.rs`:
  - `user_config_list_handler` — GET /config/user (auth required)
  - `user_config_update_handler` — PATCH /config/user/{key} (auth required)
  - `user_config_reset_handler` — DELETE /config/user/{key} (auth required)
  - `admin_config_list_handler` — GET /admin/config (admin required)
  - `admin_config_update_handler` — PATCH /admin/config/{scope}/{key} (admin required)

- [ ] **5.3** Add admin middleware:
  - `require_admin` middleware function checking `roles` in JWT claims
  - Apply to admin config routes

- [ ] **5.4** Wire handlers into router in `lib.rs`:
  - Config user routes under protected middleware
  - Config admin routes under admin middleware
  - Cookie middleware layer on the router

- [ ] **5.5** Test endpoints via curl/httpie against running dev server (both Bearer and cookie auth).

### Phase 6: nize-web Authentication

Add login/register UI to nize-web so it can authenticate independently.

- [ ] **6.1** Add dependencies to `packages/nize-web/package.json`:
  - No new deps needed (fetch is built-in, React state for auth)

- [ ] **6.2** Create `packages/nize-web/lib/api.ts`:
  - `apiUrl(path)` helper reading API base URL from `NEXT_PUBLIC_API_URL` env var
  - Default to `http://127.0.0.1:${NEXT_PUBLIC_API_PORT}` for sidecar mode

- [ ] **6.3** Create `packages/nize-web/lib/auth-context.tsx` (cookie-based, matching ref project):
  - `AuthProvider` component (wraps app)
  - State: user, isLoading, isAuthenticated
  - Token management: httpOnly cookies managed by API (no manual token handling)
  - Only non-sensitive user info (`{id, email, name, roles}`) in localStorage
  - All API calls use `credentials: "include"` — cookies sent/received automatically
  - `login(email, password)` → `POST /auth/login` with `credentials: "include"`, store user info
  - `register(email, password, name?)` → `POST /auth/register` with `credentials: "include"`, store user info
  - `logout()` → `POST /auth/logout` with `credentials: "include"`, clear user info
  - `validateSession()` on mount: call `POST /auth/refresh` with `credentials: "include"` to check if cookies are still valid
  - `useAuth()` hook
  - `useAuthFetch()` hook — wraps fetch with `credentials: "include"`, auto-logout on 401

- [ ] **6.4** Create `packages/nize-web/app/(auth)/login/page.tsx`:
  - Login form (email, password, submit)
  - Error display, loading state
  - Link to register page
  - Redirect to `/chat` (or `/`) on success

- [ ] **6.5** Create `packages/nize-web/app/(auth)/register/page.tsx`:
  - Registration form (name, email, password, confirm password)
  - Client-side validation (min 8 chars, passwords match)
  - Link to login page
  - Redirect to `/` on success

- [ ] **6.6** Update `packages/nize-web/app/layout.tsx`:
  - Wrap children in `AuthProvider`

- [ ] **6.7** Create auth gate component or middleware:
  - Redirect to `/login` if not authenticated on protected pages
  - Redirect to `/` if authenticated on auth pages

- [ ] **6.8** Pass API port/URL to nize-web sidecar:
  - Update nize-web server.mjs wrapper to accept `--api-port` arg
  - Set `NEXT_PUBLIC_API_PORT` env var when spawning Next.js
  - Update Rust sidecar spawn code in `nize_desktop` to pass the API port

### Phase 7: nize-web Settings UI

Implement user settings page in nize-web.

- [ ] **7.1** Create `packages/nize-web/app/settings/page.tsx`:
  - Fetch `GET /config/user` via `useAuthFetch()`
  - Render config items grouped by category
  - Each item renders appropriate input based on `displayType`:
    - `number` → number input
    - `text` → text input
    - `longText` → textarea
    - `selector` → dropdown from `possibleValues`
  - Save via `PATCH /config/user/{key}`
  - Reset to default via `DELETE /config/user/{key}`
  - Show isOverridden indicator
  - Success/error feedback

- [ ] **7.2** Add navigation to settings page:
  - Nav link or menu item in main layout
  - Protected route (requires auth)

- [ ] **7.3** Style with Tailwind CSS (add tailwind to nize-web if not present) or inline styles matching existing pattern.

### Phase 8: Integration & Verification

- [ ] **8.1** End-to-end flow test:
  1. Start app (`cargo tauri dev`)
  2. Register admin user (nize-desktop)
  3. Switch to Web tab
  4. Login in nize-web
  5. Navigate to /settings
  6. View config items with default values
  7. Update a config item (e.g., temperature)
  8. Verify override persists after page reload
  9. Reset config item, verify returns to default

- [ ] **8.2** Verify cache behavior:
  - Update system value, verify all users see new effective value
  - Update user override, verify only that user is affected
  - Verify TTL expiry (entries expire and refresh from DB)

- [ ] **8.3** Verify admin endpoints:
  - Admin can list all config (filtered by scope/category/user)
  - Admin can update system values
  - Admin can update user overrides

## Open Questions

1. **Admin UI in nize-web?** — The ref project has `/admin/settings` page. Do we need this now or defer to a later plan? **Recommendation: defer admin settings UI, implement API endpoints only for now.**

2. **Config definitions — which keys to seed initially?** — Port the ref project's set (system cache TTLs, agent model settings, UI theme) or start with a minimal set? **Recommendation: start with ref project's keys minus instruction-specific ones (setContent, toolPolicy) that depend on unimplemented features.**

3. **Tailwind in nize-web?** — The current nize-web has no styling framework. Should we add Tailwind now or use inline styles? **Recommendation: add Tailwind, matching the ref project pattern and enabling rapid UI development for future features.**

## Dependencies

- PLAN-008 (user-auth) — auth tables, JWT, middleware ✓ **completed**
- PLAN-012 (nize-web-sidecar) — nize-web exists, tab UI, sidecar protocol ✓ **completed**
- TypeSpec codegen pipeline — must support new endpoints
