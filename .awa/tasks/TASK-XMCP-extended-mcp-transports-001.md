# Implementation Tasks

FEATURE: Extended MCP Transport Modes (SSE + Managed HTTP)
SOURCE: PLAN-033-extended-mcp-transports.md

## Phase 1: Setup

- [x] T-XMCP-001 Add `sse-stream` as direct workspace dependency → Cargo.toml
- [x] T-XMCP-002 [P] Add `sse-stream` dependency to nize_core → crates/lib/nize_core/Cargo.toml

## Phase 2: Foundation — Transport Types and Config Models

GOAL: Extend TransportType enum, add new config types, DB migration
TEST CRITERIA: `cargo build` compiles with new enum variants; DB migration applies cleanly

- [x] T-XMCP-010 Create DB migration extending `transport_type` enum with `sse`, `managed-sse`, `managed-http` → crates/lib/nize_core/migrations/0012_extend_transport_type.sql
- [x] T-XMCP-011 Add `Sse`, `ManagedSse`, `ManagedHttp` variants to Rust `TransportType` enum → crates/lib/nize_core/src/models/mcp.rs
- [x] T-XMCP-012 Add `is_managed()` and `protocol()` helper methods to `TransportType` → crates/lib/nize_core/src/models/mcp.rs
- [x] T-XMCP-013 Add `TransportProtocol` enum (Stdio, StreamableHttp, Sse) → crates/lib/nize_core/src/models/mcp.rs
- [x] T-XMCP-014 Add `SseServerConfig` struct for external SSE servers → crates/lib/nize_core/src/models/mcp.rs
- [x] T-XMCP-015 Add `ManagedHttpServerConfig` struct for managed SSE/HTTP servers → crates/lib/nize_core/src/models/mcp.rs
- [x] T-XMCP-016 Add `Sse`, `ManagedSse`, `ManagedHttp` variants to `ServerConfig` enum → crates/lib/nize_core/src/models/mcp.rs
- [x] T-XMCP-017 Update `ServerConfig::transport_type()` to return correct type for new variants → crates/lib/nize_core/src/models/mcp.rs
- [x] T-XMCP-018 Update TypeSpec `ServerTransport` enum with `sse`, `managed-sse`, `managed-http` → .awa/specs/API-NIZE-mcp-config.tsp
- [x] T-XMCP-019 Regenerate OpenAPI + codegen from updated TypeSpec → codegen/nize-api/tsp-output/

## Phase 3: SSE Client Transport [MUST]

GOAL: Implement a Rust SSE client transport compatible with rmcp Transport<RoleClient> trait
TEST CRITERIA: SseClientTransport can connect to a legacy SSE endpoint, send/receive JSON-RPC messages

- [x] T-XMCP-030 Create `sse_transport.rs` module with `SseClientTransport` struct → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-031 Implement background worker task: GET SSE stream, discover endpoint URL from `endpoint` event, route incoming events to receive channel → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-032 Implement `send()`: POST JSON-RPC messages to discovered endpoint URL → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-033 Implement `receive()`: read next SSE event from channel, parse as JSON-RPC → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-034 Implement `close()`: cancel background tasks, drop SSE stream → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-035 Implement rmcp `Transport<RoleClient>` trait for `SseClientTransport` → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-036 Register `sse_transport` module in `mod.rs` → crates/lib/nize_core/src/mcp/mod.rs
- [x] T-XMCP-037 [P] Unit test: SSE event parsing (message event → JSON-RPC message) → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-038 [P] Unit test: endpoint discovery from SSE `endpoint` event → crates/lib/nize_core/src/mcp/sse_transport.rs
- [x] T-XMCP-039 [P] Unit test: error handling (connection refused, timeout, invalid endpoint) → crates/lib/nize_core/src/mcp/sse_transport.rs

## Phase 4: Integrate SSE into ClientPool [MUST]

GOAL: Wire SSE and managed-HTTP connections into ClientPool alongside existing transports
TEST CRITERIA: `get_or_connect()` routes all 5 transport types; managed connections spawn + connect

- [x] T-XMCP-040 Add `connect_sse()` method to ClientPool for external SSE servers → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-041 Add `spawn_managed_process()` helper: spawn child process with piped stdin, return Child handle → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-042 Add `wait_for_ready()` helper: retry HTTP GET to localhost:{port}{path} until success or timeout → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-043 Add `connect_managed()` method: spawn process, wait for ready, connect via SSE or StreamableHttp → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-044 Update `get_or_connect()` match to dispatch `Sse`, `ManagedSse`, `ManagedHttp` transport types → crates/lib/nize_core/src/mcp/execution.rs

## Phase 5: Eviction for All Managed Transports [MUST]

GOAL: Extend LRU eviction and idle timeout from stdio-only to all managed transports
TEST CRITERIA: Managed SSE/HTTP connections evicted on idle and LRU like stdio

- [x] T-XMCP-050 Rename `max_stdio_processes` → `max_managed_processes` in ClientPool → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-051 Rename `set_max_stdio_processes()` → `set_max_managed_processes()` → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-052 Replace `transport == TransportType::Stdio` checks with `transport.is_managed()` in eviction logic → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-053 Rename `DEFAULT_MAX_STDIO_PROCESSES` → `DEFAULT_MAX_MANAGED_PROCESSES` → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-054 Update config key from `mcp.max_stdio_processes` to `mcp.max_managed_processes` with DB migration → crates/lib/nize_core/migrations/0013_rename_max_managed.sql
- [x] T-XMCP-055 Update all call sites referencing renamed fields/methods → crates/

## Phase 6: Managed Process Lifecycle [MUST]

GOAL: Track child processes for managed HTTP/SSE, kill on eviction, register with terminator
TEST CRITERIA: Managed processes killed on pool entry removal; PIDs in terminator manifest

- [x] T-XMCP-060 Add `child_process: Option<tokio::process::Child>` field to `PoolEntry` → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-061 Populate `child_process` in `connect_managed()` and `connect_stdio()` → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-062 Kill child process when pool entry is removed/evicted → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-063 Write `kill <pid>` to terminator manifest after spawning managed processes → crates/lib/nize_core/src/mcp/execution.rs

## Phase 7: Connection Testing [MUST]

GOAL: Add test-connection support for SSE and managed transport types
TEST CRITERIA: test_connection works for all 5 transport types

- [x] T-XMCP-070 Add `test_sse_connection()` function using SseClientTransport → crates/lib/nize_core/src/mcp/execution.rs
- [x] T-XMCP-071 Update test_connection routing: SSE/ManagedSse → `test_sse_connection()`, ManagedHttp → `test_http_connection()` → crates/lib/nize_api/src/services/mcp_config.rs
- [x] T-XMCP-072 For managed types, test_connection spawns temporary process (same pattern as stdio test_connection) → crates/lib/nize_api/src/services/mcp_config.rs

## Phase 8: Config Validation [MUST]

GOAL: Validate new transport configs in McpConfigService
TEST CRITERIA: Invalid SSE/managed configs rejected with clear errors

- [x] T-XMCP-080 Add `validate_sse_config()`: validate URL, enforce HTTPS except localhost → crates/lib/nize_api/src/services/mcp_config.rs
- [x] T-XMCP-081 Add `validate_managed_config()`: validate command, port, admin-only creation → crates/lib/nize_api/src/services/mcp_config.rs
- [x] T-XMCP-082 Extend user server creation rules: users can create `Http` and `Sse` only → crates/lib/nize_api/src/services/mcp_config.rs
- [x] T-XMCP-083 Extend admin server creation to accept all 5 transport types → crates/lib/nize_api/src/services/mcp_config.rs

## Phase 9: UI Updates [MUST]

GOAL: nize-web UI supports creating/viewing servers with all transport types
TEST CRITERIA: Transport selector shows correct options per role; form fields adapt dynamically

- [x] T-XMCP-090 Update `TransportType` in UI types to include `sse`, `managed-sse`, `managed-http` → packages/nize-web/components/mcp-server/types.ts
- [x] T-XMCP-091 Create `SseConfigFields` component for SSE URL + auth config → packages/nize-web/components/mcp-server/SseConfigFields.tsx
- [x] T-XMCP-092 Create `ManagedHttpConfigFields` component for command + port + path + timeout → packages/nize-web/components/mcp-server/ManagedHttpConfigFields.tsx
- [x] T-XMCP-093 Update `ServerForm` to render correct fields for new transport types → packages/nize-web/components/mcp-server/ServerForm.tsx
- [x] T-XMCP-094 Update `useServerForm` hook to build configs for new transport types → packages/nize-web/components/mcp-server/useServerForm.ts
- [x] T-XMCP-095 Admin form: show all 5 transport types in selector → packages/nize-web/app/settings/admin/tools/page.tsx
- [x] T-XMCP-096 User form: show only `http` and `sse` in transport selector → packages/nize-web/app/settings/tools/page.tsx
- [x] T-XMCP-097 Update transport badge labels in server list (admin + user views) → packages/nize-web/app/settings/admin/tools/page.tsx

## Phase 10: Polish

- [ ] T-XMCP-100 Integration test: external SSE server connect → list tools → execute tool → crates/lib/nize_core/src/mcp/
- [ ] T-XMCP-101 Integration test: managed-SSE spawn → connect → list tools → evict → crates/lib/nize_core/src/mcp/
- [ ] T-XMCP-102 Integration test: managed-HTTP spawn → connect → list tools → evict → crates/lib/nize_core/src/mcp/
- [x] T-XMCP-103 Verify backward compatibility: existing `stdio` and `http` servers unaffected → crates/lib/nize_core/src/mcp/
- [x] T-XMCP-104 Update config_definitions for renamed `max_managed_processes` key → crates/lib/nize_core/src/config/

---

## Dependencies

Phase 2 → Phase 1 (dependencies must be added before models use sse-stream types)
Phase 3 → Phase 2 (SSE transport needs TransportType and config models)
Phase 4 → Phase 3, Phase 2 (pool integration needs SSE transport + new enum variants)
Phase 5 → Phase 4 (eviction changes need pool integration)
Phase 6 → Phase 4 (lifecycle tracking needs pool integration)
Phase 7 → Phase 3, Phase 4 (test_connection needs SSE transport and pool)
Phase 8 → Phase 2 (validation needs new config types)
Phase 9 → Phase 2, Phase 8 (UI needs updated types and contract)
Phase 10 → all previous phases

## Parallel Opportunities

Phase 1: T-XMCP-001, T-XMCP-002 can run in parallel
Phase 2: T-XMCP-010, T-XMCP-018 can run parallel with T-XMCP-011..T-XMCP-017
Phase 3: T-XMCP-037, T-XMCP-038, T-XMCP-039 can run parallel after T-XMCP-035
Phase 5: T-XMCP-050..T-XMCP-054 can run parallel (independent renames)
Phase 8: T-XMCP-080, T-XMCP-081 can run parallel
Phase 9: T-XMCP-091, T-XMCP-092 can run parallel

## Trace Summary

This task list is derived from PLAN-033 (not a formal REQ/DESIGN). Traceability is to PLAN-033 completion criteria.

| Completion Criterion | Task(s) | Test(s) |
|----------------------|---------|---------|
| SseClientTransport implements rmcp Transport<RoleClient> | T-XMCP-030..T-XMCP-036 | T-XMCP-037, T-XMCP-038, T-XMCP-039 |
| SSE transport tested against real SSE server | T-XMCP-037..T-XMCP-039 | T-XMCP-100 |
| DB transport_type extended with sse, managed-sse, managed-http | T-XMCP-010 | T-XMCP-103 |
| Rust TransportType has 5 variants with is_managed() | T-XMCP-011, T-XMCP-012 | T-XMCP-103 |
| ServerConfig has 5 variants with config types | T-XMCP-014..T-XMCP-017 | T-XMCP-103 |
| ClientPool.get_or_connect() routes all 5 types | T-XMCP-044 | T-XMCP-100..T-XMCP-103 |
| connect_sse() works for external SSE | T-XMCP-040 | T-XMCP-100 |
| connect_managed() spawns + connects via SSE or HTTP | T-XMCP-041..T-XMCP-043 | T-XMCP-101, T-XMCP-102 |
| Managed PID tracked in terminator manifest | T-XMCP-063 | T-XMCP-101, T-XMCP-102 |
| LRU eviction for all managed transports | T-XMCP-050..T-XMCP-055 | T-XMCP-101, T-XMCP-102 |
| test_connection for all 5 types | T-XMCP-070..T-XMCP-072 | T-XMCP-100..T-XMCP-103 |
| TypeSpec API contract updated | T-XMCP-018, T-XMCP-019 | T-XMCP-103 |
| McpConfigService validates new configs | T-XMCP-080..T-XMCP-083 | T-XMCP-103 |
| Admin-only for managed transports | T-XMCP-081, T-XMCP-082 | T-XMCP-103 |
| nize-web UI for new transport types | T-XMCP-090..T-XMCP-097 | (manual verification) |

UNCOVERED: (none — all 15 completion criteria from PLAN-033 are covered)
