# PLAN-001: Project Bootstrap

| Field              | Value                          |
|--------------------|--------------------------------|
| **Status**         | completed                      |
| **Workflow**       | bottom-up                      |
| **Reference**      | `submodules/bitmark-parser-rust` (structural template) |
| **Traceability**   | —                              |

## Goal

Bootstrap the `nize-mcp` workspace so it can produce:

1. **Rust CLI** (`nize`) — native binary
2. **TypeScript CLI** (`nize`) — Node CLI powered by WASM
3. **WASM module** (`nize-wasm`) — compiled from Rust libraries, consumed by TS CLI

Two Rust library crates underpin everything: `nize-core` (domain logic) and `nize-mcp` (MCP protocol layer).

## Reference Structure Mapping

| bitmark-parser-rust           | nize-mcp (this project)       | Notes                          |
|-------------------------------|-------------------------------|--------------------------------|
| `crates/app/bitmark`          | `crates/app/nize`             | Rust CLI                       |
| `crates/lib/bitmark_parser`   | `crates/lib/nize_core`        | Core domain library            |
| `crates/lib/bitmark_breakscape` | `crates/lib/nize_mcp`       | MCP protocol library           |
| `crates/wasm/bitmark_wasm`    | `crates/wasm/nize_wasm`       | WASM bindings                  |
| `packages/bitmark-parser`     | `packages/nize`               | TS CLI + WASM wrapper          |

## Target Directory Layout

```
nize-mcp/
├── Cargo.toml                  # workspace root
├── package.json                # npm workspace root
├── rust-toolchain.toml         # pin Rust version + wasm target
├── rustfmt.toml                # (already exists)
├── crates/
│   ├── app/
│   │   └── nize/               # Rust CLI binary
│   │       ├── Cargo.toml
│   │       ├── src/
│   │       │   └── main.rs
│   │       └── tests/
│   ├── lib/
│   │   ├── nize_core/          # Core domain logic
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       └── lib.rs
│   │   └── nize_mcp/           # MCP protocol layer
│   │       ├── Cargo.toml
│   │       └── src/
│   │           └── lib.rs
│   └── wasm/
│       └── nize_wasm/          # WASM bindings (cdylib)
│           ├── Cargo.toml
│           └── src/
│               └── lib.rs
├── packages/
│   └── nize/                   # TS CLI + npm package
│       ├── package.json
│       ├── tsconfig.json
│       ├── tsup.config.ts
│       ├── scripts/
│       │   └── build-wasm.sh
│       └── src/
│           ├── index.ts
│           └── cli.ts
├── fixtures/                   # test fixtures (future)
└── .awa/
```

## Steps

### Phase 1 — Rust Workspace & Toolchain

- [x] **1.1** Update root `Cargo.toml` workspace: replace bitmark members with nize members
- [x] **1.2** Create `rust-toolchain.toml` (pin Rust edition, include `wasm32-unknown-unknown` target)
- [x] **1.3** Update `.gitignore` to cover nize-specific paths (generated code, WASM artifacts, dist)

### Phase 2 — Rust Library Crates

- [x] **2.1** Create `crates/lib/nize_core/Cargo.toml` + `src/lib.rs` (placeholder with `pub fn version()`)
- [x] **2.2** Create `crates/lib/nize_mcp/Cargo.toml` + `src/lib.rs` (placeholder, depends on `nize-core`)

### Phase 3 — Rust CLI Crate

- [x] **3.1** Create `crates/app/nize/Cargo.toml` (depends on `nize-core`, `nize-mcp`, `clap`, `log`, `flexi_logger`)
- [x] **3.2** Create `crates/app/nize/src/main.rs` (minimal clap CLI with `--version`)
- [x] **3.3** Create `crates/app/nize/tests/` directory (placeholder)

### Phase 4 — WASM Crate

- [x] **4.1** Create `crates/wasm/nize_wasm/Cargo.toml` (cdylib, depends on `nize-core`, `nize-mcp`, `wasm-bindgen`)
- [x] **4.2** Create `crates/wasm/nize_wasm/src/lib.rs` (minimal wasm-bindgen export, e.g. `version()`)

### Phase 5 — TypeScript CLI Package

- [x] **5.1** Create `packages/nize/package.json` (bin entry, scripts for wasm build + TS build)
- [x] **5.2** Create `packages/nize/tsconfig.json`
- [x] **5.3** Create `packages/nize/tsup.config.ts` (Node-only ESM + CJS builds; no browser targets)
- [x] **5.4** Create `packages/nize/src/index.ts` (export WASM wrapper)
- [x] **5.5** Create `packages/nize/src/cli.ts` (commander-based CLI, `--version`)
- [x] **5.6** Create `packages/nize/scripts/build-wasm.sh` (wasm-pack build + wasm-opt)

### Phase 6 — Root Configuration

- [x] **6.1** Update root `package.json` (keep npm workspace pointing to `packages/*`)
- [x] **6.2** Verify `cargo build` succeeds (all Rust crates compile)
- [x] **6.3** Verify `cargo test` succeeds (no failures)
- [x] **6.4** Verify `wasm-pack build` produces output in `packages/nize/wasm/`

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| `wasm-pack` / `wasm-opt` version mismatch with Rust 1.93 | Mirror the `wasm-opt = false` workaround from ref project; run standalone `wasm-opt` |
| Submodule Cargo.toml collision (root currently mirrors bitmark members) | Root Cargo.toml will be fully replaced with nize members |
| Node ≥ 20 required for TS CLI | Already pinned in `.mise.toml` |

## Decisions

1. **No feature flags** — `nize-core` and `nize-mcp` start without `std`/`parallel` feature gates; keep simple
2. **Node-only** — TS package produces ESM + CJS for Node; no browser builds (project won't run in browser)
3. **Version** — `0.1.0`

## Completion Criteria

- `cargo build` compiles all 4 Rust crates without errors
- `cargo run -- --version` prints nize version
- `wasm-pack build` produces `.wasm` + JS glue in `packages/nize/wasm/`
- `npm run build` in `packages/nize/` produces `dist/` with ESM + CJS outputs
- `npx nize --version` prints version from TS CLI
