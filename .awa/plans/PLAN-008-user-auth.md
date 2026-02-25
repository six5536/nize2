# PLAN-008: User Authentication

## Metadata

- **Status:** in-progress
- **Workflow direction:** top-down
- **Traceability:** Reference project: `submodules/nize` (AUTH, PRM-9 specs)

## Goal

Add user authentication to nize-mcp, mirroring the reference project's DB tables, JWT auth, and API contract. The API is implemented in Rust (Axum in `nize_api_server`), defined via TypeSpec, and consumed via the generated `nize_api_client` in the Tauri desktop app. First-run requires admin user creation.

## Reference Artifacts

- `submodules/nize/.awa/specs/REQ-AUTH-authentication.md` — requirements
- `submodules/nize/.awa/specs/DESIGN-AUTH-authentication.md` — design
- `submodules/nize/packages/db/src/schema/auth.ts` — DB schema (Drizzle)
- `submodules/nize/packages/db/src/schema/permissions.ts` — `user_roles` table
- `submodules/nize/packages/api-types/src/auth.tsp` — TypeSpec auth contract
- `submodules/nize/apps/api/src/services/auth.ts` — auth service impl (TS)

## Current State

- `nize_api_server` (Axum) serves one endpoint (`GET /api/hello`)
- `nize_api_client` (progenitor) generated from OpenAPI JSON
- `nize_codegen` generates route constants and models from OpenAPI
- DB provisioning via `nize_core::db` (PGlite or native PG); creates DB + vector extension
- Desktop app spawns PGlite → API sidecar → connects via generated client
- No auth, no user tables, no middleware

## Plan

### Phase 1: DB Schema & Migrations

SQL migrations run by `nize_api_server` at startup via `sqlx::migrate!`.

**1.1 Create SQL migration file**

`crates/lib/nize_api/migrations/0001_auth.sql`:

```sql
-- Users
CREATE TABLE IF NOT EXISTS users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email VARCHAR(255) NOT NULL UNIQUE,
  name VARCHAR(255),
  password_hash VARCHAR(255),
  email_verified TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Refresh tokens (JWT rotation, SHA-256 hashed — fixes ref code security flaw)
CREATE TABLE IF NOT EXISTS refresh_tokens (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  token_hash VARCHAR(64) NOT NULL UNIQUE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ
);

-- User roles (admin bootstrap)
CREATE TYPE user_role AS ENUM ('admin');

CREATE TABLE IF NOT EXISTS user_roles (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role user_role NOT NULL,
  granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  granted_by UUID REFERENCES users(id) ON DELETE SET NULL,
  PRIMARY KEY (user_id, role)
);
```

Tables match reference project: `users`, `refresh_tokens`, `user_roles`.
Omitted for now: `accounts` (OAuth), `sessions` (legacy), permission tables.

**1.2 Run migrations at API server startup**

Add `sqlx::migrate!()` call in `nize_api_server/src/main.rs` after pool creation.

**1.3 Add sqlx `migrate` feature**

Add `"migrate"` to sqlx features in workspace `Cargo.toml`.

### Phase 2: TypeSpec Auth Contract

**2.1 Add common error models**

Extend `API-NIZE-common.tsp` with `UnauthorizedError`, `ValidationError`.

**2.2 Create `API-NIZE-auth.tsp`**

Port from reference `submodules/nize/packages/api-types/src/auth.tsp`:
- `LoginRequest`, `RegisterRequest`, `RefreshRequest`, `LogoutRequest`
- `AuthUser`, `TokenResponse`, `LogoutResponse`
- `AuthRoutes` interface (`/auth/login`, `/auth/register`, `/auth/refresh`, `/auth/logout`)
- `GET /auth/status` — returns whether admin user exists (for first-run flow)

**2.3 Import in `API-NIZE-index.tsp`**

Add `import "./API-NIZE-auth.tsp"`.

**2.4 Regenerate OpenAPI JSON**

Run `tsp compile` → produces updated `codegen/nize-api/tsp-output/openapi.json`.

**2.5 Regenerate Rust code**

Run `cargo build -p nize_codegen` → updates route constants and models.
Run `cargo build -p nize_api_client` → updates generated client.

### Phase 3: Rust Auth Implementation (nize_api)

**3.1 Add dependencies**

Add to `nize_api/Cargo.toml`:
- `bcrypt` — password hashing (ref code uses bcryptjs with cost 10; Rust equivalent)
- `jsonwebtoken` — JWT signing/verification (HS256, mirrors ref `jose`)
- `rand` — secure random token generation (ref code uses nanoid(64); Rust equivalent)
- `chrono` — timestamp handling (if not already via sqlx)

**3.2 Auth service module (`nize_api/src/services/auth.rs`)**

Core auth logic (follows ref code for implementation choices, fixes security flaws):
- `hash_password(password) → String` — bcrypt, cost 10 (ref code)
- `verify_password(password, hash) → bool` — bcrypt verify
- `generate_access_token(claims) → String` — HS256, 15min expiry
- `verify_access_token(token) → Option<TokenPayload>`
- `generate_refresh_token() → String` — cryptographic random, 64 chars (ref code)
- Token payload: `{ sub, email, roles[], exp, iat }` — standard `sub` claim (**fix**: ref code uses non-standard `userId`)
- Refresh token stored as **SHA-256 hash** in `token_hash` column (**fix**: ref code stores plaintext)
- Refresh token expiry: **30 days** (ref code; reasonable for desktop UX)
- `login(pool, email, password) → Result<TokenResponse>`
- `register(pool, email, password, name?) → Result<TokenResponse>`
  - First user auto-granted admin role
- `refresh(pool, token) → Result<TokenResponse>` (lookup by SHA-256 hash; rotation: revoke old, issue new)
- `logout(pool, token) → Result<()>` (revoke by SHA-256 hash)
- `logout_all(pool, user_id) → Result<()>`
- `admin_exists(pool) → bool`

JWT_SECRET from env var `JWT_SECRET` (fallback `AUTH_SECRET`). No hardcoded default (**fix**: ref code falls back to `"dev-secret-change-in-production"`). Generate random secret on first run, persist to `<data_dir>/nize/jwt-secret`.

**3.3 Auth middleware (`nize_api/src/middleware/auth.rs`)**

Axum middleware/extractor:
- Extract `Authorization: Bearer <token>` header
- Verify JWT, attach `AuthUser` to request extensions
- Return 401 on invalid/expired/missing token
- `AuthUser` extractor for handler functions

**3.4 Auth handlers (`nize_api/src/handlers/auth.rs`)**

Route handlers:
- `POST /auth/login` → `login_handler`
- `POST /auth/register` → `register_handler`
- `POST /auth/refresh` → `refresh_handler`
- `POST /auth/logout` → `logout_handler` (protected)
- `GET /auth/status` → `auth_status_handler` (public: returns `{ adminExists: bool }`)

**3.5 Wire routes into router**

Update `nize_api/src/lib.rs`:
- Add auth routes (public)
- Add auth middleware to protected routes
- Keep `/api/hello` public for health checks
- Protect future endpoints

**3.6 Add `AppError` variants**

Add `Unauthorized`, `Forbidden` to `AppError` enum.

**3.7 Config: JWT secret**

Extend `ApiConfig` with `jwt_secret: String` field.
Read from `JWT_SECRET` env var, fallback to generating + persisting.

### Phase 4: Desktop App Integration

**4.1 Auth status check on startup**

Desktop app calls `GET /auth/status` after sidecar ready.
If `adminExists: false` → show registration screen.
If `adminExists: true` → show login screen.

**4.2 Token storage in Tauri**

Store access token in Tauri managed state (memory).
Persist refresh token to file in app data dir (`<data_dir>/nize/auth-token`).
On startup, read persisted refresh token → call `/auth/refresh` → restore session.
Attach `Authorization: Bearer <token>` to all API client requests.
Auto-refresh on 401 (using refresh token).

**4.3 Registration flow (first run)**

Desktop shows "Create Admin Account" form when no admin exists:
- Email, password, name
- Calls `POST /auth/register`
- First user auto-granted admin role
- Stores tokens (memory + file), proceeds to main app

Subsequent users can register via admin invitation or open registration (configurable later).

**4.4 Login flow**

Desktop shows login form:
- Email, password
- Calls `POST /auth/login`
- Stores tokens (memory + file), proceeds to main app

**4.4.1 Auto-login on restart**

On app launch, if refresh token file exists:
- Call `POST /auth/refresh` with persisted token
- On success: skip login, proceed to main app
- On failure: delete stale token file, show login screen

**4.5 Protect Tauri commands**

All Tauri `invoke` commands that proxy to the API should include the auth token.

### Phase 5: Frontend UI (React)

**5.1 Auth pages**

- `LoginPage` component — email/password form
- `RegisterPage` component — email/password/name form (first-run only)
- Routing: if no token → login/register; if token → main app

**5.2 Auth context**

React context/store for:
- `accessToken`, `refreshToken`
- `user` (id, email, name, roles)
- `login()`, `register()`, `logout()`, `refresh()`
- Auto-refresh before token expiry

**5.3 Protected routes**

Wrap main app content in auth guard.

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| PGlite may not support all PG features (enums, ON DELETE CASCADE) | Migration fails | Test migration against PGlite early; fallback to simpler schema |
| JWT secret management in desktop (single-user local app) | Security | Generate random secret on first run, persist in app data dir |
| sqlx migrations with PGlite compatibility | Startup failure | Test `sqlx::migrate!` with PGlite before full implementation |
| bcrypt compilation on all platforms | Build failure | Pure-Rust `bcrypt` crate compiles everywhere; no C dependency |

## Decisions

1. **OAuth providers** — Deferred to follow-up plan (SHOULD priority, matches ref project).
2. **Token persistence** — Persist refresh token to file in app data dir.
3. **Multi-user** — Multi-user from the start (matches ref project). First registered user gets admin role.
4. **Password requirements** — Min 8 chars (matches ref project).
5. **Traceability codes** — Reuse ref project codes (`AUTH`, `PRM`) for REQ/DESIGN/TASK artifacts. Do not invent new codes where the ref project already defines them.
6. **Code over design (with security fixes)** — Where ref code differs from ref design docs, follow the code for implementation choices (bcrypt, 30d expiry) but fix security flaws (hash refresh tokens, use standard JWT `sub` claim, no hardcoded secret fallback).

## Completion Criteria

- [ ] DB migration creates `users`, `refresh_tokens`, `user_roles` tables on startup
- [ ] TypeSpec contract defines auth endpoints; OpenAPI + Rust code regenerated
- [ ] `POST /auth/register` creates user, returns JWT pair; first user gets admin role
- [ ] `POST /auth/login` validates credentials, returns JWT pair
- [ ] `POST /auth/refresh` rotates refresh token, returns new pair
- [ ] `POST /auth/logout` revokes refresh token
- [ ] `GET /auth/status` returns whether admin user exists
- [ ] Auth middleware rejects requests without valid JWT on protected routes
- [ ] Desktop app shows registration screen on first run (no admin exists)
- [ ] Desktop app shows login screen on subsequent runs
- [ ] Desktop app persists refresh token to file in app data dir
- [ ] Desktop app auto-restores session from persisted refresh token on restart
- [ ] Desktop app stores access token in memory and attaches to API requests

## Dependencies

- `sqlx` migrate feature
- `bcrypt` crate (matches ref code: bcryptjs cost 10)
- `sha2` crate (SHA-256 refresh token hashing — security fix over ref code)
- `jsonwebtoken` crate
- TypeSpec compiler (`tsp compile`)
- `nize_codegen` + `nize_api_client` regeneration pipeline

## Change Log

- 1.0.0 (2026-02-13): Initial plan
