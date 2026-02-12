# PLAN-004: Extract nize_api_client Library

| Field              | Value                          |
|--------------------|--------------------------------|
| **Status**         | in-progress                    |
| **Workflow**       | bottom-up                      |
| **Reference**      | PLAN-003 (nize_api bootstrap), `progenitor` (Oxide OpenAPI client generator), OpenAPI spec at `codegen/nize-api/tsp-output/openapi.json` |
| **Traceability**   | —                              |

## Goal

Extract a new `nize_api_client` Rust library crate that provides a **typed HTTP client** for the Nize API.
Client code (models + request methods) is **generated from the OpenAPI spec** using `progenitor` (Oxide Computer's OpenAPI client generator).

### Current State

- `nize_api` (server lib) contains generated models + route constants in `src/generated/` (via `nize_codegen`)
- `nize_desktop` (Tauri app) calls the API sidecar with raw `reqwest::get` → `serde_json::Value` — untyped
- `nize_codegen` generates `models.rs` (serde structs) and `routes.rs` (path constants) from OpenAPI YAML
- The TypeSpec → OpenAPI YAML → `nize_codegen` pipeline is established (`npm run generate:api`)
- Workspace uses `reqwest = "0.12"`

### Target State

- New `crates/lib/nize_api_client/` crate with progenitor-generated typed client
- `nize_desktop` uses the typed client instead of raw reqwest
- Server (`nize_api`) and client (`nize_api_client`) each have their own generated models — independently generated from the same OpenAPI spec, wire-compatible via JSON
- `nize_codegen` unchanged (continues to serve `nize_api`)

## Design Decision: Code Generation Tool

**Option A — Extend nize_codegen** (hand-roll client generation):
- Pros: full control, no new dependencies
- Cons: reinventing the wheel, must handle all OpenAPI patterns, maintenance burden

**Option B — `progenitor` via `generate_api!` macro:**
- Pros: single line in lib.rs, auto-regenerates on spec change, zero config
- Cons: generated code invisible (hard to debug/inspect), proc-macro compile overhead

**Option C — `progenitor` via `build.rs`:**
- Pros: generated code visible in `OUT_DIR`, auto-regenerates, customizable (interface style, tags, derives), build.rs pattern familiar from nize_codegen pipeline
- Cons: slightly more setup than macro

**Option D — `progenitor` static crate (`cargo progenitor`):**
- Pros: fully explicit, no build-time generation, generated code in source tree
- Cons: must re-run CLI manually when spec changes, conflicts with TypeSpec-first pipeline

**Decision: Option C — `progenitor` via `build.rs`.** Best balance of automation and debuggability. Generated code is visible, regenerates automatically when the OpenAPI JSON changes, and supports customization (builder style, derives). No changes needed to `nize_codegen` — the server and client codegen are independent.

## Design Decision: Model Ownership

With `progenitor`, the client crate generates its own model types (`types::HelloWorldResponse`, etc.) that are distinct Rust types from the server's `nize_codegen`-generated models. Both are derived from the same OpenAPI spec, so they are wire-compatible (identical JSON shapes).

**Decision: Independent generation, no shared models.**
- `nize_api` keeps `nize_codegen`-generated `models.rs` + `routes.rs`
- `nize_api_client` gets `progenitor`-generated `Client` + `types::*`
- No dependency between the two crates
- Wire compatibility guaranteed by the shared OpenAPI spec

This is cleaner than the shared-model approach: no circular dependency concerns, each crate is self-contained, and progenitor's generated types include extra features (builder patterns, validation) that the server doesn't need.

## Design Decision: reqwest Version

`progenitor` 0.12.0 requires `reqwest = "0.13"`. Workspace currently uses `reqwest = "0.12"`.

**Decision: Upgrade workspace `reqwest` to 0.13.** The only current consumers are `nize_desktop` and `nize_api` (dev-dependency). The reqwest 0.12 → 0.13 API is compatible for our usage (`.get()`, `.json()`, `.send()`). Upgrade as part of Phase 1.

## Target Directory Layout

```
crates/
├── lib/
│   ├── nize_api_client/              # NEW — progenitor-generated API client
│   │   ├── Cargo.toml
│   │   ├── build.rs                  # progenitor code generation
│   │   └── src/
│   │       └── lib.rs                # include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
│   ├── nize_api/                     # UNCHANGED — keeps nize_codegen models/routes
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       ├── handlers/
│   │       │   └── hello.rs
│   │       └── generated/            # ← nize_codegen output (unchanged)
│   └── ...
├── app/
│   ├── nize_desktop/
│   │   └── src/
│   │       └── lib.rs               # Uses progenitor Client instead of raw reqwest
│   └── nize_codegen/                 # UNCHANGED
```

## Steps

### Phase 1 — Create nize_api_client Crate with Progenitor

Scaffold the new library crate using progenitor's build.rs approach.

- [x] **1.1** Upgrade workspace `reqwest` from `"0.12"` to `"0.13"` in root `Cargo.toml`:
  - Also add `"stream"` and `"query"` features (required by progenitor-client)
  - Verify `cargo check -p nize_api` still compiles after upgrade
- [x] **1.2** Add new workspace dependencies in root `Cargo.toml`:
  - `progenitor = "0.12"` (under `[workspace.dependencies]`)
  - `progenitor-client = "0.12"`
  - `futures = "0.3"`
  - `prettyplease = "0.2"`, `syn = "2.0"` (build-dependencies for the client crate)
- [x] **1.3** Create `crates/lib/nize_api_client/Cargo.toml`:
  - `[dependencies]`: `progenitor-client`, `reqwest` (with json, query, stream), `serde`, `serde_json`, `futures`
  - `[build-dependencies]`: `progenitor`, `serde_json`, `prettyplease`, `syn`
  - No dependency on `nize_api` or `nize_core`
- [x] **1.4** Create `crates/lib/nize_api_client/build.rs`:
  - Read `codegen/nize-api/tsp-output/openapi.json` (relative from workspace root)
  - Use `progenitor::Generator` with `GenerationSettings` (positional interface for now)
  - Generate tokens → `prettyplease::unparse` → write to `OUT_DIR/codegen.rs`
  - `cargo:rerun-if-changed` on the OpenAPI JSON file
- [x] **1.5** Create `crates/lib/nize_api_client/src/lib.rs`:
  - `include!(concat!(env!("OUT_DIR"), "/codegen.rs"));`
  - This brings in the generated `Client` struct, `types` module, and all operation methods
- [x] **1.6** Add `nize_api_client` to workspace `Cargo.toml`:
  - Add `crates/lib/nize_api_client` to `members`
  - Add `nize_api_client = { path = "crates/lib/nize_api_client" }` to `[workspace.dependencies]`
- [x] **1.7** Verify: `cargo check -p nize_api_client` compiles and the generated `Client` struct exists

### Phase 2 — Migrate nize_desktop to Use Typed Client

Replace raw reqwest calls with the progenitor-generated `Client`.

- [x] **2.1** Update `crates/app/nize_desktop/Cargo.toml`:
  - Add `nize_api_client = { workspace = true }`
  - Remove direct `reqwest` dependency (progenitor client wraps it)
- [x] **2.2** Update `crates/app/nize_desktop/src/lib.rs`:
  - Create progenitor `Client` after sidecar port is known:
    `nize_api_client::Client::new(&format!("http://127.0.0.1:{}", port))`
  - Replace `reqwest::get(url).json::<serde_json::Value>()` with `client.hello_hello_world().await`
    (method name derived from operationId `Hello_helloWorld` by progenitor)
  - Update `hello_world` Tauri command return type to use the generated response type
  - Store the progenitor `Client` in `AppServices` struct
- [x] **2.3** Verify: `cargo check -p nize_desktop` compiles

### Phase 3 — Verify End-to-End

Ensure everything works together.

- [ ] **3.1** Run full pipeline: `npm run generate:api` → verify OpenAPI JSON is up-to-date
- [x] **3.2** Verify: `cargo build --workspace`
- [x] **3.3** Verify: `cargo test -p nize_api` passes (server tests unchanged)
- [ ] **3.4** Verify: `cargo tauri dev` → click hello button → typed response displayed

## Progenitor Generated API (Expected)

Progenitor generates a `Client` struct with methods matching OpenAPI operations.
For our spec with `operationId: "Hello_helloWorld"` on `GET /api/hello`:

```rust
// Generated by progenitor — DO NOT EDIT

pub mod types {
    #[derive(Clone, Debug, Deserialize, Serialize)]
    pub struct HelloWorldResponse {
        pub db_connected: bool,
        pub greeting: String,
        pub node_available: bool,
        pub node_version: Option<String>,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    pub struct ErrorResponse {
        pub error: String,
        pub message: String,
    }
}

impl Client {
    pub async fn hello_hello_world(&self)
        -> Result<ResponseValue<types::HelloWorldResponse>, Error<types::ErrorResponse>>
    { ... }
}
```

Usage in nize_desktop:

```rust
let client = nize_api_client::Client::new(&format!("http://127.0.0.1:{}", port));
let resp = client.hello_hello_world().await.map_err(|e| format!("{e}"))?;
let body: &nize_api_client::types::HelloWorldResponse = resp.as_ref();
```

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| reqwest 0.12 → 0.13 breaking changes | Low | Our reqwest usage is minimal (`.get()`, `.json()`); API is stable across minor versions |
| progenitor doesn't handle our OpenAPI spec | Low | Our spec is simple (one GET endpoint, basic types, no auth); progenitor handles far more complex specs |
| progenitor method naming doesn't match expectations | Low | Method names derived from `operationId`; can inspect generated code in `OUT_DIR` and adjust TypeSpec `operationId` if needed |
| Build time increase from progenitor proc-macro/build.rs | Low | build.rs only re-runs when OpenAPI JSON changes (`rerun-if-changed`); one-time generation per spec change |
| Model type mismatch between server and client | None | Both generated from same OpenAPI spec; JSON wire format is the contract |
| progenitor-client runtime dependency | Low | Lightweight crate (~626 SLoC), well-maintained by Oxide Computer |

## Open Questions

1. **Builder vs Positional interface style?** — Start with Positional (simpler). Switch to Builder when the API grows complex enough to benefit from it.
2. **Should we also generate httpmock helpers?** — progenitor supports this. Defer; useful for integration testing later.
3. **Should the client be generated for the WASM target too?** — reqwest supports WASM. Defer; address if `nize_wasm` needs API access.

## Completion Criteria

- `cargo check -p nize_api_client` compiles with progenitor-generated client
- `cargo check -p nize_desktop` compiles using the generated client
- `cargo test -p nize_api` passes (server unchanged)
- `cargo build --workspace` succeeds
- No raw `reqwest` + `serde_json::Value` API calls remain in `nize_desktop`
- Generated client code is visible in `target/*/build/nize_api_client-*/out/codegen.rs`
