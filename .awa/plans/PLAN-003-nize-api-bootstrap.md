# PLAN-003: nize_api Bootstrap

| Field              | Value                          |
|--------------------|--------------------------------|
| **Status**         | complete |
| **Workflow**       | bottom-up                      |
| **Reference**      | PLAN-002 (tauri bootstrap), `submodules/bitmark-configurator-api` (Axum patterns), codegen design doc (TypeSpec pipeline) |
| **Traceability**   | —                              |

## Goal

Create a new `nize_api` Rust library crate with a TypeSpec-driven codegen pipeline.
Bootstrap with a single `hello_world` endpoint that:

1. Calls `nize_core::hello_world()` (new exported function)
2. Verifies PostgreSQL connection (via `DbManager`)
3. Verifies Node.js sidecar execution (trivial script spawn)

The API server runs as a **sidecar process** (separate binary), started by the Tauri app.
The desktop frontend gets a button to call the endpoint and display results.

## Pre-Conditions

- PLAN-002 Phases 1–3 completed (Tauri app, PG sidecar management)
- Node.js available on PATH (mise provides Node 24)

## Reference Implementation Mapping

| Ref                              | nize-mcp (this project)                | Notes                          |
|----------------------------------|----------------------------------------|--------------------------------|
| `.awa/specs/API-Index.tsp`       | `.awa/specs/API-NIZE-index.tsp`        | TypeSpec entry point           |
| `tsp-output/openapi.yaml`       | `codegen/nize-api/tsp-output/openapi.yaml` | TypeSpec compiler output |
| `scripts/generate-openapi-json.js` | `scripts/generate-openapi-json.js`  | YAML→JSON conversion (for swagger docs / future client codegen) |
| `bitmark_codegen` (bitmark-parser-rust) | `crates/app/nize_codegen/`      | Custom Rust codegen — reads OpenAPI YAML, emits Rust models + routes |
| `bitmark-api-generated/`        | `crates/lib/nize_api/src/generated/`   | Generated code output          |
| (bitmark-configurator-api patterns) | `crates/lib/nize_api/`             | Manual handlers, error types   |
| —                                | `crates/app/nize-api/`                 | Sidecar binary                 |

## Target Directory Layout

```
nize-mcp/
├── .awa/specs/
│   ├── API-NIZE-index.tsp          # TypeSpec entry point
│   └── API-NIZE-common.tsp         # Shared types (error models, etc.)
├── scripts/
│   ├── generate-api.sh             # Pipeline orchestrator (tsp → json → nize-codegen)
│   └── generate-openapi-json.js    # YAML → JSON (for swagger docs / client codegen)
├── codegen/
│   └── nize-api/
│       └── tsp-output/              # TypeSpec compiler output (gitignored)
├── crates/
│   ├── app/
│   │   ├── nize-api/               # API sidecar binary
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       └── main.rs
│   │   └── nize_codegen/           # Custom Rust codegen (reads OpenAPI YAML → Rust)
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs          # generate() entry point with staleness detection
│   │           ├── main.rs         # CLI entry point
│   │           ├── schema.rs       # OpenAPI 3.0 serde types
│   │           ├── gen_models.rs   # Emit serde structs from OpenAPI schemas
│   │           ├── gen_routes.rs   # Emit route path constants
│   │           └── writer.rs       # Generated file header + utilities
│   └── lib/
│       ├── nize_api/               # API library
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs
│       │       ├── config.rs       # ApiConfig (bind addr, PG url, node path)
│       │       ├── error.rs        # AppError, AppResult
│       │       ├── handlers/
│       │       │   ├── mod.rs
│       │       │   └── hello.rs    # hello_world handler
│       │       └── generated/      # ← output from nize-codegen (gitignored)
│       │           ├── mod.rs
│       │           ├── models.rs   # Serde structs
│       │           └── routes.rs   # Route path constants
│       └── nize_core/
│           └── src/
│               ├── lib.rs          # + pub mod hello; pub mod node_sidecar;
│               ├── hello.rs        # hello_world() function
│               └── node_sidecar.rs  # NodeSidecar (trivial spawn)
└── packages/
    └── nize-desktop/
        └── src/
            └── App.tsx             # + HelloWorld button
```

## Steps

### Phase 1 — nize_core: hello_world + NodeSidecar

Add the functions that the API endpoint will call.

- [x] **1.1** Create `crates/lib/nize_core/src/hello.rs`:
  - `pub fn hello_world() -> String` — returns `"Hello from nize_core v{version}"`
  - Unit test
- [x] **1.2** Create `crates/lib/nize_core/src/node_sidecar.rs`:
  - `pub async fn check_node_available() -> Result<NodeInfo>` — runs `node --version`, returns version string
  - `NodeInfo { version: String, available: bool }`
  - `SidecarError` error type
  - Unit test (requires Node on PATH via mise)
- [x] **1.3** Update `crates/lib/nize_core/src/lib.rs`: add `pub mod hello;` and `pub mod node_sidecar;`
- [x] **1.4** Verify: `cargo test -p nize-core` passes

### Phase 2 — TypeSpec Tooling Setup

Install TypeSpec compiler and OpenAPI Generator.

- [x] **2.1** Add devDependencies to root `package.json`:
  - `@typespec/compiler`
  - `@typespec/http`
  - `@typespec/rest`
  - `@typespec/openapi`
  - `@typespec/openapi3`
  - `js-yaml`
- [x] **2.2** Create `tspconfig.yaml` at project root:
  ```yaml
  emit:
    - "@typespec/openapi3"
  options:
    "@typespec/openapi3":
      output-file: "openapi.yaml"
  output-dir: "{cwd}/codegen/nize-api/tsp-output"
  linter:
    extends:
      - "@typespec/http/all"
  ```
- [x] **2.3** Add to `.gitignore`: `codegen/`
- [x] **2.4** Verify: `npx tsp compile .awa/specs/API-NIZE-index.tsp --no-emit` succeeds (after Phase 3)

### Phase 3 — TypeSpec API Specification

Write the bootstrap API spec.

- [x] **3.1** Create `.awa/specs/API-NIZE-common.tsp`:
  - Service definition (`NizeApi`, localhost:3100)
  - `ErrorResponse` model
  - Shared decorators / imports
- [x] **3.2** Create `.awa/specs/API-NIZE-index.tsp`:
  - Import `API-NIZE-common.tsp`
  - `HelloWorldResponse` model: `{ greeting: string, dbConnected: boolean, nodeVersion: string | null, nodeAvailable: boolean }`
  - `GET /api/hello` → `HelloWorldResponse`
- [x] **3.3** Verify: `npx tsp compile .awa/specs/API-NIZE-index.tsp` produces `codegen/nize-api/tsp-output/openapi.yaml`

### Phase 4 — Codegen Pipeline (nize_codegen)

Custom Rust codegen crate modeled after `bitmark_codegen` from `submodules/bitmark-parser-rust`.
Reads the OpenAPI YAML produced by TypeSpec and emits clean, minimal Rust source files.

- [x] **4.1** Create `crates/app/nize_codegen/Cargo.toml`:
  - Dependencies: `serde`, `serde_json`, `serde_yaml` (all workspace)
- [x] **4.2** Create `crates/app/nize_codegen/src/schema.rs`:
  - OpenAPI 3.0 serde types: `OpenApiDoc`, `Info`, `PathItem`, `Operation`, `Components`, `SchemaObject`, `PropertyObject`
  - `BTreeMap` for deterministic output ordering
- [x] **4.3** Create `crates/app/nize_codegen/src/gen_models.rs`:
  - Generate `models.rs` with `#[derive(Debug, Clone, Serialize, Deserialize)]` structs
  - Map OpenAPI types to Rust types, handle nullable → `Option<T>`, add `#[serde(rename)]` for camelCase
- [x] **4.4** Create `crates/app/nize_codegen/src/gen_routes.rs`:
  - Generate `routes.rs` with `pub const` path constants (e.g. `GET_API_HELLO`)
- [x] **4.5** Create `crates/app/nize_codegen/src/writer.rs`:
  - Generated file header ("auto-generated by nize-codegen"), `escape_rust_str()` utility
- [x] **4.6** Create `crates/app/nize_codegen/src/lib.rs`:
  - `pub fn generate(spec_path, output_dir) -> Result<bool, String>`
  - Staleness detection via FNV-1a 128-bit hash stored in `.hash` file
  - Returns `Ok(true)` if generated, `Ok(false)` if up-to-date
- [x] **4.7** Create `crates/app/nize_codegen/src/main.rs`:
  - CLI entry point resolving workspace root
  - Reads `codegen/nize-api/tsp-output/@typespec/openapi3/openapi.yaml`
  - Outputs to `crates/lib/nize_api/src/generated/`
- [x] **4.8** Update `scripts/generate-api.sh`:
  - 3-step pipeline: `tsp compile` → `generate-openapi-json.js` → `cargo run -p nize-codegen`
- [x] **4.9** Add `/crates/lib/nize_api/src/generated/` to `.gitignore`
- [x] **4.10** Verify: `npm run generate:api` produces `models.rs`, `routes.rs`, `mod.rs` in generated dir

### Phase 5 — nize_api Library Crate

Create the API library with manual handlers + generated code.

- [x] **5.1** Create `crates/lib/nize_api/Cargo.toml`:
  - Dependencies: `nize-core`, `axum` (0.7, features: macros, json), `tokio`, `serde`, `serde_json`, `thiserror`, `tracing`, `utoipa` (for manual OpenAPI docs on handlers)
- [x] **5.2** Create `crates/lib/nize_api/src/error.rs`:
  - `AppError` enum (Validation, NotFound, Internal, DbUnavailable, SidecarUnavailable)
  - `AppResult<T>` type alias
  - `impl IntoResponse for AppError`
  - Follow `submodules/bitmark-configurator-api/src/error.rs` pattern
- [x] **5.3** Create `crates/lib/nize_api/src/config.rs`:
  - `ApiConfig { bind_addr, pg_connection_url, node_path }`
  - `ApiConfig::from_env()` — read from env vars with defaults
- [x] **5.4** Create `crates/lib/nize_api/src/handlers/hello.rs`:
  - `async fn hello_world(State(state): State<AppState>) -> AppResult<Json<HelloWorldResponse>>`
  - Calls `nize_core::hello::hello_world()` for greeting
  - Calls `sqlx::query("SELECT 1")` on PG pool to verify DB
  - Calls `nize_core::node_sidecar::check_node_available()` for Node check
  - Returns `HelloWorldResponse { greeting, db_connected, node_version, node_available }`
- [x] **5.5** Create `crates/lib/nize_api/src/handlers/mod.rs`
- [x] **5.6** Create `crates/lib/nize_api/src/lib.rs`:
  - `pub mod config; pub mod error; pub mod handlers; pub mod generated;`
  - `AppState` struct (holds `PgPool`, `ApiConfig`)
  - `pub fn router(state: AppState) -> Router` — mounts hello route + generated routes
- [x] **5.7** Add `nize-api` to workspace `Cargo.toml`:
  - Add `crates/lib/nize_api` to `members`
  - Add `axum`, `tracing`, `tracing-subscriber`, `utoipa` to `[workspace.dependencies]`
- [x] **5.8** Verify: `cargo check -p nize-api` compiles (after generated code is in place)

### Phase 6 — nize-api Sidecar Binary

Create the API server binary that Tauri will manage as a sidecar.

- [x] **6.1** Create `crates/app/nize-api/Cargo.toml`:
  - Dependencies: `nize-api` (lib), `nize-core`, `tokio`, `tracing`, `tracing-subscriber`, `clap`, `dotenvy`
- [x] **6.2** Create `crates/app/nize-api/src/main.rs`:
  - Parse CLI args (port, PG URL)
  - Initialize tracing
  - Connect to PG pool
  - Build router via `nize_api::router(state)`
  - Bind and serve on configured port
  - Print port to stdout on startup (Tauri reads this to know the sidecar is ready)
- [x] **6.3** Add `crates/app/nize-api` to workspace `members` (NOT `default-members`)
- [x] **6.4** Verify: `cargo run -p nize-api-server -- --port 3100` starts and responds to `GET /api/hello`

### Phase 7 — Tauri Desktop Integration

Update nize-desktop to start the API sidecar and add a UI button.

- [x] **7.1** Update `crates/app/nize-desktop/tauri.conf.json`:
  - Add `nize-api` to `bundle.externalBin` (or manage via Rust process spawning)
- [x] **7.2** Update `crates/app/nize-desktop/src/lib.rs`:
  - Add Tauri command: `#[tauri::command] async fn hello_world() -> Result<HelloWorldResponse, String>`
  - Start API sidecar on app startup (spawn child process, read port from stdout)
  - Implement `hello_world` command: HTTP GET to `http://localhost:<port>/api/hello`
  - Register command with `.invoke_handler(tauri::generate_handler![hello_world])`
- [x] **7.3** Update `packages/nize-desktop/src/App.tsx`:
  - Add "Hello World" button
  - On click: `invoke("hello_world")` via `@tauri-apps/api/core`
  - Display response: greeting, DB status (green/red), Node status (green/red)
- [ ] **7.4** Verify: `cargo tauri dev` → click button → see response with all three checks

### Phase 8 — Testing

- [x] **8.1** Unit tests in `nize_core`: `hello::hello_world()`, `node_sidecar::check_node_available()`
- [x] **8.2** Integration test in `nize_api`: start server, call `/api/hello`, assert response shape
  - Requires PG running (ephemeral via `DbManager`) + Node on PATH
- [x] **8.3** Verify: `cargo test -p nize-core` passes
- [x] **8.4** Verify: `cargo test -p nize-api` passes (lib)

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| TypeSpec → OpenAPI → Rust type mismatches | Low | Bootstrap has one trivial endpoint; validate early |
| nize_codegen doesn't handle complex OpenAPI patterns | Low | Bootstrap surface is minimal; extend codegen incrementally as API grows |
| API sidecar port conflicts | Low | Use ephemeral port (bind :0), report port to Tauri via stdout |
| Node not on PATH in bundled distribution | Low | Bootstrap only needs dev-time verification; bundled Node sidecar is PLAN-002 Phase 5 scope |
| Generated code + manual code coupling | Low | Generated code isolated in `src/generated/`; manual handlers import from it |

## Decisions

1. **TypeSpec-first codegen** — TypeSpec → OpenAPI YAML → custom Rust codegen (`nize_codegen`). TypeSpec is source of truth for API contracts.
2. **Custom Rust codegen over OpenAPI Generator** — `nize_codegen` (modeled after `bitmark_codegen` in bitmark-parser-rust) replaces the `@openapitools/openapi-generator-cli` `rust-axum` generator. Reasons: OpenAPI Generator produced axum 0.8 code (we use 0.7), massive dependency explosion, trait-based architecture incompatible with our State extractor pattern, required Java runtime, and fragile JS fix scripts. `nize_codegen` emits only what we need (~40 LOC vs ~1,500 LOC).
3. **OpenAPI JSON preserved** — `generate-openapi-json.js` and `js-yaml` dep remain for future swagger docs and client codegen.
4. **Staleness detection** — FNV-1a 128-bit hash of OpenAPI YAML stored in `.hash` file; `nize_codegen` skips regeneration when unchanged.
5. **Sidecar binary** — API server runs as a separate process (`crates/app/nize-api/`), not embedded in Tauri. Matches the Node MCP sidecar pattern from PLAN-002.
6. **Axum 0.7** — Match `submodules/bitmark-configurator-api` patterns (Router, State extractor, IntoResponse).
7. **Generated code in-crate** — Generated Rust goes into `crates/lib/nize_api/src/generated/`, not a separate crate. Simpler dependency graph; isolation via directory.
8. **Node verification = trivial** — `check_node_available()` runs `node --version`. No MCP protocol exercise at this stage.
9. **Port reporting** — Sidecar prints JSON `{"port": N}` to stdout on startup. Tauri reads first line to discover the port.

## Completion Criteria

- `npm run generate:api` produces generated Rust code from TypeSpec specs
- `cargo build -p nize-api` compiles (lib crate)
- `cargo build -p nize-api-server` compiles (sidecar binary)
- `cargo test -p nize-core` passes (hello_world + node check tests)
- `cargo test -p nize-api` passes (integration test)
- `cargo tauri dev` → "Hello World" button → response shows greeting, DB ✓, Node ✓
