# PLAN-003: nize_api Bootstrap

| Field              | Value                          |
|--------------------|--------------------------------|
| **Status**         | in-progress                    |
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

| Ref (codegen design doc)         | nize-mcp (this project)                | Notes                          |
|----------------------------------|----------------------------------------|--------------------------------|
| `.zen/specs/API-Index.tsp`       | `.zen/specs/API-NIZE-index.tsp`        | TypeSpec entry point           |
| `tsp-output/openapi.yaml`       | `codegen/nize-api/tsp-output/openapi.yaml` | TypeSpec compiler output |
| `scripts/generate-openapi-json.js` | `scripts/generate-openapi-json.js`  | YAML→JSON conversion          |
| `codegen-output/`               | `codegen/nize-api/openapi-generator/`  | OpenAPI Generator output (transient) |
| `scripts/install-generated-code.js` | `scripts/install-generated-code.js` | Post-gen orchestrator          |
| `scripts/lib-code-gen/*.js`      | `scripts/lib-code-gen/*.js`            | Fix scripts                    |
| `bitmark-api-generated/`        | `crates/lib/nize_api/src/generated/`   | Installed generated code       |
| (bitmark-configurator-api patterns) | `crates/lib/nize_api/`             | Manual handlers, error types   |
| —                                | `crates/app/nize-api/`                 | Sidecar binary                 |

## Target Directory Layout

```
nize-mcp/
├── .zen/specs/
│   ├── API-NIZE-index.tsp          # TypeSpec entry point
│   └── API-NIZE-common.tsp         # Shared types (error models, etc.)
├── scripts/
│   ├── generate-api.sh             # Pipeline orchestrator (all stages)
│   ├── generate-openapi-json.js    # YAML → JSON
│   ├── install-generated-code.js   # Post-gen fix + install
│   └── lib-code-gen/
│       ├── paths.js                # Path constants
│       ├── fixes.js                # Import path fixes
│       ├── copy.js                 # File copying utilities
│       └── axum-path.js            # Axum route path fixes
├── codegen/
│   └── nize-api/
│       ├── tsp-output/              # TypeSpec compiler output (gitignored)
│       └── openapi-generator/       # OpenAPI Generator output (transient, gitignored)
├── crates/
│   ├── app/
│   │   └── nize-api/               # API sidecar binary
│   │       ├── Cargo.toml
│   │       └── src/
│   │           └── main.rs
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
│       │       └── generated/      # ← installed from codegen pipeline
│       │           ├── mod.rs
│       │           └── ...
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

- [ ] **1.1** Create `crates/lib/nize_core/src/hello.rs`:
  - `pub fn hello_world() -> String` — returns `"Hello from nize_core v{version}"`
  - Unit test
- [ ] **1.2** Create `crates/lib/nize_core/src/node_sidecar.rs`:
  - `pub async fn check_node_available() -> Result<NodeInfo>` — runs `node --version`, returns version string
  - `NodeInfo { version: String, available: bool }`
  - `SidecarError` error type
  - Unit test (requires Node on PATH via mise)
- [ ] **1.3** Update `crates/lib/nize_core/src/lib.rs`: add `pub mod hello;` and `pub mod node_sidecar;`
- [ ] **1.4** Verify: `cargo test -p nize-core` passes

### Phase 2 — TypeSpec Tooling Setup

Install TypeSpec compiler and OpenAPI Generator.

- [ ] **2.1** Add devDependencies to root `package.json`:
  - `@typespec/compiler`
  - `@typespec/http`
  - `@typespec/rest`
  - `@typespec/openapi`
  - `@typespec/openapi3`
  - `@openapitools/openapi-generator-cli`
  - `js-yaml`
- [ ] **2.2** Create `tspconfig.yaml` at project root:
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
- [ ] **2.3** Add to `.gitignore`: `codegen/`
- [ ] **2.4** Verify: `npx tsp compile .zen/specs/API-NIZE-index.tsp --no-emit` succeeds (after Phase 3)

### Phase 3 — TypeSpec API Specification

Write the bootstrap API spec.

- [ ] **3.1** Create `.zen/specs/API-NIZE-common.tsp`:
  - Service definition (`NizeApi`, localhost:3100)
  - `ErrorResponse` model
  - Shared decorators / imports
- [ ] **3.2** Create `.zen/specs/API-NIZE-index.tsp`:
  - Import `API-NIZE-common.tsp`
  - `HelloWorldResponse` model: `{ greeting: string, dbConnected: boolean, nodeVersion: string | null, nodeAvailable: boolean }`
  - `GET /api/hello` → `HelloWorldResponse`
- [ ] **3.3** Verify: `npx tsp compile .zen/specs/API-NIZE-index.tsp` produces `codegen/nize-api/tsp-output/openapi.yaml`

### Phase 4 — Codegen Pipeline Scripts

Port the codegen pipeline from the ref design doc, adapted for nize.

- [ ] **4.1** Create `scripts/lib-code-gen/paths.js`:
  - Path constants: `TSP_OUTPUT_DIR` (`codegen/nize-api/tsp-output`), `CODEGEN_OUTPUT_DIR` (`codegen/nize-api/openapi-generator`), `TARGET_DIR` (`crates/lib/nize_api/src/generated`)
  - OpenAPI YAML/JSON paths
- [ ] **4.2** Create `scripts/generate-openapi-json.js`:
  - Read `codegen/nize-api/tsp-output/openapi.yaml`
  - Write `codegen/nize-api/tsp-output/openapi.json`
- [ ] **4.3** Create `scripts/lib-code-gen/fixes.js`:
  - `fixImports(content)` — replace `use crate::` → `use nize_api::generated::`, adjust model paths
- [ ] **4.4** Create `scripts/lib-code-gen/copy.js`:
  - File copy with optional content transform
- [ ] **4.5** Create `scripts/lib-code-gen/axum-path.js`:
  - Fix Axum route path parameter syntax if needed
- [ ] **4.6** Create `scripts/install-generated-code.js`:
  - Orchestrator: clean target dir → copy generated source → apply import fixes → apply Axum path fixes → generate `mod.rs`
- [ ] **4.7** Create `scripts/generate-api.sh`:
  - Full pipeline: `tsp compile` → `generate-openapi-json.js` → `openapi-generator-cli generate -g rust-axum` → `install-generated-code.js`
  - OpenAPI Generator args: `-i codegen/nize-api/tsp-output/openapi.yaml -g rust-axum -o codegen/nize-api/openapi-generator --additional-properties=packageName=nize_api,packageVersion=0.1.0`
- [ ] **4.8** Add `"generate:api"` script to root `package.json`
- [ ] **4.9** Verify: `npm run generate:api` produces files in `crates/lib/nize_api/src/generated/`

### Phase 5 — nize_api Library Crate

Create the API library with manual handlers + generated code.

- [ ] **5.1** Create `crates/lib/nize_api/Cargo.toml`:
  - Dependencies: `nize-core`, `axum` (0.7, features: macros, json), `tokio`, `serde`, `serde_json`, `thiserror`, `tracing`, `utoipa` (for manual OpenAPI docs on handlers)
- [ ] **5.2** Create `crates/lib/nize_api/src/error.rs`:
  - `AppError` enum (Validation, NotFound, Internal, DbUnavailable, SidecarUnavailable)
  - `AppResult<T>` type alias
  - `impl IntoResponse for AppError`
  - Follow `submodules/bitmark-configurator-api/src/error.rs` pattern
- [ ] **5.3** Create `crates/lib/nize_api/src/config.rs`:
  - `ApiConfig { bind_addr, pg_connection_url, node_path }`
  - `ApiConfig::from_env()` — read from env vars with defaults
- [ ] **5.4** Create `crates/lib/nize_api/src/handlers/hello.rs`:
  - `async fn hello_world(State(state): State<AppState>) -> AppResult<Json<HelloWorldResponse>>`
  - Calls `nize_core::hello::hello_world()` for greeting
  - Calls `sqlx::query("SELECT 1")` on PG pool to verify DB
  - Calls `nize_core::node_sidecar::check_node_available()` for Node check
  - Returns `HelloWorldResponse { greeting, db_connected, node_version, node_available }`
- [ ] **5.5** Create `crates/lib/nize_api/src/handlers/mod.rs`
- [ ] **5.6** Create `crates/lib/nize_api/src/lib.rs`:
  - `pub mod config; pub mod error; pub mod handlers; pub mod generated;`
  - `AppState` struct (holds `PgPool`, `ApiConfig`)
  - `pub fn router(state: AppState) -> Router` — mounts hello route + generated routes
- [ ] **5.7** Add `nize-api` to workspace `Cargo.toml`:
  - Add `crates/lib/nize_api` to `members`
  - Add `axum`, `tracing`, `tracing-subscriber`, `utoipa` to `[workspace.dependencies]`
- [ ] **5.8** Verify: `cargo check -p nize-api` compiles (after generated code is in place)

### Phase 6 — nize-api Sidecar Binary

Create the API server binary that Tauri will manage as a sidecar.

- [ ] **6.1** Create `crates/app/nize-api/Cargo.toml`:
  - Dependencies: `nize-api` (lib), `nize-core`, `tokio`, `tracing`, `tracing-subscriber`, `clap`, `dotenvy`
- [ ] **6.2** Create `crates/app/nize-api/src/main.rs`:
  - Parse CLI args (port, PG URL)
  - Initialize tracing
  - Connect to PG pool
  - Build router via `nize_api::router(state)`
  - Bind and serve on configured port
  - Print port to stdout on startup (Tauri reads this to know the sidecar is ready)
- [ ] **6.3** Add `crates/app/nize-api` to workspace `members` (NOT `default-members`)
- [ ] **6.4** Verify: `cargo run -p nize-api-server -- --port 3100` starts and responds to `GET /api/hello`

### Phase 7 — Tauri Desktop Integration

Update nize-desktop to start the API sidecar and add a UI button.

- [ ] **7.1** Update `crates/app/nize-desktop/tauri.conf.json`:
  - Add `nize-api` to `bundle.externalBin` (or manage via Rust process spawning)
- [ ] **7.2** Update `crates/app/nize-desktop/src/lib.rs`:
  - Add Tauri command: `#[tauri::command] async fn hello_world() -> Result<HelloWorldResponse, String>`
  - Start API sidecar on app startup (spawn child process, read port from stdout)
  - Implement `hello_world` command: HTTP GET to `http://localhost:<port>/api/hello`
  - Register command with `.invoke_handler(tauri::generate_handler![hello_world])`
- [ ] **7.3** Update `packages/nize-desktop/src/App.tsx`:
  - Add "Hello World" button
  - On click: `invoke("hello_world")` via `@tauri-apps/api/core`
  - Display response: greeting, DB status (green/red), Node status (green/red)
- [ ] **7.4** Verify: `cargo tauri dev` → click button → see response with all three checks

### Phase 8 — Testing

- [ ] **8.1** Unit tests in `nize_core`: `hello::hello_world()`, `node_sidecar::check_node_available()`
- [ ] **8.2** Integration test in `nize_api`: start server, call `/api/hello`, assert response shape
  - Requires PG running (ephemeral via `DbManager`) + Node on PATH
- [ ] **8.3** Verify: `cargo test -p nize-core` passes
- [ ] **8.4** Verify: `cargo test -p nize-api` passes (lib)

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| OpenAPI Generator `rust-axum` output quality varies | Medium | Post-gen fix scripts handle known issues; keep generated surface small at bootstrap |
| TypeSpec → OpenAPI → Rust type mismatches | Low | Bootstrap has one trivial endpoint; validate early |
| `rust-axum` generator may produce code incompatible with axum 0.7 | Medium | Pin generator version; post-gen scripts fix imports. Ref impl proven pattern |
| API sidecar port conflicts | Low | Use ephemeral port (bind :0), report port to Tauri via stdout |
| Node not on PATH in bundled distribution | Low | Bootstrap only needs dev-time verification; bundled Node sidecar is PLAN-002 Phase 5 scope |
| Generated code + manual code coupling | Low | Generated code isolated in `src/generated/`; manual handlers import from it |

## Decisions

1. **TypeSpec-first codegen** — TypeSpec → OpenAPI → OpenAPI Generator (rust-axum) → post-gen fix scripts. Same pipeline as ref design doc. TypeSpec is source of truth for API contracts.
2. **Sidecar binary** — API server runs as a separate process (`crates/app/nize-api/`), not embedded in Tauri. Matches the Node MCP sidecar pattern from PLAN-002.
3. **Axum 0.7** — Match `submodules/bitmark-configurator-api` patterns (Router, State extractor, IntoResponse).
4. **Generated code in-crate** — Generated Rust goes into `crates/lib/nize_api/src/generated/`, not a separate crate. Simpler dependency graph; isolation via directory.
5. **Node verification = trivial** — `check_node_available()` runs `node --version`. No MCP protocol exercise at this stage.
6. **Port reporting** — Sidecar prints JSON `{"port": N}` to stdout on startup. Tauri reads first line to discover the port.

## Completion Criteria

- `npm run generate:api` produces generated Rust code from TypeSpec specs
- `cargo build -p nize-api` compiles (lib crate)
- `cargo build -p nize-api-server` compiles (sidecar binary)
- `cargo test -p nize-core` passes (hello_world + node check tests)
- `cargo test -p nize-api` passes (integration test)
- `cargo tauri dev` → "Hello World" button → response shows greeting, DB ✓, Node ✓
