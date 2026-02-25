# PLAN-014: nize-web Dev Hot Reload

| Field              | Value                                              |
|--------------------|----------------------------------------------------|
| **Status**         | in-progress                                        |
| **Workflow**       | lateral                                            |
| **Reference**      | PLAN-012 (nize-web sidecar)                        |
| **Traceability**   | PLAN-012 D5                                        |

## Goal

Enable Next.js hot-module-reload (HMR) for nize-web during `cargo tauri dev`, and make nize-web debuggable from a browser devtools window — all via the *existing* single `cargo tauri dev` command.

## Current State

- `beforeDevCommand` builds nize-web standalone (`next build`) and bundles it into `resources/nize-web/`.
- Rust spawns `node nize-web-server.mjs` which runs the standalone `server.js` (production-like).
- Changes to nize-web require a full rebuild + Tauri restart.
- The iframe URL is `http://127.0.0.1:{sidecar_port}` — not debuggable in external browser (port ephemeral, not known upfront).

## Approach

Modify the sidecar wrapper to accept a `--dev` flag. When set:

1. Run `next dev --port <port>` instead of `node server.js`.
2. Proxy HMR websocket connections through the sidecar proxy so the iframe gets live updates.
3. Serve `/__nize-env.js` from the proxy (same as production).
4. Print `{"port": N}` via the same sidecar protocol.

Rust passes `--dev` when built with `debug_assertions` (i.e. `cargo tauri dev`).

The `beforeDevCommand` skips `build:nize-web` in dev (no standalone build needed).

## Steps

- [ ] **1** Update `packages/nize-web/scripts/nize-web-server.mjs`:
  - Add `--dev` CLI flag
  - When `--dev`: spawn `npx next dev --port <internalPort>` instead of `node server.js`
  - Proxy WebSocket upgrade events (`Upgrade` header) to the internal Next.js dev server for HMR
  - Keep all other behaviour (env injection, API proxying, sidecar protocol) identical
- [ ] **2** Update `crates/app/nize_desktop/src/lib.rs`:
  - In `start_nize_web_sidecar`, pass `--dev` when `cfg!(debug_assertions)`
- [ ] **3** Update `crates/app/nize_desktop/tauri.conf.json`:
  - Remove `npm run --prefix packages/nize-desktop build:nize-web` from `beforeDevCommand` (unnecessary in dev; saves ~15s)
- [ ] **4** Verify: `cargo tauri dev` starts nize-web in dev mode with HMR working in the iframe

## Risks

| Risk | Mitigation |
|------|------------|
| `next dev` doesn't support `--port 0` | Use ephemeral port via net.createServer, then pass fixed port to `next dev` |
| WebSocket proxy complexity | Minimal: listen for `upgrade` event on the proxy server and pipe raw sockets |
| Slower startup (next dev compiles on first request) | Acceptable for dev; first page load takes ~2s |

## Debugging

With this change, nize-web in the iframe runs at a known `http://127.0.0.1:{port}`. To debug:

1. Open that URL in Chrome/Safari — full devtools, network inspector, React DevTools all work.
2. AI agents with browser tools (MCP Playwright, etc.) can navigate to the same URL.
3. The Tauri webview devtools (already opened in debug builds) cover the desktop shell UI.
