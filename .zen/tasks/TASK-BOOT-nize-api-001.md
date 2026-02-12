# Implementation Tasks

FEATURE: nize_api Bootstrap
SOURCE: PLAN-003-nize-api-bootstrap.md

NOTE: Plan-driven bootstrap. No formal REQ/DESIGN — traceability is to plan steps.

## Phase 1: Setup

- [ ] T-BOOT-001 Add `pub mod hello` and `pub mod node_sidecar` to nize_core lib → crates/lib/nize_core/src/lib.rs
- [ ] T-BOOT-002 Add devDependencies (TypeSpec compiler, OpenAPI Generator, js-yaml) → package.json
- [ ] T-BOOT-003 [P] Create tspconfig.yaml with openapi3 emitter → tspconfig.yaml
- [ ] T-BOOT-004 [P] Add `codegen/` to gitignore → .gitignore
- [ ] T-BOOT-005 Add `nize-api` (lib) and `nize-api-server` (bin) to workspace members → Cargo.toml
- [ ] T-BOOT-006 [P] Add `axum`, `tracing`, `tracing-subscriber`, `utoipa`, `dotenvy`, `reqwest` to workspace deps → Cargo.toml

## Phase 2: Foundation — nize_core Functions

TEST CRITERIA: `cargo test -p nize-core` passes with hello + node_sidecar tests

- [ ] T-BOOT-010 Create `hello_world()` function returning greeting with version → crates/lib/nize_core/src/hello.rs
- [ ] T-BOOT-011 Unit test for `hello_world()` → crates/lib/nize_core/src/hello.rs
- [ ] T-BOOT-012 Create `NodeInfo` struct and `SidecarError` type → crates/lib/nize_core/src/node_sidecar.rs
- [ ] T-BOOT-013 Implement `check_node_available()` — spawn `node --version` → crates/lib/nize_core/src/node_sidecar.rs
- [ ] T-BOOT-014 Unit test for `check_node_available()` (requires Node on PATH) → crates/lib/nize_core/src/node_sidecar.rs
- [ ] T-BOOT-015 Verify: `cargo test -p nize-core` passes

## Phase 3: TypeSpec API Specification

TEST CRITERIA: `npx tsp compile .zen/specs/API-NIZE-index.tsp` produces openapi.yaml

- [ ] T-BOOT-020 Create common TypeSpec file — service def, ErrorResponse model → .zen/specs/API-NIZE-common.tsp
- [ ] T-BOOT-021 Create index TypeSpec file — HelloWorldResponse model, GET /api/hello → .zen/specs/API-NIZE-index.tsp
- [ ] T-BOOT-022 Verify: TypeSpec compiles to codegen/nize-api/tsp-output/openapi.yaml

## Phase 4: Codegen Pipeline Scripts

TEST CRITERIA: `npm run generate:api` produces files in crates/lib/nize_api/src/generated/

- [ ] T-BOOT-030 Create path constants module → scripts/lib-code-gen/paths.js
- [ ] T-BOOT-031 [P] Create YAML→JSON converter script → scripts/generate-openapi-json.js
- [ ] T-BOOT-032 [P] Create import path fixer — `use crate::` → `use nize_api::generated::` → scripts/lib-code-gen/fixes.js
- [ ] T-BOOT-033 [P] Create file copy utility with content transform → scripts/lib-code-gen/copy.js
- [ ] T-BOOT-034 [P] Create Axum route path fixer → scripts/lib-code-gen/axum-path.js
- [ ] T-BOOT-035 Create install orchestrator — clean, copy, fix, generate mod.rs → scripts/install-generated-code.js
- [ ] T-BOOT-036 Create pipeline shell script — tsp compile → json → openapi-generator → install → scripts/generate-api.sh
- [ ] T-BOOT-037 Add `"generate:api"` npm script → package.json
- [ ] T-BOOT-038 Verify: `npm run generate:api` produces generated Rust in target dir

## Phase 5: nize_api Library Crate

TEST CRITERIA: `cargo check -p nize-api` compiles

- [ ] T-BOOT-040 Create nize_api Cargo.toml with deps (nize-core, axum 0.7, tokio, serde, thiserror, tracing, sqlx, utoipa) → crates/lib/nize_api/Cargo.toml
- [ ] T-BOOT-041 Create AppError enum and AppResult type — follow bitmark-configurator-api pattern → crates/lib/nize_api/src/error.rs
- [ ] T-BOOT-042 Create ApiConfig struct with from_env() → crates/lib/nize_api/src/config.rs
- [ ] T-BOOT-043 Create AppState struct (PgPool, ApiConfig) → crates/lib/nize_api/src/lib.rs
- [ ] T-BOOT-044 Create handlers/mod.rs — re-export hello module → crates/lib/nize_api/src/handlers/mod.rs
- [ ] T-BOOT-045 Implement hello_world handler — call nize_core::hello, DB ping, node check → crates/lib/nize_api/src/handlers/hello.rs
- [ ] T-BOOT-046 Create lib.rs — pub modules, router() function mounting /api/hello → crates/lib/nize_api/src/lib.rs
- [ ] T-BOOT-047 Create generated/mod.rs placeholder (populated by codegen pipeline) → crates/lib/nize_api/src/generated/mod.rs
- [ ] T-BOOT-048 Verify: `cargo check -p nize-api` compiles

## Phase 6: nize-api Sidecar Binary

TEST CRITERIA: `cargo run -p nize-api-server -- --port 3100` starts, responds to GET /api/hello

- [ ] T-BOOT-050 Create nize-api-server Cargo.toml (nize-api lib, nize-core, tokio, tracing, clap, dotenvy) → crates/app/nize-api/Cargo.toml
- [ ] T-BOOT-051 Implement main.rs — CLI args, tracing init, PG pool, router, bind, port reporting to stdout → crates/app/nize-api/src/main.rs
- [ ] T-BOOT-052 Verify: server starts and responds to GET /api/hello with correct shape

## Phase 7: Tauri Desktop Integration

TEST CRITERIA: `cargo tauri dev` → click button → response shows greeting, DB status, Node status

- [ ] T-BOOT-060 Update tauri.conf.json — add nize-api to externalBin or process config → crates/app/nize-desktop/tauri.conf.json
- [ ] T-BOOT-061 Add reqwest dependency to nize-desktop → crates/app/nize-desktop/Cargo.toml
- [ ] T-BOOT-062 Implement API sidecar startup in nize-desktop — spawn child process, read port from stdout → crates/app/nize-desktop/src/lib.rs
- [ ] T-BOOT-063 Implement `hello_world` Tauri command — HTTP GET to sidecar → crates/app/nize-desktop/src/lib.rs
- [ ] T-BOOT-064 Add "Hello World" button to React UI — invoke command, display results → packages/nize-desktop/src/App.tsx
- [ ] T-BOOT-065 Verify: `cargo tauri dev` → button click → response displayed

## Phase 8: Polish

- [ ] T-BOOT-070 Integration test — start ephemeral PG, start server, call /api/hello, assert shape → crates/lib/nize_api/tests/hello_integration.rs
- [ ] T-BOOT-071 [P] Verify `cargo test -p nize-core` passes (all tests including hello + node_sidecar) → crates/lib/nize_core/
- [ ] T-BOOT-072 [P] Verify `cargo test -p nize-api` passes (integration test) → crates/lib/nize_api/

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
