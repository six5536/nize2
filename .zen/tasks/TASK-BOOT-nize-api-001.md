# Implementation Tasks

FEATURE: nize_api Bootstrap
SOURCE: PLAN-003-nize-api-bootstrap.md

NOTE: Plan-driven bootstrap. No formal REQ/DESIGN — traceability is to plan steps.

## Phase 1: Setup

- [x] T-BOOT-001 Add `pub mod hello` and `pub mod node_sidecar` to nize_core lib → crates/lib/nize_core/src/lib.rs
- [x] T-BOOT-002 Add devDependencies (TypeSpec compiler, OpenAPI Generator, js-yaml) → package.json
- [x] T-BOOT-003 [P] Create tspconfig.yaml with openapi3 emitter → tspconfig.yaml
- [x] T-BOOT-004 [P] Add `codegen/` to gitignore → .gitignore
- [x] T-BOOT-005 Add `nize-api` (lib) and `nize-api-server` (bin) to workspace members → Cargo.toml
- [x] T-BOOT-006 [P] Add `axum`, `tracing`, `tracing-subscriber`, `utoipa`, `dotenvy`, `reqwest` to workspace deps → Cargo.toml

## Phase 2: Foundation — nize_core Functions

TEST CRITERIA: `cargo test -p nize-core` passes with hello + node_sidecar tests

- [x] T-BOOT-010 Create `hello_world()` function returning greeting with version → crates/lib/nize_core/src/hello.rs
- [x] T-BOOT-011 Unit test for `hello_world()` → crates/lib/nize_core/src/hello.rs
- [x] T-BOOT-012 Create `NodeInfo` struct and `SidecarError` type → crates/lib/nize_core/src/node_sidecar.rs
- [x] T-BOOT-013 Implement `check_node_available()` — spawn `node --version` → crates/lib/nize_core/src/node_sidecar.rs
- [x] T-BOOT-014 Unit test for `check_node_available()` (requires Node on PATH) → crates/lib/nize_core/src/node_sidecar.rs
- [x] T-BOOT-015 Verify: `cargo test -p nize-core` passes

## Phase 3: TypeSpec API Specification

TEST CRITERIA: `npx tsp compile .zen/specs/API-NIZE-index.tsp` produces openapi.yaml

- [x] T-BOOT-020 Create common TypeSpec file — service def, ErrorResponse model → .zen/specs/API-NIZE-common.tsp
- [x] T-BOOT-021 Create index TypeSpec file — HelloWorldResponse model, GET /api/hello → .zen/specs/API-NIZE-index.tsp
- [x] T-BOOT-022 Verify: TypeSpec compiles to codegen/nize-api/tsp-output/openapi.yaml

## Phase 4: Codegen Pipeline (nize_codegen)

TEST CRITERIA: `npm run generate:api` produces files in crates/lib/nize_api/src/generated/

- [x] T-BOOT-030 Create nize_codegen Cargo.toml (serde, serde_json, serde_yaml) → crates/app/nize_codegen/Cargo.toml
- [x] T-BOOT-031 Create OpenAPI 3.0 serde types (schema.rs) → crates/app/nize_codegen/src/schema.rs
- [x] T-BOOT-032 Create model generator — emit serde structs from OpenAPI schemas → crates/app/nize_codegen/src/gen_models.rs
- [x] T-BOOT-033 Create route generator — emit pub const path constants → crates/app/nize_codegen/src/gen_routes.rs
- [x] T-BOOT-034 Create writer — generated file header + utilities → crates/app/nize_codegen/src/writer.rs
- [x] T-BOOT-035 Create lib.rs — generate() with FNV-1a staleness detection → crates/app/nize_codegen/src/lib.rs
- [x] T-BOOT-036 Create CLI main.rs — workspace root resolution, spec/output paths → crates/app/nize_codegen/src/main.rs
- [x] T-BOOT-037 [P] Update generate-api.sh to 3-step pipeline (tsp → json → nize-codegen) → scripts/generate-api.sh
- [x] T-BOOT-038 Verify: `npm run generate:api` produces models.rs, routes.rs, mod.rs

## Phase 5: nize_api Library Crate

TEST CRITERIA: `cargo check -p nize-api` compiles

- [x] T-BOOT-040 Create nize_api Cargo.toml with deps (nize-core, axum 0.7, tokio, serde, thiserror, tracing, sqlx, utoipa) → crates/lib/nize_api/Cargo.toml
- [x] T-BOOT-041 Create AppError enum and AppResult type — follow bitmark-configurator-api pattern → crates/lib/nize_api/src/error.rs
- [x] T-BOOT-042 Create ApiConfig struct with from_env() → crates/lib/nize_api/src/config.rs
- [x] T-BOOT-043 Create AppState struct (PgPool, ApiConfig) → crates/lib/nize_api/src/lib.rs
- [x] T-BOOT-044 Create handlers/mod.rs — re-export hello module → crates/lib/nize_api/src/handlers/mod.rs
- [x] T-BOOT-045 Implement hello_world handler — call nize_core::hello, DB ping, node check → crates/lib/nize_api/src/handlers/hello.rs
- [x] T-BOOT-046 Create lib.rs — pub modules, router() function mounting /api/hello → crates/lib/nize_api/src/lib.rs
- [x] T-BOOT-047 Generated code populated by nize-codegen pipeline → crates/lib/nize_api/src/generated/
- [x] T-BOOT-048 Verify: `cargo check -p nize-api` compiles

## Phase 6: nize-api Sidecar Binary

TEST CRITERIA: `cargo run -p nize-api-server -- --port 3100` starts, responds to GET /api/hello

- [x] T-BOOT-050 Create nize-api-server Cargo.toml (nize-api lib, nize-core, tokio, tracing, clap, dotenvy) → crates/app/nize-api/Cargo.toml
- [x] T-BOOT-051 Implement main.rs — CLI args, tracing init, PG pool, router, bind, port reporting to stdout → crates/app/nize-api/src/main.rs
- [x] T-BOOT-052 Verify: server starts and responds to GET /api/hello with correct shape

## Phase 7: Tauri Desktop Integration

TEST CRITERIA: `cargo tauri dev` → click button → response shows greeting, DB status, Node status

- [x] T-BOOT-060 Update tauri.conf.json — sidecar managed via process spawn, no externalBin needed → crates/app/nize-desktop/tauri.conf.json
- [x] T-BOOT-061 Add reqwest dependency to nize-desktop → crates/app/nize-desktop/Cargo.toml
- [x] T-BOOT-062 Implement API sidecar startup in nize-desktop — spawn child process, read port from stdout → crates/app/nize-desktop/src/lib.rs
- [x] T-BOOT-063 Implement `hello_world` Tauri command — HTTP GET to sidecar → crates/app/nize-desktop/src/lib.rs
- [x] T-BOOT-064 Add "Hello World" button to React UI — invoke command, display results → packages/nize-desktop/src/App.tsx
- [x] T-BOOT-065 Verify: `cargo tauri dev` → button click → response displayed

## Phase 8: Polish

- [x] T-BOOT-070 Integration test — start ephemeral PG, start server, call /api/hello, assert shape → crates/lib/nize_api/tests/hello_integration.rs
- [x] T-BOOT-071 [P] Verify `cargo test -p nize-core` passes (all tests including hello + node_sidecar) → crates/lib/nize_core/
- [x] T-BOOT-072 [P] Verify `cargo test -p nize-api` passes (integration test) → crates/lib/nize_api/

---

## Dependencies

Phase 2 → (none) — nize_core functions are standalone
Phase 3 → Phase 1 (TypeSpec tooling must be installed)
Phase 4 → Phase 3 (TypeSpec specs must exist to generate from)
Phase 5 → Phase 2, Phase 4 (needs nize_core functions + generated code)
Phase 6 → Phase 5 (needs nize_api library)
Phase 7 → Phase 6 (needs running sidecar binary)
Phase 8 → Phase 5, Phase 6 (needs lib + binary for integration tests)

## Parallel Opportunities

Phase 1: T-BOOT-003, T-BOOT-004, T-BOOT-006 can run parallel after T-BOOT-002
Phase 2: T-BOOT-010/011 and T-BOOT-012/013/014 can run in parallel (independent modules)
Phase 3: T-BOOT-020, T-BOOT-021 can run in parallel
Phase 4: T-BOOT-031, T-BOOT-032, T-BOOT-033, T-BOOT-034 can run parallel after T-BOOT-030
Phase 5: T-BOOT-041, T-BOOT-042 can run parallel after T-BOOT-040
Phase 8: T-BOOT-071, T-BOOT-072 can run parallel after T-BOOT-070

## Trace Summary

NOTE: No formal REQ/DESIGN documents exist. Tasks trace to PLAN-003 phases.

| Plan Phase | Tasks | Verification |
|------------|-------|--------------|
| Phase 1 (nize_core) | T-BOOT-010..015 | T-BOOT-015 |
| Phase 2 (TypeSpec tooling) | T-BOOT-002..004 | T-BOOT-022 |
| Phase 3 (TypeSpec spec) | T-BOOT-020..022 | T-BOOT-022 |
| Phase 4 (Codegen pipeline) | T-BOOT-030..038 | T-BOOT-038 |
| Phase 5 (nize_api lib) | T-BOOT-040..048 | T-BOOT-048 |
| Phase 6 (Sidecar binary) | T-BOOT-050..052 | T-BOOT-052 |
| Phase 7 (Tauri integration) | T-BOOT-060..065 | T-BOOT-065 |
| Phase 8 (Testing) | T-BOOT-070..072 | T-BOOT-072 |

UNCOVERED: (none — plan-driven, all plan steps covered)
