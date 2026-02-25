# PLAN-010: Cloud Server Split (nize_api_server + nize_mcp_server)

| Field              | Value                                                    |
|--------------------|----------------------------------------------------------|
| **Status**         | in-progress                                              |
| **Workflow**       | lateral                                                  |
| **Reference**      | PLAN-009 (mcp-server), PLAN-008 (user-auth)              |
| **Traceability**   | —                                                        |

## Goal

Split `nize_desktop_server` into two standalone cloud-ready binaries:

1. **`nize_api_server`** — REST API only (`nize_api` library)
2. **`nize_mcp_server`** — MCP Streamable HTTP only (`nize_mcp` library)

These are the cloud equivalents of `nize_desktop_server` (which remains the desktop sidecar running both services in one process with PGlite). The cloud binaries connect to an external PostgreSQL database and can scale independently.

## Current State

| Binary | What it does | DB | Mode |
|--------|--------------|----|------|
| `nize_desktop_server` | REST API (port A) + MCP (port B) in one process | PGlite via sidecar (`max_connections=1`) | Desktop sidecar (`--sidecar` stdin EOF) |

## Target State

| Binary | What it does | DB | Mode |
|--------|--------------|----|------|
| `nize_desktop_server` | REST API + MCP in one process | PGlite (single-connection) | Desktop sidecar (unchanged) |
| `nize_api_server` | REST API only | External PostgreSQL | Cloud service |
| `nize_mcp_server` | MCP Streamable HTTP only | External PostgreSQL | Cloud service |

## Decisions

### Single-concern binaries

Each cloud binary runs exactly one concern. This enables:
- Independent scaling (MCP may need more instances than API)
- Separate deployment and rollback
- Different resource profiles
- Simpler health checks and monitoring

### No sidecar mode

Cloud binaries do not need `--sidecar` stdin EOF monitoring. They run as long-lived services managed by orchestration (Docker, K8s, systemd, etc.).

### External DB with higher connection pool

Desktop uses `max_connections=1` (PGlite constraint). Cloud uses `max_connections=20` (default) with a real PostgreSQL instance. Both binaries share the same `DATABASE_URL` pointing to the same external DB.

### Bind to 0.0.0.0

Cloud binaries default to `0.0.0.0` (all interfaces) instead of `127.0.0.1` (loopback only). Port defaults: API=3100, MCP=3200.

### Shared library code

Both binaries depend on `nize_core` (migrations, auth, domain models). `nize_api_server` depends on `nize_api`. `nize_mcp_server` depends on `nize_mcp`. No code duplication — the split is purely at the binary/wiring level.

```
nize_api_server (binary)            nize_mcp_server (binary)
├── nize_api (lib)                  ├── nize_mcp (lib)
│   └── nize_core (lib)             │   └── nize_core (lib)
└── pool → external PG              └── pool → external PG
```

### CORS with configurable origins

Desktop currently uses `allow_origin(Any)` — acceptable for localhost sidecar. Cloud requires explicit allowed origins.

Both binaries accept `--cors-origin <URL>` (repeatable). When provided, only those origins are allowed. When omitted, defaults to `Any` (dev convenience). The `nize_api` library's `router()` will need a CORS config parameter (or `AppState` extension) — but for Phase 1, the cloud binary can wrap the router with its own CORS layer, overriding the library's `Any`. Refactoring CORS into `ApiConfig` is a follow-up.

The MCP server (`nize_mcp_server`) also gets `--cors-origin` since browser-based MCP clients may need CORS.

### Health check endpoint

Each binary exposes `GET /healthz` returning `200 OK` for liveness probes.

## Plan

### Phase 1 — Create `nize_api_server` binary

- [ ] **1.1** Create `crates/app/nize_api_server/Cargo.toml`:
  - Dependencies: `nize_api`, `nize_core`, `tokio`, `tracing`, `tracing-subscriber`, `clap`, `dotenvy`, `serde_json`, `sqlx`, `axum`, `tower-http`
  - No `nize_mcp`, no `tokio-util` (no MCP, no CancellationToken needed)
- [ ] **1.2** Create `crates/app/nize_api_server/src/main.rs`:
  - CLI args: `--port` (default 3100), `--database-url` (env `DATABASE_URL`), `--max-connections` (default 20), `--cors-origin` (repeatable, default `Any`)
  - No `--sidecar`, no `--mcp-port`
  - Bind to `0.0.0.0:{port}` by default
  - Build CORS layer: if `--cors-origin` provided, use explicit `AllowOrigin::list()`; else `Any`
  - Startup: connect pool → run migrations → build `nize_api::router(state)` → wrap with CORS layer → serve
  - Add `GET /healthz` route (200 OK, no auth)
  - Print `{"port": N}` to stdout on startup (consistent protocol)
- [ ] **1.3** Add `"crates/app/nize_api_server"` to workspace `members` in root `Cargo.toml`
  - Do NOT add to `default-members` (cloud binaries built on demand)
- [ ] **1.4** Verify: `cargo build -p nize_api_server` compiles

### Phase 2 — Create `nize_mcp_server` binary

- [ ] **2.1** Create `crates/app/nize_mcp_server/Cargo.toml`:
  - Dependencies: `nize_mcp`, `nize_core`, `tokio`, `tokio-util`, `tracing`, `tracing-subscriber`, `clap`, `dotenvy`, `serde_json`, `sqlx`, `axum`, `tower-http`
  - No `nize_api` dependency
- [ ] **2.2** Create `crates/app/nize_mcp_server/src/main.rs`:
  - CLI args: `--port` (default 3200), `--database-url` (env `DATABASE_URL`), `--max-connections` (default 20), `--cors-origin` (repeatable, default `Any`)
  - No `--sidecar`, no separate API port
  - Bind to `0.0.0.0:{port}` by default
  - Build CORS layer: if `--cors-origin` provided, use explicit `AllowOrigin::list()`; else `Any`
  - Startup: connect pool → run migrations → build `nize_mcp::mcp_router(pool, ct)` → wrap with CORS layer → serve
  - Add `GET /healthz` route (200 OK, no auth, merged with MCP router)
  - Print `{"port": N}` to stdout on startup
- [ ] **2.3** Add `"crates/app/nize_mcp_server"` to workspace `members` in root `Cargo.toml`
  - Do NOT add to `default-members`
- [ ] **2.4** Verify: `cargo build -p nize_mcp_server` compiles

### Phase 3 — Verify & Document

- [ ] **3.1** Verify: `cargo build --workspace` succeeds (all crates including new ones)
- [ ] **3.2** Verify: `cargo run -p nize_api_server -- --help` shows expected args
- [ ] **3.3** Verify: `cargo run -p nize_mcp_server -- --help` shows expected args
- [ ] **3.4** Verify: `nize_desktop_server` unchanged and still compiles

## Resolved Questions

1. **Auth for cloud MCP**: MCP bearer tokens only. No JWT validation in `nize_mcp_server`. JWT support deferred.
2. **CORS**: Configurable via `--cors-origin` (repeatable). Default `Any` for dev; explicit origins for production.
3. **Docker / env templates**: Deferred to a later plan.
4. **Shared migrations**: Safe. `sqlx::migrate!().run()` acquires a PostgreSQL advisory lock before running pending migrations, so concurrent calls from both binaries serialize automatically. No additional locking needed.

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Dual migration runs on simultaneous startup | Low — SQLx migrations are idempotent | Confirm with integration test; add advisory lock if needed |
| Cloud binaries not tested in CI initially | Medium — regressions possible | Add `cargo build -p nize_api_server -p nize_mcp_server` to CI |
| MCP token creation requires API server | Functional dependency | Document: API server must be running for MCP token management |

## Completion Criteria

- [ ] `cargo build -p nize_api_server` succeeds
- [ ] `cargo build -p nize_mcp_server` succeeds
- [ ] `cargo build --workspace` succeeds
- [ ] `nize_api_server` starts, connects to PG, runs migrations, serves REST API on configured port
- [ ] `nize_mcp_server` starts, connects to PG, runs migrations, serves MCP on configured port
- [ ] `nize_desktop_server` unchanged and unaffected
- [ ] Both binaries report port via JSON stdout
- [ ] Both binaries expose `/healthz`
